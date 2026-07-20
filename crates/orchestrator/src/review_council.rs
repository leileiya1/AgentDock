#[derive(Debug)]
struct CouncilMemberReview {
    agent: AgentKind,
    review: ReviewResult,
    run_dir: PathBuf,
}

#[derive(Debug)]
struct CouncilIssue {
    issue: ReviewIssueResult,
    reported_by: Vec<AgentKind>,
}

#[derive(Debug)]
struct CouncilAggregate {
    decision: ReviewDecision,
    summary: String,
    issues: Vec<CouncilIssue>,
}

impl Orchestrator {
    async fn review(&self, task: TaskRow) -> Result<(), OrchestratorError> {
        let settings = self.project(&task.project_id).await?.settings;
        let security_required = self
            .stored_integrity_report(&task)
            .await?
            .is_some_and(|report| report.requires_security_review);
        if security_required && !settings.review_council.enabled {
            let detail = "本轮修改涉及测试、CI、权限或安全控制面，必须启用至少两个独立 Provider 的审查委员会后再继续";
            sqlx::query("UPDATE tasks SET blocked_detail=? WHERE id=?")
                .bind(detail)
                .bind(&task.id)
                .execute(self.store.pool())
                .await?;
            self.store
                .transition(
                    &task.id,
                    &[TaskStatus::ReadyForReview],
                    TaskStatus::Blocked,
                    Some(BlockedReason::QualityGate),
                    Actor::Orchestrator,
                    "integrity:security_review_required",
                    &json!({"detail":detail}),
                )
                .await?;
            return Ok(());
        }
        if !settings.review_council.enabled {
            return self.review_single(task).await;
        }
        self.review_with_council(task).await
    }

    async fn review_with_council(&self, task: TaskRow) -> Result<(), OrchestratorError> {
        self.store
            .transition(
                &task.id,
                &[TaskStatus::ReadyForReview],
                TaskStatus::Reviewing,
                None,
                Actor::Orchestrator,
                "scheduler:council_slot",
                &json!({}),
            )
            .await?;
        let project = self.project(&task.project_id).await?;
        let config = self.load_trusted_config(&project).await?;
        let sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_one(self.store.pool())
        .await?;
        let actual_developer = self.actual_developer(&task).await?;
        let targets = self.council_targets(&task, &project.settings, &actual_developer);
        let minimum = usize::from(
            project
                .settings
                .review_council
                .minimum_successful_reviews
                .clamp(2, 3),
        );
        if targets.len() < minimum {
            self.block_review(
                &task,
                "审查委员会至少需要两个与开发者独立、且已获外发同意的 Provider",
            )
            .await?;
            return Ok(());
        }

        let mut members = Vec::new();
        let mut failures = Vec::new();
        for target in targets {
            self.prepare_council_input(&task, &project, &config, &sha)
                .await?;
            match self
                .collect_council_member(&task, &project, &config, &sha, target.clone())
                .await
            {
                Ok(member) => members.push(member),
                Err(error) => {
                    failures.push(format!("{target}: {error}"));
                    sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','review:council_member_failed',?,?)")
                        .bind(&task.id)
                        .bind(task.revision)
                        .bind(json!({"agent": target, "error": error.to_string()}).to_string())
                        .bind(Utc::now().to_rfc3339())
                        .execute(self.store.pool())
                        .await?;
                }
            }
            if self.enforce_budget(&self.task(&task.id).await?).await? {
                return Ok(());
            }
            if self.task(&task.id).await?.status == TaskStatus::Cancelled {
                return Ok(());
            }
        }
        if members.len() < minimum {
            self.block_review(
                &task,
                &format!(
                    "审查委员会只有 {}/{} 个成员成功：{}",
                    members.len(),
                    minimum,
                    failures.join("；")
                ),
            )
            .await?;
            return Ok(());
        }

        let aggregate = aggregate_council(
            &members,
            project.settings.review_council.require_unanimous_pass,
        );
        let mut member_ids = Vec::new();
        for member in &members {
            member_ids.push(self.persist_member_review(&task, &sha, member).await?);
        }
        let aggregate_id = self
            .persist_aggregate_review(&task, &sha, &members, &member_ids, &aggregate)
            .await?;
        let current_issue_keys = aggregate
            .issues
            .iter()
            .map(|item| review_issue_key(&item.issue))
            .collect::<HashSet<_>>();
        self.reconcile_review_issues(&task, &current_issue_keys)
            .await?;
        self.finish_review_decision(&task, aggregate.decision, &aggregate_id)
            .await
    }

