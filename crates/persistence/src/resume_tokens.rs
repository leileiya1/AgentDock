#[cfg(target_os = "macos")]
use std::time::Duration;

use uuid::Uuid;

use crate::{PersistenceError, Store, protection};

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "com.agentflow.provider-resume";
#[cfg(target_os = "macos")]
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(5);

impl Store {
    /// Stores an opaque Provider resume token outside SQLite and returns a random reference.
    /// File-backed macOS stores use the login Keychain. In-memory stores use encrypted RAM so
    /// unit tests neither depend on nor leave entries in the user's Keychain.
    pub async fn put_resume_token(&self, token: &str) -> Result<String, PersistenceError> {
        if token.is_empty() {
            return Err(PersistenceError::Crypto(
                "refusing to store an empty Provider resume token".into(),
            ));
        }
        let secret_ref = format!("resume-{}", Uuid::now_v7());
        if self.uses_ephemeral_resume_vault() {
            let protected = protection::encrypt_bytes(self.data_key.as_ref(), token.as_bytes())?;
            self.ephemeral_resume_tokens
                .lock()
                .await
                .insert(secret_ref.clone(), protected);
            return Ok(secret_ref);
        }

        keychain_set(secret_ref.clone(), token.as_bytes().to_vec()).await?;
        Ok(secret_ref)
    }

    pub async fn get_resume_token(
        &self,
        secret_ref: &str,
    ) -> Result<Option<String>, PersistenceError> {
        if self.uses_ephemeral_resume_vault() {
            let Some(protected) = self
                .ephemeral_resume_tokens
                .lock()
                .await
                .get(secret_ref)
                .cloned()
            else {
                return Ok(None);
            };
            let plaintext = protection::decrypt_bytes(self.data_key.as_ref(), &protected)?;
            return String::from_utf8(plaintext)
                .map(Some)
                .map_err(|_| PersistenceError::Crypto("resume token is not valid UTF-8".into()));
        }

        keychain_get(secret_ref.to_string()).await
    }

    pub async fn delete_resume_token(&self, secret_ref: &str) -> Result<(), PersistenceError> {
        if self.uses_ephemeral_resume_vault() {
            self.ephemeral_resume_tokens.lock().await.remove(secret_ref);
            return Ok(());
        }
        keychain_delete(secret_ref.to_string()).await
    }

    fn uses_ephemeral_resume_vault(&self) -> bool {
        self.path
            .as_deref()
            .and_then(std::path::Path::parent)
            .is_none_or(|directory| directory.starts_with(std::env::temp_dir()))
    }
}

#[cfg(target_os = "macos")]
async fn keychain_set(account: String, token: Vec<u8>) -> Result<(), PersistenceError> {
    let task = tokio::task::spawn_blocking(move || {
        security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, &account, &token)
    });
    timeout_keychain(task, "write").await
}

#[cfg(not(target_os = "macos"))]
async fn keychain_set(_account: String, _token: Vec<u8>) -> Result<(), PersistenceError> {
    Err(PersistenceError::Crypto(
        "Provider resume storage requires macOS Keychain; resume is disabled on this platform"
            .into(),
    ))
}

#[cfg(target_os = "macos")]
async fn keychain_get(account: String) -> Result<Option<String>, PersistenceError> {
    let task = tokio::task::spawn_blocking(move || {
        security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, &account)
    });
    match tokio::time::timeout(KEYCHAIN_TIMEOUT, task).await {
        Ok(Ok(Ok(bytes))) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| PersistenceError::Crypto("resume token is not valid UTF-8".into())),
        Ok(Ok(Err(error))) if error.code() == -25300 => Ok(None),
        Ok(Ok(Err(_))) => Err(PersistenceError::Crypto(
            "could not read Provider resume token from macOS Keychain".into(),
        )),
        Ok(Err(error)) => Err(PersistenceError::Crypto(format!(
            "Keychain task failed: {error}"
        ))),
        Err(_) => Err(PersistenceError::Crypto(
            "macOS Keychain did not respond within 5 seconds".into(),
        )),
    }
}

#[cfg(not(target_os = "macos"))]
async fn keychain_get(_account: String) -> Result<Option<String>, PersistenceError> {
    Err(PersistenceError::Crypto(
        "Provider resume storage requires macOS Keychain; resume is disabled on this platform"
            .into(),
    ))
}

#[cfg(target_os = "macos")]
async fn keychain_delete(account: String) -> Result<(), PersistenceError> {
    let task = tokio::task::spawn_blocking(move || {
        security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, &account)
    });
    match tokio::time::timeout(KEYCHAIN_TIMEOUT, task).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(error))) if error.code() == -25300 => Ok(()),
        Ok(Ok(Err(_))) => Err(PersistenceError::Crypto(
            "could not delete Provider resume token from macOS Keychain".into(),
        )),
        Ok(Err(error)) => Err(PersistenceError::Crypto(format!(
            "Keychain task failed: {error}"
        ))),
        Err(_) => Err(PersistenceError::Crypto(
            "macOS Keychain did not respond within 5 seconds".into(),
        )),
    }
}

#[cfg(not(target_os = "macos"))]
async fn keychain_delete(_account: String) -> Result<(), PersistenceError> {
    Err(PersistenceError::Crypto(
        "Provider resume storage requires macOS Keychain; resume is disabled on this platform"
            .into(),
    ))
}

#[cfg(target_os = "macos")]
async fn timeout_keychain<T>(
    task: tokio::task::JoinHandle<Result<T, security_framework::base::Error>>,
    operation: &str,
) -> Result<T, PersistenceError> {
    match tokio::time::timeout(KEYCHAIN_TIMEOUT, task).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(_))) => Err(PersistenceError::Crypto(format!(
            "could not {operation} Provider resume token in macOS Keychain"
        ))),
        Ok(Err(error)) => Err(PersistenceError::Crypto(format!(
            "Keychain task failed: {error}"
        ))),
        Err(_) => Err(PersistenceError::Crypto(
            "macOS Keychain did not respond within 5 seconds".into(),
        )),
    }
}
