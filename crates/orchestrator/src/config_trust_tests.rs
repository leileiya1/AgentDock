use super::*;

async fn git(repo: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .await?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

async fn repo_with_config(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let repo = root.join("repo");
    tokio::fs::create_dir_all(repo.join(".agentflow")).await?;
    git(&repo, &["init", "-q"]).await?;
    git(&repo, &["branch", "-M", "main"]).await?;
    git(&repo, &["config", "user.name", "AgentFlow Test"]).await?;
    git(&repo, &["config", "user.email", "test@agentflow.local"]).await?;
    tokio::fs::write(repo.join("README.md"), "config trust test\n").await?;
    tokio::fs::write(
        repo.join(".agentflow/project.toml"),
        concat!(
            "schema_version=1\n",
            "[[validate.steps]]\n",
            "name='write marker'\n",
            "argv=['/bin/sh','-c','printf trusted > validation-ran.txt']\n",
            "timeout_secs=10\n",
            "[agents]\n",
            "extra_allowed_commands=['bun test']\n",
        ),
    )
    .await?;
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-q", "-m", "base with config"]).await?;
    Ok(repo)
}

async fn new_task(
    orchestrator: &Orchestrator,
    project_id: &str,
    title: &str,
) -> Result<TaskSummary, OrchestratorError> {
    orchestrator
        .task_create(
            project_id,
            title,
            "exercise project config trust",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await
}

#[tokio::test]
async fn untrusted_or_changed_project_config_is_blocked_before_git_side_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = repo_with_config(root.path()).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let preview = orchestrator.project_config_trust_get(&project.id).await?;
    assert!(!preview.trusted);
    assert!(
        preview
            .changes
            .iter()
            .any(|change| { change.path == "validate.steps" && change.high_risk })
    );
    assert_eq!(preview.validation_commands[0].argv[0], "/bin/sh");
    let first = new_task(&orchestrator, &project.id, "untrusted").await?;

    let error = match orchestrator.task_start(&first.id).await {
        Ok(_) => return Err("repository commands were trusted without approval".into()),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        OrchestratorError::UntrustedProjectConfig { .. }
    ));
    let row = orchestrator.task(&first.id).await?;
    assert_eq!(row.status, TaskStatus::Draft);
    assert!(row.branch.is_none());
    assert!(row.worktree_path.is_none());

    let expected_sha = preview.sha256.as_deref().ok_or("missing config sha")?;
    orchestrator
        .project_config_trust_approve(&project.id, expected_sha)
        .await?;
    let approved = orchestrator.project_config_trust_get(&project.id).await?;
    assert!(approved.trusted);
    assert!(approved.changes.is_empty());

    // Fault injection: one-byte-equivalent content change invalidates the out-of-repo seal.
    let path = repo.join(".agentflow/project.toml");
    let mut changed = tokio::fs::read_to_string(&path).await?;
    changed.push_str("\n# changed after approval\n");
    tokio::fs::write(&path, changed).await?;
    let second = new_task(&orchestrator, &project.id, "changed").await?;
    let error = match orchestrator.task_start(&second.id).await {
        Ok(_) => return Err("changed config retained its old approval".into()),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        OrchestratorError::UntrustedProjectConfig { .. }
    ));
    let comment_only = orchestrator.project_config_trust_get(&project.id).await?;
    assert!(!comment_only.trusted);
    assert!(comment_only.byte_only_change);
    assert!(comment_only.changes.is_empty());

    let changed = tokio::fs::read_to_string(&path).await?.replace(
        "extra_allowed_commands=['bun test']",
        "extra_allowed_commands=['bun test --watch']",
    );
    tokio::fs::write(&path, changed).await?;
    let permission_change = orchestrator.project_config_trust_get(&project.id).await?;
    assert!(!permission_change.byte_only_change);
    assert!(
        permission_change
            .changes
            .iter()
            .any(|change| { change.path == "agents.extra_allowed_commands" && change.high_risk })
    );
    Ok(())
}

#[test]
fn project_config_preview_redacts_inline_credentials() {
    assert_eq!(safe_config_text("--token=top-secret-value"), "--token=***");
}

#[tokio::test]
async fn approval_rejects_a_snapshot_that_changed_after_preview()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = repo_with_config(root.path()).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let preview = orchestrator.project_config_trust_get(&project.id).await?;
    let stale_sha = preview.sha256.ok_or("missing config sha")?;

    tokio::fs::write(
        repo.join(".agentflow/project.toml"),
        "schema_version=1\n[agents]\nextra_allowed_commands=['dangerous-new-command']\n",
    )
    .await?;

    let error = match orchestrator
        .project_config_trust_approve(&project.id, &stale_sha)
        .await
    {
        Ok(_) => return Err("stale preview authorized changed commands".into()),
        Err(error) => error,
    };
    assert!(
        matches!(error, OrchestratorError::InvalidState(message) if message == "PROJECT_CONFIG_CHANGED_DURING_APPROVAL")
    );
    assert!(
        !orchestrator
            .project_config_trust_get(&project.id)
            .await?
            .trusted
    );
    Ok(())
}

#[tokio::test]
async fn approved_config_runs_its_validation_in_a_real_worktree()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = repo_with_config(root.path()).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let preview = orchestrator.project_config_trust_get(&project.id).await?;
    let expected_sha = preview.sha256.as_deref().ok_or("missing config sha")?;
    let trust = orchestrator
        .project_config_trust_approve(&project.id, expected_sha)
        .await?;
    assert!(trust.trusted);
    assert_eq!(trust.validation_steps, vec!["write marker"]);
    assert_eq!(trust.extra_allowed_commands, vec!["bun test"]);

    let task = new_task(&orchestrator, &project.id, "trusted e2e").await?;
    orchestrator.task_start(&task.id).await?;
    let task = orchestrator.task(&task.id).await?;
    let project = orchestrator.project(&project.id).await?;
    let config = orchestrator.load_trusted_config(&project).await?;
    let worktree = required_path(&task.worktree_path)?;
    let report = orchestrator
        .execute_validation(&task, &worktree, &config.validate.steps)
        .await?;
    assert!(report.passed);
    assert_eq!(
        tokio::fs::read_to_string(worktree.join("validation-ran.txt")).await?,
        "trusted"
    );
    Ok(())
}
