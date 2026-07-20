use super::*;

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
                name: "clean fixed commit".into(),
                argv: vec!["git".into(), "diff".into(), "--quiet".into(), "HEAD".into()],
                timeout_secs: 10,
            }],
        },
        reproducibility: ReproducibilityConfig {
            lock_environment: true,
            external_dependencies: [("fixture-db".into(), "snapshot-1".into())].into(),
            ..ReproducibilityConfig::default()
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
    assert_eq!(
        first.reproducibility_level,
        ReproducibilityLevel::EnvironmentLocked
    );
    assert_ne!(
        first.tool_versions.get("git").map(String::as_str),
        Some("unavailable")
    );
    assert!(!first.environment_sha256.is_empty());
    assert!(first.environment_variables.contains_key("LANG"));
    let quality = orchestrator.task_quality_replay(&task.id, Some(1)).await?;
    assert!(quality.passed);
    assert!(quality.replay);
    assert_eq!(quality.score, 100);
    // Fault injection: replay metadata now points at a different external snapshot.
    // A locked manifest must reject it before running validation.
    let repro_path = artifact_dir.join("r1-reproducibility-config.json");
    let mut repro: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&repro_path).await?)?;
    repro["external_dependencies"]["fixture-db"] = json!("snapshot-2");
    tokio::fs::write(&repro_path, serde_json::to_vec(&repro)?).await?;
    let drift = orchestrator.task_quality_replay(&task.id, Some(1)).await;
    assert!(matches!(
        drift,
        Err(OrchestratorError::InvalidState(message)) if message.contains("REPRODUCIBILITY_DRIFT")
    ));
    sqlx::query("UPDATE tasks SET status='APPROVED' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    sqlx::query("INSERT INTO approvals(id,task_id,revision,commit_sha,diff_sha256,action,created_at) VALUES(?,?,1,?,'fixture','approve',?)")
        .bind(Uuid::now_v7().to_string()).bind(&task.id).bind(&sha).bind(&now)
        .execute(orchestrator.store.pool()).await?;
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
