use super::*;
use tempfile::TempDir;

fn require_permission_error<T>(
    result: Result<T, OrchestratorError>,
    message: &'static str,
) -> Result<OrchestratorError, Box<dyn std::error::Error>> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err(message.into()),
    }
}

fn operation(root: &Path, argv: &[&str]) -> PermissionOperation {
    PermissionOperation {
        argv: argv.iter().map(|value| (*value).into()).collect(),
        cwd: root.to_string_lossy().into_owned(),
        paths: Vec::new(),
        network_domains: Vec::new(),
        environment_names: Vec::new(),
        attributes: Default::default(),
    }
}

#[test]
fn operation_seal_is_stable_after_normalized_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let mut value = operation(root.path(), &["bun", "test"]);
    value.network_domains = vec![
        "HTTPS://Registry.NPMJS.org/".into(),
        "registry.npmjs.org".into(),
    ];
    value.environment_names = vec!["path".into(), "LANG".into(), "PATH".into()];
    let first = normalize_permission_operation(&value, root.path())?;
    let second = normalize_permission_operation(&first, root.path())?;
    assert_eq!(first, second);
    assert_eq!(
        permission_operation_sha(PermissionActionType::CommandExecute, &first)?,
        permission_operation_sha(PermissionActionType::CommandExecute, &second)?
    );
    Ok(())
}

#[tokio::test]
async fn once_task_and_project_grants_are_isolated_and_once_is_consumed()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, task) = setup_permission_task().await?;
    let op = operation(Path::new(&project.repo_path), &["bun", "install"]);
    let request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: None,
            provider_id: AgentKind::Codex,
            role: RunRole::Developer,
            action_type: PermissionActionType::DependencyInstall,
            reason: "install locked dependencies".into(),
            operation: op.clone(),
            provider_resume_token: None,
        })
        .await?;
    orchestrator
        .permission_decide(PermissionDecisionInput {
            request_id: request.id,
            operation_sha256: request.operation_sha256,
            policy_sha256: request.policy_sha256,
            decision: PermissionDecisionKind::Approve,
            scope: PermissionGrantScope::Once,
            guidance: None,
        })
        .await?;
    assert_eq!(
        orchestrator
            .permission_authorize(
                &task.id,
                &AgentKind::Codex,
                RunRole::Developer,
                PermissionActionType::DependencyInstall,
                &op
            )
            .await?,
        PermissionAuthorization::AllowedByGrant
    );
    assert_eq!(
        orchestrator
            .permission_authorize(
                &task.id,
                &AgentKind::Codex,
                RunRole::Developer,
                PermissionActionType::DependencyInstall,
                &op
            )
            .await?,
        PermissionAuthorization::PermissionRequired
    );

    let other = orchestrator
        .task_create(
            &project.id,
            "other",
            "d",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;
    assert_eq!(
        orchestrator
            .permission_authorize(
                &other.id,
                &AgentKind::Codex,
                RunRole::Developer,
                PermissionActionType::DependencyInstall,
                &op
            )
            .await?,
        PermissionAuthorization::PermissionRequired
    );

    let task_request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: None,
            provider_id: AgentKind::Codex,
            role: RunRole::Developer,
            action_type: PermissionActionType::DependencyInstall,
            reason: "task grant".into(),
            operation: op.clone(),
            provider_resume_token: None,
        })
        .await?;
    orchestrator
        .permission_decide(PermissionDecisionInput {
            request_id: task_request.id,
            operation_sha256: task_request.operation_sha256,
            policy_sha256: task_request.policy_sha256,
            decision: PermissionDecisionKind::Approve,
            scope: PermissionGrantScope::Task,
            guidance: None,
        })
        .await?;
    assert_eq!(
        orchestrator
            .permission_authorize(
                &task.id,
                &AgentKind::Codex,
                RunRole::Developer,
                PermissionActionType::DependencyInstall,
                &op
            )
            .await?,
        PermissionAuthorization::AllowedByGrant
    );
    assert_eq!(
        orchestrator
            .permission_authorize(
                &other.id,
                &AgentKind::Codex,
                RunRole::Developer,
                PermissionActionType::DependencyInstall,
                &op
            )
            .await?,
        PermissionAuthorization::PermissionRequired
    );

    let project_request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: None,
            provider_id: AgentKind::Codex,
            role: RunRole::Developer,
            action_type: PermissionActionType::DependencyInstall,
            reason: "project rule".into(),
            operation: op.clone(),
            provider_resume_token: None,
        })
        .await?;
    orchestrator
        .permission_decide(PermissionDecisionInput {
            request_id: project_request.id,
            operation_sha256: project_request.operation_sha256,
            policy_sha256: project_request.policy_sha256,
            decision: PermissionDecisionKind::Approve,
            scope: PermissionGrantScope::ProjectRule,
            guidance: None,
        })
        .await?;
    assert_eq!(
        orchestrator
            .permission_authorize(
                &other.id,
                &AgentKind::Codex,
                RunRole::Developer,
                PermissionActionType::DependencyInstall,
                &op
            )
            .await?,
        PermissionAuthorization::AllowedByGrant
    );
    Ok(())
}

