use super::*;
use async_trait::async_trait;

struct TimeoutCaptureAdapter {
    captured: Arc<std::sync::Mutex<Option<(u64, u64)>>>,
}

#[async_trait]
impl agentflow_agent_adapters::AgentProvider for TimeoutCaptureAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::GeminiCli
    }

    async fn detect(
        &self,
        _env: &agentflow_agent_adapters::CliEnv,
    ) -> Result<agentflow_agent_adapters::AgentInstallation, agentflow_agent_adapters::AdapterError>
    {
        Err(agentflow_agent_adapters::AdapterError::NotFound(
            "test".into(),
        ))
    }

    fn capabilities(&self) -> agentflow_agent_adapters::AgentCapabilities {
        agentflow_agent_adapters::AgentCapabilities {
            streams_events: true,
            native_output_schema: true,
            supports_resume: false,
            read_only_mode: true,
            supports_development: true,
            supports_review: true,
        }
    }

    async fn start(
        &self,
        request: AgentRunRequest,
        _cancel: CancellationToken,
        _events: mpsc::Sender<AgentEvent>,
    ) -> Result<agentflow_agent_adapters::RunningAgent, agentflow_agent_adapters::AdapterError>
    {
        *self.captured.lock().map_err(|_| {
            agentflow_agent_adapters::AdapterError::InvalidResult("capture lock poisoned".into())
        })? = Some((request.timeout.as_secs(), request.idle_timeout.as_secs()));
        Ok(agentflow_agent_adapters::RunningAgent {
            outcome: agentflow_process_supervisor::ProcessOutcome {
                pid: 0,
                started_at: Utc::now().to_rfc3339(),
                exit_code: Some(0),
                timed_out: false,
                cancelled: false,
                log_truncated: false,
            },
            run_dir: request.run_dir,
            role: request.role,
        })
    }

    async fn collect_result(
        &self,
        _run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, agentflow_agent_adapters::AdapterError> {
        Err(agentflow_agent_adapters::AdapterError::UnsupportedRole(
            role,
        ))
    }
}

#[tokio::test]
async fn global_timeouts_are_validated_and_applied_to_runs()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let invalid = GlobalSettings {
        max_concurrent_runs: Some(0),
        ..GlobalSettings::default()
    };
    assert!(orchestrator.settings_update(&invalid).await.is_err());

    let settings = GlobalSettings {
        max_concurrent_runs: Some(3),
        developer_timeout_secs: Some(47),
        reviewer_timeout_secs: Some(59),
        idle_timeout_secs: Some(23),
        ..GlobalSettings::default()
    };
    orchestrator.settings_update(&settings).await?;

    let worktree = dir.path().join("wt");
    tokio::fs::create_dir_all(&worktree).await?;
    let project = orchestrator
        .store
        .import_project(
            "p",
            "/tmp/runtime-settings",
            "main",
            &dir.path().join("worktrees").to_string_lossy(),
        )
        .await?;
    let created = orchestrator
        .task_create(
            &project.id,
            "settings",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            None,
        )
        .await?;
    sqlx::query("UPDATE tasks SET current_revision=1,worktree_path=? WHERE id=?")
        .bind(worktree.to_string_lossy().as_ref())
        .bind(&created.id)
        .execute(orchestrator.store.pool())
        .await?;
    let captured = Arc::new(std::sync::Mutex::new(None));
    let adapter = TimeoutCaptureAdapter {
        captured: Arc::clone(&captured),
    };
    let run_dir = orchestrator.run_dir(&created.id);
    orchestrator
        .run_agent(
            &adapter,
            &orchestrator.task(&created.id).await?,
            &orchestrator.project(&project.id).await?,
            &run_dir,
            RunRole::Developer,
            "input.md",
            &ProjectConfig::default(),
            None,
        )
        .await?;
    let captured_value = *captured.lock().map_err(|_| "capture lock poisoned")?;
    assert_eq!(captured_value, Some((47, 23)));
    let recorded: (i64, i64) =
        sqlx::query_as("SELECT timeout_secs,idle_timeout_secs FROM agent_runs WHERE task_id=?")
            .bind(&created.id)
            .fetch_one(orchestrator.store.pool())
            .await?;
    assert_eq!(recorded, (47, 23));
    Ok(())
}