    async fn actual_developer(&self, task: &TaskRow) -> Result<AgentKind, OrchestratorError> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT agent FROM agent_runs WHERE task_id=? AND revision=? AND role='developer' AND status='SUCCEEDED' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_optional(self.store.pool())
        .await?
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| task.developer.clone()))
    }

    fn council_targets(
        &self,
        task: &TaskRow,
        settings: &ProjectSettings,
        developer: &AgentKind,
    ) -> Vec<AgentKind> {
        let mut candidates = vec![task.reviewer.clone()];
        candidates.extend(settings.review_council.reviewers.iter().cloned());
        let developer_family = provider_family(developer);
        let mut families = HashSet::new();
        candidates
            .into_iter()
            .filter(|agent| provider_family(agent) != developer_family)
            .filter(|agent| task.api_egress_approved || !self.provider_requires_egress(agent))
            .filter(|agent| families.insert(provider_family(agent)))
            .take(3)
            .collect()
    }

    async fn prepare_council_input(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        config: &ProjectConfig,
        sha: &str,
    ) -> Result<(), OrchestratorError> {
        let worktree = required_path(&task.worktree_path)?;
        self.git.reset_owned_worktree(&worktree, sha).await?;
        reset_input_dir(&worktree).await?;
        let input = self.build_review_input(task, project, sha, config).await?;
        tokio::fs::write(worktree.join(".agentflow-in/review-input.md"), input).await?;
        Ok(())
    }

    async fn collect_council_member(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        config: &ProjectConfig,
        sha: &str,
        agent: AgentKind,
    ) -> Result<CouncilMemberReview, OrchestratorError> {
        let adapter = self.adapter(agent.clone(), project);
        if !adapter.capabilities().supports_review {
            return Err(OrchestratorError::InvalidState(format!(
                "{agent} does not support review"
            )));
        }
        let running = self
            .run_agent(
                adapter.as_ref(),
                task,
                project,
                &self.run_dir(&task.id),
                RunRole::Reviewer,
                ".agentflow-in/review-input.md",
                config,
                Some(PermissionTier::ReadOnly),
            )
            .await?;
        if running.outcome.cancelled {
            return Err(OrchestratorError::InvalidState("review cancelled".into()));
        }
        if running.outcome.exit_code != Some(0) {
            return Err(OrchestratorError::InvalidState(format!(
                "reviewer exited with {:?}",
                running.outcome.exit_code
            )));
        }
        let collected = adapter.collect_result(&running.run_dir, RunRole::Reviewer).await;
        self.protect_run_files(&running.run_dir).await?;
        if let Ok(CollectedResult::Review(review)) = collected
            && self.valid_council_review(task, sha, &review).await
        {
            return Ok(CouncilMemberReview {
                agent,
                review,
                run_dir: running.run_dir,
            });
        }
        self.invalidate_agent_run(&running.run_dir).await?;
        let repaired = self
            .attempt_result_repair(
                adapter.as_ref(),
                task,
                project,
                RunRole::Reviewer,
                config,
                "committee member returned invalid structured review",
            )
            .await?;
        if let Some(repaired) = repaired
            && let CollectedResult::Review(review) = repaired.value
            && self.valid_council_review(task, sha, &review).await
        {
            return Ok(CouncilMemberReview {
                agent,
                review,
                run_dir: repaired.run_dir,
            });
        }
        Err(OrchestratorError::InvalidState(
            "committee member returned an invalid review".into(),
        ))
    }

    async fn valid_council_review(
        &self,
        task: &TaskRow,
        sha: &str,
        review: &ReviewResult,
    ) -> bool {
        let Ok(worktree) = required_path(&task.worktree_path) else {
            return false;
        };
        review.task_id == task.id
            && review.revision == task.revision
            && is_hex_commit_reference(&review.commit_sha)
            && self
                .git
                .resolve(&worktree, &review.commit_sha)
                .await
                .is_ok_and(|resolved| resolved == sha)
    }

    async fn persist_member_review(
        &self,
        task: &TaskRow,
        sha: &str,
        member: &CouncilMemberReview,
    ) -> Result<String, OrchestratorError> {
        let id = Uuid::now_v7().to_string();
        let run_id = run_id_from_dir(&member.run_dir)?;
        sqlx::query("INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,summary,raw_path,reviewer_agent,is_aggregate,reviewer_agents_json,created_at) VALUES(?,?,?,?,?,?,?,?,?,0,?,?)")
            .bind(&id)
            .bind(&task.id)
            .bind(task.revision)
            .bind(run_id)
            .bind(sha)
            .bind(member.review.decision.to_string())
            .bind(&member.review.summary)
            .bind(member.run_dir.join("last-message.json").to_string_lossy().as_ref())
            .bind(member.agent.to_string())
            .bind(serde_json::to_string(&vec![member.agent.clone()]).map_err(|error| OrchestratorError::Config(error.to_string()))?)
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        Ok(id)
    }

    async fn persist_aggregate_review(
        &self,
        task: &TaskRow,
        sha: &str,
        members: &[CouncilMemberReview],
        member_ids: &[String],
        aggregate: &CouncilAggregate,
    ) -> Result<String, OrchestratorError> {
        let id = Uuid::now_v7().to_string();
        let path = self
            .task_dir(&task.id)
            .join("artifacts")
            .join(format!("r{}-council.json", task.revision));
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let agents = members.iter().map(|member| member.agent.clone()).collect::<Vec<_>>();
        tokio::fs::write(&path, serde_json::to_vec_pretty(&json!({
            "decision": aggregate.decision,
            "summary": aggregate.summary,
            "members": members.iter().map(|member| json!({"agent": member.agent, "decision": member.review.decision, "summary": member.review.summary})).collect::<Vec<_>>()
        })).map_err(|error| OrchestratorError::Config(error.to_string()))?).await?;
        sqlx::query("INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,summary,raw_path,is_aggregate,member_review_ids_json,reviewer_agents_json,created_at) VALUES(?,?,?,?,?,?,?,?,1,?,?,?)")
            .bind(&id)
            .bind(&task.id)
            .bind(task.revision)
            .bind(run_id_from_dir(&members[0].run_dir)?)
            .bind(sha)
            .bind(aggregate.decision.to_string())
            .bind(&aggregate.summary)
            .bind(path.to_string_lossy().as_ref())
            .bind(serde_json::to_string(member_ids).map_err(|error| OrchestratorError::Config(error.to_string()))?)
            .bind(serde_json::to_string(&agents).map_err(|error| OrchestratorError::Config(error.to_string()))?)
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        for item in &aggregate.issues {
            sqlx::query("INSERT INTO review_issues(id,review_id,severity,file,line_start,line_end,title,description,suggested_action,reported_by_json,agreement_count) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
                .bind(Uuid::now_v7().to_string())
                .bind(&id)
                .bind(item.issue.severity.to_string())
                .bind(&item.issue.file)
                .bind(item.issue.line_start)
                .bind(item.issue.line_end)
                .bind(&item.issue.title)
                .bind(&item.issue.description)
                .bind(&item.issue.suggested_action)
                .bind(serde_json::to_string(&item.reported_by).map_err(|error| OrchestratorError::Config(error.to_string()))?)
                .bind(item.reported_by.len() as i64)
                .execute(self.store.pool())
                .await?;
        }
        Ok(id)
    }

    async fn finish_review_decision(
        &self,
        task: &TaskRow,
        decision: ReviewDecision,
        review_id: &str,
    ) -> Result<(), OrchestratorError> {
        let quality = self
            .evaluate_quality(task, &self.stored_test_report(&task.id, task.revision).await?, false)
            .await?;
        let (to, reason, event) = match decision {
            ReviewDecision::Pass if !quality.passed => (TaskStatus::Blocked, Some(BlockedReason::QualityGate), "quality:gate_failed"),
            ReviewDecision::Pass => (TaskStatus::WaitingForHumanApproval, None, "review:council_pass"),
            ReviewDecision::RequestChanges if task.revision >= task.max_revisions => (TaskStatus::Blocked, Some(BlockedReason::MaxRevisions), "review:max_revisions"),
            ReviewDecision::RequestChanges => (TaskStatus::ReadyForRevision, None, "review:council_request_changes"),
            ReviewDecision::Block => (TaskStatus::Blocked, Some(BlockedReason::ReviewBlock), "review:council_block"),
        };
        self.store.transition(&task.id, &[TaskStatus::Reviewing], to, reason, Actor::Orchestrator, event, &json!({"review_id": review_id,"quality_score":quality.score,"quality_passed":quality.passed})).await?;
        Ok(())
    }
}

