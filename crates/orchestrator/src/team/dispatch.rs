use super::*;
impl Orchestrator {
    /// One short tick; parent tasks never hold a daemon worker while waiting for dependencies.
    /// The returned future is one work item, not an entire team workflow.
    pub async fn team_claim_next(
        &self,
        global_limit: u32,
    ) -> Result<Option<TeamClaim>, OrchestratorError> {
        let tasks=sqlx::query_scalar::<_,String>("SELECT task_id FROM team_tasks WHERE phase IN ('planning','running') ORDER BY updated_at,task_id").fetch_all(self.store.pool()).await?;
        for task_id in tasks {
            let view = self
                .team_get(&task_id)
                .await?
                .ok_or_else(|| err("team missing"))?;
            if self.team_config_drift(&task_id).await? {
                self.team_set_phase(&task_id,TeamPhase::Blocked,Some("project configuration changed after the task snapshot; evidence invalidated, restore configuration or create a task with the new explicit policy")).await?;
                continue;
            }
            let project = self.team_project(&task_id).await?;
            let settings = self.settings_get().await?;
            if settings.global_daily_cost_usd.is_some() {
                self.team_set_phase(&task_id,TeamPhase::Blocked,Some("team CLI budget cannot atomically enforce the configured global hard cost cap; change that policy explicitly before running")).await?;
                continue;
            }
            for item in &view.items {
                if item.state != TeamWorkState::Pending {
                    continue;
                }
                if view.phase == TeamPhase::Planning && item.spec.kind != TeamWorkKind::Plan {
                    continue;
                }
                let retry=sqlx::query("SELECT r.finished_at,c.failure_class FROM agent_runs r JOIN team_attempt_contexts c ON c.run_id=r.id WHERE r.task_id=? AND r.team_work_item_id=? AND r.status='FAILED' ORDER BY r.created_at DESC LIMIT 1").bind(&task_id).bind(&item.spec.id).fetch_optional(self.store.pool()).await?;
                if let Some(retry) = retry {
                    let class: Option<String> = retry.get("failure_class");
                    let finished: Option<String> = retry.get("finished_at");
                    if matches!(
                        class.as_deref(),
                        Some("infrastructure_retry" | "protocol_repair")
                    ) && finished
                        .as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .is_some_and(|at| {
                            Utc::now().signed_duration_since(at).num_milliseconds() < 2000
                        })
                    {
                        continue;
                    }
                }
                let mut dependencies = item.spec.depends_on.clone();
                if matches!(
                    item.spec.kind,
                    TeamWorkKind::Implement | TeamWorkKind::Integrate
                ) && !dependencies.contains(&"__coverage".into())
                {
                    dependencies.push("__coverage".into());
                }
                if dependencies.iter().any(|d| {
                    !view
                        .items
                        .iter()
                        .any(|i| i.spec.id == *d && i.state == TeamWorkState::Accepted)
                }) {
                    continue;
                }
                let dependency_artifacts = view
                    .artifacts
                    .iter()
                    .filter(|a| {
                        dependencies.iter().any(|d| {
                            view.items
                                .iter()
                                .any(|w| w.spec.id == *d && w.artifact_id.as_ref() == Some(&a.id))
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let mut actual_runner_evidence = Vec::new();
                if matches!(
                    item.spec.kind,
                    TeamWorkKind::Review | TeamWorkKind::Implement
                ) {
                    let mut total_chars = 0;
                    for criterion in &view.criteria {
                        for check in &criterion.criterion.checks {
                            if let Some(evidence) = criterion.evidence.iter().rev().find(|e| {
                                e.check_id == check.id
                                    && view
                                        .candidate
                                        .as_ref()
                                        .is_some_and(|c| e.candidate_id == c.id)
                            }) {
                                if total_chars >= 40_000 {
                                    break;
                                }
                                let raw = self.team_evidence_read(&task_id, &evidence.id).await?;
                                let display: String = raw.chars().take(8_000).collect();
                                total_chars += display.chars().count();
                                actual_runner_evidence.push(json!({"evidenceId":evidence.id,"candidateId":evidence.candidate_id,"criterionId":evidence.criterion_id,"checkId":check.id,"originalSha256":evidence.raw_sha256,"rawDisplay":display,"displayBounded":raw.chars().count()>8_000}));
                            }
                        }
                    }
                }
                let input = json!({"schemaVersion":1,"resourceProtocolVersion":2,"originalGoal":view.original_goal,"requirements":view.requirements,"config":view.config,"plan":view.plan,"planRevision":view.plan_revision,"work":item.spec,"previousFeedback":item.detail,"taskFeedback":view.detail,"mandatoryProjectValidation":self.load_trusted_config(&project).await?.validate.steps,"dependencyArtifacts":dependency_artifacts,"candidate":view.candidate,"evidence":view.criteria,"actualRunnerEvidence":actual_runner_evidence,"issues":self.team_open_issues(&task_id).await?,"projectSettingsHash":hash(&project.settings)?});
                let input_hash = hash(&input)?;
                let pairs = dependencies
                    .iter()
                    .map(|d| {
                        (
                            d.clone(),
                            view.items
                                .iter()
                                .find(|i| i.spec.id == *d)
                                .and_then(|i| i.artifact_id.clone()),
                        )
                    })
                    .collect::<Vec<_>>();
                let run_dir = self.run_dir(&task_id);
                if item.spec.kind == TeamWorkKind::Integrate {
                    if let Some(claim) = self
                        .store
                        .team_claim(TeamClaimRequest {
                            task_id: &task_id,
                            work_item_id: &item.spec.id,
                            expected_spec_hash: &item.spec_hash,
                            member_id: "__runner",
                            provider: None,
                            input: input.clone(),
                            input_hash: &input_hash,
                            expected_dependencies: pairs.clone(),
                            run_dir: run_dir.clone(),
                            global_limit,
                            provider_account: String::new(),
                            provider_limit: 0,
                            provider_rpm: 0,
                        })
                        .await?
                    {
                        return Ok(Some(claim));
                    }
                    continue;
                }
                let Some(role) = rules::role_for(item.spec.kind) else {
                    continue;
                };
                let previous_reviews = self
                    .team_valid_reviews(
                        &view,
                        &view
                            .plan
                            .as_ref()
                            .map(hash)
                            .transpose()?
                            .unwrap_or_default(),
                    )
                    .await?;
                let mut any_eligible = false;
                let strict_cross_vendor = view.config.cross_vendor_review
                    || self.team_required_reviews(&task_id, &view).await? > 1;
                let council = &project.settings.review_council;
                for member_id in &item.spec.member_ids {
                    let Some(member) = view
                        .config
                        .members
                        .iter()
                        .find(|m| m.id == *member_id && m.roles.contains(&role))
                    else {
                        continue;
                    };
                    if item.spec.kind == TeamWorkKind::Review
                        && previous_reviews.iter().any(|(m, _)| m == member_id)
                    {
                        continue;
                    }
                    // Fallback uses the same seat. History and all consumed attempts remain.
                    let failures = view
                        .attempts
                        .iter()
                        .filter(|a| {
                            a.identity.work_item_id == item.spec.id
                                && a.identity.member_id == *member_id
                                && a.status != RunStatus::Succeeded
                                && a.status != RunStatus::Running
                        })
                        .count();
                    for provider in member
                        .providers
                        .iter()
                        .cycle()
                        .skip(failures)
                        .take(member.providers.len())
                    {
                        if let Some(candidate) = &view.candidate
                            && !rules::independent(
                                member,
                                provider,
                                item.spec.kind,
                                candidate,
                                strict_cross_vendor,
                            )
                        {
                            continue;
                        }
                        if item.spec.kind == TeamWorkKind::Review
                            && council.enabled
                            && !council.reviewers.is_empty()
                            && !council.reviewers.contains(provider)
                        {
                            continue;
                        }
                        if item.spec.kind == TeamWorkKind::Review
                            && previous_reviews
                                .iter()
                                .any(|(_, p)| rules::family(p) == rules::family(provider))
                        {
                            continue;
                        }
                        any_eligible = true;
                        let account = provider_account(&project, provider);
                        let limit = dispatch_limit(&settings, provider, &account);
                        if let Some(claim) = self
                            .store
                            .team_claim(TeamClaimRequest {
                                task_id: &task_id,
                                work_item_id: &item.spec.id,
                                expected_spec_hash: &item.spec_hash,
                                member_id: member.id.as_str(),
                                provider: Some(provider.clone()),
                                input: input.clone(),
                                input_hash: &input_hash,
                                expected_dependencies: pairs.clone(),
                                run_dir: run_dir.clone(),
                                global_limit,
                                provider_account: account,
                                provider_limit: limit.max_concurrent,
                                provider_rpm: limit.requests_per_minute,
                            })
                            .await?
                        {
                            return Ok(Some(claim));
                        }
                    }
                }
                if !any_eligible {
                    sqlx::query("UPDATE team_work_items SET state='blocked',detail=? WHERE task_id=? AND id=? AND state='pending'").bind("no independent capable member/provider; contributions/fallback history are included").bind(&task_id).bind(&item.spec.id).execute(self.store.pool()).await?;
                    continue;
                }
                let ceiling = if matches!(
                    item.spec.kind,
                    TeamWorkKind::Coverage | TeamWorkKind::Verify | TeamWorkKind::Review
                ) {
                    view.config.max_attempts
                } else {
                    view.config.max_attempts - view.config.verification_reserve
                };
                if view.attempts_used >= ceiling {
                    self.team_set_phase(&task_id,TeamPhase::Blocked,Some("shared attempt budget exhausted; mandatory verification reserve cannot be spent on development retries")).await?;
                    break;
                }
            }
            self.team_progress(&task_id).await?;
        }
        Ok(None)
    }
    pub(super) async fn team_open_issues(
        &self,
        task: &str,
    ) -> Result<Vec<Value>, OrchestratorError> {
        let rows=sqlx::query("SELECT id,candidate_id,criterion_id,owner_work_item_id,detail FROM team_issues WHERE task_id=? AND resolved_by_candidate_id IS NULL ORDER BY created_at").bind(task).fetch_all(self.store.pool()).await?;
        Ok(rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"candidateId":r.get::<Option<String>,_>("candidate_id"),"criterionId":r.get::<Option<String>,_>("criterion_id"),"ownerWorkItemId":r.get::<String,_>("owner_work_item_id"),"detail":r.get::<String,_>("detail")})).collect())
    }
    pub(super) async fn team_progress(&self, task: &str) -> Result<(), OrchestratorError> {
        let view = self
            .team_get(task)
            .await?
            .ok_or_else(|| err("team missing"))?;
        if !matches!(view.phase, TeamPhase::Running | TeamPhase::Planning) {
            return Ok(());
        }
        if view.phase == TeamPhase::Planning
            && view.items.iter().any(|w| {
                w.spec.kind == TeamWorkKind::Plan
                    && matches!(w.state, TeamWorkState::Pending | TeamWorkState::Active)
            })
        {
            self.team_report(task).await?;
            return Ok(());
        }
        if view.gate_blockers.is_empty() {
            self.team_assert_gate(task).await?;
            self.team_set_phase(task,TeamPhase::ReadyForDelivery,Some("all current candidate criteria and independent reviews passed; delivery awaits explicit action")).await?;
        } else if view
            .items
            .iter()
            .any(|i| matches!(i.state, TeamWorkState::Blocked | TeamWorkState::Cancelled))
            && !view.items.iter().any(|i| i.state == TeamWorkState::Active)
            && !view.items.iter().any(|i| {
                i.state == TeamWorkState::Pending
                    && i.spec.depends_on.iter().all(|d| {
                        view.items
                            .iter()
                            .any(|w| w.spec.id == *d && w.state == TeamWorkState::Accepted)
                    })
                    && (!matches!(
                        i.spec.kind,
                        TeamWorkKind::Implement | TeamWorkKind::Integrate
                    ) || view
                        .items
                        .iter()
                        .any(|w| w.spec.id == "__coverage" && w.state == TeamWorkState::Accepted))
            })
        {
            let reasons = view
                .items
                .iter()
                .filter(|i| matches!(i.state, TeamWorkState::Blocked | TeamWorkState::Cancelled))
                .map(|i| {
                    format!(
                        "{}: {}",
                        i.spec.id,
                        i.detail.as_deref().unwrap_or("blocked")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            self.team_set_phase(task, TeamPhase::Blocked, Some(&reasons))
                .await?;
        }
        self.team_report(task).await?;
        Ok(())
    }
}
