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
        };
        if recover {
            orchestrator.recover_interrupted_runs().await?;
        }
        Ok(orchestrator)
    }
    async fn recover_interrupted_runs(&self) -> Result<(), OrchestratorError> {
        self.recover_start_operations().await?;
        let rows = sqlx::query(
            "SELECT id,task_id,revision,role,run_dir FROM agent_runs WHERE status='RUNNING'",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in rows {
            let run_id: String = row.get("id");
            let task_id: String = row.get("task_id");
            let revision: i64 = row.get("revision");
            let role: String = row.get("role");
            let run_dir: String = row.get("run_dir");
            let lease_path = Path::new(&run_dir).join("process-lease.json");
            let lease = agentflow_process_supervisor::read_process_lease(&lease_path)
                .await
                .ok();
            let live_pid = lease.as_ref().and_then(|lease| {
                (agentflow_process_supervisor::inspect_process_lease(lease)
                    == agentflow_process_supervisor::LeaseState::Alive)
                    .then_some(lease.pid)
            });
            let completed_cleanly = agentflow_process_supervisor::read_process_exit_code(
                &Path::new(&run_dir).join("process-outcome.json"),
            )
            .await
            .is_ok_and(|code| code == 0);
            if live_pid.is_some() || completed_cleanly {
                // A crash is different from an explicit cancellation: the Provider owns durable
                // stdout/stderr descriptors and a child-side exit marker, so the new daemon can
                // adopt it without discarding already-paid work.
                let now = Utc::now().to_rfc3339();
                sqlx::query("UPDATE agent_runs SET recovery_state='ADOPTING',adopted_at=? WHERE id=? AND status='RUNNING'")
                    .bind(&now)
                    .bind(&run_id)
                    .execute(self.store.pool())
                    .await?;
                sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'system','recovery:run_adopted',?,?)")
                    .bind(&task_id)
                    .bind(revision)
                    .bind(json!({"run_id":run_id,"pid":live_pid,"role":role,"already_exited":completed_cleanly && live_pid.is_none()}).to_string())
                    .bind(&now)
                    .execute(self.store.pool())
                    .await?;
                continue;
            }
            let process_recovery = match agentflow_process_supervisor::read_process_lease(&lease_path)
                .await
            {
                Ok(lease) => match agentflow_process_supervisor::inspect_process_lease(&lease) {
                    agentflow_process_supervisor::LeaseState::Alive => "live_process_race",
                    agentflow_process_supervisor::LeaseState::Exited => {
                        "orphan_process_already_exited"
                    }
                    agentflow_process_supervisor::LeaseState::PidReused => {
                        // Never signal a recycled PID: it may now belong to an unrelated app.
                        "pid_reused_not_signaled"
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => "lease_missing",
                Err(_) => "lease_invalid_not_signaled",
            };
            let _ = tokio::fs::remove_file(&lease_path).await;
            sqlx::query("UPDATE agent_runs SET status='INTERRUPTED',finished_at=? WHERE id=?")
                .bind(Utc::now().to_rfc3339())
                .bind(&run_id)
                .execute(self.store.pool())
                .await?;
            let current = self.task(&task_id).await?;
            if matches!(current.status, TaskStatus::Cancelled | TaskStatus::Merged | TaskStatus::RolledBack) {
                continue;
            }
            // Preserve any residual edits before rolling scheduler state back. Repair Center can
            // later keep them or reset to the recorded commit without guessing what survived.
            if current.worktree_path.as_ref().is_some_and(|path| path.is_dir()) {
                let _ = self.create_checkpoint(&current, "interrupted-run").await;
                let worktree = required_path(&current.worktree_path)?;
                let reset_to = match role.as_str() {
                    "developer" => sqlx::query_scalar::<_, Option<String>>(
                        "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision<? ORDER BY revision DESC LIMIT 1",
                    )
                    .bind(&task_id)
                    .bind(revision)
                    .fetch_optional(self.store.pool())
                    .await?
                    .flatten()
                    .or_else(|| current.base_commit.clone()),
                    "reviewer" => Some(self.revision_commit_sha(&task_id, current.revision).await?),
                    _ => self.git.resolve(&worktree, "HEAD").await.ok(),
                };
                if let Some(commit) = reset_to {
                    self.git.reset_owned_worktree(&worktree, &commit).await?;
                }
            }
            let (to, new_revision) = match role.as_str() {
                "planner" => (TaskStatus::Planning, current.revision),
                "developer" => (
                    if revision <= 1 {
                        TaskStatus::ReadyForDevelopment
                    } else {
                        TaskStatus::ReadyForRevision
                    },
                    revision.saturating_sub(1),
                ),
                "validator" => (TaskStatus::Validating, current.revision),
                _ => (TaskStatus::ReadyForReview, current.revision),
            };
            sqlx::query("UPDATE tasks SET current_revision=? WHERE id=?")
                .bind(new_revision)
                .bind(&task_id)
                .execute(self.store.pool())
                .await?;
            self.store
                .transition(
                    &task_id,
                    &[current.status],
                    to,
                    None,
                    Actor::System,
                    "recovery:interrupted",
                    &json!({"run_id":run_id,"process_recovery":process_recovery}),
                )
                .await?;
        }
        // Covers the crash window after a durable state transition but before an agent_runs row
        // or final transition was written. Run this before the missing-worktree check because a
        // completed merge may have intentionally removed its worktree just before a crash.
        self.recover_orphaned_stages().await?;
        self.recover_provider_dispatches().await?;
        let active=sqlx::query("SELECT id,status,worktree_path FROM tasks WHERE status NOT IN ('DRAFT','MERGED','ROLLED_BACK','CANCELLED')").fetch_all(self.store.pool()).await?;
        for row in active {
            let path: Option<String> = row.get("worktree_path");
            if path.as_deref().is_none_or(|p| !Path::new(p).exists()) {
                let id: String = row.get("id");
                let status: TaskStatus = parse(row.get("status"))?;
                sqlx::query("UPDATE tasks SET repair_resume_status=? WHERE id=?")
                    .bind(status.to_string())
                    .bind(&id)
                    .execute(self.store.pool())
                    .await?;
                self.store
                    .transition(
                        &id,
                        &[status],
                        TaskStatus::Blocked,
                        Some(BlockedReason::WorktreeMissing),
                        Actor::System,
                        "recovery:worktree_missing",
                        &json!({}),
                    )
                    .await?;
            }
        }
        Ok(())
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
        let git =
            agentflow_agent_adapters::tool_status("git", self.cli_override("git").await, &[]).await;
        let claude_code = agentflow_agent_adapters::tool_status(
            "claude",
            self.cli_override("claude_code").await,
            &["--output-format", "--permission-mode"],
        )
        .await;
        let codex = agentflow_agent_adapters::tool_status(
            "codex",
            self.cli_override("codex").await,
            &["--json", "--sandbox"],
        )
        .await;
        let gemini_cli = agentflow_agent_adapters::tool_status(
            "gemini",
            self.cli_override("gemini_cli").await,
            &["--output-format", "--approval-mode", "--sandbox"],
        )
        .await;
        let qwen_code = agentflow_agent_adapters::tool_status(
            "qwen",
            self.cli_override("qwen_code").await,
            &[
                "--output-format",
                "--approval-mode",
                "--sandbox",
                "--max-wall-time",
            ],
        )
        .await;
        let grok_cli = agentflow_agent_adapters::tool_status(
            "grok",
            self.cli_override("grok_cli").await,
            &["--output-format", "--sandbox", "--permission-mode"],
        )
        .await;
        let kimi_cli = agentflow_agent_adapters::tool_status(
            "kimi",
            self.cli_override("kimi_cli").await,
            &["--prompt", "--output-format"],
        )
        .await;
        let minimax_cli = agentflow_agent_adapters::tool_status(
            "mmx",
            self.cli_override("minimax_cli").await,
            &[],
        )
        .await;
        EnvReport {
            git,
            claude_code,
            codex,
            gemini_cli,
            qwen_code,
            grok_cli,
            kimi_cli,
            minimax_cli,
            openai_api: api_provider_status(&ApiProviderSettings::openai_default()),
            anthropic_api: api_provider_status(&ApiProviderSettings::anthropic_default()),
            deepseek_api: api_provider_status(&ApiProviderSettings::deepseek_default()),
            grok_api: api_provider_status(&ApiProviderSettings::grok_default()),
            minimax_api: api_provider_status(&ApiProviderSettings::minimax_default()),
            kimi_api: api_provider_status(&ApiProviderSettings::kimi_default()),
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
        ]
        .into_iter()
        .filter_map(|(kind, status)| {
            (status.found && status.compatible && status.authenticated != Some(false))
                .then_some(kind)
        })
        .collect::<Vec<_>>();
        let recommended_developer = [
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            AgentKind::GeminiCli,
            AgentKind::QwenCode,
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
            "grok_cli" => (
                "grok",
                &["--output-format", "--sandbox", "--permission-mode"],
            ),
            "kimi_cli" => ("kimi", &["--prompt", "--output-format"]),
            "minimax_cli" => ("mmx", &[]),
            _ => ("git", &[]),
        };
        let status =
            agentflow_agent_adapters::tool_status(name, Some(path.to_path_buf()), flags).await;
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
