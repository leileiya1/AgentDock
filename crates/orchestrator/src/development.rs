impl Orchestrator {
    pub async fn drive_task(&self, task_id: &str) -> Result<TaskSummary, OrchestratorError> {
        // A long-running daemon notices Provider installs and compatibility-shim replacements.
        self.refresh_provider_registry().await;
        loop {
            let task = self.task(task_id).await?;
            if self.adopt_active_run(&task).await? {
                continue;
            }
            if matches!(task.status, TaskStatus::Planning | TaskStatus::ReadyForDevelopment | TaskStatus::ReadyForRevision | TaskStatus::Validating | TaskStatus::ReadyForReview)
                && self.enforce_global_budget(&task).await?
            {
                return self.store.task_summary(task_id).await.map_err(Into::into);
            }
            if matches!(task.status, TaskStatus::Planning | TaskStatus::ReadyForDevelopment | TaskStatus::ReadyForRevision | TaskStatus::Validating | TaskStatus::ReadyForReview)
                && self.enforce_budget(&task).await?
            {
                return self.store.task_summary(task_id).await.map_err(Into::into);
            }
            match task.status {
                TaskStatus::Planning => self.plan(task).await?,
                TaskStatus::ReadyForDevelopment | TaskStatus::ReadyForRevision => {
                    let task_id = task.id.clone();
                    self.develop(task).await?;
                    self.complete_development_operations(&task_id).await?;
                }
                TaskStatus::Validating => self.validate(task).await?,
                TaskStatus::ReadyForReview => self.review(task).await?,
                _ => return self.store.task_summary(task_id).await.map_err(Into::into),
            }
        }
    }
    async fn develop(&self, mut task: TaskRow) -> Result<(), OrchestratorError> {
        let from = task.status;
        let revision = task.revision + 1;
        let to = if from == TaskStatus::ReadyForDevelopment {
            TaskStatus::Developing
        } else {
            TaskStatus::Revising
        };
        self.enter_development_stage(&task, from, to, revision)
            .await?;
        task.revision = revision;
        task.status = to;
        let project = self.project(&task.project_id).await?;
        let wt = required_path(&task.worktree_path)?;
        reset_io_dirs(&wt).await?;
        let history = self.write_history(&task, &wt).await?;
        let config = self.load_trusted_config(&project).await?;
        let approved_plan = self.approved_plan_seal(&task).await?;
        let input = self
            .build_input(&task, &project, history.as_deref(), approved_plan.as_ref())
            .await?;
        tokio::fs::write(wt.join(".agentflow-in/input.md"), input).await?;
        let baseline = self.git.resolve(&wt, "HEAD").await?;
        let chain = self.provider_chain(
            task.developer.clone(),
            RunRole::Developer,
            None,
            &project.settings,
            task.api_egress_approved,
        );
        let mut result = None;
        let mut selected_developer = task.developer.clone();
        let mut previous = None;
        let mut previous_error = String::new();
        for candidate in chain {
            if let Some(from) = previous.clone() {
                self.git.reset_owned_worktree(&wt, &baseline).await?;
                reset_io_dirs(&wt).await?;
                let history = self.write_history(&task, &wt).await?;
                let input = self
                    .build_input(&task, &project, history.as_deref(), approved_plan.as_ref())
                    .await?;
                tokio::fs::write(wt.join(".agentflow-in/input.md"), input).await?;
                self.record_provider_fallback(
                    &task,
                    RunRole::Developer,
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
            let attempt = self
                .run_agent(
                    adapter.as_ref(),
                    &task,
                    &project,
                    &run_dir,
                    RunRole::Developer,
                    ".agentflow-in/input.md",
                    &config,
                    None,
                )
                .await;
            let running = match attempt {
                Ok(running) if running.outcome.cancelled => return Ok(()),
                Ok(running)
                    if running.outcome.exit_code == Some(0) && !running.outcome.timed_out =>
                {
                    running
                }
                Ok(running) => {
                    previous = Some(candidate);
                    previous_error = format!(
                        "provider exited with {:?}{}",
                        running.outcome.exit_code,
                        if running.outcome.timed_out {
                            " after timing out"
                        } else {
                            ""
                        }
                    );
                    if self.enforce_budget(&self.task(&task.id).await?).await? {
                        self.git.reset_owned_worktree(&wt, &baseline).await?;
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
                    if self.enforce_budget(&self.task(&task.id).await?).await? {
                        self.git.reset_owned_worktree(&wt, &baseline).await?;
                        return Ok(());
                    }
                    continue;
                }
            };
            let out_result = wt.join(".agentflow-out/result.json");
            if out_result.exists() {
                tokio::fs::copy(&out_result, running.run_dir.join("result.json")).await?;
            }
            let collected = adapter
                .collect_result(&running.run_dir, RunRole::Developer)
                .await;
            self.protect_run_files(&running.run_dir).await?;
            match collected {
                Ok(CollectedResult::Development(value))
                    if value.task_id == task.id
                        && value.revision == task.revision
                        && value.status != DevelopmentStatus::Failed =>
                {
                    selected_developer = candidate.clone();
                    result = Some(value);
                    break;
                }
                Ok(_) => {
                    previous_error =
                        "developer result did not match the active task or failed".into();
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
                    RunRole::Developer,
                    &config,
                    &rejection,
                )
                .await?
            {
                match repaired.value {
                    CollectedResult::Development(value)
                        if value.task_id == task.id
                            && value.revision == task.revision
                            && value.status != DevelopmentStatus::Failed =>
                    {
                        selected_developer = candidate.clone();
                        result = Some(value);
                        break;
                    }
                    _ => {
                        self.invalidate_agent_run(&repaired.run_dir).await?;
                        previous_error = "repaired developer result still mismatched".into();
                    }
                }
            }
            if self.enforce_budget(&self.task(&task.id).await?).await? {
                self.git.reset_owned_worktree(&wt, &baseline).await?;
                return Ok(());
            }
            previous = Some(candidate);
        }
        let Some(result) = result else {
            self.git.reset_owned_worktree(&wt, &baseline).await?;
            self.block(
                &task,
                BlockedReason::RunFailed,
                &format!("all developer providers failed: {previous_error}"),
            )
            .await?;
            return Ok(());
        };
        self.finalize_development_result(
            &task,
            &project,
            result,
            selected_developer,
            &baseline,
        )
        .await
    }

    /// Accept a validated Provider contract and turn its existing worktree edits into a revision.
    /// Crash recovery calls the same path, so adoption cannot bypass plan or commit protection.
    async fn finalize_development_result(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        result: DevelopmentResult,
        selected_developer: AgentKind,
        baseline: &str,
    ) -> Result<(), OrchestratorError> {
        let revision = task.revision;
        let to = task.status;
        let wt = required_path(&task.worktree_path)?;
        let config = self.load_trusted_config(project).await?;
        let approved_plan = self.approved_plan_seal(task).await?;
        if result.task_id != task.id || result.revision != task.revision {
            self.block(
                task,
                BlockedReason::RunFailed,
                "result task_id or revision does not match the active run",
            )
            .await?;
            return Ok(());
        }
        if result.status == DevelopmentStatus::NeedsClarification {
            self.block(
                task,
                BlockedReason::NeedsClarification,
                result
                    .question
                    .as_deref()
                    .unwrap_or("clarification requested"),
            )
            .await?;
            return Ok(());
        }
        let expected_plan_sha = approved_plan.as_ref().map(|plan| plan.sha256.as_str());
        if result.plan_sha256.as_deref() != expected_plan_sha {
            if let Some(plan) = approved_plan.as_ref() {
                self.return_plan_for_reapproval(
                    task,
                    plan,
                    baseline,
                    &wt,
                    &["developer result did not echo the approved plan SHA-256".into()],
                )
                .await?;
            } else {
                self.block(
                    task,
                    BlockedReason::RunFailed,
                    "developer returned a plan SHA for a task without plan approval",
                )
                .await?;
            }
            return Ok(());
        }
        if !self.git.has_changes(&wt).await? {
            self.block(
                task,
                BlockedReason::NoChanges,
                "agent completed without changes",
            )
            .await?;
            return Ok(());
        }
        if let Some(plan) = approved_plan.as_ref() {
            // Re-read the DB seal immediately before accepting filesystem changes. This catches
            // a plan edited while a provider was running, while path matching catches provider
            // deviation independently of its self-reported changed_files.
            let current = self.approved_plan_seal(task).await?;
            if current.as_ref().map(|value| value.sha256.as_str()) != Some(plan.sha256.as_str()) {
                return Err(OrchestratorError::InvalidState(
                    "approved coding plan changed during development".into(),
                ));
            }
            let deviations = self.plan_deviations(&wt, plan).await?;
            if !deviations.is_empty() {
                self.return_plan_for_reapproval(task, plan, baseline, &wt, &deviations)
                    .await?;
                return Ok(());
            }
        }
        let sha = match self
            .git
            .commit_revision(
                &wt,
                task.seq,
                revision,
                &task.title,
                &selected_developer.to_string(),
            )
            .await
        {
            Ok(sha) => sha,
            Err(GitError::UnsafeCommit(detail)) => {
                // The files stay in the isolated worktree so the user can inspect or repair them;
                // only the Git index is cleared by GitEngine before the task becomes BLOCKED.
                self.block(task, BlockedReason::CommitGuard, &detail)
                    .await?;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let base = task
            .base_commit
            .as_deref()
            .ok_or_else(|| OrchestratorError::InvalidState("base commit missing".into()))?;
        let diff = self
            .git
            .diff(
                &wt,
                base,
                &sha,
                &config.review.exclude_globs,
                config.review.max_patch_bytes,
            )
            .await?;
        let stat = summarize(&diff);
        let artifact_dir = self.task_dir(&task.id).join("artifacts");
        tokio::fs::create_dir_all(&artifact_dir).await?;
        tokio::fs::write(
            artifact_dir.join(format!("r{revision}.patch")),
            self.git.full_patch(&wt, base, &sha).await?,
        )
        .await?;
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,diff_stat_json,created_at) VALUES(?,?,?,?,?,?)").bind(Uuid::now_v7().to_string()).bind(&task.id).bind(revision).bind(&sha).bind(serde_json::to_string(&stat).map_err(|e|OrchestratorError::Config(e.to_string()))?).bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
        self.store
            .transition(
                &task.id,
                &[to],
                TaskStatus::Validating,
                None,
                Actor::Orchestrator,
                "run:succeeded",
                &json!({
                    "commit_sha": sha,
                    "summary": result.summary,
                    "changed_files": result.changed_files,
                    "notes": result.notes
                }),
            )
            .await?;
        Ok(())
    }
    async fn validate(&self, task: TaskRow) -> Result<(), OrchestratorError> {
        let wt = required_path(&task.worktree_path)?;
        let project = self.project(&task.project_id).await?;
        let config = self.load_trusted_config(&project).await?;
        let integrity = self.integrity_guard(&task, &config).await?;
        if !integrity.hard_violations.is_empty() {
            self.block(
                &task,
                BlockedReason::QualityGate,
                &format!(
                    "完整性门禁拒绝：{}",
                    integrity.hard_violations.join("；")
                ),
            )
            .await?;
            return Ok(());
        }
        let report = match self.execute_validation(&task, &wt, &config.validate.steps).await {
            Ok(report) => report,
            Err(OrchestratorError::RemoteNodeUnavailable(detail)) => {
                self.block(&task, BlockedReason::RemoteNodeUnavailable, &detail)
                    .await?;
                return Ok(());
            }
            Err(error) => {
                self.block(&task, BlockedReason::ValidationInfra, &error.to_string())
                    .await?;
                return Ok(());
            }
        };
        let artifact = self
            .task_dir(&task.id)
            .join("artifacts")
            .join(format!("r{}-tests.json", task.revision));
        tokio::fs::write(
            &artifact,
            serde_json::to_vec_pretty(&report)
                .map_err(|e| OrchestratorError::Config(e.to_string()))?,
        )
        .await?;
        self.record_reproducibility_manifest(&task, &config).await?;
        let (to, event) = if report.passed && report.steps.is_empty() {
            (TaskStatus::ReadyForReview, "validation:skipped")
        } else if report.passed {
            (TaskStatus::ReadyForReview, "validation:passed")
        } else if task.revision >= task.max_revisions {
            sqlx::query("UPDATE tasks SET blocked_detail=? WHERE id=?")
                .bind("maximum revisions reached after validation failure")
                .bind(&task.id)
                .execute(self.store.pool())
                .await?;
            (TaskStatus::Blocked, "validation:max_revisions")
        } else {
            (TaskStatus::ReadyForRevision, "validation:failed")
        };
        let reason = (to == TaskStatus::Blocked).then_some(BlockedReason::MaxRevisions);
        self.store
            .transition(
                &task.id,
                &[TaskStatus::Validating],
                to,
                reason,
                Actor::Orchestrator,
                event,
                &serde_json::to_value(report).unwrap_or(Value::Null),
            )
            .await?;
        Ok(())
    }
}
