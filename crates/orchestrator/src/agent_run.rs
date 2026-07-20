impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    async fn run_agent(
        &self,
        adapter: &dyn AgentAdapter,
        task: &TaskRow,
        project: &ProjectRow,
        run_dir: &Path,
        role: RunRole,
        input: &str,
        config: &ProjectConfig,
        permission_override: Option<PermissionTier>,
    ) -> Result<agentflow_agent_adapters::RunningAgent, OrchestratorError> {
        let worktree = required_path(&task.worktree_path)?;
        if self.git.is_repo(&worktree).await {
            self.create_checkpoint(task, &format!("before-{role}"))
                .await?;
        }
        tokio::fs::create_dir_all(run_dir).await?;
        let authoritative_input = required_path(&task.worktree_path)?.join(input);
        if authoritative_input.exists() {
            tokio::fs::copy(&authoritative_input, run_dir.join("input.md")).await?;
        }
        let run_id = run_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("run")
            .to_string();
        let now = Utc::now().to_rfc3339();
        let settings = self.settings_get().await?;
        let configured_timeout = if role == RunRole::Reviewer {
            settings.reviewer_timeout_secs.unwrap_or(900)
        } else {
            settings.developer_timeout_secs.unwrap_or(1_800)
        };
        let timeout_secs = self
            .remaining_time_budget(&task.id)
            .await?
            .map_or(configured_timeout, |remaining| configured_timeout.min(remaining));
        let idle_timeout_secs = settings.idle_timeout_secs.unwrap_or(300);
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,started_at,created_at) VALUES(?,?,?,?,?,'RUNNING',?,?,?,?,?)")
            .bind(&run_id)
            .bind(&task.id)
            .bind(task.revision)
            .bind(role.to_string())
            .bind(adapter.kind().to_string())
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(timeout_secs as i64)
            .bind(idle_timeout_secs as i64)
            .bind(&now)
            .bind(&now)
            .execute(self.store.pool())
            .await?;
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);
        let events_path = run_dir.join("agent-events.jsonl");
        let sink = tokio::spawn(async move {
            let mut file = tokio::fs::File::create(events_path).await?;
            while let Some(event) = rx.recv().await {
                let mut line = serde_json::to_vec(&event)?;
                line.push(b'\n');
                file.write_all(&line).await?;
            }
            Ok::<(), anyhow::Error>(())
        });
        // Review Providers receive the authoritative revision commit through the protocol.
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
        let resume_session_id = if project.settings.resume_sessions
            && adapter.capabilities().supports_resume
            && task.revision > 1
        {
            sqlx::query_scalar::<_, String>(
                "SELECT session_id FROM agent_runs WHERE task_id=? AND revision<? AND role=? \
                 AND agent=? AND status='SUCCEEDED' AND session_id IS NOT NULL \
                 ORDER BY revision DESC,created_at DESC LIMIT 1",
            )
            .bind(&task.id)
            .bind(task.revision)
            .bind(role.to_string())
            .bind(adapter.kind().to_string())
            .fetch_optional(self.store.pool())
            .await?
        } else {
            None
        };
        let request = AgentRunRequest {
            task_id: task.id.clone(),
            revision: task.revision,
            commit_sha,
            worktree: required_path(&task.worktree_path)?,
            run_dir: run_dir.into(),
            role,
            input_file: input.into(),
            timeout: Duration::from_secs(timeout_secs),
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            permission: permission_override.unwrap_or(if project.settings.full_access {
                PermissionTier::Yolo
            } else {
                PermissionTier::Normal
            }),
            resume_session_id,
            extra_allowed_commands: config.agents.extra_allowed_commands.clone(),
            env_denylist: project.settings.env_denylist.clone(),
        };
        let cancellation = CancellationToken::new();
        self.register_cancellation(&task.id, cancellation.clone());
        let task_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id=?")
            .bind(&task.id)
            .fetch_one(self.store.pool())
            .await?;
        if task_status == TaskStatus::Cancelled.to_string() {
            cancellation.cancel();
        }
        // Desktop and daemon use separate Orchestrator handles, so poll the authoritative DB.
        let cancellation_watcher =
            self.watch_task_cancellation(task.id.clone(), cancellation.clone());
        let lease_path = run_dir.join("process-lease.json");
        let started_future = adapter.start(request, cancellation.clone(), tx);
        tokio::pin!(started_future);
        let mut lease_recorded = false;
        let started = loop {
            tokio::select! {
                result = &mut started_future => break result,
                _ = tokio::time::sleep(Duration::from_millis(20)), if !lease_recorded => {
                    if let Ok(lease) = agentflow_process_supervisor::read_process_lease(&lease_path).await {
                        let _ = sqlx::query("UPDATE agent_runs SET child_pid=?,child_started_at=? WHERE id=? AND status='RUNNING'")
                            .bind(i64::from(lease.pid))
                            .bind(&lease.started_at)
                            .bind(&run_id)
                            .execute(self.store.pool())
                            .await;
                        lease_recorded = true;
                    }
                }
            }
        };
        cancellation_watcher.abort();
        self.unregister_cancellation(&task.id);
        let _ = sink.await;
        let running = match started {
            Ok(running) => running,
            Err(error) => {
                sqlx::query("UPDATE agent_runs SET status='FAILED',finished_at=? WHERE id=?")
                    .bind(Utc::now().to_rfc3339())
                    .bind(&run_id)
                    .execute(self.store.pool())
                    .await?;
                return Err(error.into());
            }
        };
        self.finish_agent_run(&run_id, adapter.kind(), &running)
            .await?;
        Ok(running)
    }
}
