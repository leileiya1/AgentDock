#[cfg(test)]
mod budget_cancel_tests {
    use super::*;
    use async_trait::async_trait;

    /// Emits Claude-format cumulative usage beyond the task budget, then waits for the
    /// orchestrator's live cancellation before reporting a cancelled outcome — the exact
    /// sequence a real CLI produces when the live budget breaker fires.
    struct BudgetHogAdapter;

    #[async_trait]
    impl agentflow_agent_adapters::AgentProvider for BudgetHogAdapter {
        fn kind(&self) -> AgentKind {
            AgentKind::ClaudeCode
        }

        async fn detect(
            &self,
            _env: &agentflow_agent_adapters::CliEnv,
        ) -> Result<
            agentflow_agent_adapters::AgentInstallation,
            agentflow_agent_adapters::AdapterError,
        > {
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
                permission_broker: false,
            }
        }

        async fn start(
            &self,
            request: AgentRunRequest,
            cancel: CancellationToken,
            events: mpsc::Sender<AgentEvent>,
        ) -> Result<agentflow_agent_adapters::RunningAgent, agentflow_agent_adapters::AdapterError>
        {
            tokio::fs::create_dir_all(&request.run_dir).await?;
            let telemetry = json!({
                "type": "result",
                "usage": {"input_tokens": 90, "output_tokens": 50}
            })
            .to_string();
            // The process supervisor mirrors CLI stdout into stdout.log for real runs;
            // finish_agent_run reads recorded usage from there.
            tokio::fs::write(
                request.run_dir.join("stdout.log"),
                format!("{telemetry}\n"),
            )
            .await?;
            let _ = events
                .send(AgentEvent {
                    ts: Utc::now().to_rfc3339(),
                    stream: EventStream::Stdout,
                    kind: AgentEventKind::Result,
                    summary: "usage".into(),
                    text: Some(telemetry),
                })
                .await;
            tokio::time::timeout(Duration::from_secs(5), cancel.cancelled())
                .await
                .map_err(|_| {
                    agentflow_agent_adapters::AdapterError::InvalidResult(
                        "live budget cancel never arrived".into(),
                    )
                })?;
            Ok(agentflow_agent_adapters::RunningAgent {
                outcome: agentflow_process_supervisor::ProcessOutcome {
                    pid: 0,
                    started_at: Utc::now().to_rfc3339(),
                    exit_code: None,
                    timed_out: false,
                    idle_timed_out: false,
                    cancelled: true,
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
    async fn live_budget_cancel_surfaces_as_budget_error_and_blocks_the_task()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let worktree = dir.path().join("worktree");
        tokio::fs::create_dir_all(worktree.join(".agentflow-in")).await?;
        tokio::fs::write(worktree.join(".agentflow-in/input.md"), "input").await?;
        let project = orchestrator
            .store
            .import_project(
                "budget",
                "/tmp/budget-live",
                "main",
                &dir.path().join("worktrees").to_string_lossy(),
            )
            .await?;
        let created = orchestrator
            .task_create(
                &project.id,
                "budget live cancel",
                "test",
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                None,
                None,
            )
            .await?;
        sqlx::query("UPDATE tasks SET status='DEVELOPING',current_revision=1,worktree_path=? WHERE id=?")
            .bind(worktree.to_string_lossy().as_ref())
            .bind(&created.id)
            .execute(orchestrator.store.pool())
            .await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO task_policies(task_id,require_plan_approval,token_budget,created_at,updated_at) \
             VALUES(?,0,100,?,?) ON CONFLICT(task_id) DO UPDATE SET token_budget=100",
        )
        .bind(&created.id)
        .bind(&now)
        .bind(&now)
        .execute(orchestrator.store.pool())
        .await?;
        let task = orchestrator.task(&created.id).await?;
        let project = orchestrator.project(&project.id).await?;
        let run_dir = orchestrator.run_dir(&task.id);
        let started = orchestrator
            .run_agent(
                &BudgetHogAdapter,
                &task,
                &project,
                &run_dir,
                RunRole::Developer,
                ".agentflow-in/input.md",
                &ProjectConfig::default(),
                None,
            )
            .await;
        let Err(error) = started else {
            return Err("budget cancellation must not look like a user cancel".into());
        };
        assert!(
            error.to_string().contains("BUDGET_EXCEEDED"),
            "unexpected error: {error}"
        );
        // The develop/review/planning error paths re-check the budget right after a failed
        // attempt; verify that check now lands the task in BLOCKED(budget_exceeded).
        let task = orchestrator.task(&created.id).await?;
        assert!(orchestrator.enforce_budget(&task).await?);
        let summary = orchestrator.store.task_summary(&created.id).await?;
        assert_eq!(summary.status, TaskStatus::Blocked);
        assert_eq!(summary.blocked_reason, Some(BlockedReason::BudgetExceeded));
        Ok(())
    }
}
