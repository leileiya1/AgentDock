use super::*;
impl Orchestrator {
    pub(super) async fn team_prepare_workspace(
        &self,
        claim: &TeamClaim,
    ) -> Result<(PathBuf, String, Vec<String>), OrchestratorError> {
        let task = self.task(&claim.identity.task_id).await?;
        let project = self.team_project(&task.id).await?;
        let workspace = project
            .worktree_root
            .join("teams")
            .join(&task.id)
            .join(&claim.identity.attempt_id);
        let candidate: Option<TeamCandidate> = claim
            .input
            .get("candidate")
            .filter(|v| !v.is_null())
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| err(e.to_string()))?;
        let base = candidate
            .as_ref()
            .map(|c| c.commit_sha.clone())
            .or(task.base_commit)
            .ok_or_else(|| err("input baseline missing"))?;
        tokio::fs::create_dir_all(
            workspace
                .parent()
                .ok_or_else(|| err("workspace parent missing"))?,
        )
        .await?;
        if workspace.exists() {
            return Err(err(
                "attempt workspace already exists; reconcile its startup intent before reuse",
            ));
        }
        self.git.ensure_agentflow_excluded(&project.repo).await?;
        self.git
            .worktree_add_detached(&project.repo, &workspace, &base)
            .await?;
        let mut included = candidate
            .as_ref()
            .map(|c| c.artifact_ids.clone())
            .unwrap_or_default();
        if claim.spec.kind == TeamWorkKind::Implement {
            let dependencies: Vec<TeamArtifact> = serde_json::from_value(
                claim
                    .input
                    .get("dependencyArtifacts")
                    .cloned()
                    .unwrap_or(json!([])),
            )
            .map_err(|e| err(e.to_string()))?;
            let all = self.team_artifacts(&task.id).await?;
            let ordered = ordered_artifacts(&dependencies, &all)?;
            for artifact in ordered {
                if included.contains(&artifact.id) {
                    continue;
                }
                let patch: Vec<u8> =
                    sqlx::query_scalar("SELECT patch FROM team_artifacts WHERE id=? AND task_id=?")
                        .bind(&artifact.id)
                        .bind(&task.id)
                        .fetch_one(self.store.pool())
                        .await?;
                self.git
                    .apply_team_delta(&workspace, &patch)
                    .await
                    .map_err(|e| {
                        err(format!(
                            "dependency input conflict in {}: {e}",
                            artifact.work_item_id
                        ))
                    })?;
                if self.git.has_changes(&workspace).await? {
                    self.git
                        .commit_revision(
                            &workspace,
                            task.seq,
                            0,
                            "prepare exact dependency input",
                            "runner",
                        )
                        .await?;
                }
                included.push(artifact.id.clone());
            }
        }
        let input_base = self.git.resolve(&workspace, "HEAD").await?;
        sqlx::query("UPDATE team_attempt_contexts SET worktree=?,input_base_sha=?,start_intent='prepared' WHERE run_id=? AND cancel_requested=0")
            .bind(workspace.to_string_lossy().as_ref()).bind(&input_base).bind(&claim.identity.attempt_id).execute(self.store.pool()).await?;
        Ok((workspace, input_base, included))
    }
    pub(super) async fn team_accept_output(
        &self,
        claim: &TeamClaim,
        submission: &TeamSubmission,
        workspace: &Path,
        base: &str,
        included: Vec<String>,
    ) -> Result<TeamArtifact, OrchestratorError> {
        let task = self.task(&claim.identity.task_id).await?;
        let view = self
            .team_get(&task.id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        // Include both agent-created commits and uncommitted/untracked changes.
        let head = self.git.resolve(workspace, "HEAD").await?;
        if !self.git.is_ancestor(workspace, base, &head).await? {
            return Err(err("OUTPUT_SCOPE: agent rewrote its input history"));
        }
        let mut actual = self
            .git
            .changed_paths_between(workspace, base, &head)
            .await?;
        actual.extend(self.git.changed_paths(workspace).await?);
        actual.sort();
        actual.dedup();
        let mut declared = submission.changed_files.clone();
        declared.sort();
        declared.dedup();
        if actual != declared {
            return Err(err(format!(
                "OUTPUT_SCOPE: declared paths {declared:?} differ from actual {actual:?}"
            )));
        }
        let permitted = glob_set(&view.config.write_globs)?;
        let local = glob_set(&claim.spec.write_globs)?;
        for path in &actual {
            if path.starts_with(".agentflow")
                || path.starts_with(".git")
                || !permitted.is_match(path)
                || !local.is_match(path)
            {
                return Err(err(format!(
                    "OUTPUT_SCOPE: path outside authorized product scope: {path}"
                )));
            }
        }
        for check in &claim.spec.local_checks {
            let result = self
                .team_run_check(claim, check, workspace, &format!("local-{}", check.id))
                .await?;
            if result.0 != CriterionStatus::Passed {
                return Err(err(format!("LOCAL_CHECK: {}: {}", check.id, result.1)));
            }
        }
        let result = if self.git.has_changes(workspace).await? {
            self.git
                .commit_revision(
                    workspace,
                    task.seq,
                    i64::from(claim.identity.lease_epoch),
                    &claim.spec.title,
                    &claim
                        .provider
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                )
                .await?
        } else {
            head
        };
        self.git
            .validate_commit_range(workspace, base, &result)
            .await?;
        let patch = self.git.team_binary_patch(workspace, base, &result).await?;
        let artifact = TeamArtifact {
            id: Uuid::now_v7().to_string(),
            work_item_id: claim.identity.work_item_id.clone(),
            attempt_id: claim.identity.attempt_id.clone(),
            member_id: claim.identity.member_id.clone(),
            provider: claim
                .provider
                .clone()
                .ok_or_else(|| err("missing source provider"))?,
            input_base_sha: base.into(),
            result_sha: result,
            patch_sha256: hex_digest(&patch),
            changed_files: actual,
            included_artifact_ids: included,
        };
        sqlx::query("INSERT INTO team_artifacts(id,task_id,work_item_id,attempt_id,artifact_json,patch,created_at) VALUES(?,?,?,?,?,?,?)").bind(&artifact.id).bind(&task.id).bind(&artifact.work_item_id).bind(&artifact.attempt_id).bind(encode(&artifact)?).bind(patch).bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
        Ok(artifact)
    }
    pub(super) async fn team_integrate(&self, claim: &TeamClaim) -> Result<(), OrchestratorError> {
        let task = self.task(&claim.identity.task_id).await?;
        let project = self.team_project(&task.id).await?;
        let op = claim.identity.attempt_id.clone();
        let mut tx = self.store.pool().begin().await?;
        let acquired=sqlx::query("INSERT OR IGNORE INTO team_project_locks(project_id,operation_id,task_id,kind) VALUES(?,?,?,'integrate')").bind(&project.id).bind(&op).bind(&task.id).execute(&mut *tx).await?;
        if acquired.rows_affected() != 1 {
            return Err(err("project integration resource is occupied"));
        }
        sqlx::query("INSERT INTO task_operations(id,task_id,kind,phase,status,payload_json,created_at,updated_at) VALUES(?,?,'team_integrate','intent','RUNNING',?,?,?)")
            .bind(&op).bind(&task.id).bind(json!({"attemptId":op,"identity":claim.identity}).to_string()).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
        tx.commit().await?;
        let result = self.team_integrate_locked(claim).await;
        sqlx::query("UPDATE task_operations SET status=?,phase='reconciled',last_error=?,updated_at=? WHERE id=?").bind(if result.is_ok(){"COMPLETED"}else{"FAILED"}).bind(result.as_ref().err().map(ToString::to_string)).bind(Utc::now().to_rfc3339()).bind(&op).execute(self.store.pool()).await?;
        sqlx::query("DELETE FROM team_project_locks WHERE operation_id=?")
            .bind(&op)
            .execute(self.store.pool())
            .await?;
        result
    }
    pub(super) async fn team_integrate_locked(
        &self,
        claim: &TeamClaim,
    ) -> Result<(), OrchestratorError> {
        let task = self.task(&claim.identity.task_id).await?;
        let view = self
            .team_get(&task.id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        let (workspace, _, mut included) = self.team_prepare_workspace(claim).await?;
        let dependencies: Vec<_> = view
            .items
            .iter()
            .filter(|w| {
                w.spec.kind == TeamWorkKind::Implement && w.state == TeamWorkState::Accepted
            })
            .filter_map(|w| {
                view.artifacts
                    .iter()
                    .find(|a| Some(&a.id) == w.artifact_id.as_ref())
                    .cloned()
            })
            .collect();
        let ordered = ordered_artifacts(&dependencies, &view.artifacts)?;
        for artifact in ordered {
            if included.contains(&artifact.id) {
                continue;
            }
            let patch: Vec<u8> =
                sqlx::query_scalar("SELECT patch FROM team_artifacts WHERE id=? AND task_id=?")
                    .bind(&artifact.id)
                    .bind(&task.id)
                    .fetch_one(self.store.pool())
                    .await?;
            if hex_digest(&patch) != artifact.patch_sha256 {
                return Err(err("artifact delta digest mismatch"));
            }
            let before = self.git.resolve(&workspace, "HEAD").await?;
            if let Err(error) = self.git.apply_team_delta(&workspace, &patch).await {
                self.git.reset_owned_worktree(&workspace, &before).await?;
                let partial = self
                    .team_seal_candidate(&task.id, &workspace, &included, &view)
                    .await?;
                self.team_publish_candidate(claim, &partial, &workspace, false)
                    .await?;
                self.store
                    .team_finish(
                        claim,
                        false,
                        None,
                        &format!("INTEGRATION_CONFLICT: {error}"),
                        Some("integration_conflict"),
                    )
                    .await?;
                self.team_request_rework(&task.id,std::slice::from_ref(&artifact.work_item_id),&format!("Integration conflict applying your prior delta to candidate {}: {error}. Reconcile your work with ALL existing candidate changes; preserve other members' results.",partial.id),None).await?;
                return Ok(());
            }
            if self.git.has_changes(&workspace).await? {
                self.git
                    .commit_revision(
                        &workspace,
                        task.seq,
                        0,
                        "integrate accepted team delta",
                        "runner",
                    )
                    .await?;
            }
            included.push(artifact.id.clone());
        }
        let candidate = self
            .team_seal_candidate(&task.id, &workspace, &included, &view)
            .await?;
        if !self.store.team_identity_valid(&claim.identity).await? {
            return Err(err("integration cancelled before publication"));
        }
        self.team_publish_candidate(claim, &candidate, &workspace, true)
            .await?;
        let config = self
            .load_trusted_config(&self.team_project(&task.id).await?)
            .await?;
        let integrity = self
            .integrity_guard(&self.task(&task.id).await?, &config)
            .await?;
        if !integrity.hard_violations.is_empty() {
            return Err(err(format!(
                "integrity guard: {}",
                integrity.hard_violations.join("; ")
            )));
        }
        self.store
            .team_finish(
                claim,
                true,
                None,
                &format!("sealed candidate {}", candidate.id),
                None,
            )
            .await?;
        Ok(())
    }
    pub(super) async fn team_seal_candidate(
        &self,
        task: &str,
        workspace: &Path,
        included: &[String],
        view: &TeamView,
    ) -> Result<TeamCandidate, OrchestratorError> {
        let commit = self.git.resolve(workspace, "HEAD").await?;
        let tree = self.git.resolve(workspace, "HEAD^{tree}").await?;
        let artifacts: Vec<_> = view
            .artifacts
            .iter()
            .filter(|a| included.contains(&a.id))
            .collect();
        let mut members = view
            .candidate
            .as_ref()
            .map(|c| c.contributor_member_ids.clone())
            .unwrap_or_default();
        let mut providers = view
            .candidate
            .as_ref()
            .map(|c| c.contributor_providers.clone())
            .unwrap_or_default();
        for a in artifacts.iter().filter(|a| !a.changed_files.is_empty()) {
            members.push(a.member_id.clone());
            providers.push(a.provider.clone());
        }
        members.sort();
        members.dedup();
        providers.sort_by_key(ToString::to_string);
        providers.dedup();
        Ok(TeamCandidate {
            id: Uuid::now_v7().to_string(),
            commit_sha: commit,
            tree_sha: tree,
            base_sha: self
                .task(task)
                .await?
                .base_commit
                .ok_or_else(|| err("base missing"))?,
            artifact_ids: included.to_vec(),
            contributor_member_ids: members,
            contributor_providers: providers,
            plan_hash: view
                .plan
                .as_ref()
                .map(hash)
                .transpose()?
                .unwrap_or_default(),
        })
    }
    pub(super) async fn team_publish_candidate(
        &self,
        claim: &TeamClaim,
        candidate: &TeamCandidate,
        workspace: &Path,
        complete: bool,
    ) -> Result<(), OrchestratorError> {
        let task = self.task(&claim.identity.task_id).await?;
        let project = self.team_project(&task.id).await?;
        let branch = format!("agentflow/team-{}", task.id);
        // Write operation receipt before changing any mutable ref. Recovery compares the real ref.
        sqlx::query("UPDATE task_operations SET phase='candidate_sealed',payload_json=?,updated_at=? WHERE id=?")
            .bind(json!({"candidate":candidate,"branch":branch,"worktree":workspace,"complete":complete}).to_string()).bind(Utc::now().to_rfc3339()).bind(&claim.identity.attempt_id).execute(self.store.pool()).await?;
        self.git
            .team_update_ref(
                &project.repo,
                &format!("refs/agentflow/candidates/{}", candidate.id),
                &candidate.commit_sha,
                "0000000000000000000000000000000000000000",
            )
            .await?;
        let mut tx = self.store.pool().begin().await?;
        let still_current:i64=sqlx::query_scalar("SELECT COUNT(*) FROM team_tasks t JOIN team_work_items w ON w.task_id=t.task_id JOIN team_attempt_contexts c ON c.run_id=? JOIN agent_runs r ON r.id=c.run_id WHERE t.task_id=? AND w.id=? AND w.epoch=? AND w.spec_hash=? AND t.phase='running' AND c.cancel_requested=0 AND r.status='RUNNING'")
            .bind(&claim.identity.attempt_id).bind(&task.id).bind(&claim.spec.id).bind(claim.identity.lease_epoch).bind(&claim.identity.spec_hash).fetch_one(&mut *tx).await?;
        if still_current != 1 {
            return Err(err("obsolete candidate publication rejected"));
        }
        sqlx::query(
            "INSERT INTO team_candidates(id,task_id,candidate_json,created_at) VALUES(?,?,?,?)",
        )
        .bind(&candidate.id)
        .bind(&task.id)
        .bind(encode(candidate)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE team_tasks SET current_candidate_id=?,updated_at=? WHERE task_id=?")
            .bind(&candidate.id)
            .bind(Utc::now().to_rfc3339())
            .bind(&task.id)
            .execute(&mut *tx)
            .await?;
        let revision = task.revision + 1;
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,created_at) VALUES(?,?,?,?,?)").bind(Uuid::now_v7().to_string()).bind(&task.id).bind(revision).bind(&candidate.commit_sha).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE tasks SET branch=?,worktree_path=?,current_revision=?,updated_at=? WHERE id=?",
        )
        .bind(branch)
        .bind(workspace.to_string_lossy().as_ref())
        .bind(revision)
        .bind(Utc::now().to_rfc3339())
        .bind(&task.id)
        .execute(&mut *tx)
        .await?;
        event(
            &mut tx,
            &task.id,
            "candidate_sealed",
            json!({"candidate":candidate,"completeIntegration":complete}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn team_request_rework(
        &self,
        task: &str,
        owners: &[String],
        detail: &str,
        criterion: Option<&str>,
    ) -> Result<(), OrchestratorError> {
        let view = self
            .team_get(task)
            .await?
            .ok_or_else(|| err("team missing"))?;
        if view.reworks_used >= view.config.max_reworks {
            self.team_set_phase(task,TeamPhase::Blocked,Some("cumulative product rework limit exhausted; new plans/providers do not reset it")).await?;
            return Ok(());
        }
        let mut affected: HashSet<String> = owners.iter().cloned().collect();
        loop {
            let n = affected.len();
            for w in &view.items {
                if w.spec.depends_on.iter().any(|d| affected.contains(d)) {
                    affected.insert(w.spec.id.clone());
                }
            }
            if n == affected.len() {
                break;
            }
        }
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("UPDATE team_tasks SET reworks_used=reworks_used+1,phase='running',detail=?,updated_at=? WHERE task_id=? AND phase NOT IN ('cancelled','delivered','delivering')").bind(detail).bind(Utc::now().to_rfc3339()).bind(task).execute(&mut *tx).await?;
        for owner in owners {
            sqlx::query("INSERT INTO team_issues(id,task_id,candidate_id,criterion_id,owner_work_item_id,detail,created_at) VALUES(?,?,?,?,?,?,?)").bind(Uuid::now_v7().to_string()).bind(task).bind(view.candidate.as_ref().map(|c|&c.id)).bind(criterion).bind(owner).bind(detail).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
        }
        for item in &view.items {
            if !affected.contains(&item.spec.id) {
                continue;
            }
            if item.spec.kind == TeamWorkKind::Coverage || item.spec.kind == TeamWorkKind::Plan {
                continue;
            }
            let mut spec = item.spec.clone();
            if spec.kind == TeamWorkKind::Implement {
                spec.instructions = format!(
                    "{}\n\nProduct rework {}: {}",
                    spec.instructions,
                    view.reworks_used + 1,
                    detail
                );
            }
            sqlx::query("INSERT INTO team_work_history(task_id,work_item_id,spec_json,spec_hash,artifact_id,plan_revision,created_at) VALUES(?,?,?,?,?,?,?)").bind(task).bind(&spec.id).bind(encode(&item.spec)?).bind(&item.spec_hash).bind(&item.artifact_id).bind(view.plan_revision).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
            sqlx::query("UPDATE team_work_items SET state='pending',spec_json=?,spec_hash=?,epoch=epoch+1,artifact_id=NULL,detail=? WHERE task_id=? AND id=?").bind(encode(&spec)?).bind(hash(&spec)?).bind(detail).bind(task).bind(&spec.id).execute(&mut *tx).await?;
            sqlx::query("UPDATE team_attempt_contexts SET cancel_requested=1 WHERE run_id IN (SELECT id FROM agent_runs WHERE task_id=? AND team_work_item_id=? AND status='RUNNING')").bind(task).bind(&spec.id).execute(&mut *tx).await?;
        }
        // Final checks are conservatively invalidated for the next candidate.
        sqlx::query("UPDATE team_work_items SET state='pending',artifact_id=NULL,epoch=epoch+1 WHERE task_id=? AND id IN ('__integrate','__verify','__review')").bind(task).execute(&mut *tx).await?;
        event(&mut tx,task,"rework",json!({"owners":owners,"affected":affected,"detail":detail,"round":view.reworks_used+1})).await?;
        tx.commit().await?;
        self.team_signal_cancellations(task).await?;
        Ok(())
    }
}
fn glob_set(patterns: &[String]) -> Result<GlobSet, OrchestratorError> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p).map_err(|e| err(e.to_string()))?);
    }
    b.build().map_err(|e| err(e.to_string()))
}
fn ordered_artifacts<'a>(
    selected: &'a [TeamArtifact],
    all: &'a [TeamArtifact],
) -> Result<Vec<&'a TeamArtifact>, OrchestratorError> {
    let mut wanted: HashSet<String> = selected.iter().map(|a| a.id.clone()).collect();
    loop {
        let n = wanted.len();
        for a in all {
            if wanted.contains(&a.id) {
                wanted.extend(a.included_artifact_ids.clone());
            }
        }
        if n == wanted.len() {
            break;
        }
    }
    let mut result = Vec::new();
    let mut done = HashSet::new();
    let mut sorted: Vec<_> = all.iter().filter(|a| wanted.contains(&a.id)).collect();
    sorted.sort_by(|a, b| (&a.work_item_id, &a.id).cmp(&(&b.work_item_id, &b.id)));
    loop {
        let n = done.len();
        for a in &sorted {
            if !done.contains(&a.id) && a.included_artifact_ids.iter().all(|id| done.contains(id)) {
                done.insert(a.id.clone());
                result.push(*a);
            }
        }
        if done.len() == wanted.len() {
            return Ok(result);
        }
        if n == done.len() {
            return Err(err("missing/cyclic artifact input manifest"));
        }
    }
}