#[tokio::test]
async fn api_pricing_snapshots_require_a_complete_non_negative_pair()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("p", "/tmp/api-pricing", "main", "/tmp/api-pricing-wt")
        .await?;
    let mut settings = ProjectSettings::default();
    settings.deepseek.input_cost_per_million = Some(0.5);
    assert!(
        orchestrator
            .project_settings_update(&project.id, &settings)
            .await
            .is_err()
    );
    settings.deepseek.output_cost_per_million = Some(1.5);
    let saved = orchestrator
        .project_settings_update(&project.id, &settings)
        .await?;
    assert_eq!(saved.deepseek.input_cost_per_million, Some(0.5));
    assert_eq!(saved.deepseek.output_cost_per_million, Some(1.5));
    Ok(())
}

#[tokio::test]
async fn provider_slots_enforce_concurrency_and_rate_limits_across_tasks()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Arc::new(Orchestrator::open(dir.path()).await?);
    let project = orchestrator
        .store
        .import_project("dispatch", "/tmp/dispatch", "main", "/tmp/dispatch-wt")
        .await?;
    let first = orchestrator
        .task_create(
            &project.id,
            "first",
            "dispatch",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            None,
            None,
        )
        .await?;
    let second = orchestrator
        .task_create(
            &project.id,
            "second",
            "dispatch",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            None,
            None,
        )
        .await?;
    let project_row = orchestrator.project(&project.id).await?;
    let mut settings = GlobalSettings {
        default_provider_max_concurrent: 1,
        default_provider_requests_per_minute: 600,
        ..GlobalSettings::default()
    };
    orchestrator.settings_update(&settings).await?;
    orchestrator
        .acquire_provider_dispatch(
            &first.id,
            "slot-first",
            &AgentKind::ClaudeCode,
            &project_row,
        )
        .await?;
    let waiting_owner = Arc::clone(&orchestrator);
    let waiting_project = project_row.clone();
    let second_id = second.id.clone();
    let waiting = tokio::spawn(async move {
        waiting_owner
            .acquire_provider_dispatch(
                &second_id,
                "slot-second",
                &AgentKind::ClaudeCode,
                &waiting_project,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !waiting.is_finished(),
        "second task bypassed provider concurrency"
    );
    orchestrator.release_provider_dispatch("slot-first").await?;
    tokio::time::timeout(Duration::from_secs(2), waiting).await???;
    orchestrator
        .release_provider_dispatch("slot-second")
        .await?;

    settings.default_provider_requests_per_minute = 1;
    orchestrator.settings_update(&settings).await?;
    sqlx::query("DELETE FROM provider_dispatch_history")
        .execute(orchestrator.store.pool())
        .await?;
    orchestrator
        .acquire_provider_dispatch(
            &first.id,
            "rate-first",
            &AgentKind::ClaudeCode,
            &project_row,
        )
        .await?;
    orchestrator.release_provider_dispatch("rate-first").await?;
    let rate_owner = Arc::clone(&orchestrator);
    let rate_project = project_row.clone();
    let rate_task = second.id.clone();
    let rate_wait = tokio::spawn(async move {
        rate_owner
            .acquire_provider_dispatch(
                &rate_task,
                "rate-second",
                &AgentKind::ClaudeCode,
                &rate_project,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!rate_wait.is_finished(), "RPM limit was not enforced");
    sqlx::query("UPDATE provider_dispatch_history SET dispatched_at=?")
        .bind((Utc::now() - chrono::Duration::seconds(61)).to_rfc3339())
        .execute(orchestrator.store.pool())
        .await?;
    tokio::time::timeout(Duration::from_secs(2), rate_wait).await???;
    orchestrator
        .release_provider_dispatch("rate-second")
        .await?;
    Ok(())
}

#[tokio::test]
async fn exhausted_global_daily_budget_blocks_new_work() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    orchestrator
        .settings_update(&GlobalSettings {
            global_daily_cost_usd: Some(1.0),
            ..GlobalSettings::default()
        })
        .await?;
    let project = orchestrator
        .store
        .import_project(
            "budget",
            "/tmp/global-budget",
            "main",
            "/tmp/global-budget-wt",
        )
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "budget",
            "global",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            None,
            None,
        )
        .await?;
    sqlx::query("UPDATE tasks SET status='READY_FOR_DEVELOPMENT' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,cost_usd,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES('spent-global',?,0,'planner','claude_code','SUCCEEDED',1.0,'/tmp/spent',1,1,?)")
        .bind(&task.id)
        .bind(Utc::now().to_rfc3339())
        .execute(orchestrator.store.pool())
        .await?;
    let row = orchestrator.task(&task.id).await?;
    assert!(orchestrator.enforce_global_budget(&row).await?);
    let summary = orchestrator.task_get(&task.id).await?.summary;
    assert_eq!(summary.status, TaskStatus::Blocked);
    assert_eq!(summary.blocked_reason, Some(BlockedReason::BudgetExceeded));
    Ok(())
}
