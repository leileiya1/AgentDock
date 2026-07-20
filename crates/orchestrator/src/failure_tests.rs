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

    async fn approve_revision_fixture(
        orchestrator: &Orchestrator,
        task: &TaskSummary,
        worktree: &Path,
        content: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let sha = commit_revision_fixture(orchestrator, task, worktree, content).await?;
        let diff = orchestrator.diff_get(&task.id, 1).await?;
        orchestrator
            .approve(&task.id, 1, &sha, &diff.diff_sha256)
            .await?;
        Ok(sha)
    }

    #[tokio::test]
    async fn approval_rejects_a_client_sha_that_is_not_the_recorded_revision()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(root.path().join("data")).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        commit_revision_fixture(&orchestrator, &task, &worktree, "reviewed version\n").await?;

        // Fault injection: move the mutable branch and submit its valid diff hash while the
        // orchestrator's revision row still points to the reviewed commit.
        tokio::fs::write(worktree.join("shared.txt"), "unreviewed version\n").await?;
        git(&worktree, &["add", "shared.txt"]).await?;
        git(&worktree, &["commit", "-q", "-m", "unreviewed change"]).await?;
        let injected_sha = git(&worktree, &["rev-parse", "HEAD"]).await?;
        let row = orchestrator.task(&task.id).await?;
        let injected_diff = orchestrator
            .git
            .diff(
                &worktree,
                row.base_commit.as_deref().ok_or("base commit missing")?,
                &injected_sha,
                &default_excludes(),
                default_patch_bytes(),
            )
            .await?;
        let error = match orchestrator
            .approve(&task.id, 1, &injected_sha, &injected_diff.diff_sha256)
            .await
        {
            Ok(_) => return Err("a client replaced the authoritative revision SHA".into()),
            Err(error) => error,
        };
        assert!(matches!(error, OrchestratorError::DiffStale));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM approvals WHERE task_id=?")
                .bind(&task.id)
                .fetch_one(orchestrator.store.pool())
                .await?,
            0
        );
        assert_eq!(
            orchestrator.task_get(&task.id).await?.summary.status,
            TaskStatus::WaitingForHumanApproval
        );
        Ok(())
    }

    #[tokio::test]
    async fn merge_rejects_a_branch_that_advanced_after_approval()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(root.path().join("data")).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        approve_revision_fixture(&orchestrator, &task, &worktree, "approved version\n").await?;
        let main_before = git(&repo, &["rev-parse", "HEAD"]).await?;

        // Fault injection: simulate a provider writing one more commit after the user clicked
        // approve but before the merge command was handled.
        tokio::fs::write(worktree.join("shared.txt"), "post-approval mutation\n").await?;
        git(&worktree, &["add", "shared.txt"]).await?;
        git(&worktree, &["commit", "-q", "-m", "post approval mutation"]).await?;
        let error = match orchestrator.merge(&task.id).await {
            Ok(_) => return Err("a post-approval commit was merged".into()),
            Err(error) => error,
        };
        assert!(matches!(error, OrchestratorError::MergePrecondition(_)));
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]).await?, main_before);
        assert_eq!(
            orchestrator.task_get(&task.id).await?.summary.status,
            TaskStatus::Approved
        );
        Ok(())
    }

    #[tokio::test]
    async fn external_merge_requires_git_ancestry_proof()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(root.path().join("data")).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        approve_revision_fixture(&orchestrator, &task, &worktree, "approved version\n").await?;

        let error = match orchestrator.mark_merged_external(&task.id).await {
            Ok(_) => return Err("a UI signal bypassed Git ancestry proof".into()),
            Err(error) => error,
        };
        assert!(matches!(error, OrchestratorError::MergePrecondition(_)));

        let branch = orchestrator
            .task(&task.id)
            .await?
            .branch
            .ok_or("task branch missing")?;
        git(
            &repo,
            &["merge", "--no-ff", &branch, "-m", "external merge"],
        )
        .await?;
        let merged = orchestrator.mark_merged_external(&task.id).await?;
        assert_eq!(merged.status, TaskStatus::Merged);
        Ok(())
    }

    #[tokio::test]
    async fn start_saga_rolls_forward_after_git_succeeds_but_database_state_is_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(&data).await?;
        let project = orchestrator.project_import(&repo).await?;
        let task = orchestrator
            .task_create(
                &project.id,
                "start saga",
                "recover a partial start",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                Some(2),
            )
            .await?;
        let row = orchestrator.task(&task.id).await?;
        let project_row = orchestrator.project(&project.id).await?;
        let base = orchestrator.git.resolve(&repo, "main").await?;
        let branch = format!("agentflow/TASK-{}", task.seq);
        let worktree = project_row
            .worktree_root
            .join(format!("p{}/t{}", project_row.seq, task.seq));
        let intent = StartTaskIntent {
            base_commit: base.clone(),
            branch: branch.clone(),
            worktree_path: worktree.to_string_lossy().into_owned(),
            target_status: TaskStatus::ReadyForDevelopment.to_string(),
        };
        let (operation_id, _) = orchestrator.begin_start_operation(&row, &intent).await?;

        // Fault injection: emulate a power loss after Git worktree creation but before the task
        // row and event transaction.
        if let Some(parent) = worktree.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        orchestrator
            .git
            .worktree_add(&repo, &worktree, &branch, &base)
            .await?;
        drop(orchestrator);

        let recovered = Orchestrator::open(&data).await?;
        let detail = recovered.task_get(&task.id).await?;
        assert_eq!(detail.summary.status, TaskStatus::ReadyForDevelopment);
        assert_eq!(detail.branch.as_deref(), Some(branch.as_str()));
        assert_eq!(detail.base_commit.as_deref(), Some(base.as_str()));
        let status: String = sqlx::query_scalar("SELECT status FROM task_operations WHERE id=?")
            .bind(&operation_id)
            .fetch_one(recovered.store.pool())
            .await?;
        assert_eq!(status, "COMPLETED");
        assert!(
            recovered
                .events_list(&task.id, None, None)
                .await?
                .iter()
                .any(|event| event.event_type == "recovery:saga_completed")
        );
        Ok(())
    }

    #[tokio::test]
    async fn orphaned_development_stage_preserves_then_resets_residual_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(&data).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        let row = orchestrator.task(&task.id).await?;
        let operation_id = orchestrator
            .enter_development_stage(
                &row,
                TaskStatus::ReadyForDevelopment,
                TaskStatus::Developing,
                1,
            )
            .await?;
        tokio::fs::write(worktree.join("partial.txt"), "survived only in checkpoint\n").await?;
        drop(orchestrator);

        let recovered = Orchestrator::open(&data).await?;
        let detail = recovered.task_get(&task.id).await?;
        assert_eq!(detail.summary.status, TaskStatus::ReadyForDevelopment);
        assert_eq!(detail.summary.current_revision, 0);
        assert!(!worktree.join("partial.txt").exists());
        let checkpoint_phase: String = sqlx::query_scalar(
            "SELECT phase FROM task_checkpoints WHERE task_id=? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&task.id)
        .fetch_one(recovered.store.pool())
        .await?;
        assert_eq!(checkpoint_phase, "orphaned-stage");
        let operation_status: String =
            sqlx::query_scalar("SELECT status FROM task_operations WHERE id=?")
                .bind(&operation_id)
                .fetch_one(recovered.store.pool())
                .await?;
        assert_eq!(operation_status, "COMPLETED");
        Ok(())
    }

    #[tokio::test]
    async fn orphaned_stage_with_a_durable_revision_rolls_forward_to_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(&data).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        let row = orchestrator.task(&task.id).await?;
        orchestrator
            .enter_development_stage(
                &row,
                TaskStatus::ReadyForDevelopment,
                TaskStatus::Developing,
                1,
            )
            .await?;
        tokio::fs::write(worktree.join("shared.txt"), "durable revision\n").await?;
        git(&worktree, &["add", "shared.txt"]).await?;
        git(&worktree, &["commit", "-q", "-m", "durable revision"]).await?;
        let sha = git(&worktree, &["rev-parse", "HEAD"]).await?;
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,created_at) VALUES(?,?,1,?,?)")
            .bind(Uuid::now_v7().to_string()).bind(&task.id).bind(&sha)
            .bind(Utc::now().to_rfc3339()).execute(orchestrator.store.pool()).await?;
        drop(orchestrator);

        let recovered = Orchestrator::open(&data).await?;
        let detail = recovered.task_get(&task.id).await?;
        assert_eq!(detail.summary.status, TaskStatus::Validating);
        assert_eq!(detail.summary.current_revision, 1);
        assert_eq!(git(&worktree, &["rev-parse", "HEAD"]).await?, sha);
        assert!(
            recovered
                .events_list(&task.id, None, None)
                .await?
                .iter()
                .any(|event| event.event_type == "recovery:stage_roll_forward")
        );
        Ok(())
    }

    #[tokio::test]
    async fn orphaned_merge_rolls_forward_only_when_the_approved_commit_is_reachable()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(&data).await?;
        let (_, task, worktree) = prepared_task(&orchestrator, &repo, 2).await?;
        let approved_sha =
            approve_revision_fixture(&orchestrator, &task, &worktree, "approved merge\n").await?;
        let branch = orchestrator
            .task(&task.id)
            .await?
            .branch
            .ok_or("task branch missing")?;
        sqlx::query("UPDATE tasks SET status='MERGING' WHERE id=?")
            .bind(&task.id)
            .execute(orchestrator.store.pool())
            .await?;

        // Fault injection: Git completed the merge, but power failed before merge:succeeded.
        git(&repo, &["merge", "--no-ff", &branch, "-m", "interrupted merge"]).await?;
        drop(orchestrator);
        let recovered = Orchestrator::open(&data).await?;
        assert_eq!(
            recovered.task_get(&task.id).await?.summary.status,
            TaskStatus::Merged
        );
        let head = git(&repo, &["rev-parse", "HEAD"]).await?;
        assert!(recovered.git.is_ancestor(&repo, &approved_sha, &head).await?);
        assert!(
            recovered
                .events_list(&task.id, None, None)
                .await?
                .iter()
                .any(|event| event.event_type == "recovery:merge_roll_forward")
        );
        Ok(())
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
        approve_revision_fixture(&orchestrator, &task, &worktree, "agent version\n").await?;

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
