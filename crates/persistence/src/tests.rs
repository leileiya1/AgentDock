use super::*;
use serde_json::json;
use sha2::{Digest, Sha384};

/// Every published migration is frozen byte-for-byte. sqlx records a checksum when a
/// migration is applied and refuses to open any database whose recorded checksum no longer
/// matches, so editing a published file bricks every existing installation on upgrade.
/// A new migration must be appended here; an edited one must be reverted.
const MIGRATION_MANIFEST: &[(&str, &str)] = &[
    (
        "0001_initial.sql",
        "1ce8056baec57e9a5b18f35dd6ca5770be020abacf0131219c905a719c744a403d249173cc69f82ea9f09b47d3a6928f",
    ),
    (
        "0002_daemon_queue.sql",
        "88316789ba5e829b3ac508336807dfdf846ee38a8a5fa6527a6615bd7ed1c398ac62b84fa94f962873fa6c025229f505",
    ),
    (
        "0003_storage_lifecycle.sql",
        "4f91876c98179b79a329245b4ce19f27377a19caef54eaca6f9957f1158fb9b54a914c799caccd4ce9c4e0b5353b6b76",
    ),
    (
        "0004_privacy_and_issue_lifecycle.sql",
        "e07170bf68f506846860e4cebdfcf9da995e30e18b83a1de1a4c8128d61918c7e52d783169b1ade46e28fc7525319a10",
    ),
    (
        "0005_resilience_and_council.sql",
        "40eceeed2298ba037e67fccf0c2ac513082c98a2355431ff2cae12b2a6071ce5e696d130a3ea3466abbd0dcd15cdedc5",
    ),
    (
        "0006_execution_governance.sql",
        "9d4461446c5e6206bdfa18c70fe952ad511b41b26e3ef1a80f485a2d1db952cdb206a56206a12e54af302a501f025b6d",
    ),
    (
        "0007_project_config_trust.sql",
        "68e133dcfabc2cd390f6eef0eae98336eade90fe73ef5690bb5d58eaa8fbde42547d69f482ee32caaeaf54052e2493ff",
    ),
    (
        "0008_plan_seal.sql",
        "9ba067002a9c929b69f23f9adb23379cd1da2a200f72f6c0d28604aaedbb5bed00f0d5778f5eb0d39947b3419f9b3caa",
    ),
    (
        "0009_task_sagas.sql",
        "e7393bad89da2b5219f31b0b2590eb7334576b9570efc51555ecb79dcb777e8fdd8ac9d11a6af1ed76462fb36b08760a",
    ),
    (
        "0010_delivery_head_checks.sql",
        "0d8cb49e75da5590cb4eaa93e7ed4a9489750ed94db93a77efcf733435ef67ec93952ef05b1e56406ff157ab1488dc0b",
    ),
    (
        "0011_hard_budgets.sql",
        "ebb94c79933961625bf1f367bde86341ead8591f52df232441b764d34b379ef7d12354f77f3851abd8f0da6d31ffa855",
    ),
    (
        "0012_live_run_adoption.sql",
        "163e0593c4f8a9b9f36c2f8f63f3d2b09d7bd03d5012434da8ed24925ae2e958cb82d7f6d16034fe08b5d38c075b7ca2",
    ),
    (
        "0013_scheduler_governance.sql",
        "1fdc84f5898ef04333ca6c7c5227827e892a09afd73bc60a548ccb48efc59ccd6c1eae1c3e010a9fd4eaae5a2afc88de",
    ),
    (
        "0014_repository_identity.sql",
        "66ebcec2110b3339b4b504706f4f6b70d2db77d1f97b54b0c0f016ceba3342be642bb576896b65867d86bece35def4ff",
    ),
    (
        "0015_delivery_ci_checks.sql",
        "6471be0038336c2ec221df780d1901bdd53a9de4e25786c50cdbab9faf9bf00604141a8395a7d8b06dcfbced79121d16",
    ),
    (
        "0016_quality_replay_attempts.sql",
        "2385b2d5d31b0d5aa3500d311a65d4c3a1669686b6b9e451fb52057daabf979a9679639375af52676668cc864832cb81",
    ),
    (
        "0017_execution_node_diagnostics.sql",
        "1ac7f31e5613c3a07980d7cb6b7a397b60b57064ae5abf82e193d10b1c15d64dc1483ee7947312ff3177b589a48d09b4",
    ),
    (
        "0018_project_config_trust_summary.sql",
        "9231e4eda2c461dbe8ea53f349ce69303e7d8e18575f44c3533d5ac2e807b26461287831900e8d735b170fbccfcf159e",
    ),
    (
        "0019_permission_broker.sql",
        "b42e2996d3044c69fbe1b638132e8ad2b95dd6f3eafe676fd09927859661a66e84d3c3ee8372aa2a0d2e2d2c5916afe1",
    ),
    (
        "0020_review_issue_disagreement.sql",
        "16842750ec01acbb1b8221542ebde6b4b943a728c6a9e0dcc63e6565106a0f8dd7176dae876b7200151e10b896d0e366",
    ),
    (
        "0021_task_acceptance_criteria.sql",
        "c94ef916303ff9ff6774fb5f97bd81b0f7ae5a3923dd95b841db2c579d28915ec32cd5d9e56bd76f0e18f1398e335b90",
    ),
    (
        "0022_keychain_resume_tokens.sql",
        "a6effa19c305716e63b705cec1abc330b3f00c20a77ab43eac62281d17a4b2b4b32d0d173cbdff068fc8261f1e14e851",
    ),
    (
        "0023_execution_node_identity.sql",
        "79ca74250fcdb08a9e5c3d02d8c7b24e32eea241b6408d2c077c453bb19f77f6c15750d2e15af9953440581550a11c56",
    ),
    (
        "0024_execution_node_network_isolation.sql",
        "27f4fc844852d92b6b78bbe3dfebeb06246dd80c01298636422323f9e72959879bd0531847b90050082cf42f167e33a0",
    ),
];

