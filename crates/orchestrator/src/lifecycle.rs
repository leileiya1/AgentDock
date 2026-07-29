impl Orchestrator {
    /// Open the authoritative scheduler instance. Only the daemon should use this in production,
    /// because recovery mutates RUNNING rows and may terminate abandoned work.
    pub async fn open(app_data: impl Into<PathBuf>) -> Result<Self, OrchestratorError> {
        Self::open_with_recovery(app_data.into(), true).await
    }

    /// Open a query/client instance without performing owner-only recovery. Desktop and CLI
    /// clients may read through this handle, while every mutation is forwarded to agentflowd.
    pub async fn open_client(app_data: impl Into<PathBuf>) -> Result<Self, OrchestratorError> {
        Self::open_with_recovery(app_data.into(), false).await
    }

    async fn open_with_recovery(
        app_data: PathBuf,
        recover: bool,
    ) -> Result<Self, OrchestratorError> {
        tokio::fs::create_dir_all(&app_data).await?;
        let schema_dir = app_data.join("schemas");
        tokio::fs::create_dir_all(&schema_dir).await?;
        tokio::fs::write(
            schema_dir.join("result.schema.json"),
            serde_json::to_vec_pretty(&development_result_schema())
                .map_err(|e| OrchestratorError::Config(e.to_string()))?,
        )
        .await?;
        tokio::fs::write(
            schema_dir.join("review.schema.json"),
            serde_json::to_vec_pretty(&review_result_schema())
                .map_err(|e| OrchestratorError::Config(e.to_string()))?,
        )
        .await?;
        tokio::fs::write(
            schema_dir.join("plan.schema.json"),
            serde_json::to_vec_pretty(&plan_result_schema())
                .map_err(|e| OrchestratorError::Config(e.to_string()))?,
        )
        .await?;
        let database = app_data.join("agentflow.db");
        let store = if recover {
            Store::open(&database).await?
        } else {
            Store::open_client(&database).await?
        };
        let provider_registry = ProviderRegistry::discover(&app_data.join("providers"))
            .await
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let orchestrator = Self {
            store,
            git: Git::default(),
            app_data,
            provider_registry: Arc::new(RwLock::new(provider_registry)),
            active_cancellations: Arc::new(RwLock::new(HashMap::new())),
            runtime_probe_cache: Arc::new(RwLock::new(HashMap::new())),
            run_log_cursors: Arc::new(RwLock::new(HashMap::new())),
        };
        if recover {
            orchestrator.recover_interrupted_runs().await?;
        }
        Ok(orchestrator)
    }
    pub fn app_data(&self) -> &Path {
        &self.app_data
    }

    /// Stop every daemon-owned Provider process group without changing the user's task intent.
    /// Interrupted task state is repaired by owner recovery on the next daemon start.
    pub fn interrupt_active_runs(&self) -> Vec<String> {
        let mut task_ids = Vec::new();
        if let Ok(tokens) = self.active_cancellations.read() {
            for (task_id, token) in tokens.iter() {
                task_ids.push(task_id.clone());
                token.cancel();
            }
        }
        task_ids
    }

    /// Put workflows interrupted by a daemon shutdown back into a resumable scheduler state.
    pub async fn requeue_interrupted_tasks(
        &self,
        task_ids: &[String],
    ) -> Result<(), OrchestratorError> {
        for task_id in task_ids {
            let task = self.task(task_id).await?;
            let (to, revision) = match task.status {
                TaskStatus::Planning => (TaskStatus::Planning, task.revision),
                TaskStatus::Developing => (
                    TaskStatus::ReadyForDevelopment,
                    task.revision.saturating_sub(1),
                ),
                TaskStatus::Revising => (
                    TaskStatus::ReadyForRevision,
                    task.revision.saturating_sub(1),
                ),
                TaskStatus::Reviewing => (TaskStatus::ReadyForReview, task.revision),
                _ => continue,
            };
            sqlx::query("UPDATE tasks SET current_revision=? WHERE id=?")
                .bind(revision)
                .bind(task_id)
                .execute(self.store.pool())
                .await?;
            self.store
                .transition(
                    task_id,
                    &[task.status],
                    to,
                    None,
                    Actor::System,
                    "daemon:shutdown_interrupted",
                    &json!({}),
                )
                .await?;
        }
        Ok(())
    }
    pub async fn env_check(&self) -> EnvReport {
        let openai_settings = ApiProviderSettings::openai_default();
        let anthropic_settings = ApiProviderSettings::anthropic_default();
        let deepseek_settings = ApiProviderSettings::deepseek_default();
        let grok_settings = ApiProviderSettings::grok_default();
        let minimax_settings = ApiProviderSettings::minimax_default();
        let kimi_settings = ApiProviderSettings::kimi_default();
        let (git_path, claude_path, codex_path, gemini_path, qwen_path, qoder_path, grok_path, kimi_path, minimax_path) = tokio::join!(
            self.cli_override("git"), self.cli_override("claude_code"),
            self.cli_override("codex"), self.cli_override("gemini_cli"),
            self.cli_override("qwen_code"), self.cli_override("qoder_cli"),
            self.cli_override("grok_cli"),
            self.cli_override("kimi_cli"), self.cli_override("minimax_cli"),
        );
        // All side-effect-free tool checks are independent. Run them concurrently so adding more
        // installed CLIs does not make the settings screen progressively slower.
        let (system, git, node, bun, claude_code, codex, gemini_cli, qwen_code, qoder_cli, grok_cli, kimi_cli, minimax_cli, openai_api, anthropic_api, deepseek_api, grok_api, minimax_api, kimi_api) = tokio::join!(
            system_environment(&self.app_data),
            bounded_tool_status("git", git_path, &[]),
            bounded_tool_status("node", None, &[]),
            bounded_tool_status("bun", None, &[]),
            bounded_tool_status("claude", claude_path, &["--output-format", "--permission-mode"]),
            bounded_tool_status("codex", codex_path, &["--json", "--sandbox"]),
            bounded_tool_status("gemini", gemini_path, &["--output-format", "--approval-mode", "--sandbox"]),
            bounded_tool_status("qwen", qwen_path, &["--output-format", "--approval-mode", "--sandbox", "--max-wall-time"]),
            bounded_tool_status("qodercli", qoder_path, &["--output-format", "--permission-mode", "--cwd", "--no-session-persistence"]),
            bounded_tool_status("grok", grok_path, &["--output-format", "--sandbox", "--permission-mode"]),
            bounded_tool_status("kimi", kimi_path, &["--prompt", "--output-format"]),
            bounded_tool_status("mmx", minimax_path, &[]),
            bounded_api_provider_status(&openai_settings),
            bounded_api_provider_status(&anthropic_settings),
            bounded_api_provider_status(&deepseek_settings),
            bounded_api_provider_status(&grok_settings),
            bounded_api_provider_status(&minimax_settings),
            bounded_api_provider_status(&kimi_settings),
        );
        EnvReport {
            system,
            git,
            node,
            bun,
            claude_code,
            codex,
            gemini_cli,
            qwen_code,
            qoder_cli,
            grok_cli,
            kimi_cli,
            minimax_cli,
            openai_api,
            anthropic_api,
            deepseek_api,
            grok_api,
            minimax_api,
            kimi_api,
        }
    }

    /// Returns the runtime Provider catalog used by the desktop app. External packages replace a
    /// built-in descriptor with the same id, which lets compatibility shims fix vendor CLI drift.
    pub async fn provider_list(&self) -> Vec<ProviderDescriptor> {
        self.refresh_provider_registry().await;
        let env = self.env_check().await;
        let mut providers = vec![
            cli_descriptor(AgentKind::ClaudeCode, "Claude Code", &env.claude_code),
            cli_descriptor(AgentKind::Codex, "Codex", &env.codex),
            cli_descriptor(AgentKind::GeminiCli, "Gemini CLI", &env.gemini_cli),
            cli_descriptor(AgentKind::QwenCode, "Qwen Code", &env.qwen_code),
            cli_descriptor(AgentKind::QoderCli, "Qoder CLI", &env.qoder_cli),
            cli_descriptor(AgentKind::GrokCli, "Grok CLI", &env.grok_cli),
            api_descriptor(AgentKind::OpenAiApi, "OpenAI API", &env.openai_api),
            api_descriptor(
                AgentKind::AnthropicApi,
                "Anthropic API",
                &env.anthropic_api,
            ),
            api_descriptor(
                AgentKind::DeepSeekApi,
                "DeepSeek API",
                &env.deepseek_api,
            ),
            api_descriptor(AgentKind::GrokApi, "Grok API", &env.grok_api),
            api_descriptor(
                AgentKind::MiniMaxApi,
                "MiniMax API",
                &env.minimax_api,
            ),
            api_descriptor(AgentKind::KimiApi, "Kimi API", &env.kimi_api),
        ];
        let registry = self
            .provider_registry
            .read()
            .map(|value| value.clone())
            .unwrap_or_default();
        for provider in registry.all() {
            let probe = ProtocolClient::new(provider.clone()).probe().await;
            let (available, problem) = match probe {
                Ok((_, health)) if health.status != agentflow_provider_protocol::HealthStatus::Unavailable => {
                    (true, health.message)
                }
                Ok((_, health)) => (false, health.message.or_else(|| Some("provider is unavailable".into()))),
                Err(error) => (false, Some(error.to_string())),
            };
            let descriptor = ProviderDescriptor {
                id: provider.manifest.id.clone(),
                display_name: provider.manifest.display_name.clone(),
                source: ProviderSource::External,
                protocol_version: provider.manifest.protocol_version.clone(),
                capabilities: provider.manifest.capabilities.clone(),
                execution_location: provider.manifest.execution_location,
                data_egress: provider.manifest.data_egress,
                permissions: provider.manifest.permissions.clone(),
                trust: ProviderTrust::Verified,
                available,
                problem,
            };
            if let Some(position) = providers.iter().position(|item| item.id == descriptor.id) {
                providers[position] = descriptor;
            } else {
                providers.push(descriptor);
            }
        }
        for descriptor in registry.quarantined_descriptors() {
            if let Some(position) = providers.iter().position(|item| item.id == descriptor.id) {
                providers[position] = descriptor;
            } else {
                providers.push(descriptor);
            }
        }
        providers
    }

    pub async fn onboarding_check(
        &self,
        daemon_running: bool,
    ) -> Result<OnboardingReport, OrchestratorError> {
        let env = self.env_check().await;
        let available_cli = [
            (AgentKind::ClaudeCode, &env.claude_code),
            (AgentKind::Codex, &env.codex),
            (AgentKind::GeminiCli, &env.gemini_cli),
            (AgentKind::QwenCode, &env.qwen_code),
            (AgentKind::QoderCli, &env.qoder_cli),
            (AgentKind::GrokCli, &env.grok_cli),
        ]
        .into_iter()
        .filter_map(|(kind, status)| {
            (status.found && status.compatible && status.authenticated != Some(false))
                .then_some(kind)
        })
        .collect::<Vec<_>>();
        let recommended_developer = [
            AgentKind::QoderCli,
            AgentKind::GrokCli,
        ]
        .into_iter()
        .find(|kind| available_cli.contains(kind));
        let mut reviewers = available_cli.clone();
        if env.openai_api.available {
            reviewers.push(AgentKind::OpenAiApi);
        }
        if env.anthropic_api.available {
            reviewers.push(AgentKind::AnthropicApi);
        }
        if env.deepseek_api.available {
            reviewers.push(AgentKind::DeepSeekApi);
        }
        if env.grok_api.available {
            reviewers.push(AgentKind::GrokApi);
        }
        if env.minimax_api.available {
            reviewers.push(AgentKind::MiniMaxApi);
        }
        if env.kimi_api.available {
            reviewers.push(AgentKind::KimiApi);
        }
        let recommended_reviewer = [
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            AgentKind::GeminiCli,
            AgentKind::QwenCode,
            AgentKind::OpenAiApi,
            AgentKind::AnthropicApi,
            AgentKind::DeepSeekApi,
            AgentKind::GrokApi,
            AgentKind::MiniMaxApi,
            AgentKind::KimiApi,
        ]
        .into_iter()
        .find(|kind| {
            reviewers.contains(kind)
                && recommended_developer
                    .as_ref()
                    .is_none_or(|developer| developer != kind)
        });
        let mut notices = Vec::new();
        if !env.git.compatible {
            notices.push("Git 未安装或版本过低，请安装 Git 2.38 以上版本。".into());
        }
        if recommended_developer.is_none() {
            notices.push("还没有可用于开发的 CLI，请安装并登录至少一个。".into());
        }
        if recommended_reviewer.is_none() {
            notices.push("还没有独立审查 Provider，请连接第二个 CLI 或配置 API。".into());
        }
        if !daemon_running {
            notices.push("后台服务未连接，关闭桌面窗口后任务会停止。".into());
        }
        for (name, status) in [("Claude Code", &env.claude_code), ("Codex", &env.codex)] {
            if status.authenticated == Some(false) {
                notices.push(
                    status
                        .auth_problem
                        .as_ref()
                        .map_or_else(|| format!("{name} 已安装但尚未登录。"), |problem| {
                            format!("{name}：{problem}。")
                        }),
                );
            }
        }
        let completed: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key='onboarding:completed'")
                .fetch_optional(self.store.pool())
                .await?;
        Ok(OnboardingReport {
            first_run: completed.is_none(),
            daemon_running,
            app_ready: env.git.compatible && env.system.disk_available_bytes > 100 * 1024 * 1024,
            workflow_ready: env.git.compatible
                && recommended_developer.is_some()
                && recommended_reviewer.is_some(),
            ready: env.git.compatible
                && recommended_developer.is_some()
                && recommended_reviewer.is_some(),
            data_dir: self.app_data.to_string_lossy().into_owned(),
            env,
            recommended_developer,
            recommended_reviewer,
            notices,
            storage: self.storage_report().await?,
        })
    }

    pub async fn onboarding_complete(&self) -> Result<(), OrchestratorError> {
        sqlx::query("INSERT INTO settings(key,value_json) VALUES('onboarding:completed','true') ON CONFLICT(key) DO UPDATE SET value_json='true'")
            .execute(self.store.pool())
            .await?;
        Ok(())
    }
    async fn cli_override(&self, tool: &str) -> Option<PathBuf> {
        sqlx::query_scalar::<_, String>("SELECT value_json FROM settings WHERE key=?")
            .bind(format!("cli:{tool}"))
            .fetch_optional(self.store.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<String>(&v).ok())
            .map(PathBuf::from)
    }
    pub async fn env_set_cli_path(
        &self,
        tool: &str,
        path: &Path,
    ) -> Result<EnvReport, OrchestratorError> {
        if !matches!(
            tool,
            "claude_code"
                | "codex"
                | "gemini_cli"
                | "qwen_code"
                | "qoder_cli"
                | "grok_cli"
                | "kimi_cli"
                | "minimax_cli"
                | "git"
        ) {
            return Err(OrchestratorError::InvalidState("unknown CLI tool".into()));
        }
        let (name, flags): (&str, &[&str]) = match tool {
            "claude_code" => ("claude", &["--output-format", "--permission-mode"]),
            "codex" => ("codex", &["--json", "--sandbox"]),
            "gemini_cli" => (
                "gemini",
                &["--output-format", "--approval-mode", "--sandbox"],
            ),
            "qwen_code" => (
                "qwen",
                &[
                    "--output-format",
                    "--approval-mode",
                    "--sandbox",
                    "--max-wall-time",
                ],
            ),
            "qoder_cli" => (
                "qodercli",
                &["--output-format", "--permission-mode", "--cwd", "--no-session-persistence"],
            ),
            "grok_cli" => (
                "grok",
                &["--output-format", "--sandbox", "--permission-mode"],
            ),
            "kimi_cli" => ("kimi", &["--prompt", "--output-format"]),
            "minimax_cli" => ("mmx", &[]),
            _ => ("git", &[]),
        };
        let status = bounded_tool_status(name, Some(path.to_path_buf()), flags).await;
        if !status.found || !status.compatible {
            return Err(OrchestratorError::InvalidState(
                status.problem.unwrap_or_else(|| "CLI incompatible".into()),
            ));
        }
        sqlx::query("INSERT INTO settings(key,value_json) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json")
            .bind(format!("cli:{tool}")).bind(serde_json::to_string(&path.to_string_lossy().as_ref()).map_err(|e|OrchestratorError::Config(e.to_string()))?).execute(self.store.pool()).await?;
        Ok(self.env_check().await)
    }
}

