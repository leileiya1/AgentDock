/// An ordered provider chain partitioned by whether each provider can actually run. The `ready`
/// list drives which providers a role will attempt (in order); `entries` keeps the whole chain,
/// including the skipped providers and the reason each was skipped, so a failure card can name
/// every root cause rather than only the last provider that was tried (P0-02).
struct ChainAssessment {
    ready: Vec<AgentKind>,
    entries: Vec<ProviderReadiness>,
}

impl ChainAssessment {
    /// Human-readable "not tried" lines for the providers filtered out before any run started.
    fn skipped_reasons(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| !entry.available)
            .map(|entry| {
                format!(
                    "{}：{}（未尝试）",
                    entry.display_name,
                    entry.problem.as_deref().unwrap_or("当前不可用")
                )
            })
            .collect()
    }
}

impl Orchestrator {
    const RUNTIME_PROBE_TTL: Duration = Duration::from_secs(10 * 60);

    /// Assess an ordered provider chain against the live catalog. A provider is runnable only when
    /// its catalog descriptor reports it installed, protocol-compatible AND authenticated — the same
    /// signal the desktop shows as "已连接". Anything else is recorded with the probe's own reason.
    fn assess_chain(chain: &[AgentKind], catalog: &[ProviderDescriptor]) -> ChainAssessment {
        let mut ready = Vec::new();
        let mut entries = Vec::new();
        for kind in chain {
            let descriptor = catalog.iter().find(|item| &item.id == kind);
            let available = descriptor.is_some_and(|item| item.available);
            let display_name = descriptor
                .map(|item| item.display_name.clone())
                .unwrap_or_else(|| kind.to_string());
            let problem = match descriptor {
                Some(item) if item.available => None,
                Some(item) => Some(
                    item.problem
                        .clone()
                        .unwrap_or_else(|| "Provider 当前不可用".into()),
                ),
                None => Some("Provider 尚未纳入自动降级链".into()),
            };
            if available {
                ready.push(kind.clone());
            }
            entries.push(ProviderReadiness {
                provider: kind.clone(),
                display_name,
                available,
                problem,
            });
        }
        ChainAssessment { ready, entries }
    }

    /// Probe whether a task with this developer/reviewer selection can actually run, before any
    /// task or run is created. The desktop calls this to block "创建并立即开始" and to list, in one
    /// place, exactly which providers are unavailable and why (P0-01/P0-02).
    pub async fn provider_preflight(
        &self,
        project_id: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        allow_api_egress: bool,
        require_plan_approval: bool,
    ) -> Result<TaskPreflightReport, OrchestratorError> {
        let project = self.project(project_id).await?;
        // First run the side-effect-free catalog checks. Only providers in the actual developer or
        // reviewer fallback chains are then allowed to spend a tiny real request.
        let mut catalog = self.provider_list().await;
        let preliminary = self.build_preflight_report(
            &project.settings,
            developer.clone(),
            reviewer.clone(),
            allow_api_egress,
            require_plan_approval,
            &catalog,
        );
        let candidates = preliminary
            .roles
            .iter()
            .flat_map(|role| role.chain.iter())
            .map(|entry| entry.provider.clone())
            .collect::<Vec<_>>();
        self.apply_runtime_probes(&mut catalog, &candidates).await;
        Ok(self.build_preflight_report(
            &project.settings,
            developer,
            reviewer,
            allow_api_egress,
            require_plan_approval,
            &catalog,
        ))
    }

    async fn apply_runtime_probes(
        &self,
        catalog: &mut [ProviderDescriptor],
        candidates: &[AgentKind],
    ) {
        let mut seen = HashSet::new();
        let targets = candidates
            .iter()
            .filter(|kind| seen.insert((*kind).clone()))
            .filter(|kind| {
                catalog
                    .iter()
                    .find(|item| &item.id == *kind)
                    .is_some_and(|item| {
                        // `available` already means installed + required flags + authentication.
                        // Missing or logged-out CLIs never consume a real probe request.
                        item.available && item.source == ProviderSource::Builtin
                    })
            })
            .filter_map(|kind| {
                Self::cli_probe_target(kind)
                    .filter(|(_, name, _)| agentflow_agent_adapters::runtime_probe_supported(name))
            })
            .collect::<Vec<_>>();
        // `join_all` supports any number of independently installed CLIs. Extending the matrix and
        // adding a provider-specific probe strategy automatically joins the same concurrent batch.
        let results = join_all(targets.into_iter().map(|(kind, name, setting_key)| async move {
            let probe = self
                .cached_cli_runtime_probe(&kind, name, setting_key)
                .await;
            (kind, probe)
        }))
        .await;
        for (kind, probe) in results {
            if let Some(item) = catalog.iter_mut().find(|item| item.id == kind)
                && !probe.passed
            {
                item.available = false;
                item.problem = probe.problem;
            }
        }
    }