#[test]
fn published_migration_bytes_never_change() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut on_disk = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".sql") {
            continue;
        }
        let bytes = std::fs::read(entry.path())?;
        on_disk.insert(name, format!("{:x}", Sha384::digest(&bytes)));
    }
    for (name, frozen) in MIGRATION_MANIFEST {
        let Some(actual) = on_disk.remove(*name) else {
            return Err(format!(
                "{name} is listed in MIGRATION_MANIFEST but missing from migrations/"
            )
            .into());
        };
        assert_eq!(
            &actual, frozen,
            "{name} was modified after publication; published SQLx migrations are immutable — revert it and add a new numbered migration instead"
        );
    }
    assert!(
        on_disk.is_empty(),
        "new migrations must be appended to MIGRATION_MANIFEST so future edits are caught: {:?}",
        on_disk.keys().collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn legacy_0001_checksum_is_repaired_on_open() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let database = root.path().join("agentflow.db");
    let store = Store::open(&database).await?;
    // Emulate a database migrated by a pre-reformat build: it recorded the original checksum.
    sqlx::query("UPDATE _sqlx_migrations SET checksum=? WHERE version=1")
        .bind(hex_to_bytes("3f72f0c6a1318452306ae68aeff0944f1626f14f3228e0e052c9c2d44341ecd3ec30b402185f97bf54159837211f236e"))
        .execute(store.pool())
        .await?;
    drop(store);
    let reopened = Store::open(&database).await?;
    let checksum: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=1")
            .fetch_one(reopened.pool())
            .await?;
    assert_eq!(
        checksum,
        hex_to_bytes(
            "1ce8056baec57e9a5b18f35dd6ca5770be020abacf0131219c905a719c744a403d249173cc69f82ea9f09b47d3a6928f"
        )
    );
    Ok(())
}

#[tokio::test]
async fn cached_file_key_is_reused_without_keychain_access()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let expected = [42_u8; 32];
    let path = root.path().join("local-data.key");
    tokio::fs::write(&path, expected).await?;

    let first = protection::load_data_key(root.path()).await?;
    let second = protection::load_data_key(root.path()).await?;
    assert_eq!(first.as_ref(), &expected);
    assert_eq!(second.as_ref(), &expected);
    Ok(())
}

#[tokio::test]
async fn resume_tokens_are_opaque_references_and_can_be_revoked()
-> Result<(), Box<dyn std::error::Error>> {
    let store = Store::in_memory().await?;
    let secret = "provider-session-secret";
    let secret_ref = store.put_resume_token(secret).await?;
    assert!(secret_ref.starts_with("resume-"));
    assert!(!secret_ref.contains(secret));
    assert_eq!(
        store.get_resume_token(&secret_ref).await?.as_deref(),
        Some(secret)
    );

    store.delete_resume_token(&secret_ref).await?;
    assert_eq!(store.get_resume_token(&secret_ref).await?, None);
    Ok(())
}

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
