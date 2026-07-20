#[derive(Debug, Clone, Copy)]
struct DispatchLimit {
    max_concurrent: u32,
    requests_per_minute: u32,
}

impl Orchestrator {
    async fn acquire_provider_dispatch(
        &self,
        task_id: &str,
        run_id: &str,
        provider: &AgentKind,
        project: &ProjectRow,
    ) -> Result<(), OrchestratorError> {
        let settings = self.settings_get().await?;
        let account = provider_account(project, provider);
        let limit = dispatch_limit(&settings, provider, &account);
        loop {
            if self.task(task_id).await?.status == TaskStatus::Cancelled {
                return Err(OrchestratorError::InvalidState(
                    "provider dispatch cancelled".into(),
                ));
            }
            let minute_ago = (Utc::now() - chrono::Duration::seconds(60)).to_rfc3339();
            let recent: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM provider_dispatch_history WHERE provider=? AND account=? AND dispatched_at>=?",
            )
            .bind(provider.to_string())
            .bind(&account)
            .bind(minute_ago)
            .fetch_one(self.store.pool())
            .await?;
            if recent < i64::from(limit.requests_per_minute) {
                for slot in 0..limit.max_concurrent {
                    let inserted = sqlx::query(
                        "INSERT OR IGNORE INTO provider_slots(provider,account,slot,run_id,acquired_at) VALUES(?,?,?,?,?)",
                    )
                    .bind(provider.to_string())
                    .bind(&account)
                    .bind(i64::from(slot))
                    .bind(run_id)
                    .bind(Utc::now().to_rfc3339())
                    .execute(self.store.pool())
                    .await?;
                    if inserted.rows_affected() == 1 {
                        sqlx::query("INSERT INTO provider_dispatch_history(provider,account,dispatched_at) VALUES(?,?,?)")
                            .bind(provider.to_string())
                            .bind(&account)
                            .bind(Utc::now().to_rfc3339())
                            .execute(self.store.pool())
                            .await?;
                        return Ok(());
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn release_provider_dispatch(&self, run_id: &str) -> Result<(), OrchestratorError> {
        sqlx::query("DELETE FROM provider_slots WHERE run_id=?")
            .bind(run_id)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn recover_provider_dispatches(&self) -> Result<(), OrchestratorError> {
        sqlx::query(
            "DELETE FROM provider_slots WHERE run_id NOT IN (SELECT id FROM agent_runs WHERE status='RUNNING')",
        )
        .execute(self.store.pool())
        .await?;
        let cutoff = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        sqlx::query("DELETE FROM provider_dispatch_history WHERE dispatched_at<?")
            .bind(cutoff)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn global_daily_cost_remaining(&self) -> Result<Option<f64>, OrchestratorError> {
        let Some(limit) = self.settings_get().await?.global_daily_cost_usd else {
            return Ok(None);
        };
        let day = Utc::now().date_naive().format("%Y-%m-%dT00:00:00Z").to_string();
        let used: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(COALESCE(cost_usd,reserved_cost_usd,0)),0) FROM agent_runs WHERE created_at>=? AND status IN ('RUNNING','SUCCEEDED','FAILED','TIMED_OUT')",
        )
        .bind(day)
        .fetch_one(self.store.pool())
        .await?;
        Ok(Some((limit - used).max(0.0)))
    }

    async fn enforce_global_budget(&self, task: &TaskRow) -> Result<bool, OrchestratorError> {
        if self.global_daily_cost_remaining().await? != Some(0.0) {
            return Ok(false);
        }
        let detail = "AgentFlow 全局每日费用预算已用尽；UTC 次日或提高预算后可恢复";
        sqlx::query("UPDATE tasks SET blocked_detail=? WHERE id=?")
            .bind(detail)
            .bind(&task.id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                &task.id,
                &[
                    TaskStatus::Planning,
                    TaskStatus::ReadyForDevelopment,
                    TaskStatus::ReadyForRevision,
                    TaskStatus::Validating,
                    TaskStatus::ReadyForReview,
                ],
                TaskStatus::Blocked,
                Some(BlockedReason::BudgetExceeded),
                Actor::System,
                "scheduler:global_budget_exhausted",
                &json!({}),
            )
            .await?;
        Ok(true)
    }
}

fn dispatch_limit(
    settings: &GlobalSettings,
    provider: &AgentKind,
    account: &str,
) -> DispatchLimit {
    let matched = settings
        .provider_limits
        .iter()
        .filter(|item| &item.provider == provider)
        .filter(|item| item.account.as_deref().is_none_or(|value| value == account))
        .max_by_key(|item| usize::from(item.account.is_some()));
    DispatchLimit {
        max_concurrent: matched
            .map(|item| item.max_concurrent)
            .unwrap_or(settings.default_provider_max_concurrent)
            .clamp(1, 16),
        requests_per_minute: matched
            .map(|item| item.requests_per_minute)
            .unwrap_or(settings.default_provider_requests_per_minute)
            .clamp(1, 600),
    }
}

fn provider_account(project: &ProjectRow, provider: &AgentKind) -> String {
    let key = match provider {
        AgentKind::OpenAiApi => Some(&project.settings.openai.api_key_env),
        AgentKind::AnthropicApi => Some(&project.settings.anthropic.api_key_env),
        AgentKind::DeepSeekApi => Some(&project.settings.deepseek.api_key_env),
        AgentKind::GrokApi => Some(&project.settings.grok.api_key_env),
        AgentKind::MiniMaxApi => Some(&project.settings.minimax.api_key_env),
        AgentKind::KimiApi => Some(&project.settings.kimi.api_key_env),
        _ => None,
    };
    key.filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| "local-login".into())
}
