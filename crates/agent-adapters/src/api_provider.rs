#[derive(Clone)]
pub struct ApiProviderAdapter {
    kind: AgentKind,
    settings: ApiProviderSettings,
    client: Client,
    #[cfg(test)]
    api_key_override: Option<String>,
}

struct ApiCallResult {
    text: String,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    cost_usd: Option<f64>,
}

impl std::fmt::Debug for ApiProviderAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiProviderAdapter")
            .field("kind", &self.kind)
            .field("base_url", &self.settings.base_url)
            .field("model", &self.settings.model)
            .field("api_key_env", &self.settings.api_key_env)
            .finish_non_exhaustive()
    }
}

impl ApiProviderAdapter {
    pub fn new(kind: AgentKind, settings: ApiProviderSettings) -> Self {
        Self {
            kind,
            settings,
            client: Client::new(),
            #[cfg(test)]
            api_key_override: None,
        }
    }

    fn api_key(&self) -> Result<String, AdapterError> {
        #[cfg(test)]
        if let Some(key) = &self.api_key_override {
            return Ok(key.clone());
        }
        resolve_api_key(&self.settings).map_err(|message| AdapterError::Provider {
            provider: self.kind.clone(),
            status: None,
            message,
            retryable: false,
        })
    }

    #[cfg(test)]
    fn with_api_key(mut self, key: &str) -> Self {
        self.api_key_override = Some(key.into());
        self
    }