#[tokio::test]
async fn stale_seal_forbidden_system_action_and_broad_rule_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, task) = setup_permission_task().await?;
    let forbidden = operation(
        Path::new(&project.repo_path),
        &["sudo", "launchctl", "load"],
    );
    let denied = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: None,
            provider_id: AgentKind::ClaudeCode,
            role: RunRole::Developer,
            action_type: PermissionActionType::SystemChange,
            reason: "need service".into(),
            operation: forbidden,
            provider_resume_token: None,
        })
        .await?;
    assert!(!denied.grantable);
    assert_eq!(denied.status, PermissionRequestStatus::Denied);

    let broad = operation(Path::new(&project.repo_path), &["*"]);
    let request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: None,
            provider_id: AgentKind::Codex,
            role: RunRole::Developer,
            action_type: PermissionActionType::CommandExecute,
            reason: "all commands".into(),
            operation: broad,
            provider_resume_token: None,
        })
        .await?;
    let error = require_permission_error(
        orchestrator
            .permission_decide(PermissionDecisionInput {
                request_id: request.id.clone(),
                operation_sha256: request.operation_sha256.clone(),
                policy_sha256: request.policy_sha256.clone(),
                decision: PermissionDecisionKind::Approve,
                scope: PermissionGrantScope::ProjectRule,
                guidance: None,
            })
            .await,
        "overbroad project rule was accepted",
    )?;
    assert!(error.to_string().contains("PERMISSION_RULE_TOO_BROAD"));
    sqlx::query("UPDATE tasks SET current_revision=current_revision+1 WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    let stale = require_permission_error(
        orchestrator
            .permission_decide(PermissionDecisionInput {
                request_id: request.id,
                operation_sha256: request.operation_sha256,
                policy_sha256: request.policy_sha256,
                decision: PermissionDecisionKind::Approve,
                scope: PermissionGrantScope::Once,
                guidance: None,
            })
            .await,
        "stale permission decision was accepted",
    )?;
    assert!(stale.to_string().contains("PERMISSION_REQUEST_STALE"));
    Ok(())
}

