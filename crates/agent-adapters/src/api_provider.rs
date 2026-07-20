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
        cancel: &CancellationToken,
        tx: &mpsc::Sender<AgentEvent>,
    ) -> Result<ApiCallResult, AdapterError> {
        let key = self.api_key()?;
        let attempts = self.settings.max_retries.saturating_add(1);
        for attempt in 0..attempts {
            let request = match self.kind {
                AgentKind::OpenAiApi | AgentKind::GrokApi => self.responses_request(prompt, &key),
                AgentKind::AnthropicApi => self.anthropic_request(prompt, &key),
                AgentKind::DeepSeekApi | AgentKind::MiniMaxApi | AgentKind::KimiApi => {
                    self.chat_completions_request(prompt, &key)
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

    fn responses_request(&self, prompt: &str, key: &str) -> reqwest::RequestBuilder {
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

    fn anthropic_request(&self, prompt: &str, key: &str) -> reqwest::RequestBuilder {
        let schema = serde_json::to_string(&review_result_schema()).unwrap_or_default();
        self.client
            .post(format!("{}/messages", self.settings.base_url.trim_end_matches('/')))
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.settings.model,
                "max_tokens": self.settings.max_output_tokens,
                "system": format!(
                    "You are AgentFlow's read-only code reviewer. Return only one JSON object matching this schema: {schema}"
                ),
                "messages": [{"role": "user", "content": prompt}]
            }))
    }

    fn chat_completions_request(&self, prompt: &str, key: &str) -> reqwest::RequestBuilder {
        let schema = serde_json::to_string(&review_result_schema()).unwrap_or_default();
        let mut body = json!({
            "model": self.settings.model,
            "max_tokens": self.settings.max_output_tokens,
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
        let output = tokio::time::timeout(req.timeout, self.call_api(&prompt, &cancel, &tx))
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

fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(500_u64.saturating_mul(1_u64 << attempt.min(6)))
}

async fn emit_provider_event(tx: &mpsc::Sender<AgentEvent>, summary: String) {
    let _ = tx
        .send(AgentEvent {
            ts: Utc::now().to_rfc3339(),
            stream: EventStream::Stdout,
            kind: AgentEventKind::System,
            summary,
            text: None,
        })
        .await;
}

fn safe_error_body(body: &str) -> String {
    agentflow_process_supervisor::redact(body.chars().take(2_000).collect())
}

pub fn api_provider_status(settings: &ApiProviderSettings) -> ProviderStatus {
    let configured = !settings.model.trim().is_empty()
        && !settings.base_url.trim().is_empty()
        && !settings.api_key_env.trim().is_empty();
    let key_available = resolve_api_key(settings).is_ok();
    ProviderStatus {
        configured,
        available: configured && key_available,
        model: settings.model.clone(),
        base_url: settings.base_url.clone(),
        key_env: settings.api_key_env.clone(),
        problem: if !configured {
            Some("base URL, model, and key environment variable are required".into())
        } else if !key_available {
            Some(format!(
                "{} is not set and Keychain service {} has no key",
                settings.api_key_env, settings.keychain_service
            ))
        } else {
            None
        },
    }
}

#[cfg(test)]
mod api_telemetry_tests {
    use super::*;

    #[test]
    fn extracts_openai_compatible_usage_and_estimates_configured_cost() {
        let mut settings = ApiProviderSettings::deepseek_default();
        settings.input_cost_per_million = Some(1.0);
        settings.output_cost_per_million = Some(2.0);
        let adapter = ApiProviderAdapter::new(AgentKind::DeepSeekApi, settings);
        let (input, output, cost) = adapter.extract_telemetry(&json!({
            "usage": {"prompt_tokens": 1000, "completion_tokens": 500}
        }));
        assert_eq!((input, output), (Some(1000), Some(500)));
        assert_eq!(cost, Some(0.002));
    }

    #[test]
    fn provider_reported_cost_wins_over_estimate_and_anthropic_cache_is_counted() {
        let adapter = ApiProviderAdapter::new(
            AgentKind::AnthropicApi,
            ApiProviderSettings::anthropic_default(),
        );
        let (input, output, cost) = adapter.extract_telemetry(&json!({
            "usage": {
                "input_tokens": 100,
                "cache_creation_input_tokens": 30,
                "cache_read_input_tokens": 20,
                "output_tokens": 10,
                "cost_usd": 0.03
            }
        }));
        assert_eq!((input, output), (Some(150), Some(10)));
        assert_eq!(cost, Some(0.03));
    }
}

fn resolve_api_key(settings: &ApiProviderSettings) -> Result<String, String> {
    if let Ok(key) = std::env::var(&settings.api_key_env)
        && !key.trim().is_empty()
    {
        return Ok(key);
    }
    #[cfg(target_os = "macos")]
    {
        // New desktop credentials use Security.framework directly so the secret never appears in
        // a process argument. Keep the `security` fallback for credentials created by older builds.
        if let Ok(bytes) = security_framework::passwords::get_generic_password(
            &settings.keychain_service,
            "AgentFlow",
        ) && let Ok(key) = String::from_utf8(bytes)
            && !key.trim().is_empty()
        {
            return Ok(key.trim().to_string());
        }
        let output = std::process::Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                &settings.keychain_service,
                "-w",
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }
    Err(format!(
        "{} is not set and no Keychain credential is available",
        settings.api_key_env
    ))
}
