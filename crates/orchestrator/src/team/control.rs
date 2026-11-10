use super::*;
impl Orchestrator {
    pub async fn team_create(&self, input: TeamCreateInput) -> Result<TeamView, OrchestratorError> {
        rules::validate_config(&input.config).map_err(err)?;
        if input.title.trim().is_empty()
            || input.goal.trim().is_empty()
            || input.goal.len() > 30000
            || input.requirements.len() > 100
        {
            return Err(err(
                "provide a title, original goal and bounded requirements",
            ));
        }
        for g in &input.config.write_globs {
            Glob::new(g).map_err(|e| err(e.to_string()))?;
        }
        let project = self.project(&input.project_id).await?;
        self.load_trusted_config(&project).await?;
        let target = input
            .target_branch
            .clone()
            .unwrap_or(project.default_branch.clone());
        self.git.resolve(&project.repo, &target).await?;
        let mut requirements = vec![TeamRequirement {
            id: "goal".into(),
            original_text: input.goal.clone(),
            source: "user.goal".into(),
            required: true,
        }];
        for (index, text) in input.requirements.iter().enumerate() {
            if text.trim().is_empty() || text.len() > 5000 {
                return Err(err("empty or oversized original requirement"));
            }
            requirements.push(TeamRequirement {
                id: format!("r{}", index + 1),
                original_text: text.clone(),
                source: format!("user.requirements[{index}]"),
                required: true,
            });
        }
        let dev = input
            .config
            .members
            .iter()
            .find(|m| m.roles.contains(&TeamRole::Developer))
            .and_then(|m| m.providers.first())
            .cloned()
            .ok_or_else(|| err("developer missing"))?;
        let reviewer = input
            .config
            .members
            .iter()
            .find(|m| m.roles.contains(&TeamRole::Reviewer))
            .and_then(|m| m.providers.first())
            .cloned()
            .ok_or_else(|| err("reviewer missing"))?;
        let policy = TaskPolicy {
            require_plan_approval: false,
            ..TaskPolicy::default()
        };
        let task = self
            .store
            .create_governed_task_with_api_egress(
                &input.project_id,
                input.title.trim(),
                &input.goal,
                dev,
                reviewer,
                &target,
                i64::from(input.config.max_reworks.max(1)),
                false,
                &policy,
            )
            .await?;
        let now = Utc::now().to_rfc3339();
        let mut settings_snapshot =
            serde_json::to_value(&project.settings).map_err(|e| err(e.to_string()))?;
        settings_snapshot
            .as_object_mut()
            .ok_or_else(|| err("invalid settings snapshot"))?
            .insert(
                "_teamProjectConfigSha".into(),
                json!(team_config_file_hash(&project.repo).await?),
            );
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("INSERT INTO team_tasks(task_id,phase,goal,config_json,requirements_json,project_settings_json,created_at,updated_at) VALUES(?,'draft',?,?,?,?,?,?)")
            .bind(&task.id).bind(&input.goal).bind(encode(&input.config)?).bind(encode(&requirements)?).bind(settings_snapshot.to_string()).bind(&now).bind(&now).execute(&mut *tx).await?;
        event(
            &mut tx,
            &task.id,
            "created",
            json!({"schemaVersion":1,"teamCapacity":input.config.team_capacity}),
        )
        .await?;
        tx.commit().await?;
        self.team_get(&task.id)
            .await?
            .ok_or_else(|| err("team creation disappeared"))
    }
    pub async fn team_action(
        &self,
        task: &str,
        action: TeamAction,
    ) -> Result<TeamView, OrchestratorError> {
        let view = self
            .team_get(task)
            .await?
            .ok_or_else(|| err("not a team task"))?;
        if matches!(view.phase, TeamPhase::Delivered | TeamPhase::Delivering)
            && !matches!(action, TeamAction::Deliver)
        {
            return Err(err(
                "delivery is sealed; reconcile it before changing the task",
            ));
        }
        if view.phase == TeamPhase::Cancelled
            && !matches!(action, TeamAction::Cancel | TeamAction::ReconcileResources)
        {
            return Err(err("cancelled tasks cannot be resurrected"));
        }
        match action {
            TeamAction::ReconcileResources => self.team_reconcile_resources(task).await?,
            TeamAction::ReviseAcceptance {
                expected_revision,
                criteria,
                reason,
            } => {
                if reason.trim().is_empty() || reason.len() > 4000 {
                    return Err(err("an explicit acceptance change reason is required"));
                }
                let mut plan = view
                    .plan
                    .clone()
                    .ok_or_else(|| err("active plan required"))?;
                plan.criteria = criteria;
                self.team_activate_plan(
                    task,
                    expected_revision,
                    plan,
                    &format!("explicit user acceptance revision: {reason}"),
                    true,
                )
                .await?;
            }
            TeamAction::RefreshTarget => {
                if view.attempts.iter().any(|a| a.status == RunStatus::Running) {
                    return Err(err(
                        "stop/reconcile active attempts before refreshing target baseline",
                    ));
                }
                if view.plan.is_none() {
                    return Err(err("no plan to reintegrate"));
                }
                let task_row = self.task(task).await?;
                let project = self.team_project(task).await?;
                let new_base = self
                    .git
                    .resolve(&project.repo, &task_row.target_branch)
                    .await?;
                let mut tx = self.store.pool().begin().await?;
                sqlx::query(
                    "UPDATE tasks SET base_commit=?,status='DEVELOPING',updated_at=? WHERE id=?",
                )
                .bind(&new_base)
                .bind(Utc::now().to_rfc3339())
                .bind(task)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE team_tasks SET current_candidate_id=NULL,phase='running',detail='target baseline refreshed; all final evidence must be rerun',updated_at=? WHERE task_id=?").bind(Utc::now().to_rfc3339()).bind(task).execute(&mut *tx).await?;
                sqlx::query("UPDATE team_work_items SET state='pending',epoch=epoch+1,artifact_id=NULL WHERE task_id=? AND id IN ('__coverage','__integrate','__verify','__review')").bind(task).execute(&mut *tx).await?;
                event(
                    &mut tx,
                    task,
                    "target_refreshed",
                    json!({"baseSha":new_base,"priorCandidate":view.candidate}),
                )
                .await?;
                tx.commit().await?;
            }
            TeamAction::Start => self.team_start(task).await?,
            TeamAction::ApprovePlan { expected_revision } => {
                if view.phase != TeamPhase::PlanApproval || view.plan_revision != expected_revision
                {
                    return Err(err("stale plan approval"));
                }
                sqlx::query("UPDATE team_plans SET approved_at=? WHERE task_id=? AND revision=?")
                    .bind(Utc::now().to_rfc3339())
                    .bind(task)
                    .bind(expected_revision)
                    .execute(self.store.pool())
                    .await?;
                self.team_set_phase(task, TeamPhase::Running, None).await?;
            }
            TeamAction::Pause => {
                if !matches!(
                    view.phase,
                    TeamPhase::Running | TeamPhase::Planning | TeamPhase::PlanApproval
                ) {
                    return Err(err("only an executing team can pause dispatch"));
                }
                self.team_set_phase(
                    task,
                    TeamPhase::Paused,
                    Some("dispatch paused; active attempts may finish"),
                )
                .await?
            }
            TeamAction::Resume => {
                if !matches!(view.phase, TeamPhase::Paused | TeamPhase::Blocked) {
                    return Err(err("task is not paused/blocked"));
                }
                if view.config.require_plan_approval && view.plan.is_some() {
                    let approved:i64=sqlx::query_scalar("SELECT COUNT(*) FROM team_plans WHERE task_id=? AND revision=? AND approved_at IS NOT NULL").bind(task).bind(view.plan_revision).fetch_one(self.store.pool()).await?;
                    if approved == 0 {
                        self.team_set_phase(task, TeamPhase::PlanApproval, None)
                            .await?;
                        self.team_report(task).await?;
                        return self
                            .team_get(task)
                            .await?
                            .ok_or_else(|| err("team missing"));
                    }
                }
                if view.phase == TeamPhase::Blocked {
                    if view.attempts.iter().any(|a| a.status == RunStatus::Running) {
                        return Err(err("old process/resources still require reconciliation"));
                    }
                    for item in view
                        .items
                        .iter()
                        .filter(|w| w.state == TeamWorkState::Blocked)
                    {
                        let failure = view
                            .attempts
                            .iter()
                            .rev()
                            .find(|a| a.identity.work_item_id == item.spec.id)
                            .and_then(|a| a.failure_class.as_deref());
                        if matches!(
                            failure,
                            Some(
                                "auth"
                                    | "infrastructure_retry"
                                    | "protocol_repair"
                                    | "crash_recovery"
                            )
                        ) {
                            sqlx::query("UPDATE team_work_items SET state='pending',detail=NULL WHERE task_id=? AND id=? AND state='blocked'").bind(task).bind(&item.spec.id).execute(self.store.pool()).await?;
                        }
                    }
                }
                self.team_set_phase(
                    task,
                    if view.plan.is_none()
                        || view.items.iter().any(|w| {
                            w.spec.kind == TeamWorkKind::Plan
                                && matches!(
                                    w.state,
                                    TeamWorkState::Pending
                                        | TeamWorkState::Active
                                        | TeamWorkState::Blocked
                                )
                        })
                    {
                        TeamPhase::Planning
                    } else {
                        TeamPhase::Running
                    },
                    None,
                )
                .await?;
            }
            TeamAction::Cancel => {
                let mut tx = self.store.pool().begin().await?;
                sqlx::query("UPDATE team_tasks SET phase='cancelled',detail='cancel requested',updated_at=? WHERE task_id=?").bind(Utc::now().to_rfc3339()).bind(task).execute(&mut *tx).await?;
                sqlx::query("UPDATE tasks SET status='CANCELLED',updated_at=? WHERE id=?")
                    .bind(Utc::now().to_rfc3339())
                    .bind(task)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE team_work_items SET state='cancelled',epoch=epoch+1 WHERE task_id=? AND state NOT IN ('superseded','output_accepted')").bind(task).execute(&mut *tx).await?;
                sqlx::query("UPDATE team_attempt_contexts SET cancel_requested=1 WHERE run_id IN (SELECT id FROM agent_runs WHERE task_id=? AND status='RUNNING')").bind(task).execute(&mut *tx).await?;
                event(&mut tx, task, "cancelled", json!({})).await?;
                tx.commit().await?;
                self.team_signal_cancellations(task).await?;
            }
            TeamAction::CancelWork { work_item_id } => {
                let mut tx = self.store.pool().begin().await?;
                sqlx::query("UPDATE team_work_items SET state='cancelled',epoch=epoch+1,detail='cancelled by user' WHERE task_id=? AND id=? AND state!='output_accepted'").bind(task).bind(&work_item_id).execute(&mut *tx).await?;
                sqlx::query("UPDATE team_attempt_contexts SET cancel_requested=1 WHERE run_id IN (SELECT id FROM agent_runs WHERE task_id=? AND team_work_item_id=? AND status='RUNNING')").bind(task).bind(&work_item_id).execute(&mut *tx).await?;
                event(
                    &mut tx,
                    task,
                    "work_cancelled",
                    json!({"workItemId":work_item_id}),
                )
                .await?;
                tx.commit().await?;
                self.team_signal_cancellations(task).await?;
            }
            TeamAction::RetryWork { work_item_id } => {
                let active:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND team_work_item_id=? AND status='RUNNING'").bind(task).bind(&work_item_id).fetch_one(self.store.pool()).await?;
                if active > 0 {
                    return Err(err("old process/resources have not been reclaimed"));
                }
                let item = view
                    .items
                    .iter()
                    .find(|i| i.spec.id == work_item_id)
                    .ok_or_else(|| err("work item missing"))?;
                if !matches!(
                    item.state,
                    TeamWorkState::Blocked | TeamWorkState::Cancelled
                ) {
                    return Err(err("only blocked/cancelled work can be retried"));
                }
                if item.spec.kind == TeamWorkKind::Implement
                    && view
                        .attempts
                        .iter()
                        .rev()
                        .find(|a| a.identity.work_item_id == work_item_id)
                        .is_some_and(|a| a.failure_class.as_deref() == Some("product_rework"))
                {
                    if view.reworks_used >= view.config.max_reworks {
                        return Err(err("cumulative product rework limit exhausted"));
                    }
                    self.team_request_rework(
                        task,
                        std::slice::from_ref(&work_item_id),
                        "explicit retry of failed product work",
                        None,
                    )
                    .await?;
                    self.team_report(task).await?;
                    return self
                        .team_get(task)
                        .await?
                        .ok_or_else(|| err("team missing"));
                }
                sqlx::query("UPDATE team_work_items SET state='pending',detail=NULL WHERE task_id=? AND id=?").bind(task).bind(work_item_id).execute(self.store.pool()).await?;
                self.team_set_phase(
                    task,
                    if item.spec.kind == TeamWorkKind::Plan || view.plan.is_none() {
                        TeamPhase::Planning
                    } else {
                        TeamPhase::Running
                    },
                    None,
                )
                .await?;
            }
            TeamAction::ReplaceMember { member } => {
                let mut tx = self.store.pool().begin().await?;
                // Serialize seat replacement against claims and other replacements. A stale UI
                // cannot overwrite changes to another seat or change a newly occupied binding.
                sqlx::query("UPDATE team_tasks SET updated_at=updated_at WHERE task_id=?")
                    .bind(task)
                    .execute(&mut *tx)
                    .await?;
                let current: String =
                    sqlx::query_scalar("SELECT config_json FROM team_tasks WHERE task_id=?")
                        .bind(task)
                        .fetch_one(&mut *tx)
                        .await?;
                let mut config: TeamConfig = decode(current)?;
                let old = config
                    .members
                    .iter_mut()
                    .find(|m| m.id == member.id)
                    .ok_or_else(|| err("replacement must use an existing seat"))?;
                let prior = old.clone();
                let busy: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND team_member_id=? AND status='RUNNING'")
                    .bind(task).bind(&member.id).fetch_one(&mut *tx).await?;
                if busy > 0 {
                    return Err(err("stop/reconcile this seat before replacing its binding"));
                }
                *old = member.clone();
                rules::validate_config(&config).map_err(err)?;
                sqlx::query("UPDATE team_tasks SET config_json=?,updated_at=? WHERE task_id=?")
                    .bind(encode(&config)?)
                    .bind(Utc::now().to_rfc3339())
                    .bind(task)
                    .execute(&mut *tx)
                    .await?;
                event(
                    &mut tx,
                    task,
                    "member_rebound",
                    json!({"before":prior,"member":member}),
                )
                .await?;
                tx.commit().await?;
            }
            TeamAction::RevisePlan {
                expected_revision,
                plan,
            } => {
                self.team_activate_plan(
                    task,
                    expected_revision,
                    plan,
                    "explicit local plan revision",
                    false,
                )
                .await?
            }
            TeamAction::Deliver => self.team_deliver(task).await?,
        }
        self.team_report(task).await?;
        self.team_get(task)
            .await?
            .ok_or_else(|| err("team missing"))
    }
    pub(super) async fn team_project(
        &self,
        task_id: &str,
    ) -> Result<ProjectRow, OrchestratorError> {
        let task = self.task(task_id).await?;
        let mut project = self.project(&task.project_id).await?;
        let settings: String =
            sqlx::query_scalar("SELECT project_settings_json FROM team_tasks WHERE task_id=?")
                .bind(task_id)
                .fetch_one(self.store.pool())
                .await?;
        project.settings = decode(settings)?;
        Ok(project)
    }
    pub(super) async fn team_start(&self, task_id: &str) -> Result<(), OrchestratorError> {
        let task = self.task(task_id).await?;
        let view = self
            .team_get(task_id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        if view.phase != TeamPhase::Draft {
            return Err(err("team already started"));
        }
        let project = self.team_project(task_id).await?;
        self.load_trusted_config(&project).await?;
        let compatibility = self.git.compatibility_report(&project.repo).await?;
        if !compatibility.blockers.is_empty() {
            return Err(err(compatibility.blockers.join("; ")));
        }
        let base = self.git.resolve(&project.repo, &task.target_branch).await?;
        let spec = system_spec(
            "__plan",
            TeamWorkKind::Plan,
            vec![],
            vec![view.config.planner_member_id.clone()],
            "Read the original goal, requirements and source. Produce a bounded development DAG and per-criterion executable checks. Do not modify product source.",
        );
        let mut tx = self.store.pool().begin().await?;
        let changed = sqlx::query(
            "UPDATE team_tasks SET phase='planning',updated_at=? WHERE task_id=? AND phase='draft'",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(task_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(err("concurrent start"));
        }
        sqlx::query("UPDATE tasks SET status='PLANNING',base_commit=?,updated_at=? WHERE id=?")
            .bind(&base)
            .bind(Utc::now().to_rfc3339())
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        put_work(&mut tx, task_id, &spec).await?;
        event(&mut tx, task_id, "started", json!({"baseSha":base})).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn team_set_phase(
        &self,
        task: &str,
        phase: TeamPhase,
        detail: Option<&str>,
    ) -> Result<(), OrchestratorError> {
        let mut tx = self.store.pool().begin().await?;
        let changed=sqlx::query("UPDATE team_tasks SET phase=?,detail=?,updated_at=? WHERE task_id=? AND phase NOT IN ('cancelled','delivered','delivering') AND (phase<>? OR detail IS NOT ?)").bind(phase.to_string()).bind(detail).bind(Utc::now().to_rfc3339()).bind(task).bind(phase.to_string()).bind(detail).execute(&mut *tx).await?;
        if changed.rows_affected() == 1 {
            let status = match phase {
                TeamPhase::Planning => TaskStatus::Planning,
                TeamPhase::ReadyForDelivery => TaskStatus::WaitingForHumanApproval,
                TeamPhase::Blocked | TeamPhase::Paused => TaskStatus::Blocked,
                TeamPhase::PlanApproval => TaskStatus::WaitingForPlanApproval,
                _ => TaskStatus::Developing,
            };
            sqlx::query("UPDATE tasks SET status=?,blocked_detail=?,updated_at=? WHERE id=?")
                .bind(status.to_string())
                .bind(detail)
                .bind(Utc::now().to_rfc3339())
                .bind(task)
                .execute(&mut *tx)
                .await?;
            event(
                &mut tx,
                task,
                "phase",
                json!({"phase":phase,"detail":detail}),
            )
            .await?;
        }
        tx.commit().await?;
        if changed.rows_affected() == 1
            && matches!(
                phase,
                TeamPhase::Blocked | TeamPhase::PlanApproval | TeamPhase::ReadyForDelivery
            )
            && let Ok(summary) = self.store.task_summary(task).await
        {
            self.notify_task(&summary).await;
        }
        Ok(())
    }
    pub(super) async fn team_activate_plan(
        &self,
        task: &str,
        expected: u32,
        plan: TeamPlan,
        reason: &str,
        explicit_acceptance_change: bool,
    ) -> Result<(), OrchestratorError> {
        let view = self
            .team_get(task)
            .await?
            .ok_or_else(|| err("team missing"))?;
        let preview = self
            .team_plan_preview_with(task, expected, &plan, explicit_acceptance_change)
            .await?;
        if !preview.blockers.is_empty() {
            return Err(err(preview.blockers.join("; ")));
        }
        let affected: std::collections::BTreeSet<_> = preview.affected.into_iter().collect();
        let mut tx = self.store.pool().begin().await?;
        let phase = if view.config.require_plan_approval && !explicit_acceptance_change {
            "plan_approval"
        } else {
            "running"
        };
        let changed=sqlx::query("UPDATE team_tasks SET phase=?,plan_revision=plan_revision+1,current_candidate_id=NULL,detail=NULL,updated_at=? WHERE task_id=? AND plan_revision=? AND phase NOT IN ('cancelled','delivering','delivered')")
            .bind(phase).bind(Utc::now().to_rfc3339()).bind(task).bind(expected).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(err("stale plan activation"));
        }
        // The write above serializes this activation with atomic claims. Recheck the phase
        // boundary inside that same transaction, rather than trusting a read-only preview.
        let running_ids: Vec<String> = sqlx::query_scalar("SELECT team_work_item_id FROM agent_runs WHERE task_id=? AND status='RUNNING' AND team_work_item_id IS NOT NULL")
            .bind(task).fetch_all(&mut *tx).await?;
        if running_ids
            .iter()
            .any(|id| id.starts_with("__") || affected.contains(id))
        {
            return Err(err(
                "affected attempts started after preview; stop/reconcile them before activating",
            ));
        }
        sqlx::query("INSERT INTO team_plans(task_id,revision,hash,plan_json,reason,created_at) VALUES(?,?,?,?,?,?)")
            .bind(task).bind(expected+1).bind(hash(&plan)?).bind(encode(&plan)?).bind(reason).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
        if explicit_acceptance_change {
            sqlx::query("UPDATE team_plans SET approved_at=? WHERE task_id=? AND revision=?")
                .bind(Utc::now().to_rfc3339())
                .bind(task)
                .bind(expected + 1)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'human','team:acceptance_authorized',?,?)").bind(task).bind(json!({"fromRevision":expected,"toRevision":expected+1,"reason":reason,"oldCriteria":view.plan.as_ref().map(|p|&p.criteria),"newCriteria":plan.criteria,"requirementsHash":hash(&view.requirements)?}).to_string()).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
        }
        for old in &view.items {
            if old.spec.id == "__plan" {
                continue;
            }
            if old.spec.id.starts_with("__") || affected.contains(&old.spec.id) {
                sqlx::query("INSERT INTO team_work_history(task_id,work_item_id,spec_json,spec_hash,artifact_id,plan_revision,created_at) VALUES(?,?,?,?,?,?,?)")
                    .bind(task).bind(&old.spec.id).bind(encode(&old.spec)?).bind(&old.spec_hash).bind(&old.artifact_id).bind(expected).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
                sqlx::query("DELETE FROM team_work_items WHERE task_id=? AND id=?")
                    .bind(task)
                    .bind(&old.spec.id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        for item in &plan.items {
            if !view
                .items
                .iter()
                .any(|i| i.spec.id == item.id && !affected.contains(&item.id))
            {
                put_work(&mut tx, task, item).await?;
            }
        }
        let testers = view
            .config
            .members
            .iter()
            .filter(|m| m.roles.contains(&TeamRole::Tester))
            .map(|m| m.id.clone())
            .collect::<Vec<_>>();
        put_work(&mut tx,task,&system_spec("__coverage",TeamWorkKind::Coverage,vec![],testers.clone(),"Independently compare the original user text with EVERY requirement, work item and acceptance criterion. Reject omissions, ambiguous expectations, self-fulfilling tests and nonfunctional checks. Do not modify source.")).await?;
        put_work(
            &mut tx,
            task,
            &system_spec(
                "__integrate",
                TeamWorkKind::Integrate,
                plan.items.iter().map(|i| i.id.clone()).collect(),
                vec![],
                "Integrate accepted deltas in stable topological order.",
            ),
        )
        .await?;
        put_work(&mut tx,task,&system_spec("__verify",TeamWorkKind::Verify,vec!["__integrate".into()],testers,"Independently inspect the sealed candidate and acceptance contract. Select the exact check IDs to actually execute via the backend Runner. Report omissions or environment blockers. No source edits. A textual claim is not execution evidence.")).await?;
        put_work(&mut tx,task,&system_spec("__review",TeamWorkKind::Review,vec!["__verify".into()],view.config.members.iter().filter(|m|m.roles.contains(&TeamRole::Reviewer)).map(|m|m.id.clone()).collect(),"Independently review actual source, all original requirements, candidate deltas and actual per-criterion evidence. Flag concrete unresolved issues; never infer all requirements passed from process success.")).await?;
        sqlx::query("UPDATE tasks SET status=?,updated_at=? WHERE id=?")
            .bind(if phase == "plan_approval" {
                "WAITING_FOR_PLAN_APPROVAL"
            } else {
                "DEVELOPING"
            })
            .bind(Utc::now().to_rfc3339())
            .bind(task)
            .execute(&mut *tx)
            .await?;
        event(&mut tx,task,"plan_activated",json!({"revision":expected+1,"affected":affected,"preserved":view.items.iter().filter(|i|!affected.contains(&i.spec.id)&&!i.spec.id.starts_with("__")).map(|i|&i.spec.id).collect::<Vec<_>>()})).await?;
        tx.commit().await?;
        if phase == "plan_approval"
            && let Ok(summary) = self.store.task_summary(task).await
        {
            self.notify_task(&summary).await;
        }
        Ok(())
    }
}
