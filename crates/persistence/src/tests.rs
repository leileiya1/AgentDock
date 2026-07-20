use super::*;
use serde_json::json;

#[tokio::test]
async fn transition_and_event_are_atomic_and_invalid_transition_changes_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Store::in_memory().await?;
    let p = store
        .import_project("p", "/tmp/p", "main", "/tmp/w")
        .await?;
    let t = store
        .create_task(
            &p.id,
            "t",
            "d",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            "main",
            3,
        )
        .await?;
    store
        .transition(
            &t.id,
            &[TaskStatus::Draft],
            TaskStatus::ReadyForDevelopment,
            None,
            Actor::Human,
            "user:start",
            &json!({}),
        )
        .await?;
    let events = store.events(&t.id, 0, 10).await?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].revision, Some(0));
    assert!(
        store
            .transition(
                &t.id,
                &[TaskStatus::Draft],
                TaskStatus::Cancelled,
                None,
                Actor::Human,
                "bad",
                &json!({})
            )
            .await
            .is_err()
    );
    assert_eq!(
        store.task_summary(&t.id).await?.status,
        TaskStatus::ReadyForDevelopment
    );
    assert_eq!(store.events(&t.id, 0, 10).await?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn importing_the_same_repo_reuses_the_project() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::in_memory().await?;
    let first = store
        .import_project("p", "/tmp/p", "main", "/tmp/w")
        .await?;
    let second = store
        .import_project("renamed", "/tmp/p", "trunk", "/tmp/w2")
        .await?;
    assert_eq!(first.id, second.id);
    assert_eq!(first.seq, second.seq);
    assert_eq!(second.name, "renamed");
    assert_eq!(second.default_branch, "trunk");
    assert_eq!(store.projects().await?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn legacy_user_event_actor_is_read_as_human() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::in_memory().await?;
    let p = store
        .import_project("p", "/tmp/p", "main", "/tmp/w")
        .await?;
    let t = store
        .create_task(
            &p.id,
            "t",
            "d",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            "main",
            3,
        )
        .await?;
    sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,0,'user','privacy:api_egress_approved','{}','now')")
        .bind(&t.id)
        .execute(store.pool())
        .await?;

    let events = store.events(&t.id, 0, 10).await?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].actor, Actor::Human);
    Ok(())
}

#[tokio::test]
async fn encrypted_backup_integrity_and_restore_survive_corruption_injection()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let database = root.path().join("agentflow.db");
    let store = Store::open(&database).await?;
    sqlx::query("INSERT INTO settings(key,value_json) VALUES('probe','\"before\"')")
        .execute(store.pool())
        .await?;
    store.integrity_check().await?;
    let backup = store.backup_now().await?;
    let protected = tokio::fs::read(&backup).await?;
    assert!(protected.starts_with(b"AFENC1"));
    assert!(!protected.windows(6).any(|window| window == b"before"));

    let corrupted = backup.with_file_name("corrupted.afbak");
    let mut bytes = protected;
    let last = bytes.len().saturating_sub(1);
    bytes[last] ^= 0x55;
    tokio::fs::write(&corrupted, bytes).await?;
    let error = match store.restore_backup(&corrupted).await {
        Err(error) => error,
        Ok(_) => return Err("tampered encrypted backup was accepted".into()),
    };
    assert!(matches!(error, PersistenceError::Crypto(_)));
    store.integrity_check().await?;

    sqlx::query("UPDATE settings SET value_json='\"after\"' WHERE key='probe'")
        .execute(store.pool())
        .await?;
    let previous = store.restore_backup(&backup).await?;
    assert!(previous.exists());
    drop(store);
    let restored = Store::open(&database).await?;
    let value: String = sqlx::query_scalar("SELECT value_json FROM settings WHERE key='probe'")
        .fetch_one(restored.pool())
        .await?;
    assert_eq!(value, "\"before\"");
    restored.integrity_check().await?;
    Ok(())
}

#[tokio::test]
async fn protected_logs_are_authenticated_and_transparently_readable()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let store = Store::open(&root.path().join("agentflow.db")).await?;
    let log = root.path().join("agent-events.jsonl");
    tokio::fs::write(&log, b"private prompt and model output\n").await?;
    store.protect_file(&log).await?;
    let protected = tokio::fs::read(&log).await?;
    assert!(protected.starts_with(b"AFENC1"));
    assert!(!String::from_utf8_lossy(&protected).contains("private prompt"));
    assert_eq!(
        store.read_protected_file(&log).await?,
        b"private prompt and model output\n"
    );
    Ok(())
}
