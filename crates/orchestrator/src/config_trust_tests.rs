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

    orchestrator
        .project_config_trust_approve(&project.id)
        .await?;
    let approved = orchestrator.project_config_trust_get(&project.id).await?;
    assert!(approved.trusted);

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
    let trust = orchestrator
        .project_config_trust_approve(&project.id)
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