    async fn call_api(
        &self,
        prompt: &str,
        budget: &RunBudget,
        cancel: &CancellationToken,
        tx: &mpsc::Sender<AgentEvent>,
    ) -> Result<ApiCallResult, AdapterError> {
        let key = self.api_key()?;
        let max_output_tokens = self.budgeted_max_output_tokens(prompt, budget)?;
        // Retrying an ambiguous network failure can be billed twice. A hard dollar
        // budget therefore gets exactly one request; ordinary soft-budget runs keep
        // the configured transport retries.
        let attempts = if budget.remaining_cost_usd.is_some()
            && self.budget_capabilities().cost == BudgetMode::Hard
        {
            1
        } else {
            self.settings.max_retries.saturating_add(1)
        };
        for attempt in 0..attempts {
            let request = match self.kind {
                AgentKind::OpenAiApi | AgentKind::GrokApi => {
                    self.responses_request(prompt, &key, max_output_tokens)
                }
                AgentKind::AnthropicApi => {
                    self.anthropic_request(prompt, &key, max_output_tokens)
                }
                AgentKind::DeepSeekApi | AgentKind::MiniMaxApi | AgentKind::KimiApi => {
                    self.chat_completions_request(prompt, &key, max_output_tokens)
                }
                _ => return Err(AdapterError::Incompatible("not an API provider".into())),
            };
            let response = tokio::select! {
                _ = cancel.cancelled() => {
                    return Err(AdapterError::Provider {
                        provider: self.kind.clone(),
                        status: None,
                        message: "request cancelled".into(),
                        retryable: false,
                    });
                }
                response = request.send() => response,
            };
            match response {
                Ok(response) if response.status().is_success() => {
                    let value: Value =
                        response
                            .json()
                            .await
                            .map_err(|error| AdapterError::Provider {
                                provider: self.kind.clone(),
                                status: None,
                                message: format!("invalid JSON response: {error}"),
                                retryable: false,
                            })?;
                    let text = self.extract_output(&value)?;
                    let (tokens_in, tokens_out, cost_usd) = self.extract_telemetry(&value);
                    return Ok(ApiCallResult {
                        text,
                        tokens_in,
                        tokens_out,
                        cost_usd,
                    });
                }
                Ok(response) => {
                    let status = response.status();
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok());
                    let body = response.text().await.unwrap_or_default();
                    let retryable =
                        status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                    if retryable && attempt + 1 < attempts {
                        let delay = retry_after
                            .map(Duration::from_secs)
                            .unwrap_or_else(|| retry_delay(attempt));
                        emit_provider_event(
                            tx,
                            format!(
                                "{} returned {}; retry {}/{} in {:?}",
                                self.kind,
                                status.as_u16(),
                                attempt + 1,
                                attempts - 1,
                                delay
                            ),
                        )
                        .await;
                        tokio::select! {
                            _ = cancel.cancelled() => {
                                return Err(AdapterError::Provider {
                                    provider: self.kind.clone(),
                                    status: Some(status.as_u16()),
                                    message: "request cancelled during retry delay".into(),
                                    retryable: false,
                                });
                            }
                            _ = tokio::time::sleep(delay) => {}
                        }
                        continue;
                    }
                    return Err(AdapterError::Provider {
                        provider: self.kind.clone(),
                        status: Some(status.as_u16()),
                        message: safe_error_body(&body),
                        retryable,
                    });
                }
                Err(error) if attempt + 1 < attempts => {
                    let delay = retry_delay(attempt);
                    emit_provider_event(
                        tx,
                        format!(
                            "{} transport error; retry {}/{} in {:?}",
                            self.kind,
                            attempt + 1,
                            attempts - 1,
                            delay
                        ),
                    )
                    .await;
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            return Err(AdapterError::Provider {
                                provider: self.kind.clone(),
                                status: None,
                                message: "request cancelled during retry delay".into(),
                                retryable: false,
                            });
                        }
                        _ = tokio::time::sleep(delay) => {}
                    }
                    let _ = error;
                }
                Err(error) => {
                    return Err(AdapterError::Provider {
                        provider: self.kind.clone(),
                        status: None,
                        message: error.to_string(),
                        retryable: true,
                    });
                }
            }
        }
        Err(AdapterError::Provider {
            provider: self.kind.clone(),
            status: None,
            message: "retry budget exhausted".into(),
            retryable: true,
        })
    }

    fn responses_request(
        &self,
        prompt: &str,
        key: &str,
        max_output_tokens: u32,
    ) -> reqwest::RequestBuilder {
        self.client
            .post(format!(
                "{}/responses",
                self.settings.base_url.trim_end_matches('/')
            ))
            .bearer_auth(key)
            .json(&json!({
                "model": self.settings.model,
                "input": prompt,
                "store": false,
                "max_output_tokens": max_output_tokens,
                "text": {
                    "format": {
                        "type": "json_schema",
                        "name": "agentflow_review",
                        "description": "AgentFlow code review result",
                        "schema": review_result_schema(),
                        "strict": false
                    }
                }
            }))
    }

    fn anthropic_request(
        &self,
        prompt: &str,
        key: &str,
        max_output_tokens: u32,
    ) -> reqwest::RequestBuilder {
        let schema = serde_json::to_string(&review_result_schema()).unwrap_or_default();
        self.client
            .post(format!("{}/messages", self.settings.base_url.trim_end_matches('/')))
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.settings.model,
                "max_tokens": max_output_tokens,
                "system": format!(
                    "You are AgentFlow's read-only code reviewer. Return only one JSON object matching this schema: {schema}"
                ),
                "messages": [{"role": "user", "content": prompt}]
            }))
    }

    fn chat_completions_request(
        &self,
        prompt: &str,
        key: &str,
        max_output_tokens: u32,
    ) -> reqwest::RequestBuilder {
        let schema = serde_json::to_string(&review_result_schema()).unwrap_or_default();
        let mut body = json!({
            "model": self.settings.model,
            "max_tokens": max_output_tokens,
            "stream": false,
            "response_format": {"type": "json_object"},
            "messages": [
                {
                    "role": "system",
                    "content": format!(
                        "You are AgentFlow's read-only code reviewer. Return only one JSON object matching this JSON schema: {schema}"
                    )
                },
                {"role": "user", "content": prompt}
            ]
        });
        // DeepSeek accepts this extension; other OpenAI-compatible services may reject it.
        if self.kind == AgentKind::DeepSeekApi {
            body["thinking"] = json!({"type": "disabled"});
        }
        self.client
            .post(format!(
                "{}/chat/completions",
                self.settings.base_url.trim_end_matches('/')
            ))
            .bearer_auth(key)
            // Kimi requires clients to identify themselves honestly; use the same real identity
            // for every compatible service rather than impersonating another coding tool.
            .header(
                reqwest::header::USER_AGENT,
                concat!("AgentFlow/", env!("CARGO_PKG_VERSION")),
            )
            .json(&body)
    }

    /// Returns a conservative output cap. UTF-8 bytes are an upper bound for BPE
    /// token count; schema/system bytes and fixed request overhead are included so
    /// the Provider cannot cross the saved token or price-snapshot budget.
    fn budgeted_max_output_tokens(
        &self,
        prompt: &str,
        budget: &RunBudget,
    ) -> Result<u32, AdapterError> {
        let schema_bytes = serde_json::to_vec(&review_result_schema())
            .map_err(|error| AdapterError::InvalidResult(error.to_string()))?
            .len() as u64;
        let input_upper = (prompt.len() as u64)
            .saturating_add(schema_bytes)
            .saturating_add(self.settings.model.len() as u64)
            .saturating_add(1_024);
        let mut cap = u64::from(self.settings.max_output_tokens);
        if let Some(remaining) = budget.remaining_tokens {
            cap = cap.min(remaining.saturating_sub(input_upper));
        }
        if let Some(remaining_cost) = budget.remaining_cost_usd
            && let (Some(input_rate), Some(output_rate)) = (
                self.settings.input_cost_per_million,
                self.settings.output_cost_per_million,
            )
        {
            if !input_rate.is_finite()
                || !output_rate.is_finite()
                || input_rate < 0.0
                || output_rate <= 0.0
            {
                return Err(self.budget_error("invalid API pricing snapshot"));
            }
            let input_cost = input_upper as f64 * input_rate / 1_000_000.0;
            let output_allowance = (remaining_cost - input_cost).max(0.0);
            let cost_cap = (output_allowance * 1_000_000.0 / output_rate).floor() as u64;
            cap = cap.min(cost_cap);
        }
        if cap == 0 {
            return Err(self.budget_error(
                "remaining hard budget cannot cover request context plus one output token",
            ));
        }
        Ok(cap.min(u64::from(u32::MAX)) as u32)
    }

    fn budget_error(&self, message: &str) -> AdapterError {
        AdapterError::Provider {
            provider: self.kind.clone(),
            status: None,
            message: format!("BUDGET_EXCEEDED: {message}"),
            retryable: false,
        }
    }

    fn extract_output(&self, value: &Value) -> Result<String, AdapterError> {
        let text = match self.kind {
            AgentKind::OpenAiApi | AgentKind::GrokApi => value
                .get("output_text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    value
                        .get("output")?
                        .as_array()?
                        .iter()
                        .flat_map(|item| {
                            item.get("content")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                        })
                        .find_map(|content| {
                            (content.get("type").and_then(Value::as_str) == Some("output_text"))
                                .then(|| content.get("text").and_then(Value::as_str))
                                .flatten()
                                .map(str::to_string)
                        })
                }),
            AgentKind::AnthropicApi => {
                value
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| {
                        content.iter().find_map(|item| {
                            (item.get("type").and_then(Value::as_str) == Some("text"))
                                .then(|| item.get("text").and_then(Value::as_str))
                                .flatten()
                                .map(str::to_string)
                        })
                    })
            }
            AgentKind::DeepSeekApi | AgentKind::MiniMaxApi | AgentKind::KimiApi => value
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("message"))
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        };
        text.ok_or_else(|| AdapterError::Provider {
            provider: self.kind.clone(),
            status: None,
            message: "response did not contain output text".into(),
            retryable: false,
        })
    }

    fn extract_telemetry(&self, value: &Value) -> (Option<i64>, Option<i64>, Option<f64>) {
        let usage = value.get("usage").unwrap_or(&Value::Null);
        let tokens_in = usage
            .get("input_tokens")
            .and_then(Value::as_i64)
            .or_else(|| usage.get("prompt_tokens").and_then(Value::as_i64))
            .map(|tokens| {
                tokens
                    .saturating_add(
                        usage
                            .get("cache_creation_input_tokens")
                            .and_then(Value::as_i64)
                            .unwrap_or(0),
                    )
                    .saturating_add(
                        usage
                            .get("cache_read_input_tokens")
                            .and_then(Value::as_i64)
                            .unwrap_or(0),
                    )
            });
        let tokens_out = usage
            .get("output_tokens")
            .and_then(Value::as_i64)
            .or_else(|| usage.get("completion_tokens").and_then(Value::as_i64));
        let reported_cost = value
            .get("cost_usd")
            .and_then(Value::as_f64)
            .or_else(|| usage.get("cost_usd").and_then(Value::as_f64))
            .or_else(|| usage.get("total_cost_usd").and_then(Value::as_f64))
            .or_else(|| usage.get("cost").and_then(Value::as_f64));
        let estimated_cost = match (
            tokens_in,
            tokens_out,
            self.settings.input_cost_per_million,
            self.settings.output_cost_per_million,
        ) {
            (Some(input), Some(output), Some(input_rate), Some(output_rate)) => {
                Some((input as f64 * input_rate + output as f64 * output_rate) / 1_000_000.0)
            }
            _ => None,
        };
        (tokens_in, tokens_out, reported_cost.or(estimated_cost))
    }
}

