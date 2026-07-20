#[cfg(test)]
mod failure_tests {
    use super::*;
    use async_trait::async_trait;

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

    async fn init_repo(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let repo = root.join("repo");
        tokio::fs::create_dir_all(&repo).await?;
        git(&repo, &["init", "-q"]).await?;
        git(&repo, &["branch", "-M", "main"]).await?;
        git(&repo, &["config", "user.name", "AgentFlow Test"]).await?;
        git(&repo, &["config", "user.email", "test@agentflow.local"]).await?;
        tokio::fs::write(repo.join("shared.txt"), "base\n").await?;
        git(&repo, &["add", "shared.txt"]).await?;
        git(&repo, &["commit", "-q", "-m", "base"]).await?;
        Ok(repo)
    }

    async fn prepared_task(
        orchestrator: &Orchestrator,
        repo: &Path,
        max_revisions: i64,
    ) -> Result<(Project, TaskSummary, PathBuf), Box<dyn std::error::Error>> {
        let project = orchestrator.project_import(repo).await?;
        let task = orchestrator
            .task_create(
                &project.id,
                "failure path",
                "exercise recovery behavior",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                Some(max_revisions),
            )
            .await?;
        orchestrator.task_start(&task.id).await?;
        let row = orchestrator.task(&task.id).await?;
        Ok((project, task, required_path(&row.worktree_path)?))
    }