#[tokio::test]
async fn permission_pause_survives_restart_and_approval_resumes_same_revision()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, orchestrator, project, task) = setup_permission_task().await?;
    sqlx::query(
        "UPDATE tasks SET status='DEVELOPING',current_revision=1,worktree_path=? WHERE id=?",
    )
    .bind(&project.repo_path)
    .bind(&task.id)
    .execute(orchestrator.store.pool())
    .await?;
    let mut op = operation(Path::new(&project.repo_path), &[]);
    op.network_domains = vec!["https://packages.example:443/archive".into()];
    let request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: Some("run-1".into()),
            provider_id: AgentKind::External("fixture_provider".into()),
            role: RunRole::Developer,
            action_type: PermissionActionType::NetworkAccess,
            reason: "download a locked fixture".into(),
            operation: op,
            provider_resume_token: Some("opaque".into()),
        })
        .await?;
    let summary = orchestrator.store.task_summary(&task.id).await?;
    assert_eq!(summary.status, TaskStatus::Blocked);
    assert_eq!(
        summary.blocked_reason,
        Some(BlockedReason::PermissionRequired)
    );
    drop(orchestrator);

    let reopened = Orchestrator::open(root.path()).await?;
    assert_eq!(reopened.permission_requests(&task.id).await?.len(), 1);
    reopened
        .permission_decide(PermissionDecisionInput {
            request_id: request.id,
            operation_sha256: request.operation_sha256,
            policy_sha256: request.policy_sha256,
            decision: PermissionDecisionKind::Approve,
            scope: PermissionGrantScope::Once,
            guidance: None,
        })
        .await?;
    let resumed = reopened.store.task_summary(&task.id).await?;
    assert_eq!(resumed.status, TaskStatus::ReadyForDevelopment);
    assert_eq!(resumed.current_revision, 0);
    sqlx::query("UPDATE tasks SET status='DEVELOPING',current_revision=1 WHERE id=?")
        .bind(&task.id)
        .execute(reopened.store.pool())
        .await?;
    let effective = reopened
        .permission_effective_for_run(
            &task.id,
            &AgentKind::External("fixture_provider".into()),
            RunRole::Developer,
        )
        .await?;
    assert_eq!(effective.network_domains, ["packages.example:443"]);
    let second = reopened
        .permission_effective_for_run(
            &task.id,
            &AgentKind::External("fixture_provider".into()),
            RunRole::Developer,
        )
        .await?;
    assert!(second.network_domains.is_empty());
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_escape_and_readonly_write_are_never_grantable()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;
    let (root, orchestrator, project, task) = setup_permission_task().await?;
    let outside = root.path().join("outside");
    tokio::fs::create_dir_all(&outside).await?;
    symlink(&outside, Path::new(&project.repo_path).join("escape"))?;
    let mut escaped = operation(Path::new(&project.repo_path), &[]);
    escaped.paths.push(PermissionPath {
        path: "escape/secret".into(),
        access: PermissionPathAccess::Read,
        outside_worktree: false,
    });
    let error = require_permission_error(
        orchestrator
            .permission_request(PermissionRequestInput {
                task_id: task.id.clone(),
                run_id: None,
                provider_id: AgentKind::Codex,
                role: RunRole::Developer,
                action_type: PermissionActionType::WorktreeRead,
                reason: "read".into(),
                operation: escaped,
                provider_resume_token: None,
            })
            .await,
        "symlink escape was accepted",
    )?;
    assert!(error.to_string().contains("PERMISSION_PATH_ESCAPE"));

    let mut write = operation(Path::new(&project.repo_path), &[]);
    write.paths.push(PermissionPath {
        path: Path::new(&project.repo_path)
            .join("file.rs")
            .to_string_lossy()
            .into_owned(),
        access: PermissionPathAccess::Write,
        outside_worktree: false,
    });
    let denied = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id,
            run_id: None,
            provider_id: AgentKind::ClaudeCode,
            role: RunRole::Reviewer,
            action_type: PermissionActionType::WorktreeWrite,
            reason: "review fix".into(),
            operation: write,
            provider_resume_token: None,
        })
        .await?;
    assert_eq!(denied.risk_level, PermissionRiskLevel::Forbidden);
    assert!(!denied.grantable);
    Ok(())
}

