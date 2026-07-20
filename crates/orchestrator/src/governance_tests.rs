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

async fn fixture()
-> Result<(tempfile::TempDir, Orchestrator, Project, PathBuf), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let repo = root.path().join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    git(&repo, &["init", "-q", "-b", "main"]).await?;
    git(&repo, &["config", "user.email", "test@agentflow.local"]).await?;
    git(&repo, &["config", "user.name", "AgentFlow Test"]).await?;
    tokio::fs::write(repo.join("tracked.txt"), "base\n").await?;
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-q", "-m", "base"]).await?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator.project_import(&repo).await?;
    Ok((root, orchestrator, project, repo))
}

async fn governed_task(
    orchestrator: &Orchestrator,
    project: &Project,
    title: &str,
    policy: TaskPolicy,
) -> Result<TaskSummary, OrchestratorError> {
    orchestrator
        .task_create_governed(
            &project.id,
            title,
            "exercise execution governance",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(3),
            false,
            policy,
        )
        .await
}

async fn seed_pending_plan(
    orchestrator: &Orchestrator,
    task_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO task_plans(id,task_id,version,status,summary,steps_json,risks_json,created_at) VALUES(?,?,1,'pending','safe plan','[]','[]',?)")
        .bind(&id).bind(task_id).bind(&now).execute(orchestrator.store.pool()).await?;
    sqlx::query("UPDATE tasks SET status='WAITING_FOR_PLAN_APPROVAL' WHERE id=?")
        .bind(task_id)
        .execute(orchestrator.store.pool())
        .await?;
    Ok(id)
}

#[tokio::test]
async fn plan_gate_requires_an_explicit_human_decision() -> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, _repo) = fixture().await?;
    let approved = governed_task(
        &orchestrator,
        &project,
        "approved plan",
        TaskPolicy::default(),
    )
    .await?;
    assert_eq!(
        orchestrator.task_start(&approved.id).await?.status,
        TaskStatus::Planning
    );
    let plan_id = seed_pending_plan(&orchestrator, &approved.id).await?;
    let result = orchestrator
        .task_plan_approve(&approved.id, &plan_id)
        .await?;
    assert_eq!(result.status, TaskStatus::ReadyForDevelopment);
    let Some(plan) = orchestrator.task_get(&approved.id).await?.plan else {
        return Err("approved plan was not persisted".into());
    };
    assert_eq!(plan.status, PlanStatus::Approved);

    let rejected = governed_task(
        &orchestrator,
        &project,
        "rejected plan",
        TaskPolicy::default(),
    )
    .await?;
    orchestrator.task_start(&rejected.id).await?;
    let plan_id = seed_pending_plan(&orchestrator, &rejected.id).await?;
    let result = orchestrator
        .task_plan_reject(&rejected.id, &plan_id, "add rollback validation")
        .await?;
    assert_eq!(result.status, TaskStatus::Planning);
    let Some(plan) = orchestrator.task_get(&rejected.id).await?.plan else {
        return Err("rejected plan was not persisted".into());
    };
    assert_eq!(plan.status, PlanStatus::Rejected);
    Ok(())
}

