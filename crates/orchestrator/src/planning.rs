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
        // Filter the fallback chain to providers that can actually run before spawning anything, so
        // an uninstalled, not-logged-in or protocol-incompatible provider is never invoked (P0-02).
        // Skipped providers are recorded up front, so the final failure names every root cause
        // instead of only the last provider that happened to be tried.
        let mut catalog = self.provider_list().await;
        self.apply_runtime_probes(&mut catalog, &chain).await;
        let assessment = Self::assess_chain(&chain, &catalog);
        let mut attempts = assessment.skipped_reasons();
        if assessment.ready.is_empty() {
            self.block(
                &task,
                BlockedReason::RunFailed,
                &planner_failure_detail(&attempts),
            )
            .await?;
            return Ok(());
        }
        let mut plan = None;
        let mut previous: Option<AgentKind> = None;
        let mut previous_error = String::new();
        // Classify the last planner attempt (idle hang §15 / auth expiry §5 / normal) so the final
        // block can surface the matching recovery instead of a generic run failure.
        let mut last_attempt_class = RunFailureClass::Normal;
        for candidate in assessment.ready {
            if let Some(from) = previous.clone() {
                self.record_provider_fallback(
                    &task,
                    RunRole::Planner,
                    from,
                    candidate.clone(),
                    &previous_error,
                )
                .await?;
            }
            let candidate_name = provider_display_name(&candidate, &catalog);
            let adapter = self.adapter(candidate.clone(), &project);
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
                    last_attempt_class = if value.outcome.idle_timed_out {
                        RunFailureClass::Unresponsive
                    } else if run_output_indicates_auth_failure(&value.run_dir).await {
                        RunFailureClass::AuthExpired
                    } else {
                        RunFailureClass::Normal
                    };
                    attempts.push(format!(
                        "{candidate_name}：规划进程退出码 {:?}{}",
                        value.outcome.exit_code,
                        if value.outcome.idle_timed_out {
                            "，且没有任何输出（判定为无响应）"
                        } else {
                            ""
                        }
                    ));
                    previous = Some(candidate);
                    previous_error = attempts.last().cloned().unwrap_or_default();
                    continue;
                }
                Err(value) => {
                    last_attempt_class = if adapter_error_is_auth(&value) {
                        RunFailureClass::AuthExpired
                    } else {
                        RunFailureClass::Normal
                    };
                    attempts.push(format!("{candidate_name}：{value}"));
                    previous = Some(candidate);
                    previous_error = attempts.last().cloned().unwrap_or_default();
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
                Ok(_) => attempts.push(format!("{candidate_name}：计划结果与任务标识不匹配")),
                Err(value) => attempts.push(format!("{candidate_name}：{value}")),
            }
            previous = Some(candidate);
            previous_error = attempts.last().cloned().unwrap_or_default();
            self.invalidate_agent_run(&running.run_dir).await?;
        }
        let Some(plan) = plan else {
            self.block(
                &task,
                run_failure_reason(last_attempt_class, BlockedReason::RunFailed),
                &planner_failure_detail(&attempts),
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
        let acceptance = self.acceptance_criteria_markdown(&task.id).await?;
        let schema = serde_json::to_string_pretty(&plan_result_schema())
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        Ok(format!(
            "# AgentFlow 编码前计划 TASK-{} v{}\n\n\
             你处于只读规划阶段，禁止修改、创建或删除项目文件。先检查仓库结构和现有实现，再拟定可执行计划。\n\n\
             ## 需求\n\n{}\n\n## 结构化验收条件\n\n{}\n\n{}\n\n\
             ## 项目规则\n\n{}\n\n\
             ## 输出要求\n\n只输出符合 schema 的 JSON；task_id=`{}`，plan_version={}。每个步骤必须说明改什么以及如何验证。`allowed_paths` 必须列出实现允许修改的仓库相对路径 glob（例如 `src/**`、`package.json`），不能为空。\n\n```json\n{}\n```\n",
            task.seq,
            version,
            task.description,
            acceptance,
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

/// Prefer the catalog's display name so the failure card reads like the Provider page ("Claude
/// Code"), falling back to the stable id for providers the catalog does not surface.
fn provider_display_name(kind: &AgentKind, catalog: &[ProviderDescriptor]) -> String {
    catalog
        .iter()
        .find(|item| &item.id == kind)
        .map(|item| item.display_name.clone())
        .unwrap_or_else(|| kind.to_string())
}

/// Aggregate every skipped-provider reason and every attempt failure into one detail string, so
/// the blocked task explains all root causes at once instead of only the last provider tried.
fn planner_failure_detail(attempts: &[String]) -> String {
    if attempts.is_empty() {
        "没有可用于规划的 Provider，请先在设置中登录或安装至少一个开发 CLI。".into()
    } else {
        format!("没有 Provider 能完成规划：{}", attempts.join("；"))
    }
}
