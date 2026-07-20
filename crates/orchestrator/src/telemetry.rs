#[derive(Debug, Default, Deserialize, PartialEq)]
struct ProviderTelemetry {
    session_id: Option<String>,
    cost_usd: Option<f64>,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
}

impl Orchestrator {
    fn register_cancellation(&self, task_id: &str, token: CancellationToken) {
        if let Ok(mut active) = self.active_cancellations.write() {
            active.insert(task_id.to_string(), token);
        }
    }

    fn unregister_cancellation(&self, task_id: &str) {
        if let Ok(mut active) = self.active_cancellations.write() {
            active.remove(task_id);
        }
    }

    fn cancel_active_run(&self, task_id: &str) -> bool {
        self.active_cancellations
            .read()
            .ok()
            .and_then(|active| active.get(task_id).cloned())
            .is_some_and(|token| {
                token.cancel();
                true
            })
    }

    fn watch_task_cancellation(
        &self,
        task_id: String,
        cancellation: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let pool = self.store.pool().clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {
                        let status = sqlx::query_scalar::<_, String>("SELECT status FROM tasks WHERE id=?")
                            .bind(&task_id)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten();
                        if status.as_deref() == Some("CANCELLED") {
                            cancellation.cancel();
                            break;
                        }
                    }
                }
            }
        })
    }

    /// A zero exit code only means the provider process ended cleanly. If its contract output is
    /// missing, malformed or belongs to another task, the run must not remain visibly successful.
    async fn invalidate_agent_run(&self, run_dir: &Path) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE agent_runs SET status='FAILED' WHERE run_dir=?")
            .bind(run_dir.to_string_lossy().as_ref())
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn finish_agent_run(
        &self,
        run_id: &str,
        provider: AgentKind,
        running: &agentflow_agent_adapters::RunningAgent,
    ) -> Result<(), OrchestratorError> {
        let telemetry = if provider == AgentKind::ClaudeCode {
            tokio::fs::read_to_string(running.run_dir.join("stdout.log"))
                .await
                .ok()
                .map(|text| parse_claude_telemetry(&text))
                .unwrap_or_default()
        } else if provider == AgentKind::Codex {
            tokio::fs::read_to_string(running.run_dir.join("stdout.log"))
                .await
                .ok()
                .map(|text| parse_codex_telemetry(&text))
                .unwrap_or_default()
        } else {
            tokio::fs::read_to_string(running.run_dir.join("provider-telemetry.json"))
                .await
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default()
        };
        let status = if running.outcome.cancelled {
            "CANCELLED"
        } else if running.outcome.timed_out {
            "TIMED_OUT"
        } else if running.outcome.exit_code == Some(0) {
            "SUCCEEDED"
        } else {
            "FAILED"
        };
        sqlx::query(
            "UPDATE agent_runs SET status=?,child_pid=?,child_started_at=?,exit_code=?, \
             session_id=?,cost_usd=?,tokens_in=?,tokens_out=?,finished_at=? WHERE id=?",
        )
        .bind(status)
        .bind(running.outcome.pid as i64)
        .bind(&running.outcome.started_at)
        .bind(running.outcome.exit_code)
        .bind(telemetry.session_id)
        .bind(telemetry.cost_usd)
        .bind(telemetry.tokens_in)
        .bind(telemetry.tokens_out)
        .bind(Utc::now().to_rfc3339())
        .bind(run_id)
        .execute(self.store.pool())
        .await?;
        Ok(())
    }
}

fn parse_claude_telemetry(jsonl: &str) -> ProviderTelemetry {
    let mut telemetry = ProviderTelemetry::default();
    for value in jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if let Some(session_id) = value.get("session_id").and_then(Value::as_str) {
            telemetry.session_id = Some(session_id.to_string());
        }
        if value.get("type").and_then(Value::as_str) != Some("result") {
            continue;
        }
        telemetry.cost_usd = value.get("total_cost_usd").and_then(Value::as_f64);
        if let Some(usage) = value.get("usage") {
            telemetry.tokens_in = sum_token_fields(
                usage,
                &[
                    "input_tokens",
                    "cache_creation_input_tokens",
                    "cache_read_input_tokens",
                ],
            );
            telemetry.tokens_out = usage.get("output_tokens").and_then(Value::as_i64);
        }
    }
    telemetry
}

fn parse_codex_telemetry(jsonl: &str) -> ProviderTelemetry {
    let mut telemetry = ProviderTelemetry::default();
    for value in jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if value.get("type").and_then(Value::as_str) != Some("turn.completed") {
            continue;
        }
        let Some(usage) = value.get("usage") else {
            continue;
        };
        // Codex reports cached_input_tokens as a subset of input_tokens, so do
        // not add it a second time.
        telemetry.tokens_in = usage.get("input_tokens").and_then(Value::as_i64);
        telemetry.tokens_out = usage.get("output_tokens").and_then(Value::as_i64);
    }
    telemetry
}

fn sum_token_fields(value: &Value, fields: &[&str]) -> Option<i64> {
    let values = fields
        .iter()
        .filter_map(|field| value.get(field).and_then(Value::as_i64))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.into_iter().sum())
}

#[cfg(test)]
mod telemetry_tests {
    use super::*;

    #[test]
    fn claude_and_codex_usage_formats_are_normalized_without_double_counting_cache() {
        let claude = parse_claude_telemetry(
            r#"{"type":"result","total_cost_usd":0.42,"usage":{"input_tokens":100,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":40}}"#,
        );
        assert_eq!(claude.tokens_in, Some(150));
        assert_eq!(claude.tokens_out, Some(40));
        assert_eq!(claude.cost_usd, Some(0.42));
        let codex = parse_codex_telemetry(
            r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":40,"reasoning_output_tokens":10}}"#,
        );
        assert_eq!(codex.tokens_in, Some(100));
        assert_eq!(codex.tokens_out, Some(40));
    }
}