    fn cli_probe_target(kind: &AgentKind) -> Option<(AgentKind, &'static str, &'static str)> {
        let (name, setting_key) = match kind {
            AgentKind::ClaudeCode => ("claude", "claude_code"),
            AgentKind::Codex => ("codex", "codex"),
            AgentKind::GeminiCli => ("gemini", "gemini_cli"),
            AgentKind::QwenCode => ("qwen", "qwen_code"),
            AgentKind::GrokCli => ("grok", "grok_cli"),
            AgentKind::KimiCli => ("kimi", "kimi_cli"),
            AgentKind::MiniMaxCli => ("mmx", "minimax_cli"),
            _ => return None,
        };
        Some((kind.clone(), name, setting_key))
    }

    async fn cached_cli_runtime_probe(
        &self,
        kind: &AgentKind,
        name: &str,
        setting_key: &str,
    ) -> CliRuntimeProbe {
        let status = agentflow_agent_adapters::tool_status(
            name,
            self.cli_override(setting_key).await,
            &[],
        )
        .await;
        let Some(path) = status.path.as_deref().map(PathBuf::from) else {
            return CliRuntimeProbe {
                passed: false,
                problem: status.problem.or(status.auth_problem),
            };
        };
        let key = format!(
            "{}|{}|{}|{}",
            kind,
            path.display(),
            status.version.as_deref().unwrap_or("unknown"),
            status.auth_method.as_deref().unwrap_or("unknown")
        );
        let cached = self.runtime_probe_cache.read().ok().and_then(|cache| {
            cache
                .get(&key)
                .filter(|entry| entry.checked_at.elapsed() < Self::RUNTIME_PROBE_TTL)
                .map(|entry| entry.result.clone())
        });
        if let Some(result) = cached {
            return result;
        }
        let result = agentflow_agent_adapters::probe_cli_runtime(name, &path).await;
        if let Ok(mut cache) = self.runtime_probe_cache.write() {
            cache.retain(|_, entry| entry.checked_at.elapsed() < Self::RUNTIME_PROBE_TTL);
            cache.insert(
                key,
                CachedRuntimeProbe {
                    checked_at: Instant::now(),
                    result: result.clone(),
                },
            );
        }
        result
    }

    /// Map the developer and reviewer selections to their ordered fallback chains and assess each
    /// against an already-probed catalog. Kept separate from the live probe so the wiring can be
    /// tested deterministically without shelling out to real CLIs.
    fn build_preflight_report(
        &self,
        settings: &ProjectSettings,
        developer: AgentKind,
        reviewer: AgentKind,
        allow_api_egress: bool,
        require_plan_approval: bool,
        catalog: &[ProviderDescriptor],
    ) -> TaskPreflightReport {
        // The first developer stage is planning when a plan gate is on, otherwise development; both
        // draw from the developer fallback chain, so probing that chain covers the whole run.
        let developer_role = if require_plan_approval {
            RunRole::Planner
        } else {
            RunRole::Developer
        };
        let developer_chain =
            self.provider_chain(developer.clone(), developer_role, None, settings, allow_api_egress);
        let reviewer_chain = self.provider_chain(
            reviewer.clone(),
            RunRole::Reviewer,
            None,
            settings,
            allow_api_egress,
        );
        let developer_assessment = Self::assess_chain(&developer_chain, catalog);
        let reviewer_assessment = Self::assess_chain(&reviewer_chain, catalog);
        let roles = vec![
            RoleReadiness {
                role: PreflightRole::Developer,
                primary: developer,
                ready: !developer_assessment.ready.is_empty(),
                chain: developer_assessment.entries,
            },
            RoleReadiness {
                role: PreflightRole::Reviewer,
                primary: reviewer,
                ready: !reviewer_assessment.ready.is_empty(),
                chain: reviewer_assessment.entries,
            },
        ];
        TaskPreflightReport {
            ready: roles.iter().all(|role| role.ready),
            roles,
        }
    }
}
