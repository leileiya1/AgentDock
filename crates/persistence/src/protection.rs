use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use chrono::Utc;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

use crate::PersistenceError;

const ENCRYPTED_HEADER: &[u8] = b"AFENC1";
const NONCE_BYTES: usize = 24;
const BACKUP_KEEP: usize = 5;

pub(crate) fn new_data_key() -> Result<Arc<[u8; 32]>, PersistenceError> {
    let mut key = [0_u8; 32];
    getrandom::fill(&mut key).map_err(|error| PersistenceError::Crypto(error.to_string()))?;
    Ok(Arc::new(key))
}

pub(crate) async fn load_data_key(data_dir: &Path) -> Result<Arc<[u8; 32]>, PersistenceError> {
    // Release builds use Keychain. Debug/test builds deliberately use an app-data
    // key file so ephemeral test databases never trigger a macOS Keychain prompt.
    #[cfg(all(target_os = "macos", not(test), not(debug_assertions)))]
    {
        const SERVICE: &str = "com.agentflow.local-data";
        const ACCOUNT: &str = "AgentFlow";
        if let Ok(bytes) = security_framework::passwords::get_generic_password(SERVICE, ACCOUNT)
            && let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice())
        {
            return Ok(Arc::new(key));
        }
        let key = new_data_key()?;
        if security_framework::passwords::set_generic_password(SERVICE, ACCOUNT, key.as_ref())
            .is_ok()
        {
            return Ok(key);
        }
    }
    let path = data_dir.join("local-data.key");
    if path.exists() {
        let bytes = tokio::fs::read(&path).await?;
        let key = <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| PersistenceError::Crypto("invalid local data key length".into()))?;
        return Ok(Arc::new(key));
    }
    let key = new_data_key()?;
    tokio::fs::write(&path, key.as_ref()).await?;
    restrict_file(&path).await?;
    Ok(key)
}

pub(crate) fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, PersistenceError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce = [0_u8; NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|error| PersistenceError::Crypto(error.to_string()))?;
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|error| PersistenceError::Crypto(error.to_string()))?;
    let mut protected = Vec::with_capacity(ENCRYPTED_HEADER.len() + NONCE_BYTES + ciphertext.len());
    protected.extend_from_slice(ENCRYPTED_HEADER);
    protected.extend_from_slice(&nonce);
    protected.extend_from_slice(&ciphertext);
    Ok(protected)
}

pub(crate) fn decrypt_bytes(key: &[u8; 32], protected: &[u8]) -> Result<Vec<u8>, PersistenceError> {
    if !protected.starts_with(ENCRYPTED_HEADER) {
        return Ok(protected.to_vec());
    }
    let nonce_start = ENCRYPTED_HEADER.len();
    let body_start = nonce_start + NONCE_BYTES;
    if protected.len() <= body_start {
        return Err(PersistenceError::Crypto("truncated encrypted file".into()));
    }
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&protected[nonce_start..body_start]),
            &protected[body_start..],
        )
        .map_err(|_| PersistenceError::Crypto("encrypted file authentication failed".into()))
}

pub(crate) async fn integrity_check(pool: &SqlitePool) -> Result<(), PersistenceError> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(pool)
        .await?;
    if rows.len() == 1 && rows.first().is_some_and(|value| value == "ok") {
        Ok(())
    } else {
        Err(PersistenceError::Integrity(rows.join("; ")))
    }
}

pub(crate) async fn create_encrypted_backup(
    pool: &SqlitePool,
    database: &Path,
    key: &[u8; 32],
) -> Result<PathBuf, PersistenceError> {
    let data_dir = database
        .parent()
        .ok_or_else(|| PersistenceError::InvalidBackup("database has no parent".into()))?;
    let backup_dir = data_dir.join("backups");
    tokio::fs::create_dir_all(&backup_dir).await?;
    let suffix = format!("{}-{}", Utc::now().format("%Y%m%dT%H%M%S"), Uuid::now_v7());
    let plain = backup_dir.join(format!(".{suffix}.db.tmp"));
    sqlx::query("VACUUM INTO ?")
        .bind(plain.to_string_lossy().as_ref())
        .execute(pool)
        .await?;
    let bytes = tokio::fs::read(&plain).await?;
    let protected = encrypt_bytes(key, &bytes)?;
    let destination = backup_dir.join(format!("agentflow-{suffix}.afbak"));
    tokio::fs::write(&destination, protected).await?;
    restrict_file(&destination).await?;
    let _ = tokio::fs::remove_file(&plain).await;
    rotate_backups(&backup_dir).await?;
    Ok(destination)
}

async fn rotate_backups(directory: &Path) -> Result<(), PersistenceError> {
    let mut entries = tokio::fs::read_dir(directory).await?;
    let mut backups = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("afbak") {
            backups.push(path);
        }
    }
    backups.sort();
    let remove = backups.len().saturating_sub(BACKUP_KEEP);
    for path in backups.into_iter().take(remove) {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

pub(crate) async fn restore_encrypted_backup(
    pool: &SqlitePool,
    database: &Path,
    backup: &Path,
    key: &[u8; 32],
) -> Result<PathBuf, PersistenceError> {
    let canonical_backup = backup.canonicalize()?;
    let backup_root = database
        .parent()
        .ok_or_else(|| PersistenceError::InvalidBackup("database has no parent".into()))?
        .join("backups")
        .canonicalize()?;
    if !canonical_backup.starts_with(&backup_root)
        || canonical_backup
            .extension()
            .and_then(|value| value.to_str())
            != Some("afbak")
    {
        return Err(PersistenceError::InvalidBackup(
            "backup must be an AgentFlow .afbak file in the backup directory".into(),
        ));
    }
    let plaintext = decrypt_bytes(key, &tokio::fs::read(&canonical_backup).await?)?;
    let temporary = database.with_extension(format!("restore-{}.db", Uuid::now_v7()));
    tokio::fs::write(&temporary, plaintext).await?;
    let verification = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&temporary)
                .read_only(true),
        )
        .await?;
    let checked = integrity_check(&verification).await;
    verification.close().await;
    checked?;

    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;
    pool.close().await;
    let previous = database.with_extension(format!(
        "pre-restore-{}.db",
        Utc::now().format("%Y%m%dT%H%M%S")
    ));
    if database.exists() {
        tokio::fs::rename(database, &previous).await?;
    }
    if let Err(error) = tokio::fs::rename(&temporary, database).await {
        if previous.exists() {
            let _ = tokio::fs::rename(&previous, database).await;
        }
        return Err(error.into());
    }
    for suffix in ["db-wal", "db-shm"] {
        let _ = tokio::fs::remove_file(database.with_extension(suffix)).await;
    }
    restrict_file(database).await?;
    Ok(previous)
}

pub(crate) async fn restrict_file(path: &Path) -> Result<(), PersistenceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(())
}