async fn system_environment(app_data: &Path) -> SystemEnvironment {
    let disk_available_bytes = {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks
            .iter()
            .filter(|disk| app_data.starts_with(disk.mount_point()))
            .max_by_key(|disk| disk.mount_point().as_os_str().len())
            .map_or(0, sysinfo::Disk::available_space)
    };
    let network = match tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::lookup_host(("api.github.com", 443)),
    )
    .await
    {
        Ok(Ok(addresses)) => {
            if addresses.count() > 0 {
                EnvironmentCheck {
                    available: true,
                    detail: Some("DNS 可用（api.github.com）".into()),
                    problem: None,
                }
            } else {
                EnvironmentCheck {
                    available: false,
                    detail: None,
                    problem: Some("DNS 未返回地址".into()),
                }
            }
        }
        Ok(Err(error)) => EnvironmentCheck {
            available: false,
            detail: None,
            problem: Some(format!("网络解析失败：{error}")),
        },
        Err(_) => EnvironmentCheck {
            available: false,
            detail: None,
            problem: Some("网络检测超时".into()),
        },
    };
    #[cfg(target_os = "macos")]
    let keychain = match Command::new("/usr/bin/security")
        .args(["default-keychain", "-d", "user"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => EnvironmentCheck {
            available: true,
            detail: Some(String::from_utf8_lossy(&output.stdout).trim().trim_matches('"').into()),
            problem: None,
        },
        Ok(output) => EnvironmentCheck {
            available: false,
            detail: None,
            problem: Some(String::from_utf8_lossy(&output.stderr).trim().into()),
        },
        Err(error) => EnvironmentCheck {
            available: false,
            detail: None,
            problem: Some(format!("无法调用系统钥匙串：{error}")),
        },
    };
    #[cfg(not(target_os = "macos"))]
    let keychain = EnvironmentCheck {
        available: false,
        detail: None,
        problem: Some("当前平台没有 macOS 钥匙串".into()),
    };
    SystemEnvironment {
        os: std::env::consts::OS.into(),
        os_version: sysinfo::System::long_os_version(),
        architecture: std::env::consts::ARCH.into(),
        agentflow_version: env!("CARGO_PKG_VERSION").into(),
        shell: std::env::var("SHELL").ok(),
        disk_available_bytes,
        network,
        keychain,
    }
}

