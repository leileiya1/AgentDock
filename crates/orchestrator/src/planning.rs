impl Orchestrator {
    async fn plan(&self, task: TaskRow) -> Result<(), OrchestratorError> {
        if self.enforce_budget(&task).await? {
            return Ok(());
        }
        let project = self.project(&task.project_id).await?;
        let wt = required_path(&task.worktree_path)?;
        reset_io_dirs(&wt).await?;
        let config = self.load_trusted_config(&project).await?;
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version),0)+1 FROM task_plans WHERE task_id=?",
        )
        .bind(&task.id)
        .fetch_one(self.store.pool())
        .await?;
        let rejection: Option<String> = sqlx::query_scalar(
            "SELECT rejection_reason FROM task_plans WHERE task_id=? ORDER BY version DESC LIMIT 1",
        )
        .bind(&task.id)
        .fetch_optional(self.store.pool())
        .await?
        .flatten();
        let input = self.build_plan_input(&task, &project, version, rejection.as_deref()).await?;
        tokio::fs::write(wt.join(".agentflow-in/plan-input.md"), input).await?;
        let baseline = self.git.resolve(&wt, "HEAD").await?;
        let chain = self.provider_chain(
            task.developer.clone(),
            RunRole::Planner,
            None,
            &project.settings,
            task.api_egress_approved,
        );
        let mut plan = None;
        let mut error = String::new();
        for candidate in chain {
            let adapter = self.adapter(candidate, &project);
            let run_dir = self.run_dir(&task.id);
            let running = self
                .run_agent(
                    adapter.as_ref(),
                    &task,
                    &project,
                    &run_dir,
                    RunRole::Planner,
                    ".agentflow-in/plan-input.md",
                    &config,
                    Some(PermissionTier::ReadOnly),
                )
                .await;
            let running = match running {
                Ok(value) if value.outcome.cancelled => return Ok(()),
                Ok(value)
                    if value.outcome.exit_code == Some(0) && !value.outcome.timed_out => value,
                Ok(value) => {
                    error = format!("planner exited with {:?}", value.outcome.exit_code);
                    continue;
                }
                Err(value) => {
                    error = value.to_string();
                    continue;
                }
            };
            if self.enforce_budget(&self.task(&task.id).await?).await? {
                return Ok(());
            }
            let collected = adapter.collect_result(&running.run_dir, RunRole::Planner).await;
            self.protect_run_files(&running.run_dir).await?;
            match collected {
                Ok(CollectedResult::Plan(value))
                    if value.task_id == task.id && value.plan_version == version => {
                        plan = Some(value);
                        break;
                    }
                Ok(_) => error = "planner result identity did not match".into(),
                Err(value) => error = value.to_string(),
            }
            self.invalidate_agent_run(&running.run_dir).await?;
        }
        let Some(plan) = plan else {
            self.block(
                &task,
                BlockedReason::RunFailed,
                &format!("all planner providers failed: {error}"),
            )
            .await?;
            return Ok(());
        };
        self.finalize_plan_result(&task, plan, &baseline).await
    }

    async fn finalize_plan_result(
        &self,
        task: &TaskRow,
        plan: PlanResult,
        baseline: &str,
    ) -> Result<(), OrchestratorError> {
        let wt = required_path(&task.worktree_path)?;
        if self.git.resolve(&wt, "HEAD").await? != baseline || self.git.has_changes(&wt).await? {
            self.git.reset_owned_worktree(&wt, baseline).await?;
            self.block(
                task,
                BlockedReason::CommitGuard,
                "只读规划阶段检测到项目文件变更，已重置",
            )
            .await?;
            return Ok(());
        }
        let version = plan.plan_version;
        let id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO task_plans(id,task_id,version,status,summary,steps_json,risks_json,allowed_paths_json,created_at) VALUES(?,?,?,'pending',?,?,?,?,?)")
            .bind(&id).bind(&task.id).bind(version).bind(&plan.summary)
            .bind(serde_json::to_string(&plan.steps).map_err(|value|OrchestratorError::Config(value.to_string()))?)
            .bind(serde_json::to_string(&plan.risks).map_err(|value|OrchestratorError::Config(value.to_string()))?)
            .bind(serde_json::to_string(&plan.allowed_paths).map_err(|value|OrchestratorError::Config(value.to_string()))?)
            .bind(&now).execute(self.store.pool()).await?;
        self.store.transition(
            &task.id,
            &[TaskStatus::Planning],
            TaskStatus::WaitingForPlanApproval,
            None,
            Actor::Agent,
            "plan:proposed",
            &json!({"plan_id":id,"version":version,"summary":plan.summary}),
        ).await?;
        Ok(())
    }

    async fn build_plan_input(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        version: i64,
        rejection: Option<&str>,
    ) -> Result<String, OrchestratorError> {
        let rules = load_rules(&project.repo).await?;
        let schema = serde_json::to_string_pretty(&plan_result_schema())
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        Ok(format!(
            "# AgentFlow 编码前计划 TASK-{} v{}\n\n\
             你处于只读规划阶段，禁止修改、创建或删除项目文件。先检查仓库结构和现有实现，再拟定可执行计划。\n\n\
             ## 需求\n\n{}\n\n{}\n\n\
             ## 项目规则\n\n{}\n\n\
             ## 输出要求\n\n只输出符合 schema 的 JSON；task_id=`{}`，plan_version={}。每个步骤必须说明改什么以及如何验证。`allowed_paths` 必须列出实现允许修改的仓库相对路径 glob（例如 `src/**`、`package.json`），不能为空。\n\n```json\n{}\n```\n",
            task.seq,
            version,
            task.description,
            rejection.map(|value| format!("## 上次驳回理由\n\n{value}")).unwrap_or_default(),
            rules,
            task.id,
            version,
            schema,
        ))
    }

    pub async fn task_plan_approve(
        &self,
        task_id: &str,
        plan_id: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::WaitingForPlanApproval {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let row = sqlx::query(
            "SELECT version,summary,steps_json,risks_json,allowed_paths_json FROM task_plans \
             WHERE id=? AND task_id=? AND status='pending'",
        )
        .bind(plan_id)
        .bind(task_id)
        .fetch_optional(self.store.pool())
        .await?
        .ok_or_else(|| OrchestratorError::InvalidState("PLAN_APPROVAL_REQUIRED".into()))?;
        let allowed_paths: Vec<String> = serde_json::from_str(&row.get::<String, _>("allowed_paths_json"))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        compile_allowed_paths(&allowed_paths)?;
        let steps: Value = serde_json::from_str(&row.get::<String, _>("steps_json"))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let risks: Value = serde_json::from_str(&row.get::<String, _>("risks_json"))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let payload = plan_payload(
            plan_id,
            row.get("version"),
            &row.get::<String, _>("summary"),
            &steps,
            &risks,
            &allowed_paths,
        );
        let (plan_sha, _) = hash_plan(&payload)?;
        let changed = sqlx::query("UPDATE task_plans SET status='approved',approved_at=?,plan_sha256=? WHERE id=? AND task_id=? AND status='pending'")
            .bind(Utc::now().to_rfc3339()).bind(&plan_sha).bind(plan_id).bind(task_id)
            .execute(self.store.pool()).await?;
        if changed.rows_affected() != 1 {
            return Err(OrchestratorError::InvalidState("PLAN_APPROVAL_REQUIRED".into()));
        }
        self.store.transition(
            task_id,
            &[TaskStatus::WaitingForPlanApproval],
            TaskStatus::ReadyForDevelopment,
            None,
            Actor::Human,
            "human:plan_approve",
            &json!({"plan_id":plan_id,"plan_sha256":plan_sha}),
        ).await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }

    pub async fn task_plan_reject(
        &self,
        task_id: &str,
        plan_id: &str,
        reason: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        if reason.trim().is_empty() {
            return Err(OrchestratorError::InvalidState("plan rejection reason is required".into()));
        }
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::WaitingForPlanApproval {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let changed = sqlx::query("UPDATE task_plans SET status='rejected',rejection_reason=? WHERE id=? AND task_id=? AND status='pending'")
            .bind(reason.trim()).bind(plan_id).bind(task_id).execute(self.store.pool()).await?;
        if changed.rows_affected() != 1 {
            return Err(OrchestratorError::InvalidState("PLAN_APPROVAL_REQUIRED".into()));
        }
        self.store.transition(
            task_id,
            &[TaskStatus::WaitingForPlanApproval],
            TaskStatus::Planning,
            None,
            Actor::Human,
            "human:plan_reject",
            &json!({"plan_id":plan_id,"reason":reason.trim()}),
        ).await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
}
