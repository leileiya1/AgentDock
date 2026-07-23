use super::*;

async fn git(repo: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned().into())
    }
}

async fn test_repo(root: &Path, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let repo = root.join(name);
    tokio::fs::create_dir_all(&repo).await?;
    git(&repo, &["init", "-q"]).await?;
    git(&repo, &["branch", "-M", "main"]).await?;
    git(&repo, &["config", "user.name", "AgentFlow Test"]).await?;
    git(&repo, &["config", "user.email", "test@agentflow.local"]).await?;
    tokio::fs::write(repo.join("README.md"), name).await?;
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-q", "-m", "base"]).await?;
    Ok(repo)
}

async fn insert_event(
    orchestrator: &Orchestrator,
    task_id: Option<&str>,
    event_type: &str,
    payload: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "INSERT INTO events(task_id,actor,event_type,payload_json,created_at) \
         VALUES(?,'test',?,?,?)",
    )
    .bind(task_id)
    .bind(event_type)
    .bind(payload.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(orchestrator.store.pool())
    .await?;
    Ok(())
}

#[tokio::test]
async fn audit_export_is_scoped_atomic_and_recursively_redacted()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let first_project = orchestrator
        .project_import(&test_repo(root.path(), "first").await?)
        .await?;
    let second_project = orchestrator
        .project_import(&test_repo(root.path(), "second").await?)
        .await?;
    let first_task = orchestrator
        .task_create(
            &first_project.id,
            "first task",
            "audit export",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;
    let second_task = orchestrator
        .task_create(
            &second_project.id,
            "second task",
            "must not leak",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;

    insert_event(
        &orchestrator,
        Some(&first_task.id),
        "test:own",
        json!({
            "api_key": "plain-secret-value",
            "nested": {"authorization": "Bearer private-value"},
            "message": "credential sk-abcdefghijklmnopqrstuvwxyz"
        }),
    )
    .await?;
    insert_event(
        &orchestrator,
        Some(&second_task.id),
        "test:other-project",
        json!({"message": "other-project-marker"}),
    )
    .await?;
    insert_event(
        &orchestrator,
        None,
        "test:own-project-global",
        json!({"project_id": first_project.id, "password": "global-password"}),
    )
    .await?;
    insert_event(
        &orchestrator,
        None,
        "test:unscoped-global",
        json!({"message": "unscoped-global-marker"}),
    )
    .await?;

    let result = orchestrator.events_export(&first_project.id, None).await?;
    assert_eq!(result.scope, AuditExportScope::Project);
    assert!(result.redacted);
    assert!(result.bytes > 0);
    let exported = tokio::fs::read_to_string(&result.path).await?;
    assert!(exported.contains("test:own"));
    assert!(exported.contains("test:own-project-global"));
    assert!(!exported.contains("plain-secret-value"));
    assert!(!exported.contains("private-value"));
    assert!(!exported.contains("abcdefghijklmnopqrstuvwxyz"));
    assert!(!exported.contains("global-password"));
    assert!(!exported.contains("other-project-marker"));
    assert!(!exported.contains("unscoped-global-marker"));

    let records = exported
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(records[0]["record_type"], "agentflow.audit_export");
    assert_eq!(records[0]["event_count"], records.len() - 1);
    assert!(
        records
            .iter()
            .skip(1)
            .all(|record| record["record_type"] == "event")
    );
    assert!(!root.path().join("data/exports").read_dir()?.any(|entry| {
        entry
            .ok()
            .and_then(|entry| entry.file_name().to_str().map(str::to_owned))
            .is_some_and(|name| name.contains(".tmp-"))
    }));

    let error = match orchestrator
        .events_export(&first_project.id, Some(&second_task.id))
        .await
    {
        Ok(_) => return Err("a task from another project was exported".into()),
        Err(error) => error,
    };
    assert!(
        matches!(error, OrchestratorError::InvalidState(message) if message == "AUDIT_EXPORT_TASK_OUTSIDE_PROJECT")
    );
    Ok(())
}

#[tokio::test]
async fn audit_export_scrubs_local_paths_and_all_secret_shapes()
-> Result<(), Box<dyn std::error::Error>> {
    // §58/§71: an export is shared, so it must strip local home paths and every secret shape the
    // committer would block (previously private keys and non-ghp GitHub tokens leaked through).
    let root = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator
        .project_import(&test_repo(root.path(), "privacy").await?)
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "privacy",
            "audit export",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;
    let home = std::env::var("HOME").unwrap_or_default();
    let home_path = format!("{home}/secret-workspace/app.log");
    insert_event(
        &orchestrator,
        Some(&task.id),
        "test:privacy",
        json!({
            "message": format!("crash log at {home_path}"),
            "gh": "gho_ABCDEFGHIJKLMNOPQRSTUVWXYZ012",
            "pem": "-----BEGIN RSA PRIVATE KEY-----\nMIIsecretkeymaterial\n-----END RSA PRIVATE KEY-----",
        }),
    )
    .await?;

    let result = orchestrator.events_export(&project.id, None).await?;
    let exported = tokio::fs::read_to_string(result.path).await?;
    assert!(
        !exported.contains("gho_ABCDEFGHIJKLMNOPQRSTUVWXYZ012"),
        "GitHub OAuth token leaked"
    );
    assert!(
        !exported.contains("secretkeymaterial"),
        "private key leaked"
    );
    if home.len() > 5 {
        assert!(
            !exported.contains(&home),
            "local home path leaked into export"
        );
    }
    Ok(())
}

#[tokio::test]
async fn task_audit_export_excludes_project_level_events() -> Result<(), Box<dyn std::error::Error>>
{
    let root = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator
        .project_import(&test_repo(root.path(), "task-scope").await?)
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "task scope",
            "audit export",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;
    insert_event(
        &orchestrator,
        Some(&task.id),
        "test:task-only",
        json!({"message": "task marker"}),
    )
    .await?;
    insert_event(
        &orchestrator,
        None,
        "test:project-only",
        json!({"project_id": project.id, "message": "project marker"}),
    )
    .await?;

    let result = orchestrator
        .events_export(&project.id, Some(&task.id))
        .await?;
    assert_eq!(result.scope, AuditExportScope::Task);
    assert_eq!(result.task_id.as_deref(), Some(task.id.as_str()));
    assert_eq!(result.task_count, 1);
    let exported = tokio::fs::read_to_string(result.path).await?;
    assert!(exported.contains("task marker"));
    assert!(!exported.contains("project marker"));
    Ok(())
}
