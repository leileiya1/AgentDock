impl Orchestrator {
    /// Owner-only startup repair. Every per-task step is isolated: a single damaged task is
    /// quarantined into BLOCKED(recovery_failed) instead of aborting open(), because one corrupt
    /// row or externally mutated worktree must never brick the daemon for every project.
    async fn recover_interrupted_runs(&self) -> Result<(), OrchestratorError> {
        let start_rows = sqlx::query(
            "SELECT id,task_id,payload_json FROM task_operations WHERE kind='task_start' AND status='RUNNING' ORDER BY created_at",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in start_rows {
            let operation_id: String = row.get("id");
            let task_id: String = row.get("task_id");
            let payload: String = row.get("payload_json");
            if let Err(error) = self
                .recover_start_operation(&operation_id, &task_id, &payload)
                .await
            {
                self.quarantine_recovery_failure(&task_id, "task_start", &error)
                    .await;
            }
        }
        let run_rows = sqlx::query(
            "SELECT id,task_id,revision,role,run_dir FROM agent_runs WHERE status='RUNNING'",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in run_rows {
            let task_id: String = row.get("task_id");
            if let Err(error) = self.recover_interrupted_run(&row).await {
                self.quarantine_recovery_failure(&task_id, "interrupted_run", &error)
                    .await;
            }
        }
        // Covers the crash window after a durable state transition but before an agent_runs row
        // or final transition was written. Run this before the missing-worktree check because a
        // completed merge may have intentionally removed its worktree just before a crash.
        let stage_rows = sqlx::query(
            "SELECT id,status FROM tasks t WHERE t.deleted_at IS NULL \
             AND t.status IN ('DEVELOPING','REVISING','REVIEWING','MERGING') \
             AND NOT EXISTS (SELECT 1 FROM agent_runs r WHERE r.task_id=t.id AND r.status='RUNNING')",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in stage_rows {
            let task_id: String = row.get("id");
            let recovered = match parse::<TaskStatus>(row.get("status")) {
                Ok(status) => self.recover_orphaned_stage(&task_id, status).await,
                Err(error) => Err(error),
            };
            if let Err(error) = recovered {
                self.quarantine_recovery_failure(&task_id, "orphaned_stage", &error)
                    .await;
            }
        }
        self.recover_provider_dispatches().await?;
        let active = sqlx::query(
            "SELECT id,status,worktree_path FROM tasks WHERE status NOT IN ('DRAFT','MERGED','ROLLED_BACK','CANCELLED')",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in active {
            let path: Option<String> = row.get("worktree_path");
            if path.as_deref().is_some_and(|p| Path::new(p).exists()) {
                continue;
            }
            let task_id: String = row.get("id");
            if let Err(error) = self.block_missing_worktree(&task_id, row.get("status")).await {
                self.quarantine_recovery_failure(&task_id, "worktree_missing", &error)
                    .await;
            }
        }
        Ok(())
    }

    async fn recover_start_operation(
        &self,
        operation_id: &str,
        task_id: &str,
        payload: &str,
    ) -> Result<(), OrchestratorError> {
        let intent: StartTaskIntent = serde_json::from_str(payload)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        self.continue_start_operation(task_id, operation_id, &intent)
            .await?;
        sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'system','recovery:saga_completed',?,?)")
            .bind(task_id)
            .bind(json!({"operation_id":operation_id,"kind":"task_start"}).to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool()).await?;
        Ok(())
    }

    async fn recover_interrupted_run(
        &self,
        row: &sqlx::sqlite::SqliteRow,
    ) -> Result<(), OrchestratorError> {
        let run_id: String = row.get("id");
        let task_id: String = row.get("task_id");
        let revision: i64 = row.get("revision");
        let role: String = row.get("role");
        let run_dir: String = row.get("run_dir");
        let lease_path = Path::new(&run_dir).join("process-lease.json");
        let lease = agentflow_process_supervisor::read_process_lease(&lease_path)
            .await
            .ok();
        let live_pid = lease.as_ref().and_then(|lease| {
            (agentflow_process_supervisor::inspect_process_lease(lease)
                == agentflow_process_supervisor::LeaseState::Alive)
                .then_some(lease.pid)
        });
        let completed_cleanly = agentflow_process_supervisor::read_process_exit_code(
            &Path::new(&run_dir).join("process-outcome.json"),
        )
        .await
        .is_ok_and(|code| code == 0);
        if live_pid.is_some() || completed_cleanly {
            // A crash is different from an explicit cancellation: the Provider owns durable
            // stdout/stderr descriptors and a child-side exit marker, so the new daemon can
            // adopt it without discarding already-paid work.
            let now = Utc::now().to_rfc3339();
            sqlx::query("UPDATE agent_runs SET recovery_state='ADOPTING',adopted_at=? WHERE id=? AND status='RUNNING'")
                .bind(&now)
                .bind(&run_id)
                .execute(self.store.pool())
                .await?;
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'system','recovery:run_adopted',?,?)")
                .bind(&task_id)
                .bind(revision)
                .bind(json!({"run_id":run_id,"pid":live_pid,"role":role,"already_exited":completed_cleanly && live_pid.is_none()}).to_string())
                .bind(&now)
                .execute(self.store.pool())
                .await?;
            return Ok(());
        }
        let process_recovery = match agentflow_process_supervisor::read_process_lease(&lease_path)
            .await
        {
            Ok(lease) => match agentflow_process_supervisor::inspect_process_lease(&lease) {
                agentflow_process_supervisor::LeaseState::Alive => "live_process_race",
                agentflow_process_supervisor::LeaseState::Exited => "orphan_process_already_exited",
                agentflow_process_supervisor::LeaseState::PidReused => {
                    // Never signal a recycled PID: it may now belong to an unrelated app.
                    "pid_reused_not_signaled"
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "lease_missing",
            Err(_) => "lease_invalid_not_signaled",
        };
        let _ = tokio::fs::remove_file(&lease_path).await;
        sqlx::query("UPDATE agent_runs SET status='INTERRUPTED',finished_at=? WHERE id=?")
            .bind(Utc::now().to_rfc3339())
            .bind(&run_id)
            .execute(self.store.pool())
            .await?;
        let current = self.task(&task_id).await?;
        if matches!(
            current.status,
            TaskStatus::Cancelled | TaskStatus::Merged | TaskStatus::RolledBack
        ) {
            return Ok(());
        }
        // Preserve any residual edits before rolling scheduler state back. Repair Center can
        // later keep them or reset to the recorded commit without guessing what survived.
        if current.worktree_path.as_ref().is_some_and(|path| path.is_dir()) {
            let _ = self.create_checkpoint(&current, "interrupted-run").await;
            let worktree = required_path(&current.worktree_path)?;
            let reset_to = match role.as_str() {
                "developer" => sqlx::query_scalar::<_, Option<String>>(
                    "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision<? ORDER BY revision DESC LIMIT 1",
                )
                .bind(&task_id)
                .bind(revision)
                .fetch_optional(self.store.pool())
                .await?
                .flatten()
                .or_else(|| current.base_commit.clone()),
                "reviewer" => Some(self.revision_commit_sha(&task_id, current.revision).await?),
                _ => self.git.resolve(&worktree, "HEAD").await.ok(),
            };
            if let Some(commit) = reset_to {
                self.git.reset_owned_worktree(&worktree, &commit).await?;
            }
        }
        let (to, new_revision) = match role.as_str() {
            "planner" => (TaskStatus::Planning, current.revision),
            "developer" => (
                if revision <= 1 {
                    TaskStatus::ReadyForDevelopment
                } else {
                    TaskStatus::ReadyForRevision
                },
                revision.saturating_sub(1),
            ),
            "validator" => (TaskStatus::Validating, current.revision),
            _ => (TaskStatus::ReadyForReview, current.revision),
        };
        sqlx::query("UPDATE tasks SET current_revision=? WHERE id=?")
            .bind(new_revision)
            .bind(&task_id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                &task_id,
                &[current.status],
                to,
                None,
                Actor::System,
                "recovery:interrupted",
                &json!({"run_id":run_id,"process_recovery":process_recovery}),
            )
            .await?;
        Ok(())
    }

    async fn recover_orphaned_stage(
        &self,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<(), OrchestratorError> {
        let task = self.task(task_id).await?;
        match status {
            TaskStatus::Developing | TaskStatus::Revising => {
                let revision_sha: Option<String> = sqlx::query_scalar(
                    "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
                )
                .bind(task_id)
                .bind(task.revision)
                .fetch_optional(self.store.pool())
                .await?
                .flatten();
                if let Some(commit_sha) = revision_sha {
                    // The commit and revision row are the durable completion point. If only
                    // the final state transition was interrupted, roll forward to validation.
                    let worktree = required_path(&task.worktree_path)?;
                    if self.git.resolve(&worktree, "HEAD").await? != commit_sha {
                        return Err(OrchestratorError::InvalidState(
                            "orphaned development revision does not match worktree HEAD".into(),
                        ));
                    }
                    self.store
                        .transition(
                            task_id,
                            &[status],
                            TaskStatus::Validating,
                            None,
                            Actor::System,
                            "recovery:stage_roll_forward",
                            &json!({"commit_sha":commit_sha,"revision":task.revision}),
                        )
                        .await?;
                } else {
                    if task.worktree_path.as_ref().is_some_and(|path| path.is_dir()) {
                        let _ = self.create_checkpoint(&task, "orphaned-stage").await;
                        let worktree = required_path(&task.worktree_path)?;
                        let head = self.git.resolve(&worktree, "HEAD").await?;
                        self.git.reset_owned_worktree(&worktree, &head).await?;
                    }
                    let to = if status == TaskStatus::Developing {
                        TaskStatus::ReadyForDevelopment
                    } else {
                        TaskStatus::ReadyForRevision
                    };
                    sqlx::query("UPDATE tasks SET current_revision=? WHERE id=?")
                        .bind(task.revision.saturating_sub(1))
                        .bind(task_id)
                        .execute(self.store.pool())
                        .await?;
                    self.store
                        .transition(
                            task_id,
                            &[status],
                            to,
                            None,
                            Actor::System,
                            "recovery:orphaned_stage",
                            &json!({"revision":task.revision}),
                        )
                        .await?;
                }
                self.complete_development_operations(task_id).await?;
            }
            TaskStatus::Reviewing => {
                if task.worktree_path.as_ref().is_some_and(|path| path.is_dir()) {
                    let _ = self.create_checkpoint(&task, "orphaned-review").await;
                    let sha = self.revision_commit_sha(task_id, task.revision).await?;
                    self.git
                        .reset_owned_worktree(&required_path(&task.worktree_path)?, &sha)
                        .await?;
                }
                self.store
                    .transition(
                        task_id,
                        &[TaskStatus::Reviewing],
                        TaskStatus::ReadyForReview,
                        None,
                        Actor::System,
                        "recovery:orphaned_stage",
                        &json!({"revision":task.revision}),
                    )
                    .await?;
            }
            TaskStatus::Merging => {
                let project = self.project(&task.project_id).await?;
                let seal = self.approval_seal(&task).await?;
                let head = self.git.resolve(&project.repo, "HEAD").await?;
                if self
                    .git
                    .is_ancestor(&project.repo, &seal.commit_sha, &head)
                    .await?
                {
                    self.store
                        .transition(
                            task_id,
                            &[TaskStatus::Merging],
                            TaskStatus::Merged,
                            None,
                            Actor::System,
                            "recovery:merge_roll_forward",
                            &json!({"merge_commit":head,"approved_commit":seal.commit_sha}),
                        )
                        .await?;
                    if let Some(worktree) = task.worktree_path.as_ref() {
                        let _ = self.git.worktree_remove(&project.repo, worktree).await;
                    }
                    sqlx::query("UPDATE delivery_records SET state='merged',ci_status='passed',merge_commit=?,updated_at=? WHERE task_id=?")
                        .bind(&head).bind(Utc::now().to_rfc3339()).bind(task_id)
                        .execute(self.store.pool()).await?;
                } else {
                    let _ = self.git.abort_merge(&project.repo).await;
                    self.store
                        .transition(
                            task_id,
                            &[TaskStatus::Merging],
                            TaskStatus::Approved,
                            None,
                            Actor::System,
                            "recovery:merge_retry",
                            &json!({}),
                        )
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn block_missing_worktree(
        &self,
        task_id: &str,
        status: String,
    ) -> Result<(), OrchestratorError> {
        let status: TaskStatus = parse(status)?;
        sqlx::query("UPDATE tasks SET repair_resume_status=? WHERE id=?")
            .bind(status.to_string())
            .bind(task_id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                task_id,
                &[status],
                TaskStatus::Blocked,
                Some(BlockedReason::WorktreeMissing),
                Actor::System,
                "recovery:worktree_missing",
                &json!({}),
            )
            .await?;
        Ok(())
    }

    /// Best-effort quarantine: never returns an error, because it runs inside the startup path
    /// that must stay available. Residual edits are checkpointed first so nothing is lost.
    async fn quarantine_recovery_failure(
        &self,
        task_id: &str,
        stage: &str,
        error: &OrchestratorError,
    ) {
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'system','recovery:task_failed',?,?)")
            .bind(task_id)
            .bind(json!({"stage":stage,"error":error.to_string()}).to_string())
            .bind(&now)
            .execute(self.store.pool())
            .await;
        let Ok(task) = self.task(task_id).await else {
            return;
        };
        if matches!(
            task.status,
            TaskStatus::Cancelled | TaskStatus::Merged | TaskStatus::RolledBack | TaskStatus::Blocked
        ) {
            return;
        }
        if task.worktree_path.as_ref().is_some_and(|path| path.is_dir()) {
            let _ = self.create_checkpoint(&task, "recovery-failed").await;
        }
        let _ = sqlx::query("UPDATE tasks SET repair_resume_status=?,blocked_detail=? WHERE id=?")
            .bind(task.status.to_string())
            .bind(format!("恢复失败（{stage}）：{error}"))
            .bind(task_id)
            .execute(self.store.pool())
            .await;
        let _ = self
            .store
            .transition(
                task_id,
                &[task.status],
                TaskStatus::Blocked,
                Some(BlockedReason::RecoveryFailed),
                Actor::System,
                "recovery:task_failed",
                &json!({"stage":stage,"error":error.to_string()}),
            )
            .await;
    }
}