    async fn commit_revision_fixture(
        orchestrator: &Orchestrator,
        task: &TaskSummary,
        worktree: &Path,
        content: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        tokio::fs::write(worktree.join("shared.txt"), content).await?;
        git(worktree, &["add", "shared.txt"]).await?;
        git(worktree, &["commit", "-q", "-m", "agent change"]).await?;
        let sha = git(worktree, &["rev-parse", "HEAD"]).await?;
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,created_at) VALUES(?,?,1,?,?)")
            .bind(Uuid::now_v7().to_string())
            .bind(&task.id)
            .bind(&sha)
            .bind(Utc::now().to_rfc3339())
            .execute(orchestrator.store.pool())
            .await?;
        sqlx::query("UPDATE tasks SET status='WAITING_FOR_HUMAN_APPROVAL',current_revision=1 WHERE id=?")
            .bind(&task.id)
            .execute(orchestrator.store.pool())
            .await?;
        Ok(sha)
    }

    #[tokio::test]
    async fn rejection_enters_rework_and_is_carried_into_next_round_memory()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(root.path().join("data")).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 3).await?;
        commit_revision_fixture(&orchestrator, &task, &worktree, "agent version\n").await?;

        let rejected = orchestrator
            .reject(&task.id, 1, "保留输出格式，并补充边界测试")
            .await?;
        assert_eq!(rejected.status, TaskStatus::ReadyForRevision);
        let approvals: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM approvals WHERE task_id=? AND action='reject'",
        )
        .bind(&task.id)
        .fetch_one(orchestrator.store.pool())
        .await?;
        assert_eq!(approvals, 1);

        let mut next = orchestrator.task(&task.id).await?;
        next.revision = 2;
        tokio::fs::create_dir_all(worktree.join(".agentflow-in")).await?;
        let Some(history) = orchestrator.write_history(&next, &worktree).await? else {
            return Err("rework did not receive memory".into());
        };
        assert!(history.contains("保留输出格式，并补充边界测试"));
        assert!(history.contains("人工反馈"));
        Ok(())
    }

    #[tokio::test]
    async fn conflicting_main_change_becomes_recoverable_merge_conflict()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(root.path().join("data")).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        commit_revision_fixture(&orchestrator, &task, &worktree, "agent version\n").await?;
        sqlx::query("UPDATE tasks SET status='APPROVED' WHERE id=?")
            .bind(&task.id)
            .execute(orchestrator.store.pool())
            .await?;

        tokio::fs::write(repo.join("shared.txt"), "main version\n").await?;
        git(&repo, &["add", "shared.txt"]).await?;
        git(&repo, &["commit", "-q", "-m", "conflicting main change"]).await?;
        let merged = orchestrator.merge(&task.id).await?;
        assert_eq!(merged.status, TaskStatus::MergeConflict);
        assert!(git(&repo, &["status", "--porcelain"]).await?.is_empty());
        assert!(!repo.join(".git/MERGE_HEAD").exists());
        assert!(orchestrator
            .events_list(&task.id, None, None)
            .await?
            .iter()
            .any(|event| event.event_type == "merge:conflict"));
        Ok(())
    }

    struct SlowShellAdapter;

    #[async_trait]
    impl agentflow_agent_adapters::AgentProvider for SlowShellAdapter {
        fn kind(&self) -> AgentKind {
            AgentKind::Codex
        }
        async fn detect(
            &self,
            _env: &agentflow_agent_adapters::CliEnv,
        ) -> Result<agentflow_agent_adapters::AgentInstallation, agentflow_agent_adapters::AdapterError>
        {
            Err(agentflow_agent_adapters::AdapterError::NotFound("test".into()))
        }
        fn capabilities(&self) -> agentflow_agent_adapters::AgentCapabilities {
            agentflow_agent_adapters::AgentCapabilities {
                streams_events: true,
                native_output_schema: false,
                supports_resume: false,
                read_only_mode: false,
                supports_development: true,
                supports_review: false,
            }
        }
        async fn start(
            &self,
            req: AgentRunRequest,
            cancel: CancellationToken,
            tx: mpsc::Sender<AgentEvent>,
        ) -> Result<agentflow_agent_adapters::RunningAgent, agentflow_agent_adapters::AdapterError>
        {
            let outcome = agentflow_process_supervisor::run(
                agentflow_process_supervisor::ProcessSpec {
                    program: "/bin/sh".into(),
                    args: vec!["-c".into(), "sleep 30 & wait".into()],
                    cwd: req.worktree,
                    env: HashMap::new(),
                    env_denylist: Vec::new(),
                    timeout: req.timeout,
                    idle_timeout: req.idle_timeout,
                    stdout_path: req.run_dir.join("stdout.log"),
                    stderr_path: req.run_dir.join("stderr.log"),
                    lease_path: req.run_dir.join("process-lease.json"),
                },
                cancel,
                tx,
            )
            .await?;
            Ok(agentflow_agent_adapters::RunningAgent {
                outcome,
                run_dir: req.run_dir,
                role: req.role,
            })
        }
        async fn collect_result(
            &self,
            _run_dir: &Path,
            role: RunRole,
        ) -> Result<CollectedResult, agentflow_agent_adapters::AdapterError> {
            Err(agentflow_agent_adapters::AdapterError::UnsupportedRole(role))
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn client_cancellation_reaches_daemon_run_and_cleans_the_process_tree()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let worktree = root.path().join("worktree");
        tokio::fs::create_dir_all(&worktree).await?;
        let owner = Arc::new(Orchestrator::open(&data).await?);
        let project = owner
            .store
            .import_project("cancel", "/tmp/cancel-e2e", "main", "/tmp/cancel-e2e-wt")
            .await?;
        let task = owner
            .task_create(
                &project.id,
                "cancel tree",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                None,
            )
            .await?;
        sqlx::query("UPDATE tasks SET status='DEVELOPING',current_revision=1,worktree_path=? WHERE id=?")
            .bind(worktree.to_string_lossy().as_ref())
            .bind(&task.id)
            .execute(owner.store.pool())
            .await?;
        let task_row = owner.task(&task.id).await?;
        let project_row = owner.project(&project.id).await?;
        let run_dir = owner.run_dir(&task.id);
        let run_owner = Arc::clone(&owner);
        let run = tokio::spawn(async move {
            run_owner
                .run_agent(
                    &SlowShellAdapter,
                    &task_row,
                    &project_row,
                    &run_dir,
                    RunRole::Developer,
                    ".agentflow-in/input.md",
                    &ProjectConfig::default(),
                    None,
                )
                .await
        });
        for _ in 0..100 {
            let running: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND status='RUNNING' AND child_pid IS NOT NULL",
            )
            .bind(&task.id)
            .fetch_one(owner.store.pool())
            .await?;
            if running == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let client = Orchestrator::open_client(&data).await?;
        assert_eq!(client.cancel(&task.id).await?.status, TaskStatus::Cancelled);
        let outcome = run.await??;
        assert!(outcome.outcome.cancelled);
        let status: String = sqlx::query_scalar(
            "SELECT status FROM agent_runs WHERE task_id=? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&task.id)
        .fetch_one(owner.store.pool())
        .await?;
        assert_eq!(status, "CANCELLED");
        Ok(())
    }
}
