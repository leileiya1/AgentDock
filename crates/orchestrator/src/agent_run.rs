impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    async fn run_agent(
        &self,
        adapter: &dyn AgentAdapter,
        task: &TaskRow,
        project: &ProjectRow,
        run_dir: &Path,
        role: RunRole,
        input: &str,
        config: &ProjectConfig,
        permission_override: Option<PermissionTier>,
    ) -> Result<agentflow_agent_adapters::RunningAgent, OrchestratorError> {
        let worktree = required_path(&task.worktree_path)?;
        if self.git.is_repo(&worktree).await {
            self.create_checkpoint(task, &format!("before-{role}"))
                .await?;
        }
        tokio::fs::create_dir_all(run_dir).await?;
        let authoritative_input = required_path(&task.worktree_path)?.join(input);
        if authoritative_input.exists() {
            tokio::fs::copy(&authoritative_input, run_dir.join("input.md")).await?;
        }
        let run_id = run_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("run")
            .to_string();
        let now = Utc::now().to_rfc3339();
        let settings = self.settings_get().await?;
        let configured_timeout = if role == RunRole::Reviewer {
            settings.reviewer_timeout_secs.unwrap_or(900)
        } else {
            settings.developer_timeout_secs.unwrap_or(1_800)
        };
        let timeout_secs = self
            .remaining_time_budget(&task.id)
            .await?
            .map_or(configured_timeout, |remaining| configured_timeout.min(remaining));
        let idle_timeout_secs = settings.idle_timeout_secs.unwrap_or(300);
        let usage = self.budget_usage(&task.id).await?;
        let remaining_tokens = usage.token_budget.map(|limit| {
            limit
                .saturating_sub(usage.tokens_used)
                .saturating_sub(usage.tokens_reserved)
                .max(0) as u64
        });
        let task_remaining_cost = usage
            .cost_budget_usd
            .map(|limit| (limit - usage.cost_usd - usage.cost_reserved_usd).max(0.0));
        let global_remaining_cost = self.global_daily_cost_remaining().await?;
        if global_remaining_cost.is_some()
            && adapter.budget_capabilities().cost != BudgetMode::Hard
        {
            return Err(OrchestratorError::InvalidState(format!(
                "GLOBAL_BUDGET_UNENFORCEABLE: {} cannot enforce a hard cost ceiling",
                adapter.kind()
            )));
        }
        let remaining_cost_usd = match (task_remaining_cost, global_remaining_cost) {
            (Some(task), Some(global)) => Some(task.min(global)),
            (Some(task), None) => Some(task),
            (None, Some(global)) => Some(global),
            (None, None) => None,
        };
        if remaining_tokens == Some(0) || remaining_cost_usd == Some(0.0) {
            return Err(OrchestratorError::InvalidState(
                "BUDGET_EXCEEDED: no budget remains for another Provider run".into(),
            ));
        }
        let budget = RunBudget {
            remaining_tokens,
            remaining_cost_usd,
        };
        let budget_capabilities = adapter.budget_capabilities();
        let token_mode = budget_mode_text(budget_capabilities.tokens);
        let cost_mode = budget_mode_text(budget_capabilities.cost);
        let reserved_tokens = (budget_capabilities.tokens == BudgetMode::Hard)
            .then_some(remaining_tokens)
            .flatten()
            .map(|value| value.min(i64::MAX as u64) as i64);
        let reserved_cost = (budget_capabilities.cost == BudgetMode::Hard)
            .then_some(remaining_cost_usd)
            .flatten();
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,started_at,created_at,token_budget_mode,cost_budget_mode,reserved_tokens,reserved_cost_usd) VALUES(?,?,?,?,?,'RUNNING',?,?,?,?,?,?,?,?,?)")
            .bind(&run_id)
            .bind(&task.id)
            .bind(task.revision)
            .bind(role.to_string())
            .bind(adapter.kind().to_string())
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(timeout_secs as i64)
            .bind(idle_timeout_secs as i64)
            .bind(&now)
            .bind(&now)
            .bind(token_mode)
            .bind(cost_mode)
            .bind(reserved_tokens)
            .bind(reserved_cost)
            .execute(self.store.pool())
            .await?;
        let cancellation = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);
        let events_path = run_dir.join("agent-events.jsonl");
        let live_cancel = cancellation.clone();
        let live_provider = adapter.kind();
        let live_budget = budget.clone();
        let sink = tokio::spawn(async move {
            let mut file = tokio::fs::File::create(events_path).await?;
            while let Some(event) = rx.recv().await {
                if live_budget_exceeded(&live_provider, &event, &live_budget) {
                    live_cancel.cancel();
                }
                let mut line = serde_json::to_vec(&event)?;
                line.push(b'\n');
                file.write_all(&line).await?;
            }
            Ok::<(), anyhow::Error>(())
        });
        // Review Providers receive the authoritative revision commit through the protocol.
        let commit_sha = if role == RunRole::Reviewer {
            sqlx::query_scalar::<_, String>(
                "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
            )
            .bind(&task.id)
            .bind(task.revision)
            .fetch_optional(self.store.pool())
            .await?
        } else {
            None
        };
        let resume_session_id = if project.settings.resume_sessions
            && adapter.capabilities().supports_resume
            && task.revision > 1
        {
            sqlx::query_scalar::<_, String>(
                "SELECT session_id FROM agent_runs WHERE task_id=? AND revision<? AND role=? \
                 AND agent=? AND status='SUCCEEDED' AND session_id IS NOT NULL \
                 ORDER BY revision DESC,created_at DESC LIMIT 1",
            )
            .bind(&task.id)
            .bind(task.revision)
            .bind(role.to_string())
            .bind(adapter.kind().to_string())
            .fetch_optional(self.store.pool())
            .await?
        } else {
            None
        };
        let request = AgentRunRequest {
            task_id: task.id.clone(),
            revision: task.revision,
            commit_sha,
            worktree: required_path(&task.worktree_path)?,
            run_dir: run_dir.into(),
            role,
            input_file: input.into(),
            timeout: Duration::from_secs(timeout_secs),
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            permission: permission_override.unwrap_or(if project.settings.full_access {
                PermissionTier::Yolo
            } else {
                PermissionTier::Normal
            }),
            resume_session_id,
            extra_allowed_commands: config.agents.extra_allowed_commands.clone(),
            env_denylist: project.settings.env_denylist.clone(),
            budget,
        };
        if let Err(error) = self
            .acquire_provider_dispatch(&task.id, &run_id, &adapter.kind(), project)
            .await
        {
            sqlx::query("UPDATE agent_runs SET status='FAILED',finished_at=? WHERE id=?")
                .bind(Utc::now().to_rfc3339())
                .bind(&run_id)
                .execute(self.store.pool())
                .await?;
            return Err(error);
        }
        self.register_cancellation(&task.id, cancellation.clone());
        let task_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id=?")
            .bind(&task.id)
            .fetch_one(self.store.pool())
            .await?;
        if task_status == TaskStatus::Cancelled.to_string() {
            cancellation.cancel();
        }
        // Desktop and daemon use separate Orchestrator handles, so poll the authoritative DB.
        let cancellation_watcher =
            self.watch_task_cancellation(task.id.clone(), cancellation.clone());
        let lease_path = run_dir.join("process-lease.json");
        let started_future = adapter.start(request, cancellation.clone(), tx);
        tokio::pin!(started_future);
        let mut lease_recorded = false;
        let started = loop {
            tokio::select! {
                result = &mut started_future => break result,
                _ = tokio::time::sleep(Duration::from_millis(20)), if !lease_recorded => {
                    if let Ok(lease) = agentflow_process_supervisor::read_process_lease(&lease_path).await {
                        let _ = sqlx::query("UPDATE agent_runs SET child_pid=?,child_started_at=? WHERE id=? AND status='RUNNING'")
                            .bind(i64::from(lease.pid))
                            .bind(&lease.started_at)
                            .bind(&run_id)
                            .execute(self.store.pool())
                            .await;
                        lease_recorded = true;
                    }
                }
            }
        };
        cancellation_watcher.abort();
        self.unregister_cancellation(&task.id);
        let _ = sink.await;
        let running = match started {
            Ok(running) => running,
            Err(error) => {
                let failed = sqlx::query(
                    "UPDATE agent_runs SET status='FAILED',finished_at=? WHERE id=?",
                )
                    .bind(Utc::now().to_rfc3339())
                    .bind(&run_id)
                    .execute(self.store.pool())
                    .await;
                let released = self.release_provider_dispatch(&run_id).await;
                failed?;
                released?;
                self.protect_run_files(run_dir).await?;
                return Err(error.into());
            }
        };
        let finished = self.finish_agent_run(&run_id, adapter.kind(), &running).await;
        let released = self.release_provider_dispatch(&run_id).await;
        finished?;
        released?;
        if running.outcome.exit_code != Some(0)
            || running.outcome.cancelled
            || running.outcome.timed_out
        {
            self.protect_run_files(&running.run_dir).await?;
        }
        Ok(running)
    }
}