#[tokio::test]
async fn permission_database_never_stores_secret_argument_or_resume_token()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, task) = setup_permission_task().await?;
    let secret = "sk-1234567890123456";
    let token_arg = format!("--token={secret}");
    let op = operation(Path::new(&project.repo_path), &["curl", token_arg.as_str()]);
    let denied = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id,
            run_id: None,
            provider_id: AgentKind::Codex,
            role: RunRole::Developer,
            action_type: PermissionActionType::CommandExecute,
            reason: format!("use {secret}"),
            operation: op,
            provider_resume_token: Some(secret.into()),
        })
        .await?;
    assert!(!denied.grantable);
    let stored: (String, String, Option<String>) = sqlx::query_as(
        "SELECT operation_json,reason,provider_resume_token FROM permission_requests WHERE id=?",
    )
    .bind(&denied.id)
    .fetch_one(orchestrator.store.pool())
    .await?;
    assert!(!stored.0.contains(secret));
    assert!(!stored.1.contains(secret));
    assert!(stored.2.is_none());
    Ok(())
}

#[tokio::test]
async fn grantable_resume_token_is_keychain_referenced_and_never_exposed()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, task) = setup_permission_task().await?;
    let secret = "provider-resume-secret";
    let request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id,
            run_id: Some("run-protected".into()),
            provider_id: AgentKind::ClaudeCode,
            role: RunRole::Developer,
            action_type: PermissionActionType::DependencyInstall,
            reason: "install locked dependencies".into(),
            operation: operation(Path::new(&project.repo_path), &["bun", "install"]),
            provider_resume_token: Some(secret.into()),
        })
        .await?;
    assert_eq!(request.provider_resume_token, None);

    let stored: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT provider_resume_token,provider_resume_secret_ref FROM permission_requests WHERE id=?",
    )
    .bind(&request.id)
    .fetch_one(orchestrator.store.pool())
    .await?;
    assert_eq!(stored.0, None);
    let secret_ref = stored
        .1
        .ok_or_else(|| std::io::Error::other("missing opaque secret reference"))?;
    assert!(!secret_ref.contains(secret));
    assert_eq!(
        orchestrator
            .store
            .get_resume_token(&secret_ref)
            .await?
            .as_deref(),
        Some(secret)
    );
    Ok(())
}

#[tokio::test]
async fn expired_request_is_persisted_and_cannot_be_approved()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, task) = setup_permission_task().await?;
    let op = operation(Path::new(&project.repo_path), &["bun", "install"]);
    let request = orchestrator
        .permission_request(PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: None,
            provider_id: AgentKind::Codex,
            role: RunRole::Developer,
            action_type: PermissionActionType::DependencyInstall,
            reason: "expiry probe".into(),
            operation: op,
            provider_resume_token: None,
        })
        .await?;
    sqlx::query("UPDATE permission_requests SET expires_at='2000-01-01T00:00:00Z' WHERE id=?")
        .bind(&request.id)
        .execute(orchestrator.store.pool())
        .await?;
    assert_eq!(orchestrator.permission_expire_pending().await?, 1);
    assert_eq!(
        orchestrator.permission_requests(&task.id).await?[0].status,
        PermissionRequestStatus::Expired
    );
    let error = require_permission_error(
        orchestrator
            .permission_decide(PermissionDecisionInput {
                request_id: request.id,
                operation_sha256: request.operation_sha256,
                policy_sha256: request.policy_sha256,
                decision: PermissionDecisionKind::Approve,
                scope: PermissionGrantScope::Once,
                guidance: None,
            })
            .await,
        "expired permission was accepted",
    )?;
    assert!(error.to_string().contains("PERMISSION_EXPIRED"));
    Ok(())
}

async fn setup_permission_task()
-> Result<(TempDir, Orchestrator, Project, TaskSummary), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let repo = root.path().join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    let orchestrator = Orchestrator::open(root.path()).await?;
    let project = orchestrator
        .store
        .import_project(
            "p",
            &repo.to_string_lossy(),
            "main",
            &root.path().join("wt").to_string_lossy(),
        )
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "t",
            "d",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;
    Ok((root, orchestrator, project, task))
}
