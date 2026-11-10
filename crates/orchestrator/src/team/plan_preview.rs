use super::*;
impl Orchestrator {
    pub async fn team_plan_preview(
        &self,
        input: TeamPlanPreviewInput,
    ) -> Result<TeamPlanPreview, OrchestratorError> {
        self.team_plan_preview_with(&input.task_id, input.expected_revision, &input.plan, false)
            .await
    }
    pub(super) async fn team_plan_preview_with(
        &self,
        task: &str,
        expected: u32,
        plan: &TeamPlan,
        explicit_acceptance: bool,
    ) -> Result<TeamPlanPreview, OrchestratorError> {
        let view = self
            .team_get(task)
            .await?
            .ok_or_else(|| err("team missing"))?;
        let mut blockers = Vec::new();
        if view.plan_revision != expected {
            blockers.push("计划版本已经变化，请重新读取后再预览".into());
        }
        if expected >= view.config.max_plan_revisions {
            blockers.push("累计计划修订额度已用尽".into());
        }
        if matches!(
            view.phase,
            TeamPhase::Draft | TeamPhase::Cancelled | TeamPhase::Delivering | TeamPhase::Delivered
        ) {
            blockers.push("先启动任务建立输入基线；已取消或交付已封存的任务不能修订计划".into());
        }
        if let Err(e) = rules::validate_plan(plan, &view.requirements, &view.config) {
            blockers.push(e);
        }
        for item in &plan.items {
            for pattern in &item.write_globs {
                if let Err(e) = Glob::new(pattern) {
                    blockers.push(e.to_string());
                }
            }
        }
        let trusted = self
            .load_trusted_config(&self.team_project(task).await?)
            .await?;
        for step in &trusted.validate.steps {
            if !plan.criteria.iter().any(|c| {
                c.required
                    && c.checks
                        .iter()
                        .any(|check| check.argv == step.argv && check.expected_exit == 0)
            }) {
                blockers.push(format!(
                    "plan omitted mandatory trusted project validation step: {}",
                    step.name
                ));
            }
        }
        let mut affected = view
            .plan
            .as_ref()
            .map(|old| rules::affected_items(old, plan))
            .unwrap_or_else(|| plan.items.iter().map(|i| i.id.clone()).collect());
        loop {
            let before = affected.len();
            let invalid: HashSet<_> = view
                .artifacts
                .iter()
                .filter(|a| affected.contains(&a.work_item_id))
                .map(|a| &a.id)
                .collect();
            for a in &view.artifacts {
                if a.included_artifact_ids
                    .iter()
                    .any(|id| invalid.contains(id))
                {
                    affected.insert(a.work_item_id.clone());
                }
            }
            if before == affected.len() {
                break;
            }
        }
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        let mut preserved = Vec::new();
        let old_items = view
            .plan
            .as_ref()
            .map(|p| p.items.as_slice())
            .unwrap_or_default();
        for item in &plan.items {
            match old_items.iter().find(|p| p.id == item.id) {
                None => added.push(item.id.clone()),
                Some(old) if old != item => changed.push(item.id.clone()),
                Some(_) => {}
            }
            if !affected.contains(&item.id) {
                preserved.push(item.id.clone());
            }
        }
        for old in old_items {
            if !plan.items.iter().any(|i| i.id == old.id) {
                removed.push(old.id.clone());
            }
        }
        let acceptance_changed = view
            .plan
            .as_ref()
            .is_some_and(|old| old.criteria != plan.criteria);
        if let Some(old) = &view.plan {
            let coverage:i64=sqlx::query_scalar("SELECT COUNT(*) FROM team_audits a JOIN agent_runs r ON r.id=a.attempt_id WHERE a.task_id=? AND a.kind='coverage' AND a.approved=1 AND r.status='SUCCEEDED'").bind(task).fetch_one(self.store.pool()).await?;
            let sealed = coverage > 0 || !view.artifacts.is_empty() || view.candidate.is_some();
            if sealed
                && !explicit_acceptance
                && old.criteria.iter().any(|c| !plan.criteria.contains(c))
            {
                blockers.push("local replan cannot delete or change an acceptance contract".into());
            }
        }
        let blocking_attempt_ids = view
            .attempts
            .iter()
            .filter(|a| {
                a.status == RunStatus::Running
                    && (affected.contains(&a.identity.work_item_id)
                        || a.identity.work_item_id.starts_with("__"))
            })
            .map(|a| a.identity.attempt_id.clone())
            .collect::<Vec<_>>();
        // Program integration has no logical seat and is intentionally absent from attempts.
        // Check every canonical run too, so previews and activation share the same phase boundary.
        let program_active:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND status='RUNNING' AND team_work_item_id='__integrate'").bind(task).fetch_one(self.store.pool()).await?;
        if !blocking_attempt_ids.is_empty() || program_active > 0 {
            blockers.push("local replan requires a phase boundary: stop/reconcile affected attempts first; unrelated attempts may continue".into());
        }
        Ok(TeamPlanPreview {
            base_revision: view.plan_revision,
            proposal_hash: hash(plan)?,
            added,
            removed,
            changed,
            affected: affected.into_iter().collect(),
            preserved,
            blocking_attempt_ids,
            acceptance_changed,
            blockers,
        })
    }
}