async fn bounded_tool_status(name: &str, path: Option<PathBuf>, flags: &[&str]) -> ToolStatus {
    match tokio::time::timeout(
        Duration::from_secs(10),
        agentflow_agent_adapters::tool_status(name, path.clone(), flags),
    )
    .await
    {
        Ok(status) => status,
        Err(_) => ToolStatus {
            // Reaching the timeout normally means the executable was found but its version/help or
            // authentication command hung. Keep that distinct from "未安装" so recovery is honest.
            found: true,
            path: path.map(|value| value.to_string_lossy().into_owned()),
            version: None,
            compatible: false,
            problem: Some(format!("{name} 环境检测超过 10 秒，已停止等待")),
            authenticated: None,
            auth_method: None,
            auth_problem: None,
            support_level: CliSupportLevel::Untracked,
            verified_versions: Vec::new(),
        },
    }
}

async fn bounded_api_provider_status(settings: &ApiProviderSettings) -> ProviderStatus {
    let configured = !settings.model.trim().is_empty()
        && !settings.base_url.trim().is_empty()
        && !settings.api_key_env.trim().is_empty();
    let environment_key_available = std::env::var(&settings.api_key_env)
        .is_ok_and(|value| !value.trim().is_empty());
    #[cfg(target_os = "macos")]
    let keychain_key_available = if environment_key_available {
        false
    } else {
        // Environment status must never retrieve the secret or synchronously enter
        // Security.framework. An ad-hoc/development signature can otherwise leave the settings
        // screen waiting on a Keychain authorization mutex forever. The metadata-only lookup is
        // independently bounded; actual Provider execution still resolves the secret securely.
        matches!(
            tokio::time::timeout(
                Duration::from_secs(2),
                Command::new("/usr/bin/security")
                    .args([
                        "find-generic-password",
                        "-s",
                        &settings.keychain_service,
                        "-a",
                        "AgentFlow",
                    ])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status(),
            )
            .await,
            Ok(Ok(status)) if status.success()
        )
    };
    #[cfg(not(target_os = "macos"))]
    let keychain_key_available = false;
    let key_available = environment_key_available || keychain_key_available;
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
