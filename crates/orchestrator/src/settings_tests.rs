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