#[async_trait]
impl AgentProvider for ApiProviderAdapter {
    fn kind(&self) -> AgentKind {
        self.kind.clone()
    }

    async fn detect(&self, _env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        let _ = self.api_key()?;
        Ok(AgentInstallation {
            path: PathBuf::from(&self.settings.base_url),
            version: self.settings.model.clone(),
            capabilities: self.capabilities(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streams_events: true,
            native_output_schema: true,
            supports_resume: false,
            read_only_mode: true,
            supports_development: false,
            supports_review: true,
        }
    }

    fn budget_capabilities(&self) -> BudgetCapabilities {
        let priced = self
            .settings
            .input_cost_per_million
            .zip(self.settings.output_cost_per_million)
            .is_some_and(|(input, output)| {
                input.is_finite() && output.is_finite() && input >= 0.0 && output > 0.0
            });
        BudgetCapabilities {
            tokens: BudgetMode::Hard,
            cost: if priced {
                BudgetMode::Hard
            } else {
                BudgetMode::Soft
            },
        }
    }

    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        if req.role != RunRole::Reviewer {
            return Err(AdapterError::UnsupportedRole(req.role));
        }
        tokio::fs::create_dir_all(&req.run_dir).await?;
        let input_path = req.worktree.join(&req.input_file);
        let prompt = tokio::fs::read_to_string(&input_path).await?;
        let started_at = Utc::now().to_rfc3339();
        emit_provider_event(
            &tx,
            format!("calling {} model {}", self.kind, self.settings.model),
        )
        .await;
        let output = tokio::time::timeout(
            req.timeout,
            self.call_api(&prompt, &req.budget, &cancel, &tx),
        )
            .await
            .map_err(|_| AdapterError::Provider {
                provider: self.kind.clone(),
                status: None,
                message: "request timed out".into(),
                retryable: true,
            })??;
        tokio::fs::write(req.run_dir.join("last-message.json"), &output.text).await?;
        tokio::fs::write(req.run_dir.join("stdout.log"), format!("{}\n", output.text)).await?;
        tokio::fs::write(req.run_dir.join("stderr.log"), "").await?;
        tokio::fs::write(
            req.run_dir.join("provider-telemetry.json"),
            serde_json::to_vec(&json!({
                "cost_usd": output.cost_usd,
                "tokens_in": output.tokens_in,
                "tokens_out": output.tokens_out,
            }))
            .map_err(|error| AdapterError::InvalidResult(error.to_string()))?,
        )
        .await?;
        let _ = tx
            .send(AgentEvent {
                ts: Utc::now().to_rfc3339(),
                stream: EventStream::Stdout,
                kind: AgentEventKind::Result,
                summary: format!("{} returned structured review", self.kind),
                text: None,
            })
            .await;
        Ok(RunningAgent {
            outcome: ProcessOutcome {
                pid: 0,
                started_at,
                exit_code: Some(0),
                timed_out: false,
                cancelled: false,
                log_truncated: false,
            },
            run_dir: req.run_dir,
            role: req.role,
        })
    }

    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        if role != RunRole::Reviewer {
            return Err(AdapterError::UnsupportedRole(role));
        }
        read_review(&run_dir.join("last-message.json"))
            .await
            .map(CollectedResult::Review)
    }
}

include!("api_provider/support.rs");

#[cfg(test)]
#[path = "api_provider/api_telemetry_tests.rs"]
mod api_telemetry_tests;
