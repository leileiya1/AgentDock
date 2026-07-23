#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_failure_class_maps_to_reason_for_every_role() {
        // §15/§5: idle hangs and auth expiry always get their dedicated reason regardless of the
        // role's fallback, so developer/planner/reviewer loops surface the same recovery card.
        for fallback in [BlockedReason::RunFailed, BlockedReason::ReviewFailed] {
            assert_eq!(
                run_failure_reason(RunFailureClass::Unresponsive, fallback),
                BlockedReason::AgentUnresponsive
            );
            assert_eq!(
                run_failure_reason(RunFailureClass::AuthExpired, fallback),
                BlockedReason::AuthExpired
            );
            // A normal failure keeps its role-specific fallback untouched.
            assert_eq!(run_failure_reason(RunFailureClass::Normal, fallback), fallback);
        }
    }

    #[test]
    fn merge_suggestions_keeps_distinct_and_collapses_duplicates() {
        // §23 不同建议: different fixes are both kept; identical or subsumed ones collapse to one.
        assert_eq!(merge_suggestions(None, Some("拆分函数")).as_deref(), Some("拆分函数"));
        assert_eq!(merge_suggestions(Some("拆分函数"), None).as_deref(), Some("拆分函数"));
        assert_eq!(merge_suggestions(Some("拆分函数"), Some("拆分函数")).as_deref(), Some("拆分函数"));
        let combined = merge_suggestions(Some("拆分函数"), Some("保持现状")).unwrap_or_default();
        assert!(combined.contains("拆分函数") && combined.contains("保持现状"));
        // A suggestion that already contains the other is not duplicated.
        assert_eq!(
            merge_suggestions(Some("先加判空再拆分函数"), Some("拆分函数")).as_deref(),
            Some("先加判空再拆分函数")
        );
        assert_eq!(merge_suggestions(None, None), None);
    }

    #[test]
    fn auth_markers_are_detected_without_matching_ordinary_failures() {
        // §5: real auth failures are recognised…
        assert!(output_indicates_auth_failure("Error: 401 Unauthorized"));
        assert!(output_indicates_auth_failure("codex: not logged in, please run `codex login`"));
        assert!(output_indicates_auth_failure("Invalid API key provided"));
        assert!(output_indicates_auth_failure("认证已失效，请重新登录"));
        // …while ordinary build/test failures are not misclassified as auth problems.
        assert!(!output_indicates_auth_failure("error[E0599]: no method named `foo`"));
        assert!(!output_indicates_auth_failure("test result: FAILED. 2 passed; 1 failed"));
    }

    #[tokio::test]
    async fn planner_can_be_blocked_from_the_planning_state()
    -> Result<(), Box<dyn std::error::Error>> {
        // Regression: plan() blocks while the task is still in Planning. block() must therefore
        // accept Planning as a from-state, otherwise the transition errors with InvalidState and
        // the planner failure never surfaces as a Blocked task.
        let dir = tempfile::tempdir()?;
        let owner = Orchestrator::open(&dir.path().join("data")).await?;
        let project = owner
            .store
            .import_project("plan", "/tmp/plan-e2e", "main", "/tmp/plan-e2e-wt")
            .await?;
        let task = owner
            .task_create(
                &project.id,
                "plan block",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                None,
            )
            .await?;
        sqlx::query("UPDATE tasks SET status='PLANNING',current_revision=1 WHERE id=?")
            .bind(&task.id)
            .execute(owner.store.pool())
            .await?;
        let task_row = owner.task(&task.id).await?;
        owner
            .block(&task_row, BlockedReason::AuthExpired, "login expired")
            .await?;
        let summary = owner.store.task_summary(&task.id).await?;
        assert_eq!(summary.status, TaskStatus::Blocked);
        assert_eq!(summary.blocked_reason, Some(BlockedReason::AuthExpired));
        Ok(())
    }

    #[tokio::test]
    async fn planner_recovery_resumes_the_same_plan_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let owner = Orchestrator::open(&dir.path().join("data")).await?;
        let project = owner
            .store
            .import_project("plan", "/tmp/plan-resume", "main", "/tmp/plan-resume-wt")
            .await?;
        let task = owner
            .task_create_governed(
                &project.id,
                "plan resume",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                None,
                false,
                TaskPolicy { require_plan_approval: true, ..TaskPolicy::default() },
            )
            .await?;
        sqlx::query("UPDATE tasks SET status='PLANNING',current_revision=0 WHERE id=?")
            .bind(&task.id)
            .execute(owner.store.pool())
            .await?;
        let task_row = owner.task(&task.id).await?;
        owner.block(&task_row, BlockedReason::AuthExpired, "login expired").await?;
        let resumed = owner.resume_with_guidance(&task.id, "认证已修复").await?;
        assert_eq!(resumed.status, TaskStatus::Planning);
        assert_eq!(resumed.current_revision, 0);
        Ok(())
    }

    #[tokio::test]
    async fn task_creation_persists_structured_acceptance_and_rejects_bad_revision_limits()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let owner = Orchestrator::open(&dir.path().join("data")).await?;
        let project = owner
            .store
            .import_project("acceptance", "/tmp/acceptance", "main", "/tmp/acceptance-wt")
            .await?;
        let criteria = vec![
            AcceptanceCriterionInput {
                kind: AcceptanceCriterionKind::Build,
                text: "生产构建成功".into(),
            },
            AcceptanceCriterionInput {
                kind: AcceptanceCriterionKind::Manual,
                text: "人工确认空状态文案".into(),
            },
        ];
        let task = owner
            .task_create_governed_with_acceptance(
                &project.id,
                "acceptance",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                Some(4),
                false,
                criteria,
                TaskPolicy::default(),
            )
            .await?;
        let detail = owner.task_get(&task.id).await?;
        assert_eq!(detail.max_revisions, 4);
        assert_eq!(detail.acceptance_criteria.len(), 2);
        assert_eq!(detail.acceptance_criteria[0].kind, AcceptanceCriterionKind::Build);
        assert_eq!(detail.acceptance_criteria[1].text, "人工确认空状态文案");

        let invalid = owner
            .task_create_governed_with_acceptance(
                &project.id,
                "invalid",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                Some(0),
                false,
                Vec::new(),
                TaskPolicy::default(),
            )
            .await;
        assert!(matches!(invalid, Err(OrchestratorError::Config(_))));
        Ok(())
    }

    #[tokio::test]
    async fn config_requires_argv_array() {
        let dir = tempfile::tempdir().ok();
        let Some(dir) = dir else { return };
        let root = dir.path().join(".agentflow");
        assert!(tokio::fs::create_dir_all(&root).await.is_ok());
        assert!(
            tokio::fs::write(
                root.join("project.toml"),
                "schema_version=1\n[[validate.steps]]\nname='x'\nargv='bun test'"
            )
            .await
            .is_ok()
        );
        assert!(load_config(dir.path()).await.is_err());
    }

    #[tokio::test]
    async fn database_cancellation_reaches_a_daemon_owned_token()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let project = orchestrator
            .store
            .import_project("p", "/tmp/cancel-watch", "main", "/tmp/cancel-watch-wt")
            .await?;
        let task = orchestrator
            .task_create(
                &project.id,
                "cancel watch",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                None,
            )
            .await?;
        let cancellation = CancellationToken::new();
        let watcher =
            orchestrator.watch_task_cancellation(task.id.clone(), cancellation.clone());
        sqlx::query("UPDATE tasks SET status='CANCELLED' WHERE id=?")
            .bind(&task.id)
            .execute(orchestrator.store.pool())
            .await?;
        tokio::time::timeout(Duration::from_secs(2), cancellation.cancelled()).await?;
        watcher.await?;
        assert!(cancellation.is_cancelled());
        Ok(())
    }

    #[tokio::test]
    async fn client_open_never_recovers_daemon_owned_runs()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let owner = Orchestrator::open(dir.path()).await?;
        let project = owner
            .store
            .import_project("p", "/tmp/client-open", "main", "/tmp/client-open-wt")
            .await?;
        let task = owner
            .task_create(
                &project.id,
                "owner run",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                None,
            )
            .await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE tasks SET status='DEVELOPING' WHERE id=?")
            .bind(&task.id)
            .execute(owner.store.pool())
            .await?;
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES('live-run',?,1,'developer','codex','RUNNING','/tmp/live-run',10,10,?)")
            .bind(&task.id)
            .bind(now)
            .execute(owner.store.pool())
            .await?;

        let client = Orchestrator::open_client(dir.path()).await?;
        let run_status: String =
            sqlx::query_scalar("SELECT status FROM agent_runs WHERE id='live-run'")
                .fetch_one(client.store.pool())
                .await?;
        assert_eq!(run_status, "RUNNING");
        assert_eq!(client.task_get(&task.id).await?.summary.status, TaskStatus::Developing);
        Ok(())
    }

    #[tokio::test]
    async fn owner_recovery_never_signals_a_reused_pid()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let owner = Orchestrator::open(dir.path()).await?;
        let worktree = dir.path().join("worktree");
        let run_dir = dir.path().join("run-reused");
        tokio::fs::create_dir_all(&worktree).await?;
        tokio::fs::create_dir_all(&run_dir).await?;
        let project = owner
            .store
            .import_project(
                "p",
                "/tmp/reused-pid",
                "main",
                &dir.path().join("worktrees").to_string_lossy(),
            )
            .await?;
        let task = owner
            .task_create(
                &project.id,
                "safe recovery",
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
        let lease = agentflow_process_supervisor::ProcessLease {
            pid: std::process::id(),
            process_group: std::process::id(),
            os_started_at_secs: u64::MAX,
            started_at: Utc::now().to_rfc3339(),
            owner_pid: 1,
            program: "unrelated-reused-process".into(),
        };
        tokio::fs::write(
            run_dir.join("process-lease.json"),
            serde_json::to_vec(&lease)?,
        )
        .await?;
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES('run-reused',?,1,'developer','codex','RUNNING',?,10,10,?)")
            .bind(&task.id)
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(Utc::now().to_rfc3339())
            .execute(owner.store.pool())
            .await?;
        drop(owner);

        let recovered = Orchestrator::open(dir.path()).await?;
        let run_status: String =
            sqlx::query_scalar("SELECT status FROM agent_runs WHERE id='run-reused'")
                .fetch_one(recovered.store.pool())
                .await?;
        assert_eq!(run_status, "INTERRUPTED");
        let detail = recovered.task_get(&task.id).await?;
        assert_eq!(detail.summary.status, TaskStatus::ReadyForDevelopment);
        assert_eq!(detail.summary.current_revision, 0);
        let events = recovered.events_list(&task.id, None, None).await?;
        assert!(events.iter().any(|event| {
            event.event_type == "recovery:interrupted"
                && event.payload.get("process_recovery").and_then(Value::as_str)
                    == Some("pid_reused_not_signaled")
        }));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn owner_recovery_adopts_a_matching_live_process_without_terminating_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let owner = Orchestrator::open(dir.path()).await?;
        let worktree = dir.path().join("worktree-live");
        let run_dir = dir.path().join("run-live-orphan");
        tokio::fs::create_dir_all(&worktree).await?;
        tokio::fs::create_dir_all(&run_dir).await?;
        let project = owner
            .store
            .import_project(
                "p",
                "/tmp/live-orphan",
                "main",
                &dir.path().join("worktrees").to_string_lossy(),
            )
            .await?;
        let task = owner
            .task_create(
                &project.id,
                "orphan recovery",
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
        let lease_path = run_dir.join("process-lease.json");
        let (tx, mut rx) = mpsc::channel(8);
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        let supervisor = tokio::spawn(agentflow_process_supervisor::run(
            agentflow_process_supervisor::ProcessSpec {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 30 & wait".into()],
                cwd: worktree.clone(),
                env: HashMap::new(),
                clear_environment: false,
                env_denylist: Vec::new(),
                timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(30),
                stdout_path: run_dir.join("stdout.log"),
                stderr_path: run_dir.join("stderr.log"),
                lease_path: lease_path.clone(),
            },
            CancellationToken::new(),
            tx,
        ));
        for _ in 0..100 {
            if lease_path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let lease = agentflow_process_supervisor::read_process_lease(&lease_path).await?;
        assert_eq!(
            agentflow_process_supervisor::inspect_process_lease(&lease),
            agentflow_process_supervisor::LeaseState::Alive
        );
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,child_pid,child_started_at,created_at) VALUES('run-live-orphan',?,1,'developer','codex','RUNNING',?,30,30,?,?,?)")
            .bind(&task.id)
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(i64::from(lease.pid))
            .bind(&lease.started_at)
            .bind(Utc::now().to_rfc3339())
            .execute(owner.store.pool())
            .await?;

        // Aborting the owner future simulates the daemon disappearing without running its normal
        // cancellation path. The provider process remains alive and must be handled on restart.
        supervisor.abort();
        let _ = supervisor.await;
        drop(owner);
        let recovered = Orchestrator::open(dir.path()).await?;
        assert_eq!(
            agentflow_process_supervisor::inspect_process_lease(&lease),
            agentflow_process_supervisor::LeaseState::Alive
        );
        let (run_status, recovery_state): (String, Option<String>) = sqlx::query_as(
            "SELECT status,recovery_state FROM agent_runs WHERE id='run-live-orphan'",
        )
        .fetch_one(recovered.store.pool())
        .await?;
        assert_eq!(run_status, "RUNNING");
        assert_eq!(recovery_state.as_deref(), Some("ADOPTING"));
        assert_eq!(
            recovered.task_get(&task.id).await?.summary.status,
            TaskStatus::Developing
        );
        assert!(recovered
            .events_list(&task.id, None, None)
            .await?
            .iter()
            .any(|event| event.event_type == "recovery:run_adopted"));
        agentflow_process_supervisor::terminate_process_lease(&lease, Duration::from_secs(2))
            .await?;
        tokio::time::timeout(Duration::from_secs(2), drain).await??;
        Ok(())
    }

    #[tokio::test]
    async fn invalid_contract_gets_one_read_only_repair_attempt()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let worktree = dir.path().join("worktree");
        tokio::fs::create_dir_all(worktree.join(".agentflow-in")).await?;
        let project = orchestrator
            .store
            .import_project(
                "repair",
                "/tmp/repair-contract",
                "main",
                &dir.path().join("worktrees").to_string_lossy(),
            )
            .await?;
        let created = orchestrator
            .task_create(
                &project.id,
                "repair result",
                "test",
                AgentKind::GeminiCli,
                AgentKind::ClaudeCode,
                None,
                None,
            )
            .await?;
        sqlx::query("UPDATE tasks SET status='DEVELOPING',current_revision=1,worktree_path=? WHERE id=?")
            .bind(worktree.to_string_lossy().as_ref())
            .bind(&created.id)
            .execute(orchestrator.store.pool())
            .await?;
        let task = orchestrator.task(&created.id).await?;
        let project = orchestrator.project(&project.id).await?;
        let Some(repaired) = orchestrator
            .attempt_result_repair(
                &RepairOnlyAdapter,
                &task,
                &project,
                RunRole::Developer,
                &ProjectConfig::default(),
                "missing result.json",
            )
            .await?
        else {
            return Err("repair did not return a result".into());
        };
        assert!(matches!(
            repaired.value,
            CollectedResult::Development(ref value)
                if value.summary == "结构化结果已自动修复"
        ));
        let events = orchestrator.events_list(&created.id, None, None).await?;
        assert!(events
            .iter()
            .any(|event| event.event_type == "result:repair_started"));
        assert!(events
            .iter()
            .any(|event| event.event_type == "result:repair_succeeded"));
        Ok(())
    }

    #[tokio::test]
    async fn storage_cleanup_trash_restore_and_purge_are_recoverable()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let project = orchestrator
            .store
            .import_project(
                "p",
                "/tmp/agentflow-storage-test-project",
                "main",
                &dir.path().join("wt").to_string_lossy(),
            )
            .await?;
        let task = orchestrator
            .task_create(
                &project.id,
                "storage lifecycle",
                "test",
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                None,
                None,
            )
            .await?;
        let run_dir = orchestrator.task_dir(&task.id).join("runs/run-1");
        tokio::fs::create_dir_all(&run_dir).await?;
        tokio::fs::write(run_dir.join("stdout.log"), "raw provider output").await?;
        tokio::fs::write(run_dir.join("result.json"), "{}").await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES('run-1',?,0,'developer','claude_code','SUCCEEDED',?,10,10,?)")
            .bind(&task.id)
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(&now)
            .execute(orchestrator.store.pool())
            .await?;

        assert!(orchestrator.storage_report().await?.log_bytes > 0);
        let cleaned = orchestrator
            .task_cleanup(&task.id, StorageCleanupScope::Logs)
            .await?;
        assert_eq!(cleaned.files_removed, 1);
        assert!(!run_dir.join("stdout.log").exists());
        assert!(run_dir.join("result.json").exists());

        let trashed = orchestrator
            .task_cleanup(&task.id, StorageCleanupScope::Everything)
            .await?;
        assert_eq!(trashed.tasks_trashed, 1);
        assert!(orchestrator.task_get(&task.id).await.is_err());
        assert_eq!(orchestrator.trash_list().await?.len(), 1);

        let restored = orchestrator.task_restore(&task.id).await?;
        assert_eq!(restored.id, task.id);
        assert!(run_dir.join("result.json").exists());

        orchestrator
            .task_cleanup(&task.id, StorageCleanupScope::Everything)
            .await?;
        let purged = orchestrator.trash_empty().await?;
        assert_eq!(purged.tasks_purged, 1);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE id=?")
            .bind(&task.id)
            .fetch_one(orchestrator.store.pool())
            .await?;
        assert_eq!(remaining, 0);
        Ok(())
    }

    #[tokio::test]
    async fn provider_fallback_chain_is_role_aware_and_keeps_review_independent()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let settings = ProjectSettings::default();
        let developers =
            orchestrator.provider_chain(AgentKind::ClaudeCode, RunRole::Developer, None, &settings, false);
        assert_eq!(developers.first(), Some(&AgentKind::ClaudeCode));
        assert!(!developers.contains(&AgentKind::OpenAiApi));
        assert_eq!(
            &developers[1..],
            &[AgentKind::QoderCli, AgentKind::GrokCli]
        );

        let reviewers = orchestrator.provider_chain(
            AgentKind::Codex,
            RunRole::Reviewer,
            Some(AgentKind::ClaudeCode),
            &settings,
            true,
        );
        assert_eq!(reviewers.first(), Some(&AgentKind::Codex));
        assert!(!reviewers.contains(&AgentKind::ClaudeCode));
        assert_eq!(
            &reviewers[1..],
            &[AgentKind::QoderCli, AgentKind::GrokCli]
        );
        let private_reviewers = orchestrator.provider_chain(
            AgentKind::Codex,
            RunRole::Reviewer,
            Some(AgentKind::ClaudeCode),
            &settings,
            false,
        );
        assert!(private_reviewers.iter().all(|provider| !provider.is_api()));
        Ok(())
    }

    #[test]
    fn review_commit_reference_requires_a_git_hex_prefix() {
        assert!(is_hex_commit_reference("d3f81c3"));
        assert!(is_hex_commit_reference(
            "d3f81c32b9c62f4d5831b1d7cf0b7fa06baaafbb"
        ));
        assert!(!is_hex_commit_reference("HEAD"));
        assert!(!is_hex_commit_reference("d3f81c"));
        assert!(!is_hex_commit_reference("d3f81cg"));
    }

    #[tokio::test]
    async fn revision_history_is_provider_neutral_and_bounded()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let project = orchestrator
            .store
            .import_project("p", "/tmp/history-project", "main", "/tmp/history-wt")
            .await?;
        let created = orchestrator
            .task_create(
                &project.id,
                "history",
                "test",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                None,
            )
            .await?;
        let run_dir = orchestrator.task_dir(&created.id).join("runs/r1-dev");
        tokio::fs::create_dir_all(&run_dir).await?;
        tokio::fs::write(
            run_dir.join("result.json"),
            serde_json::to_vec(&DevelopmentResult {
                schema_version: 1,
                task_id: created.id.clone(),
                revision: 1,
                status: DevelopmentStatus::Completed,
                summary: "implemented provider protocol".into(),
                question: None,
                changed_files: None,
                notes: None,
                plan_sha256: None,
            })?,
        )
        .await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,created_at) VALUES('rev-1',?,1,'d3f81c3',?)")
            .bind(&created.id).bind(&now).execute(orchestrator.store.pool()).await?;
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES('run-r1',?,1,'developer','codex','SUCCEEDED',?,10,10,?)")
            .bind(&created.id).bind(run_dir.to_string_lossy().as_ref()).bind(&now)
            .execute(orchestrator.store.pool()).await?;
        sqlx::query("INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,summary,raw_path,created_at) VALUES('review-r1',?,1,'run-r1','d3f81c3','request_changes','needs a regression test','raw.json',?)")
            .bind(&created.id).bind(&now).execute(orchestrator.store.pool()).await?;
        sqlx::query("INSERT INTO review_issues(id,review_id,severity,title,description,suggested_action) VALUES('issue-r1','review-r1','high','missing test','coverage is missing','add coverage')")
            .execute(orchestrator.store.pool()).await?;
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,1,'human','human:resume_with_guidance','{\"guidance\":\"keep the public API stable\"}',?)")
            .bind(&created.id).bind(&now).execute(orchestrator.store.pool()).await?;
        let mut task = orchestrator.task(&created.id).await?;
        task.revision = 2;
        let worktree = dir.path().join("worktree");
        tokio::fs::create_dir_all(worktree.join(".agentflow-in")).await?;
        let Some(history) = orchestrator.write_history(&task, &worktree).await? else {
            return Err("revision two did not have a history snapshot".into());
        };
        assert!(history.contains("implemented provider protocol"));
        assert!(history.contains("needs a regression test"));
        assert!(history.contains("missing test"));
        assert!(history.contains("add coverage"));
        assert!(history.contains("keep the public API stable"));
        assert!(history.len() <= HISTORY_MAX_BYTES);
        let snapshot: Value = serde_json::from_slice(
            &tokio::fs::read(worktree.join(".agentflow-in/history.json")).await?,
        )?;
        assert_eq!(snapshot["schema_version"], 1);
        assert_eq!(snapshot["next_revision"], 2);

        let project = orchestrator.project(&project.id).await?;
        let input = orchestrator
            .build_input(&task, &project, Some(&history), None)
            .await?;
        assert!(input.contains("跨轮记忆（权威快照）"));
        assert!(input.contains("missing test"));
        Ok(())
    }

    #[test]
    fn claude_stream_telemetry_is_extracted_for_optional_resume() {
        let jsonl = concat!(
            r#"{"type":"system","session_id":"session-1"}"#,
            "\n",
            r#"{"type":"result","session_id":"session-1","total_cost_usd":0.25,"usage":{"input_tokens":10,"cache_creation_input_tokens":2,"cache_read_input_tokens":3,"output_tokens":4}}"#,
        );
        assert_eq!(
            parse_claude_telemetry(jsonl),
            ProviderTelemetry {
                session_id: Some("session-1".into()),
                cost_usd: Some(0.25),
                tokens_in: Some(15),
                tokens_out: Some(4),
            }
        );
    }
}
#[test]
fn interactive_provider_permission_prompts_are_detected_without_false_shell_errors() {
    assert!(looks_like_permission_prompt("Permission required: approve this action?"));
    assert!(looks_like_permission_prompt("需要授权，等待批准"));
    assert!(!looks_like_permission_prompt("command exited with code 1"));
    assert!(!looks_like_permission_prompt("test assertion failed"));
}