fn budget_mode_text(mode: BudgetMode) -> &'static str {
    match mode {
        BudgetMode::Hard => "hard",
        BudgetMode::Soft => "soft",
        BudgetMode::Unavailable => "unavailable",
    }
}

/// Providers that emit cumulative usage before exit can be cancelled immediately.
/// Providers without such events remain explicitly `soft`; API and Claude cost caps
/// are enforced inside the Provider request/CLI instead.
fn live_budget_exceeded(provider: &AgentKind, event: &AgentEvent, budget: &RunBudget) -> bool {
    let Some(raw) = event.text.as_deref() else {
        return false;
    };
    let telemetry = if provider == &AgentKind::ClaudeCode {
        parse_claude_telemetry(raw)
    } else if provider == &AgentKind::Codex {
        parse_codex_telemetry(raw)
    } else {
        return false;
    };
    let tokens = telemetry
        .tokens_in
        .zip(telemetry.tokens_out)
        .map(|(input, output)| input.saturating_add(output).max(0) as u64);
    tokens
        .zip(budget.remaining_tokens)
        .is_some_and(|(used, remaining)| used > remaining)
        || telemetry
            .cost_usd
            .zip(budget.remaining_cost_usd)
            .is_some_and(|(used, remaining)| used > remaining)
}
