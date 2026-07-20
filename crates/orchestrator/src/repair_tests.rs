use super::*;

async fn git(cwd: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned().into())
    }
}

async fn fixture()
-> Result<(tempfile::TempDir, Orchestrator, TaskSummary), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = root.path().join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    git(&repo, &["init", "-q", "-b", "main"]).await?;
    git(&repo, &["config", "user.email", "test@example.com"]).await?;
    git(&repo, &["config", "user.name", "Test"]).await?;
    tokio::fs::write(repo.join("tracked.txt"), "base\n").await?;
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-q", "-m", "base"]).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "repair fixture",
            "test recovery",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            None,
        )
        .await?;
    orchestrator.task_start(&task.id).await?;
    Ok((root, orchestrator, task))
}

#[tokio::test]
async fn missing_owned_worktree_is_rebuilt_on_the_same_branch()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, task) = fixture().await?;
    let row = orchestrator.task(&task.id).await?;
    let project = orchestrator.project(&row.project_id).await?;
    let worktree = required_path(&row.worktree_path)?;
    orchestrator
        .git
        .worktree_remove(&project.repo, &worktree)
        .await?;
    sqlx::query("UPDATE tasks SET status='BLOCKED',blocked_reason='worktree_missing',repair_resume_status='READY_FOR_DEVELOPMENT' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;

    let report = orchestrator.task_repair_inspect(&task.id).await?;
    assert_eq!(report.actions, vec![RepairAction::RebuildWorktree]);
    let repaired = orchestrator
        .task_repair_apply(&task.id, RepairAction::RebuildWorktree)
        .await?;
    assert_eq!(repaired.status, TaskStatus::ReadyForDevelopment);
    assert_eq!(
        tokio::fs::read_to_string(worktree.join("tracked.txt")).await?,
        "base\n"
    );
    Ok(())
}

#[tokio::test]
async fn reset_preserves_residual_checkpoint_then_restores_clean_head()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, task) = fixture().await?;
    let row = orchestrator.task(&task.id).await?;
    let worktree = required_path(&row.worktree_path)?;
    orchestrator.create_checkpoint(&row, "before-test").await?;
    tokio::fs::write(worktree.join("tracked.txt"), "dirty\n").await?;
    tokio::fs::write(worktree.join("untracked.txt"), "keep me\n").await?;
    sqlx::query("UPDATE tasks SET status='BLOCKED',blocked_reason='run_failed',repair_resume_status='READY_FOR_DEVELOPMENT' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;

    orchestrator
        .task_repair_apply(&task.id, RepairAction::ResetToCheckpoint)
        .await?;
    assert_eq!(
        tokio::fs::read_to_string(worktree.join("tracked.txt")).await?,
        "base\n"
    );
    assert!(!worktree.join("untracked.txt").exists());
    let preserved: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_checkpoints WHERE task_id=? AND phase='repair-preserve' AND patch_sha256 IS NOT NULL AND untracked_files=1")
        .bind(&task.id)
        .fetch_one(orchestrator.store.pool())
        .await?;
    assert_eq!(preserved, 1);
    Ok(())
}

#[tokio::test]
async fn repair_refuses_a_path_outside_the_owned_worktree_slot()
-> Result<(), Box<dyn std::error::Error>> {
    let (root, orchestrator, task) = fixture().await?;
    let outside = root.path().join("outside");
    tokio::fs::create_dir_all(&outside).await?;
    sqlx::query("UPDATE tasks SET status='BLOCKED',blocked_reason='worktree_missing',worktree_path=? WHERE id=?")
        .bind(outside.to_string_lossy().as_ref())
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    let result = orchestrator
        .task_repair_apply(&task.id, RepairAction::ResetToCheckpoint)
        .await;
    assert!(
        matches!(result, Err(OrchestratorError::InvalidState(message)) if message.contains("non-AgentFlow"))
    );
    Ok(())
}

#[tokio::test]
async fn repair_still_accepts_the_exact_legacy_slot_for_existing_tasks()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, task) = fixture().await?;
    let row = orchestrator.task(&task.id).await?;
    let project = orchestrator.project(&row.project_id).await?;
    let legacy = project
        .worktree_root
        .join(format!("p{}/t{}", project.seq, row.seq));
    orchestrator.validate_owned_worktree(&row, &project, &legacy)?;
    Ok(())
}
