#[derive(Debug, Clone, Serialize, Deserialize)]
struct StartTaskIntent {
    base_commit: String,
    branch: String,
    worktree_path: String,
    target_status: String,
}

impl Orchestrator {
    async fn begin_start_operation(
        &self,
        task: &TaskRow,
        intent: &StartTaskIntent,
    ) -> Result<(String, StartTaskIntent), OrchestratorError> {
        if let Some((id, payload)) = sqlx::query_as::<_, (String, String)>(
            "SELECT id,payload_json FROM task_operations WHERE task_id=? AND kind='task_start' AND status='RUNNING'",
        )
        .bind(&task.id)
        .fetch_optional(self.store.pool())
        .await?
        {
            let stored = serde_json::from_str(&payload)
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
            return Ok((id, stored));
        }
        let id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO task_operations(id,task_id,kind,phase,status,payload_json,created_at,updated_at) VALUES(?,?,'task_start','intent','RUNNING',?,?,?)")
            .bind(&id)
            .bind(&task.id)
            .bind(serde_json::to_string(intent).map_err(|error|OrchestratorError::Config(error.to_string()))?)
            .bind(&now)
            .bind(&now)
            .execute(self.store.pool()).await?;
        Ok((id, intent.clone()))
    }

    async fn operation_phase(
        &self,
        operation_id: &str,
        phase: &str,
    ) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE task_operations SET phase=?,updated_at=? WHERE id=? AND status='RUNNING'")
            .bind(phase)
            .bind(Utc::now().to_rfc3339())
            .bind(operation_id)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn operation_complete(&self, operation_id: &str) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE task_operations SET phase='complete',status='COMPLETED',updated_at=? WHERE id=?")
            .bind(Utc::now().to_rfc3339())
            .bind(operation_id)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn continue_start_operation(
        &self,
        task_id: &str,
        operation_id: &str,
        intent: &StartTaskIntent,
    ) -> Result<(), OrchestratorError> {
        let task = self.task(task_id).await?;
        let target: TaskStatus = parse(intent.target_status.clone())?;
        if task.status != TaskStatus::Draft {
            if task.status == target
                && task.base_commit.as_deref() == Some(&intent.base_commit)
                && task.branch.as_deref() == Some(&intent.branch)
            {
                self.operation_complete(operation_id).await?;
                return Ok(());
            }
            return Err(OrchestratorError::InvalidState(format!(
                "start saga found task in unexpected state {}",
                task.status
            )));
        }
        let project = self.project(&task.project_id).await?;
        let worktree = PathBuf::from(&intent.worktree_path);
        if let Some(parent) = worktree.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if !worktree.exists() {
            if self.git.resolve(&project.repo, &intent.branch).await.is_ok() {
                self.git
                    .worktree_add_existing(&project.repo, &worktree, &intent.branch)
                    .await?;
            } else {
                self.git
                    .worktree_add(
                        &project.repo,
                        &worktree,
                        &intent.branch,
                        &intent.base_commit,
                    )
                    .await?;
            }
        }
        self.operation_phase(operation_id, "worktree_ready").await?;
        self.git.ensure_agentflow_excluded(&project.repo).await?;
        let compatibility = self.git.compatibility_report(&project.repo).await?;
        self.git
            .prepare_linked_worktree(&worktree, &compatibility)
            .await?;

        // Metadata and workflow state become visible in one SQLite commit. Recovery can safely
        // roll the Git intent forward before this point, or merely close the operation after it.
        let now = Utc::now().to_rfc3339();
        let mut tx = self.store.pool().begin().await?;
        let changed = sqlx::query(
            "UPDATE tasks SET base_commit=?,branch=?,worktree_path=?,status=?,blocked_reason=NULL,updated_at=? WHERE id=? AND status='DRAFT'",
        )
        .bind(&intent.base_commit)
        .bind(&intent.branch)
        .bind(&intent.worktree_path)
        .bind(&intent.target_status)
        .bind(&now)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(OrchestratorError::InvalidState(
                "task start state changed while committing saga".into(),
            ));
        }
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,0,'human','user:start',?,?)")
            .bind(task_id)
            .bind(json!({"base_commit":intent.base_commit,"branch":intent.branch,"plan_required":target==TaskStatus::Planning,"operation_id":operation_id}).to_string())
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.operation_complete(operation_id).await
    }

    async fn recover_start_operations(&self) -> Result<(), OrchestratorError> {
        let rows = sqlx::query(
            "SELECT id,task_id,payload_json FROM task_operations WHERE kind='task_start' AND status='RUNNING' ORDER BY created_at",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in rows {
            let id: String = row.get("id");
            let task_id: String = row.get("task_id");
            let intent: StartTaskIntent = serde_json::from_str(&row.get::<String, _>("payload_json"))
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
            self.continue_start_operation(&task_id, &id, &intent).await?;
            sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'system','recovery:saga_completed',?,?)")
                .bind(&task_id)
                .bind(json!({"operation_id":id,"kind":"task_start"}).to_string())
                .bind(Utc::now().to_rfc3339())
                .execute(self.store.pool()).await?;
        }
        Ok(())
    }

    async fn enter_development_stage(
        &self,
        task: &TaskRow,
        from: TaskStatus,
        to: TaskStatus,
        revision: i64,
    ) -> Result<String, OrchestratorError> {
        let operation_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("INSERT INTO task_operations(id,task_id,kind,phase,status,payload_json,created_at,updated_at) VALUES(?,?,'development_stage','state_entered','RUNNING',?,?,?)")
            .bind(&operation_id).bind(&task.id)
            .bind(json!({"from":from,"to":to,"revision":revision}).to_string())
            .bind(&now).bind(&now).execute(&mut *tx).await?;
        let changed = sqlx::query("UPDATE tasks SET current_revision=?,status=?,blocked_reason=NULL,updated_at=? WHERE id=? AND status=?")
            .bind(revision).bind(to.to_string()).bind(&now).bind(&task.id).bind(from.to_string())
            .execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(OrchestratorError::InvalidState("development stage CAS failed".into()));
        }
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','scheduler:slot',?,?)")
            .bind(&task.id).bind(revision)
            .bind(json!({"revision":revision,"operation_id":operation_id}).to_string())
            .bind(&now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(operation_id)
    }

    async fn complete_development_operations(&self, task_id: &str) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE task_operations SET phase='complete',status='COMPLETED',updated_at=? WHERE task_id=? AND kind='development_stage' AND status='RUNNING'")
            .bind(Utc::now().to_rfc3339()).bind(task_id).execute(self.store.pool()).await?;
        Ok(())
    }

    async fn recover_orphaned_stages(&self) -> Result<(), OrchestratorError> {
        let rows = sqlx::query(
            "SELECT id,status FROM tasks t WHERE t.deleted_at IS NULL \
             AND t.status IN ('DEVELOPING','REVISING','REVIEWING','MERGING') \
             AND NOT EXISTS (SELECT 1 FROM agent_runs r WHERE r.task_id=t.id AND r.status='RUNNING')",
        )
        .fetch_all(self.store.pool())
        .await?;
        for row in rows {
            let task_id: String = row.get("id");
            let status: TaskStatus = parse(row.get("status"))?;
            let task = self.task(&task_id).await?;
            match status {
                TaskStatus::Developing | TaskStatus::Revising => {
                    let revision_sha: Option<String> = sqlx::query_scalar(
                        "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
                    )
                    .bind(&task_id)
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
                        self.store.transition(
                            &task_id,
                            &[status],
                            TaskStatus::Validating,
                            None,
                            Actor::System,
                            "recovery:stage_roll_forward",
                            &json!({"commit_sha":commit_sha,"revision":task.revision}),
                        ).await?;
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
                            .bind(&task_id)
                            .execute(self.store.pool()).await?;
                        self.store.transition(
                            &task_id,
                            &[status],
                            to,
                            None,
                            Actor::System,
                            "recovery:orphaned_stage",
                            &json!({"revision":task.revision}),
                        ).await?;
                    }
                    self.complete_development_operations(&task_id).await?;
                }
                TaskStatus::Reviewing => {
                    if task.worktree_path.as_ref().is_some_and(|path| path.is_dir()) {
                        let _ = self.create_checkpoint(&task, "orphaned-review").await;
                        let sha = self.revision_commit_sha(&task_id, task.revision).await?;
                        self.git
                            .reset_owned_worktree(&required_path(&task.worktree_path)?, &sha)
                            .await?;
                    }
                    self.store.transition(
                        &task_id,
                        &[TaskStatus::Reviewing],
                        TaskStatus::ReadyForReview,
                        None,
                        Actor::System,
                        "recovery:orphaned_stage",
                        &json!({"revision":task.revision}),
                    ).await?;
                }
                TaskStatus::Merging => {
                    let project = self.project(&task.project_id).await?;
                    let seal = self.approval_seal(&task).await?;
                    let head = self.git.resolve(&project.repo, "HEAD").await?;
                    if self.git.is_ancestor(&project.repo, &seal.commit_sha, &head).await? {
                        self.store.transition(
                            &task_id,
                            &[TaskStatus::Merging],
                            TaskStatus::Merged,
                            None,
                            Actor::System,
                            "recovery:merge_roll_forward",
                            &json!({"merge_commit":head,"approved_commit":seal.commit_sha}),
                        ).await?;
                        if let Some(worktree) = task.worktree_path.as_ref() {
                            let _ = self.git.worktree_remove(&project.repo, worktree).await;
                        }
                        sqlx::query("UPDATE delivery_records SET state='merged',ci_status='passed',merge_commit=?,updated_at=? WHERE task_id=?")
                            .bind(&head).bind(Utc::now().to_rfc3339()).bind(&task_id)
                            .execute(self.store.pool()).await?;
                    } else {
                        let _ = self.git.abort_merge(&project.repo).await;
                        self.store.transition(
                            &task_id,
                            &[TaskStatus::Merging],
                            TaskStatus::Approved,
                            None,
                            Actor::System,
                            "recovery:merge_retry",
                            &json!({}),
                        ).await?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