fn aggregate_council(members: &[CouncilMemberReview], require_unanimous: bool) -> CouncilAggregate {
    let mut issues = std::collections::BTreeMap::<String, CouncilIssue>::new();
    for member in members {
        for issue in &member.review.issues {
            let key = review_issue_key(issue);
            let entry = issues.entry(key).or_insert_with(|| CouncilIssue {
                issue: issue.clone(),
                reported_by: Vec::new(),
            });
            if severity_rank(issue.severity) > severity_rank(entry.issue.severity) {
                entry.issue = issue.clone();
            }
            if !entry.reported_by.contains(&member.agent) {
                entry.reported_by.push(member.agent.clone());
            }
        }
    }
    let block = members.iter().any(|member| member.review.decision == ReviewDecision::Block);
    let changes = members.iter().filter(|member| member.review.decision == ReviewDecision::RequestChanges).count();
    let serious = issues.values().any(|item| matches!(item.issue.severity, Severity::Critical | Severity::High));
    let decision = if block {
        ReviewDecision::Block
    } else if serious || (require_unanimous && changes > 0) || changes * 2 >= members.len() {
        ReviewDecision::RequestChanges
    } else {
        ReviewDecision::Pass
    };
    let votes = members.iter().map(|member| format!("{}={}", member.agent, member.review.decision)).collect::<Vec<_>>().join("；");
    CouncilAggregate {
        decision,
        summary: format!("审查委员会 {} 人完成：{}；合并 {} 个独立问题。", members.len(), votes, issues.len()),
        issues: issues.into_values().collect(),
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
    }
}

fn provider_family(agent: &AgentKind) -> String {
    match agent {
        AgentKind::Codex | AgentKind::OpenAiApi => "openai".into(),
        AgentKind::ClaudeCode | AgentKind::AnthropicApi => "anthropic".into(),
        AgentKind::GrokCli | AgentKind::GrokApi => "xai".into(),
        AgentKind::KimiCli | AgentKind::KimiApi => "kimi".into(),
        AgentKind::MiniMaxCli | AgentKind::MiniMaxApi => "minimax".into(),
        other => other.to_string(),
    }
}

fn run_id_from_dir(path: &Path) -> Result<String, OrchestratorError> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| OrchestratorError::InvalidState("run directory has no id".into()))
}
