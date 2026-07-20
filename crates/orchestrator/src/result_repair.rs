struct RepairedResult {
    value: CollectedResult,
    run_dir: PathBuf,
}

impl Orchestrator {
    /// Give the same Provider one bounded, read-only chance to correct only its contract output.
    /// This is intentionally not a general task retry: code changes remain untouched and a second
    /// invalid response immediately falls through to the configured Provider fallback chain.
    async fn attempt_result_repair(
        &self,
        adapter: &dyn AgentAdapter,
        task: &TaskRow,
        project: &ProjectRow,
        role: RunRole,
        config: &ProjectConfig,
        rejection: &str,
    ) -> Result<Option<RepairedResult>, OrchestratorError> {
        if matches!(adapter.kind(), AgentKind::External(_)) {
            // Protocol 1.0 has no enforceable read-only repair permission. Do not risk allowing an
            // older third-party sidecar to mutate a completed worktree during schema recovery.
            return Ok(None);
        }
        let schema = match role {
            RunRole::Reviewer => review_result_schema(),
            RunRole::Planner => plan_result_schema(),
            _ => development_result_schema(),
        };
        let commit_sha = if role == RunRole::Reviewer {
            sqlx::query_scalar::<_, String>(
                "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
            )
            .bind(&task.id)
            .bind(task.revision)
            .fetch_optional(self.store.pool())
            .await?
        } else {
            None
        };
        let rejection = agentflow_process_supervisor::redact(
            rejection.chars().take(2_000).collect::<String>(),
        );
        let prompt = format!(
            "# AgentFlow 结构化结果修复\n\n\
             上一次任务执行已经结束，但结果契约未通过校验。只修复结果，不重新开发。\n\n\
             - 禁止修改、创建或删除任何项目文件。\n\
             - 可只读检查当前 worktree、原始任务输入和 Git diff。\n\
             - 最终只返回一个符合下方 schema 的 JSON 对象，不要 Markdown 代码块。\n\
             - task_id 必须是 `{}`，revision 必须是 `{}`。\n\
             {}\n\
             上次拒绝原因：{}\n\n## JSON Schema\n\n{}\n",
            task.id,
            task.revision,
            commit_sha
                .as_deref()
                .map(|sha| format!("- commit_sha 必须绑定当前提交 `{sha}`。"))
                .unwrap_or_default(),
            rejection,
            serde_json::to_string_pretty(&schema)
                .map_err(|error| OrchestratorError::Config(error.to_string()))?,
        );
        let wt = required_path(&task.worktree_path)?;
        let input_name = ".agentflow-in/result-repair.md";
        tokio::fs::write(wt.join(input_name), prompt).await?;
        self.record_result_repair(task, role, adapter.kind(), "started", rejection.as_str())
            .await?;

        let run_dir = self.run_dir(&task.id);
        let running = self
            .run_agent(
                adapter,
                task,
                project,
                &run_dir,
                role,
                input_name,
                config,
                Some(PermissionTier::ReadOnly),
            )
            .await;
        let running = match running {
            Ok(running)
                if running.outcome.exit_code == Some(0)
                    && !running.outcome.timed_out
                    && !running.outcome.cancelled =>
            {
                running
            }
            Ok(running) => {
                self.record_result_repair(
                    task,
                    role,
                    adapter.kind(),
                    "failed",
                    &format!("repair exited with {:?}", running.outcome.exit_code),
                )
                .await?;
                return Ok(None);
            }
            Err(error) => {
                self.record_result_repair(
                    task,
                    role,
                    adapter.kind(),
                    "failed",
                    &error.to_string(),
                )
                .await?;
                return Ok(None);
            }
        };
        let output = wt.join(".agentflow-out/result.json");
        if output.exists() {
            tokio::fs::copy(&output, running.run_dir.join("result.json")).await?;
        }
        let collected = adapter.collect_result(&running.run_dir, role).await;
        self.protect_run_files(&running.run_dir).await?;
        match collected {
            Ok(value) => {
                self.record_result_repair(task, role, adapter.kind(), "succeeded", "")
                    .await?;
                Ok(Some(RepairedResult {
                    value,
                    run_dir: running.run_dir,
                }))
            }
            Err(error) => {
                self.invalidate_agent_run(&running.run_dir).await?;
                self.record_result_repair(
                    task,
                    role,
                    adapter.kind(),
                    "failed",
                    &error.to_string(),
                )
                .await?;
                Ok(None)
            }
        }
    }

    async fn record_result_repair(
        &self,
        task: &TaskRow,
        role: RunRole,
        agent: AgentKind,
        outcome: &str,
        detail: &str,
    ) -> Result<(), OrchestratorError> {
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator',?,?,?)")
            .bind(&task.id)
            .bind(task.revision)
            .bind(format!("result:repair_{outcome}"))
            .bind(json!({"role":role,"agent":agent,"detail":detail}).to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        Ok(())
    }
}
