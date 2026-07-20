impl Orchestrator {
    async fn review_single(&self, task: TaskRow) -> Result<(), OrchestratorError> {
        self.store
            .transition(
                &task.id,
                &[TaskStatus::ReadyForReview],
                TaskStatus::Reviewing,
                None,
                Actor::Orchestrator,
                "scheduler:slot",
                &json!({}),
            )
            .await?;
        let project = self.project(&task.project_id).await?;
        let wt = required_path(&task.worktree_path)?;
        reset_input_dir(&wt).await?;
        let config = load_config(&project.repo).await?;
        let sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_one(self.store.pool())
        .await?;
        let input = self
            .build_review_input(&task, &project, &sha, &config)
            .await?;
        tokio::fs::write(wt.join(".agentflow-in/review-input.md"), input).await?;
        let actual_developer = sqlx::query_scalar::<_, String>(
            "SELECT agent FROM agent_runs WHERE task_id=? AND revision=? AND role='developer' AND status='SUCCEEDED' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_optional(self.store.pool())
        .await?
        .and_then(|value| value.parse::<AgentKind>().ok())
        .unwrap_or_else(|| task.developer.clone());
        let chain = self.provider_chain(
            task.reviewer.clone(),
            RunRole::Reviewer,
            Some(actual_developer),
            &project.settings,
            task.api_egress_approved,
        );
        let mut accepted = None;
        let mut previous = None;
        let mut previous_error = String::new();
        for candidate in chain {
            if let Some(from) = previous.clone() {
                self.git.reset_owned_worktree(&wt, &sha).await?;
                reset_input_dir(&wt).await?;
                let input = self
                    .build_review_input(&task, &project, &sha, &config)
                    .await?;
                tokio::fs::write(wt.join(".agentflow-in/review-input.md"), input).await?;
                self.record_provider_fallback(
                    &task,
                    RunRole::Reviewer,
                    from,
                    candidate.clone(),
                    &previous_error,
                )
                .await?;
            }
            let run_dir = self.run_dir(&task.id);
            let adapter = self.adapter(candidate.clone(), &project);
            if self.task(&task.id).await?.status == TaskStatus::Cancelled {
                return Ok(());
            }
            let running = match self
                .run_agent(
                    adapter.as_ref(),
                    &task,
                    &project,
                    &run_dir,
                    RunRole::Reviewer,
                    ".agentflow-in/review-input.md",
                    &config,
                    Some(PermissionTier::ReadOnly),
                )
                .await
            {
                Ok(running) if running.outcome.cancelled => return Ok(()),
                Ok(running) if running.outcome.exit_code == Some(0) => running,
                Ok(running) => {
                    previous = Some(candidate);
                    previous_error =
                        format!("provider exited with {:?}", running.outcome.exit_code);
                    if self.enforce_budget(&self.task(&task.id).await?).await? {
                        return Ok(());
                    }
                    continue;
                }
                Err(error) => {
                    if self.task(&task.id).await?.status == TaskStatus::Cancelled {
                        return Ok(());
                    }
                    previous = Some(candidate);
                    previous_error = error.to_string();
                    continue;
                }
            };
            if self.enforce_budget(&self.task(&task.id).await?).await? {
                return Ok(());
            }
            match adapter
                .collect_result(&running.run_dir, RunRole::Reviewer)
                .await
            {
                Ok(CollectedResult::Review(review)) => {
                    let commit_matches = is_hex_commit_reference(&review.commit_sha)
                        && self
                            .git
                            .resolve(&wt, &review.commit_sha)
                            .await
                            .is_ok_and(|resolved| resolved == sha);
                    if commit_matches
                        && review.task_id == task.id
                        && review.revision == task.revision
                    {
                        accepted = Some((review, running.run_dir, candidate.clone()));
                        break;
                    }
                    previous_error = "review output did not match the active revision".into();
                }
                Ok(CollectedResult::Development(_) | CollectedResult::Plan(_)) => {
                    previous_error = "review provider returned a development result".into();
                }
                Err(error) => previous_error = error.to_string(),
            }
            self.invalidate_agent_run(&running.run_dir).await?;
            let rejection = previous_error.clone();
            if let Some(repaired) = self
                .attempt_result_repair(
                    adapter.as_ref(),
                    &task,
                    &project,
                    RunRole::Reviewer,
                    &config,
                    &rejection,
                )
                .await?
            {
                match repaired.value {
                    CollectedResult::Review(review) => {
                        let commit_matches = is_hex_commit_reference(&review.commit_sha)
                            && self
                                .git
                                .resolve(&wt, &review.commit_sha)
                                .await
                                .is_ok_and(|resolved| resolved == sha);
                        if commit_matches
                            && review.task_id == task.id
                            && review.revision == task.revision
                        {
                            accepted = Some((review, repaired.run_dir, candidate.clone()));
                            break;
                        }
                        self.invalidate_agent_run(&repaired.run_dir).await?;
                        previous_error = "repaired review still mismatched".into();
                    }
                    CollectedResult::Development(_) | CollectedResult::Plan(_) => {
                        self.invalidate_agent_run(&repaired.run_dir).await?;
                        previous_error = "repair returned a development result".into();
                    }
                }
            }
            previous = Some(candidate);
        }
        let Some((review, run_dir, reviewer_agent)) = accepted else {
            self.block_review(
                &task,
                &format!("all reviewer providers failed: {previous_error}"),
            )
            .await?;
            return Ok(());
        };
        let review_id = Uuid::now_v7().to_string();
        let run_id = run_id_from_dir(&run_dir)?;
        sqlx::query("INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,summary,raw_path,reviewer_agent,is_aggregate,reviewer_agents_json,created_at) VALUES(?,?,?,?,?,?,?,?,?,1,?,?)")
            .bind(&review_id).bind(&task.id).bind(task.revision).bind(run_id).bind(&sha)
            .bind(review.decision.to_string()).bind(&review.summary)
            .bind(run_dir.join("last-message.json").to_string_lossy().as_ref())
            .bind(reviewer_agent.to_string())
            .bind(serde_json::to_string(&vec![reviewer_agent.clone()]).map_err(|error| OrchestratorError::Config(error.to_string()))?)
            .bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
        let current_issue_keys = review
            .issues
            .iter()
            .map(review_issue_key)
            .collect::<HashSet<_>>();
        for issue in &review.issues {
            sqlx::query("INSERT INTO review_issues(id,review_id,severity,file,line_start,line_end,title,description,suggested_action,reported_by_json,agreement_count) VALUES(?,?,?,?,?,?,?,?,?,?,1)").bind(Uuid::now_v7().to_string()).bind(&review_id).bind(issue.severity.to_string()).bind(&issue.file).bind(issue.line_start).bind(issue.line_end).bind(&issue.title).bind(&issue.description).bind(&issue.suggested_action).bind(serde_json::to_string(&vec![reviewer_agent.clone()]).map_err(|error| OrchestratorError::Config(error.to_string()))?).execute(self.store.pool()).await?;
        }
        self.reconcile_review_issues(&task, &current_issue_keys).await?;
        let quality = self
            .evaluate_quality(&task, &self.stored_test_report(&task.id, task.revision).await?, false)
            .await?;
        let (to, reason, event) = match review.decision {
            ReviewDecision::Pass if !quality.passed => (
                TaskStatus::Blocked,
                Some(BlockedReason::QualityGate),
                "quality:gate_failed",
            ),
            ReviewDecision::Pass => (TaskStatus::WaitingForHumanApproval, None, "review:pass"),
            ReviewDecision::RequestChanges if task.revision >= task.max_revisions => (
                TaskStatus::Blocked,
                Some(BlockedReason::MaxRevisions),
                "review:max_revisions",
            ),
            ReviewDecision::RequestChanges => {
                (TaskStatus::ReadyForRevision, None, "review:request_changes")
            }
            ReviewDecision::Block => (
                TaskStatus::Blocked,
                Some(BlockedReason::ReviewBlock),
                "review:block",
            ),
        };
        self.store
            .transition(
                &task.id,
                &[TaskStatus::Reviewing],
                to,
                reason,
                Actor::Orchestrator,
                event,
                &json!({"review_id":review_id,"quality_score":quality.score,"quality_passed":quality.passed}),
            )
            .await?;
        Ok(())
    }
    fn adapter(&self, kind: AgentKind, project: &ProjectRow) -> Box<dyn AgentAdapter> {
        let external = self
            .provider_registry
            .read()
            .ok()
            .and_then(|registry| registry.get(&kind).cloned());
        if let Some(provider) = external {
            return Box::new(ExternalProviderAdapter::new(provider));
        }
        match kind {
            AgentKind::ClaudeCode => Box::new(ClaudeCodeAdapter::new(
                project.settings.claude_path.as_deref().unwrap_or("claude"),
            )),
            AgentKind::Codex => Box::new(CodexAdapter::new(
                project.settings.codex_path.as_deref().unwrap_or("codex"),
                self.app_data.join("schemas/review.schema.json"),
            )),
            AgentKind::GeminiCli => Box::new(GeminiCliAdapter::new(
                project.settings.gemini_path.as_deref().unwrap_or("gemini"),
            )),
            AgentKind::QwenCode => Box::new(QwenCodeAdapter::new(
                project.settings.qwen_path.as_deref().unwrap_or("qwen"),
                self.app_data.join("schemas/review.schema.json"),
            )),
            AgentKind::OpenAiApi => Box::new(ApiProviderAdapter::new(
                AgentKind::OpenAiApi,
                project.settings.openai.clone(),
            )),
            AgentKind::AnthropicApi => Box::new(ApiProviderAdapter::new(
                AgentKind::AnthropicApi,
                project.settings.anthropic.clone(),
            )),
            AgentKind::DeepSeekApi => Box::new(ApiProviderAdapter::new(
                AgentKind::DeepSeekApi,
                project.settings.deepseek.clone(),
            )),
            AgentKind::GrokApi => Box::new(ApiProviderAdapter::new(
                AgentKind::GrokApi,
                project.settings.grok.clone(),
            )),
            AgentKind::MiniMaxApi => Box::new(ApiProviderAdapter::new(
                AgentKind::MiniMaxApi,
                project.settings.minimax.clone(),
            )),
            AgentKind::KimiApi => Box::new(ApiProviderAdapter::new(
                AgentKind::KimiApi,
                project.settings.kimi.clone(),
            )),
            AgentKind::GrokCli | AgentKind::KimiCli | AgentKind::MiniMaxCli => {
                Box::new(UnavailableProviderAdapter::new(kind))
            }
            AgentKind::External(id) => Box::new(UnavailableProviderAdapter::new(
                AgentKind::External(id),
            )),
        }
    }

    fn provider_chain(
        &self,
        primary: AgentKind,
        role: RunRole,
        excluded: Option<AgentKind>,
        settings: &ProjectSettings,
        allow_api_egress: bool,
    ) -> Vec<AgentKind> {
        let configured = if matches!(role, RunRole::Planner | RunRole::Developer) {
            &settings.developer_fallbacks
        } else {
            &settings.reviewer_fallbacks
        };
        let mut candidates = vec![primary.clone()];
        if role == RunRole::Reviewer
            && primary.is_api()
            && let Some(legacy) = settings.api_fallback_provider.clone()
        {
            candidates.push(legacy);
        }
        candidates.extend(configured.iter().cloned());
        let mut seen = HashSet::new();
        candidates
            .into_iter()
            .filter(|kind| excluded.as_ref() != Some(kind))
            .filter(|kind| !matches!(role, RunRole::Planner | RunRole::Developer) || !kind.is_api())
            .filter(|kind| role != RunRole::Planner || !matches!(kind, AgentKind::External(_)))
            .filter(|kind| allow_api_egress || !self.provider_requires_egress(kind))
            .filter(|kind| seen.insert(kind.clone()))
            .collect()
    }

    fn provider_requires_egress(&self, kind: &AgentKind) -> bool {
        if kind.is_api() {
            return true;
        }
        self.provider_registry
            .read()
            .ok()
            .and_then(|registry| registry.get(kind).cloned())
            .is_some_and(|provider| {
                provider.manifest.execution_location != ExecutionLocation::Local
                    || provider.manifest.data_egress != DataEgress::None
                    || !provider.manifest.permissions.network_domains.is_empty()
            })
    }

    async fn reconcile_review_issues(
        &self,
        task: &TaskRow,
        current_issue_keys: &HashSet<String>,
    ) -> Result<(), OrchestratorError> {
        let prior = sqlx::query(
            "SELECT ri.id,ri.file,ri.title FROM review_issues ri \
             JOIN reviews r ON r.id=ri.review_id \
             WHERE r.task_id=? AND r.revision<? AND ri.resolved=0",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_all(self.store.pool())
        .await?;
        let now = Utc::now().to_rfc3339();
        let mut resolved = 0usize;
        for row in prior {
            let key = review_issue_key_parts(
                row.get::<Option<String>, _>("file").as_deref(),
                &row.get::<String, _>("title"),
            );
            if current_issue_keys.contains(&key) {
                continue;
            }
            let issue_id: String = row.get("id");
            sqlx::query("UPDATE review_issues SET resolved=1,resolved_at=?,resolved_by_revision=? WHERE id=? AND resolved=0")
                .bind(&now)
                .bind(task.revision)
                .bind(issue_id)
                .execute(self.store.pool())
                .await?;
            resolved += 1;
        }
        if resolved > 0 {
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','review:issues_resolved',?,?)")
                .bind(&task.id)
                .bind(task.revision)
                .bind(json!({"count": resolved}).to_string())
                .bind(now)
                .execute(self.store.pool())
                .await?;
        }
        Ok(())
    }

    async fn record_provider_fallback(
        &self,
        task: &TaskRow,
        role: RunRole,
        from: AgentKind,
        to: AgentKind,
        reason: &str,
    ) -> Result<(), OrchestratorError> {
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','provider:fallback',?,?)")
            .bind(&task.id)
            .bind(task.revision)
            .bind(json!({
                "role": role,
                "from": from,
                "to": to,
                "reason": reason
            }).to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        let settings = self.settings_get().await?;
        if settings.notifications.enabled && settings.notifications.on_fallback {
            self.send_notification(
                "AgentFlow Provider 降级",
                &format!("{}：{} 从 {} 切换到 {}", task.title, role, from, to),
            )
            .await;
        }
        Ok(())
    }
    async fn build_input(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        history_digest: Option<&str>,
    ) -> Result<String, OrchestratorError> {
        let rules = load_rules(&project.repo).await?;
        let schema = serde_json::to_string_pretty(&development_result_schema())
            .map_err(|e| OrchestratorError::Config(e.to_string()))?;
        // Inline the bounded snapshot so a missing sidecar file can never erase cross-round state.
        let history = history_digest
            .map(|digest| {
                format!(
                    "## 跨轮记忆（权威快照）\n\n以下内容由 AgentFlow 从提交、验证、审查和人工事件生成；以当前 worktree 为准。\n\n{digest}"
                )
            })
            .unwrap_or_default();
        Ok(include_str!("../templates/input.md")
            .replace("{{TASK_SEQ}}", &task.seq.to_string())
            .replace("{{TASK_ID}}", &task.id)
            .replace("{{REVISION}}", &task.revision.to_string())
            .replace("{{TITLE}}", &task.title)
            .replace("{{DESCRIPTION}}", &task.description)
            .replace("{{GUIDANCE}}", task.blocked_detail.as_deref().unwrap_or(""))
            .replace("{{RULES}}", &rules)
            .replace("{{HISTORY}}", &history)
            .replace("{{RESULT_SCHEMA}}", &schema))
    }
    async fn build_review_input(
        &self,
        task: &TaskRow,
        _project: &ProjectRow,
        sha: &str,
        config: &ProjectConfig,
    ) -> Result<String, OrchestratorError> {
        let wt = required_path(&task.worktree_path)?;
        let base = task
            .base_commit
            .as_deref()
            .ok_or_else(|| OrchestratorError::InvalidState("base commit missing".into()))?;
        let diff = self
            .git
            .diff(
                &wt,
                base,
                sha,
                &config.review.exclude_globs,
                config.review.max_patch_bytes,
            )
            .await?;
        let stat = summarize(&diff);
        let flagged = if stat.flagged.is_empty() {
            String::new()
        } else {
            format!(
                "警告：以下控制面文件必须重点审查：{}",
                stat.flagged.join(", ")
            )
        };
        let diff_text = if diff.truncated {
            "补丁超过大小上限，请在 worktree 内使用 git show 审查。".into()
        } else {
            diff.files
                .iter()
                .filter_map(|f| f.patch.clone())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let report = tokio::fs::read_to_string(
            self.task_dir(&task.id)
                .join("artifacts")
                .join(format!("r{}-tests.json", task.revision)),
        )
        .await
        .unwrap_or_else(|_| "{}".into());
        let schema = serde_json::to_string_pretty(&review_result_schema())
            .map_err(|e| OrchestratorError::Config(e.to_string()))?;
        Ok(include_str!("../templates/review-input.md")
            .replace("{{TITLE}}", &task.title)
            .replace("{{TASK_ID}}", &task.id)
            .replace("{{REVISION}}", &task.revision.to_string())
            .replace("{{DESCRIPTION}}", &task.description)
            .replace("{{GUIDANCE}}", task.blocked_detail.as_deref().unwrap_or(""))
            .replace("{{COMMIT_SHA}}", sha)
            .replace("{{BASE_COMMIT}}", base)
            .replace(
                "{{DIFF_STAT}}",
                &format!(
                    "{} files, +{}, -{}",
                    stat.files, stat.insertions, stat.deletions
                ),
            )
            .replace("{{FLAGGED_WARNING}}", &flagged)
            .replace("{{DIFF}}", &diff_text)
            .replace("{{TEST_REPORT}}", &report)
            .replace("{{REVIEW_SCHEMA}}", &schema))
    }
    async fn block(
        &self,
        task: &TaskRow,
        reason: BlockedReason,
        detail: &str,
    ) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE tasks SET blocked_detail=? WHERE id=?")
            .bind(detail)
            .bind(&task.id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                &task.id,
                &[
                    TaskStatus::Developing,
                    TaskStatus::Revising,
                    TaskStatus::Validating,
                ],
                TaskStatus::Blocked,
                Some(reason),
                Actor::Orchestrator,
                "task:blocked",
                &json!({"detail":detail}),
            )
            .await?;
        Ok(())
    }
    async fn block_review(&self, task: &TaskRow, detail: &str) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE tasks SET blocked_detail=? WHERE id=?")
            .bind(detail)
            .bind(&task.id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                &task.id,
                &[TaskStatus::Reviewing],
                TaskStatus::Blocked,
                Some(BlockedReason::ReviewFailed),
                Actor::Orchestrator,
                "review:failed",
                &json!({"detail":detail}),
            )
            .await?;
        Ok(())
    }
}
