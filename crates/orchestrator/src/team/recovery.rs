use super::*;
impl Orchestrator {
    /// Owner startup only. Lease expiry alone never releases a slot or authorizes publication.
    pub(crate) async fn team_recover(&self) -> Result<(), OrchestratorError> {
        let rows=sqlx::query("SELECT r.id,r.task_id,r.agent,r.run_dir,c.* FROM agent_runs r JOIN team_attempt_contexts c ON c.run_id=r.id WHERE r.status='RUNNING' ORDER BY r.created_at").fetch_all(self.store.pool()).await?;
        for row in rows {
            let claim = TeamClaim {
                identity: decode(row.get("identity_json"))?,
                spec: decode(row.get("spec_json"))?,
                input: decode(row.get("input_json"))?,
                provider: row
                    .get::<Option<String>, _>("agent")
                    .map(parse)
                    .transpose()?,
                run_dir: PathBuf::from(row.get::<String, _>("run_dir")),
            };
            self.team_reconcile_claim(&claim, &row.get::<String, _>("start_intent"), false)
                .await?;
        }
        // Reconcile Git side effects before dropping resource locks.
        let operations=sqlx::query("SELECT id,task_id,kind,payload_json FROM task_operations WHERE kind IN ('team_integrate','team_deliver') AND status='RUNNING'").fetch_all(self.store.pool()).await?;
        for operation in operations {
            let id: String = operation.get("id");
            let task: String = operation.get("task_id");
            let kind: String = operation.get("kind");
            if kind == "team_deliver" {
                let project = self.team_project(&task).await?;
                self.git.team_recover_head_lock(&project.repo, &id).await?;
                self.team_reconcile_delivery(&task, &id).await?;
            } else {
                // Candidate publication is transactional. Ref movement without a database pointer
                // is reconciled by the next fresh integration (never reuses an old passing seal).
                sqlx::query("UPDATE task_operations SET status='FAILED',phase='reconciled',last_error='interrupted integration; immutable candidate/history retained',updated_at=? WHERE id=?").bind(Utc::now().to_rfc3339()).bind(&id).execute(self.store.pool()).await?;
                sqlx::query("DELETE FROM team_project_locks WHERE operation_id=?")
                    .bind(&id)
                    .execute(self.store.pool())
                    .await?;
            }
        }
        Ok(())
    }
    pub(super) async fn team_deliver(&self, task_id: &str) -> Result<(), OrchestratorError> {
        let key = format!("delivery/{task_id}");
        {
            let mut active = self
                .active_cancellations
                .write()
                .map_err(|_| err("operation guard poisoned"))?;
            if active.contains_key(&key) {
                return Err(err(
                    "delivery operation still active; do not retry a live side effect",
                ));
            }
            active.insert(key.clone(), CancellationToken::new());
        }
        let result = self.team_deliver_inner(task_id).await;
        self.unregister_cancellation(&key);
        result
    }
    pub(super) async fn team_deliver_inner(&self, task_id: &str) -> Result<(), OrchestratorError> {
        let existing=sqlx::query("SELECT id,status FROM task_operations WHERE task_id=? AND kind='team_deliver' ORDER BY created_at DESC LIMIT 1").bind(task_id).fetch_optional(self.store.pool()).await?;
        if let Some(row) = existing {
            let status: String = row.get("status");
            let id: String = row.get("id");
            if status == "RUNNING" {
                return self.team_reconcile_delivery(task_id, &id).await;
            }
            if status == "COMPLETED" {
                return Ok(());
            }
        }
        self.team_assert_gate(task_id).await?;
        let view = self
            .team_get(task_id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        if view.phase != TeamPhase::ReadyForDelivery {
            return Err(err("task has not reached ready_for_delivery"));
        }
        let candidate = view
            .candidate
            .as_ref()
            .ok_or_else(|| err("candidate missing"))?;
        let task = self.task(task_id).await?;
        let project = self.team_project(task_id).await?;
        let target = self.git.resolve(&project.repo, &task.target_branch).await?;
        if target != candidate.base_sha {
            return Err(err(
                "delivery target branch changed; rebase/reintegrate and rerun all required checks before delivery",
            ));
        }
        let operation_id = Uuid::now_v7().to_string();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("INSERT INTO team_project_locks(project_id,operation_id,task_id,kind) VALUES(?,?,?,'deliver')").bind(&project.id).bind(&operation_id).bind(task_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO task_operations(id,task_id,kind,phase,status,payload_json,created_at,updated_at) VALUES(?,?,'team_deliver','intent','RUNNING',?,?,?)")
            .bind(&operation_id).bind(task_id).bind(json!({"candidate":candidate,"targetBranch":task.target_branch,"targetBaseSha":target,"planRevision":view.plan_revision,"configHash":hash(&view.config)?,"evidenceIds":view.criteria.iter().flat_map(|c|c.evidence.iter().filter(|e|e.candidate_id==candidate.id).map(|e|&e.id)).collect::<Vec<_>>()} ).to_string()).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
        tx.commit().await?;
        let branch = task
            .branch
            .as_ref()
            .ok_or_else(|| err("task branch missing"))?;
        let old = self
            .git
            .resolve(&project.repo, branch)
            .await
            .unwrap_or_else(|_| "0000000000000000000000000000000000000000".into());
        if old != "0000000000000000000000000000000000000000" && old != candidate.commit_sha {
            let known = self.store.pool();
            let receipts = sqlx::query_scalar::<_, String>(
                "SELECT candidate_json FROM team_candidates WHERE task_id=?",
            )
            .bind(task_id)
            .fetch_all(known)
            .await?;
            if !receipts
                .into_iter()
                .filter_map(|s| decode::<TeamCandidate>(s).ok())
                .any(|c| c.commit_sha == old)
            {
                return Err(err(
                    "team delivery branch moved outside recorded candidates; inspect actual state",
                ));
            }
        }
        if old != candidate.commit_sha {
            self.git
                .team_update_ref(
                    &project.repo,
                    &format!("refs/heads/{branch}"),
                    &candidate.commit_sha,
                    &old,
                )
                .await?;
        }
        // Reuse the existing diff approval seal and local merge path. The team gate is also
        // enforced inside those legacy public entrypoints so callers cannot bypass it.
        if task.status == TaskStatus::WaitingForHumanApproval {
            let diff = self.diff_get(task_id, task.revision).await?;
            self.approve(
                task_id,
                task.revision,
                &candidate.commit_sha,
                &diff.diff_sha256,
            )
            .await?;
        }
        sqlx::query("UPDATE team_tasks SET phase='delivering' WHERE task_id=? AND phase='ready_for_delivery'").bind(task_id).execute(self.store.pool()).await?;
        let merged = self.merge(task_id).await;
        match merged {
            Ok(_) => self.team_reconcile_delivery(task_id, &operation_id).await,
            Err(e) => {
                // Unknown is intentionally left RUNNING until real Git state is reconciled.
                sqlx::query(
                    "UPDATE task_operations SET phase='result_unknown',last_error=? WHERE id=?",
                )
                .bind(e.to_string())
                .bind(&operation_id)
                .execute(self.store.pool())
                .await?;
                Err(e)
            }
        }
    }
    pub(super) async fn team_reconcile_delivery(
        &self,
        task_id: &str,
        op: &str,
    ) -> Result<(), OrchestratorError> {
        let payload:String=sqlx::query_scalar("SELECT payload_json FROM task_operations WHERE id=? AND task_id=? AND kind='team_deliver'").bind(op).bind(task_id).fetch_one(self.store.pool()).await?;
        let payload: Value = decode(payload)?;
        let candidate: TeamCandidate =
            serde_json::from_value(payload["candidate"].clone()).map_err(|e| err(e.to_string()))?;
        let task = self.task(task_id).await?;
        let project = self.team_project(task_id).await?;
        let actual = self.git.resolve(&project.repo, &task.target_branch).await?;
        let tree = self
            .git
            .resolve(&project.repo, &format!("{}^{{tree}}", task.target_branch))
            .await?;
        if tree == candidate.tree_sha
            && self.git.is_clean(&project.repo).await?
            && self
                .git
                .is_ancestor(&project.repo, &candidate.commit_sha, &actual)
                .await?
        {
            let mut tx = self.store.pool().begin().await?;
            sqlx::query("UPDATE team_tasks SET phase='delivered',detail='local delivery reconciled against tested tree',updated_at=? WHERE task_id=?").bind(Utc::now().to_rfc3339()).bind(task_id).execute(&mut *tx).await?;
            sqlx::query("UPDATE tasks SET status='MERGED',updated_at=? WHERE id=?")
                .bind(Utc::now().to_rfc3339())
                .bind(task_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE task_operations SET status='COMPLETED',phase='reconciled',updated_at=? WHERE id=?").bind(Utc::now().to_rfc3339()).bind(op).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM team_project_locks WHERE operation_id=?")
                .bind(op)
                .execute(&mut *tx)
                .await?;
            event(
                &mut tx,
                task_id,
                "delivered",
                json!({"candidateId":candidate.id,"testedTree":tree,"actualHead":actual}),
            )
            .await?;
            tx.commit().await?;
            if let Ok(summary) = self.store.task_summary(task_id).await {
                self.notify_task(&summary).await;
            }
            return Ok(());
        }
        if actual == candidate.base_sha {
            // Proven not applied; do not blindly repeat inside recovery. A subsequent explicit
            // delivery request may create a new intent after checking the complete seal again.
            let mut tx = self.store.pool().begin().await?;
            sqlx::query("UPDATE task_operations SET status='FAILED',phase='not_applied',updated_at=? WHERE id=?").bind(Utc::now().to_rfc3339()).bind(op).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM team_project_locks WHERE operation_id=?")
                .bind(op)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE team_tasks SET phase='ready_for_delivery',detail='delivery was not applied; real target reconciled' WHERE task_id=?").bind(task_id).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(());
        }
        Err(err(
            "delivery result unknown: target differs from both expected base and tested tree; operation/lock retained for reconciliation",
        ))
    }
}
