use async_trait::async_trait;

struct RepairOnlyAdapter;

#[async_trait]
impl agentflow_agent_adapters::AgentProvider for RepairOnlyAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::GeminiCli
    }

    async fn detect(
        &self,
        _env: &agentflow_agent_adapters::CliEnv,
    ) -> Result<agentflow_agent_adapters::AgentInstallation, agentflow_agent_adapters::AdapterError>
    {
        Err(agentflow_agent_adapters::AdapterError::NotFound(
            "test adapter".into(),
        ))
    }

    fn capabilities(&self) -> agentflow_agent_adapters::AgentCapabilities {
        agentflow_agent_adapters::AgentCapabilities {
            streams_events: true,
            native_output_schema: true,
            supports_resume: false,
            read_only_mode: true,
            supports_development: true,
            supports_review: false,
        }
    }

    async fn start(
        &self,
        request: AgentRunRequest,
        _cancel: CancellationToken,
        _events: mpsc::Sender<AgentEvent>,
    ) -> Result<agentflow_agent_adapters::RunningAgent, agentflow_agent_adapters::AdapterError> {
        if !matches!(request.permission, PermissionTier::ReadOnly) {
            return Err(agentflow_agent_adapters::AdapterError::InvalidResult(
                "repair was not read-only".into(),
            ));
        }
        tokio::fs::create_dir_all(&request.run_dir).await?;
        let result = DevelopmentResult {
            schema_version: 1,
            task_id: request.task_id,
            revision: request.revision,
            status: DevelopmentStatus::Completed,
            summary: "结构化结果已自动修复".into(),
            question: None,
            changed_files: None,
            notes: None,
        };
        tokio::fs::write(
            request.run_dir.join("result.json"),
            serde_json::to_vec(&result).map_err(|error| {
                agentflow_agent_adapters::AdapterError::InvalidResult(error.to_string())
            })?,
        )
        .await?;
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
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, agentflow_agent_adapters::AdapterError> {
        if role != RunRole::Developer {
            return Err(agentflow_agent_adapters::AdapterError::UnsupportedRole(
                role,
            ));
        }
        let bytes = tokio::fs::read(run_dir.join("result.json")).await?;
        let result = serde_json::from_slice(&bytes).map_err(|error| {
            agentflow_agent_adapters::AdapterError::InvalidResult(error.to_string())
        })?;
        Ok(CollectedResult::Development(result))
    }
}
