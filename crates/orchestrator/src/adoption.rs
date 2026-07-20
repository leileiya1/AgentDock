#[derive(Debug)]
struct AdoptedRun {
    id: String,
    revision: i64,
    role: RunRole,
    agent: AgentKind,
    run_dir: PathBuf,
    timeout_secs: u64,
    idle_timeout_secs: u64,
    started_at: String,
    child_pid: Option<u32>,
}

impl Orchestrator {
    async fn adopting_run(&self, task_id: &str) -> Result<Option<AdoptedRun>, OrchestratorError> {
        let row = sqlx::query(
            "SELECT id,revision,role,agent,run_dir,timeout_secs,idle_timeout_secs,started_at,child_pid \
             FROM agent_runs WHERE task_id=? AND status='RUNNING' AND recovery_state='ADOPTING' \
             ORDER BY created_at LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(self.store.pool())
        .await?;
        row.map(|row| {
            Ok(AdoptedRun {
                id: row.get("id"),
                revision: row.get("revision"),
                role: parse(row.get("role"))?,
                agent: parse(row.get("agent"))?,
                run_dir: row.get::<String, _>("run_dir").into(),
                timeout_secs: row.get::<i64, _>("timeout_secs").max(1) as u64,
                idle_timeout_secs: row.get::<i64, _>("idle_timeout_secs").max(1) as u64,
                started_at: row
                    .get::<Option<String>, _>("started_at")
                    .unwrap_or_else(|| Utc::now().to_rfc3339()),
                child_pid: row
                    .get::<Option<i64>, _>("child_pid")
                    .and_then(|value| u32::try_from(value).ok()),
            })
        })
        .transpose()
    }

    /// Wait for a child that survived its previous daemon, reconnect its logs and consume the
    /// original structured result. No second Provider request is made on the successful path.
    async fn adopt_active_run(&self, task: &TaskRow) -> Result<bool, OrchestratorError> {
        let Some(run) = self.adopting_run(&task.id).await? else {
            return Ok(false);
        };
        let lease_path = run.run_dir.join("process-lease.json");
        let outcome_path = run.run_dir.join("process-outcome.json");
        let lease = agentflow_process_supervisor::read_process_lease(&lease_path)
            .await
            .ok();
        let started = chrono::DateTime::parse_from_rfc3339(&run.started_at)
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let deadline = started + chrono::Duration::seconds(run.timeout_secs as i64);
        let mut last_log_size = 0_u64;

        loop {
            let exit = agentflow_process_supervisor::read_process_exit_code(&outcome_path).await;
            let state = lease
                .as_ref()
                .map(agentflow_process_supervisor::inspect_process_lease)
                .unwrap_or(agentflow_process_supervisor::LeaseState::Exited);
            let current_status = self.task(&task.id).await?.status;
            if current_status == TaskStatus::Cancelled {
                if let Some(lease) = lease.as_ref() {
                    let _ = agentflow_process_supervisor::terminate_process_lease(
                        lease,
                        Duration::from_secs(2),
                    )
                    .await;
                }
                self.finish_failed_adoption(task, &run, "adopted run cancelled")
                    .await?;
                return Ok(true);
            }
            if let Ok(exit_code) = exit {
                self.rebuild_adopted_events(&run.run_dir).await?;
                return self.finish_adopted_run(task, run, exit_code).await.map(|_| true);
            }
            if state != agentflow_process_supervisor::LeaseState::Alive {
                // The child writes its marker atomically immediately after the Provider exits.
                tokio::time::sleep(Duration::from_millis(100)).await;
                if let Ok(exit_code) =
                    agentflow_process_supervisor::read_process_exit_code(&outcome_path).await
                {
                    self.rebuild_adopted_events(&run.run_dir).await?;
                    return self.finish_adopted_run(task, run, exit_code).await.map(|_| true);
                }
                self.finish_failed_adoption(task, &run, "provider exited without an outcome marker")
                    .await?;
                return Ok(true);
            }
            if Utc::now() >= deadline || log_idle_for(&run.run_dir) >= run.idle_timeout_secs {
                if let Some(lease) = lease.as_ref() {
                    let _ = agentflow_process_supervisor::terminate_process_lease(
                        lease,
                        Duration::from_secs(2),
                    )
                    .await;
                }
                self.finish_failed_adoption(task, &run, "adopted run exceeded its timeout")
                    .await?;
                return Ok(true);
            }
            let log_size = durable_log_size(&run.run_dir).await;
            if log_size != last_log_size {
                self.rebuild_adopted_events(&run.run_dir).await?;
                last_log_size = log_size;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn rebuild_adopted_events(&self, run_dir: &Path) -> Result<(), OrchestratorError> {
        let events = agentflow_process_supervisor::replay_durable_logs(
            &run_dir.join("stdout.log"),
            &run_dir.join("stderr.log"),
        )
        .await?;
        let mut bytes = Vec::new();
        for event in events {
            serde_json::to_writer(&mut bytes, &event)
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
            bytes.push(b'\n');
        }
        tokio::fs::write(run_dir.join("agent-events.jsonl"), bytes).await?;
        Ok(())
    }

    async fn finish_adopted_run(
        &self,
        task: &TaskRow,
        run: AdoptedRun,
        exit_code: i32,
    ) -> Result<(), OrchestratorError> {
        if exit_code != 0 {
            return self
                .finish_failed_adoption(task, &run, &format!("provider exited with {exit_code}"))
                .await;
        }
        let project = self.project(&task.project_id).await?;
        let adapter = self.adapter(run.agent.clone(), &project);
        let wt = required_path(&task.worktree_path)?;
        let provider_result = wt.join(".agentflow-out/result.json");
        if provider_result.exists() {
            tokio::fs::copy(&provider_result, run.run_dir.join("result.json")).await?;
        }
        let collected = adapter.collect_result(&run.run_dir, run.role).await;
        let running = agentflow_agent_adapters::RunningAgent {
            outcome: agentflow_process_supervisor::ProcessOutcome {
                pid: run.child_pid.unwrap_or(0),
                started_at: run.started_at.clone(),
                exit_code: Some(exit_code),
                timed_out: false,
                cancelled: false,
                log_truncated: false,
            },
            run_dir: run.run_dir.clone(),
            role: run.role,
        };
        let finished = self
            .finish_agent_run(&run.id, run.agent.clone(), &running)
            .await;
        let released = self.release_provider_dispatch(&run.id).await;
        finished?;
        released?;
        let result = match collected {
            Ok(value) => value,
            Err(error) => {
                self.invalidate_agent_run(&run.run_dir).await?;
                return self
                    .finish_failed_adoption(task, &run, &format!("invalid result: {error}"))
                    .await;
            }
        };
        match (run.role, result) {
            (RunRole::Planner, CollectedResult::Plan(plan))
                if plan.task_id == task.id && plan.plan_version >= 1 =>
            {
                let baseline = self.git.resolve(&wt, "HEAD").await?;
                self.finalize_plan_result(task, plan, &baseline).await?;
            }
            (RunRole::Developer, CollectedResult::Development(result))
                if result.task_id == task.id && result.revision == run.revision =>
            {
                let baseline = self.git.resolve(&wt, "HEAD").await?;
                self.finalize_development_result(task, &project, result, run.agent.clone(), &baseline)
                    .await?;
            }
            (RunRole::Reviewer, CollectedResult::Review(review)) => {
                let sha = self.revision_commit_sha(&task.id, task.revision).await?;
                let commit_matches = is_hex_commit_reference(&review.commit_sha)
                    && self
                        .git
                        .resolve(&wt, &review.commit_sha)
                        .await
                        .is_ok_and(|resolved| resolved == sha);
                if !commit_matches
                    || review.task_id != task.id
                    || review.revision != run.revision
                {
                    self.invalidate_agent_run(&run.run_dir).await?;
                    return self
                        .finish_failed_adoption(task, &run, "review result identity mismatch")
                        .await;
                }
                self.finalize_review_result(task, review, &run.run_dir, run.agent.clone(), &sha)
                    .await?;
            }
            _ => {
                self.invalidate_agent_run(&run.run_dir).await?;
                return self
                    .finish_failed_adoption(task, &run, "result role or identity mismatch")
                    .await;
            }
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE agent_runs SET recovery_state='ADOPTED' WHERE id=?")
            .bind(&run.id)
            .execute(self.store.pool())
            .await?;
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'system','recovery:run_recovered',?,?)")
            .bind(&task.id)
            .bind(run.revision)
            .bind(json!({"run_id":run.id,"agent":run.agent,"reused_result":true}).to_string())
            .bind(now)
            .execute(self.store.pool())
            .await?;
        let _ = tokio::fs::remove_file(run.run_dir.join("process-lease.json")).await;
        self.protect_run_files(&run.run_dir).await?;
        Ok(())
    }

    async fn finish_failed_adoption(
        &self,
        task: &TaskRow,
        run: &AdoptedRun,
        detail: &str,
    ) -> Result<(), OrchestratorError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE agent_runs SET status='INTERRUPTED',recovery_state='FAILED',finished_at=? WHERE id=? AND status='RUNNING'")
            .bind(&now)
            .bind(&run.id)
            .execute(self.store.pool())
            .await?;
        self.release_provider_dispatch(&run.id).await?;
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'system','recovery:run_adoption_failed',?,?)")
            .bind(&task.id)
            .bind(run.revision)
            .bind(json!({"run_id":run.id,"detail":detail}).to_string())
            .bind(&now)
            .execute(self.store.pool())
            .await?;
        if self.task(&task.id).await?.status == TaskStatus::Cancelled {
            return Ok(());
        }
        let wt = required_path(&task.worktree_path)?;
        if self.git.is_repo(&wt).await {
            let _ = self.create_checkpoint(task, "failed-adoption").await;
            let head = self.git.resolve(&wt, "HEAD").await?;
            self.git.reset_owned_worktree(&wt, &head).await?;
        }
        match run.role {
            RunRole::Planner | RunRole::Validator => Ok(()),
            RunRole::Developer => {
                let to = if run.revision <= 1 {
                    TaskStatus::ReadyForDevelopment
                } else {
                    TaskStatus::ReadyForRevision
                };
                sqlx::query("UPDATE tasks SET current_revision=? WHERE id=?")
                    .bind(run.revision.saturating_sub(1))
                    .bind(&task.id)
                    .execute(self.store.pool())
                    .await?;
                self.store
                    .transition(
                        &task.id,
                        &[TaskStatus::Developing, TaskStatus::Revising],
                        to,
                        None,
                        Actor::System,
                        "recovery:retry_required",
                        &json!({"run_id":run.id}),
                    )
                    .await?;
                Ok(())
            }
            RunRole::Reviewer => {
                self.store
                    .transition(
                        &task.id,
                        &[TaskStatus::Reviewing],
                        TaskStatus::ReadyForReview,
                        None,
                        Actor::System,
                        "recovery:retry_required",
                        &json!({"run_id":run.id}),
                    )
                    .await?;
                Ok(())
            }
        }
    }
}

async fn durable_log_size(run_dir: &Path) -> u64 {
    let stdout = tokio::fs::metadata(run_dir.join("stdout.log"))
        .await
        .map(|value| value.len())
        .unwrap_or(0);
    let stderr = tokio::fs::metadata(run_dir.join("stderr.log"))
        .await
        .map(|value| value.len())
        .unwrap_or(0);
    stdout.saturating_add(stderr)
}

fn log_idle_for(run_dir: &Path) -> u64 {
    [run_dir.join("stdout.log"), run_dir.join("stderr.log")]
        .into_iter()
        .filter_map(|path| fs::metadata(path).ok()?.modified().ok())
        .filter_map(|modified| modified.elapsed().ok())
        .map(|elapsed| elapsed.as_secs())
        .min()
        .unwrap_or(0)
}