#[tokio::test]
async fn exhausted_budget_stops_and_resumes_from_the_saved_checkpoint()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, _repo) = fixture().await?;
    let policy = TaskPolicy {
        require_plan_approval: false,
        token_budget: Some(100),
        cost_budget_usd: None,
        time_budget_secs: None,
        ..TaskPolicy::default()
    };
    let task = governed_task(&orchestrator, &project, "budget", policy).await?;
    orchestrator.task_start(&task.id).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,tokens_in,tokens_out,started_at,finished_at,created_at) VALUES('budget-run',?,0,'planner','codex','SUCCEEDED','/tmp/budget-run',10,10,60,40,?,?,?)")
        .bind(&task.id).bind(&now).bind(&now).bind(&now).execute(orchestrator.store.pool()).await?;
    let row = orchestrator.task(&task.id).await?;
    assert!(orchestrator.enforce_budget(&row).await?);
    let stopped = orchestrator.task_get(&task.id).await?;
    assert_eq!(stopped.summary.status, TaskStatus::Blocked);
    assert_eq!(
        stopped.summary.blocked_reason,
        Some(BlockedReason::BudgetExceeded)
    );

    let resumed = orchestrator
        .task_budget_update(
            &task.id,
            BudgetLimitPatch {
                token_budget: Some(200),
                cost_budget_usd: None,
                time_budget_secs: None,
            },
        )
        .await?;
    assert_eq!(resumed.status, TaskStatus::ReadyForDevelopment);
    let usage = orchestrator.budget_usage(&task.id).await?;
    assert_eq!(usage.tokens_used, 100);
    assert_eq!(usage.token_budget, Some(200));
    assert!(!usage.exceeded);

    sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,tokens_in,tokens_out,created_at) VALUES('review-budget-run',?,1,'reviewer','claude_code','SUCCEEDED','/tmp/review-budget-run',10,10,50,50,?)")
        .bind(&task.id).bind(Utc::now().to_rfc3339()).execute(orchestrator.store.pool()).await?;
    sqlx::query("UPDATE tasks SET status='REVIEWING',current_revision=1 WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    let reviewing = orchestrator.task(&task.id).await?;
    assert!(orchestrator.enforce_budget(&reviewing).await?);
    let resumed = orchestrator
        .task_budget_update(
            &task.id,
            BudgetLimitPatch {
                token_budget: Some(300),
                cost_budget_usd: None,
                time_budget_secs: None,
            },
        )
        .await?;
    assert_eq!(resumed.status, TaskStatus::ReadyForReview);
    assert_eq!(resumed.current_revision, 1);
    Ok(())
}

#[tokio::test]
async fn execution_nodes_are_checked_and_referenced_nodes_cannot_be_deleted()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, _repo) = fixture().await?;
    let node = orchestrator
        .execution_node_upsert(ExecutionNode {
            id: String::new(),
            name: "offline test node".into(),
            host: "127.0.0.1".into(),
            port: 1,
            username: "runner".into(),
            work_root: "/tmp/agentflow-node".into(),
            enabled: true,
            status: NodeStatus::Unknown,
            platform: None,
            git_version: None,
            problem: None,
            last_checked_at: None,
        })
        .await?;
    assert_eq!(
        orchestrator.execution_node_check(&node.id).await?.status,
        NodeStatus::Offline
    );
    let policy = TaskPolicy {
        require_plan_approval: false,
        execution_node_id: Some(node.id.clone()),
        ..TaskPolicy::default()
    };
    governed_task(&orchestrator, &project, "remote node reference", policy).await?;
    assert!(orchestrator.execution_node_delete(&node.id).await.is_err());

    let disposable = orchestrator
        .execution_node_upsert(ExecutionNode {
            id: String::new(),
            name: "disposable".into(),
            host: "runner.local".into(),
            port: 22,
            username: "runner".into(),
            work_root: "/tmp/agentflow".into(),
            enabled: false,
            status: NodeStatus::Unknown,
            platform: None,
            git_version: None,
            problem: None,
            last_checked_at: None,
        })
        .await?;
    orchestrator.execution_node_delete(&disposable.id).await?;
    assert!(
        orchestrator
            .execution_node_list()
            .await?
            .iter()
            .all(|item| item.id != disposable.id)
    );
    Ok(())
}

async fn prepare_quality_revision(
    orchestrator: &Orchestrator,
    project: &Project,
    title: &str,
) -> Result<(TaskSummary, String), Box<dyn std::error::Error>> {
    let policy = TaskPolicy {
        require_plan_approval: false,
        ..TaskPolicy::default()
    };
    let task = governed_task(orchestrator, project, title, policy).await?;
    orchestrator.task_start(&task.id).await?;
    let row = orchestrator.task(&task.id).await?;
    let worktree = required_path(&row.worktree_path)?;
    tokio::fs::write(
        worktree.join(format!("feature-{}.txt", task.seq)),
        "implemented\n",
    )
    .await?;
    git(&worktree, &["add", "."]).await?;
    git(&worktree, &["commit", "-q", "-m", "feature revision"]).await?;
    let sha = git(&worktree, &["rev-parse", "HEAD"]).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,diff_stat_json,created_at) VALUES(?,?,1,?,?,?)")
        .bind(Uuid::now_v7().to_string()).bind(&task.id).bind(&sha)
        .bind(serde_json::to_string(&DiffStat { files: 1, insertions: 1, deletions: 0, flagged: vec![] })?)
        .bind(&now).execute(orchestrator.store.pool()).await?;
    sqlx::query("UPDATE tasks SET current_revision=1,status='READY_FOR_REVIEW' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    let run_dir = orchestrator
        .task_dir(&task.id)
        .join("runs/developer-fixture");
    tokio::fs::create_dir_all(&run_dir).await?;
    tokio::fs::write(run_dir.join("input.md"), "reproducible input").await?;
    sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES(?, ?,1,'developer','codex','SUCCEEDED',?,10,10,?)")
        .bind(format!("developer-{}", task.id)).bind(&task.id).bind(run_dir.to_string_lossy().as_ref()).bind(&now)
        .execute(orchestrator.store.pool()).await?;
    let review_run = format!("reviewer-{}", task.id);
    sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES(?, ?,1,'reviewer','claude_code','SUCCEEDED',?,10,10,?)")
        .bind(&review_run).bind(&task.id).bind(run_dir.to_string_lossy().as_ref()).bind(&now)
        .execute(orchestrator.store.pool()).await?;
    sqlx::query("INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,summary,raw_path,created_at) VALUES(?,?,1,?,?,'pass','independent pass',?,?)")
        .bind(format!("review-{}", task.id)).bind(&task.id).bind(&review_run).bind(&sha)
        .bind(run_dir.join("result.json").to_string_lossy().as_ref()).bind(&now)
        .execute(orchestrator.store.pool()).await?;
    let artifact_dir = orchestrator.task_dir(&task.id).join("artifacts");
    tokio::fs::create_dir_all(&artifact_dir).await?;
    tokio::fs::write(artifact_dir.join("r1.patch"), b"fixed patch bytes").await?;
    let config = ProjectConfig {
        validate: ValidateConfig {
            steps: vec![ValidateStep {
                name: "tracked file".into(),
                argv: vec!["/bin/sh".into(), "-c".into(), "test -f tracked.txt".into()],
                timeout_secs: 10,
            }],
        },
        ..ProjectConfig::default()
    };
    let row = orchestrator.task(&task.id).await?;
    let first = orchestrator
        .record_reproducibility_manifest(&row, &config)
        .await?;
    let second = orchestrator
        .record_reproducibility_manifest(&row, &config)
        .await?;
    assert_eq!(first.manifest_sha256, second.manifest_sha256);
    let quality = orchestrator.task_quality_replay(&task.id, Some(1)).await?;
    assert!(quality.passed);
    assert!(quality.replay);
    assert_eq!(quality.score, 100);
    sqlx::query("UPDATE tasks SET status='APPROVED' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    Ok((task, sha))
}

#[tokio::test]
async fn reproducible_quality_gate_local_delivery_and_both_rollbacks_work()
-> Result<(), Box<dyn std::error::Error>> {
    let (_root, orchestrator, project, repo) = fixture().await?;
    let base = git(&repo, &["rev-parse", "HEAD"]).await?;
    let (undo_task, _) = prepare_quality_revision(&orchestrator, &project, "undo delivery").await?;
    assert_eq!(
        orchestrator
            .task_delivery_start(&undo_task.id)
            .await?
            .status,
        TaskStatus::Merged
    );
    assert_eq!(
        orchestrator
            .task_rollback(&undo_task.id, RollbackStrategy::Undo)
            .await?
            .status,
        TaskStatus::RolledBack
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]).await?, base);

    let (revert_task, _) =
        prepare_quality_revision(&orchestrator, &project, "revert delivery").await?;
    assert_eq!(
        orchestrator
            .task_delivery_start(&revert_task.id)
            .await?
            .status,
        TaskStatus::Merged
    );
    tokio::fs::write(repo.join("later.txt"), "later work\n").await?;
    git(&repo, &["add", "later.txt"]).await?;
    git(&repo, &["commit", "-q", "-m", "later unrelated work"]).await?;
    assert!(matches!(
        orchestrator
            .task_rollback(&revert_task.id, RollbackStrategy::Undo)
            .await,
        Err(OrchestratorError::RollbackUnsafe(_))
    ));
    assert_eq!(
        orchestrator
            .task_rollback(&revert_task.id, RollbackStrategy::Revert)
            .await?
            .status,
        TaskStatus::RolledBack
    );
    assert!(repo.join("later.txt").exists());
    assert!(
        !repo
            .join(format!("feature-{}.txt", revert_task.seq))
            .exists()
    );
    Ok(())
}
