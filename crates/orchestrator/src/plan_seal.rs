struct ApprovedPlanSeal {
    id: String,
    version: i64,
    sha256: String,
    rendered_json: String,
    allowed_paths: Vec<String>,
    allowed_set: GlobSet,
}

fn plan_payload(
    id: &str,
    version: i64,
    summary: &str,
    steps: &Value,
    risks: &Value,
    allowed_paths: &[String],
) -> Value {
    json!({
        "id": id,
        "version": version,
        "summary": summary,
        "steps": steps,
        "risks": risks,
        "allowed_paths": allowed_paths,
    })
}

fn hash_plan(value: &Value) -> Result<(String, String), OrchestratorError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| OrchestratorError::Config(error.to_string()))?;
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|error| OrchestratorError::Config(error.to_string()))?;
    Ok((format!("{:x}", Sha256::digest(bytes)), rendered))
}

fn compile_allowed_paths(patterns: &[String]) -> Result<GlobSet, OrchestratorError> {
    if patterns.is_empty() {
        return Err(OrchestratorError::Config(
            "coding plan must declare at least one allowed_paths pattern".into(),
        ));
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let path = Path::new(pattern);
        if pattern.trim().is_empty()
            || path.is_absolute()
            || path.components().any(|part| matches!(part, std::path::Component::ParentDir))
            || pattern.starts_with(".git")
            || pattern.starts_with(".agentflow-in")
            || pattern.starts_with(".agentflow-out")
        {
            return Err(OrchestratorError::Config(format!(
                "unsafe allowed_paths pattern: {pattern}"
            )));
        }
        builder.add(
            Glob::new(pattern)
                .map_err(|error| OrchestratorError::Config(format!("invalid plan glob {pattern}: {error}")))?,
        );
    }
    builder
        .build()
        .map_err(|error| OrchestratorError::Config(error.to_string()))
}

impl Orchestrator {
    async fn approved_plan_seal(
        &self,
        task: &TaskRow,
    ) -> Result<Option<ApprovedPlanSeal>, OrchestratorError> {
        if !task.policy.require_plan_approval {
            return Ok(None);
        }
        let row = sqlx::query(
            "SELECT id,version,summary,steps_json,risks_json,allowed_paths_json,plan_sha256 \
             FROM task_plans WHERE task_id=? AND status='approved' ORDER BY version DESC LIMIT 1",
        )
        .bind(&task.id)
        .fetch_optional(self.store.pool())
        .await?
        .ok_or_else(|| OrchestratorError::InvalidState("approved coding plan missing".into()))?;
        let id: String = row.get("id");
        let version: i64 = row.get("version");
        let summary: String = row.get("summary");
        let steps: Value = serde_json::from_str(&row.get::<String, _>("steps_json"))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let risks: Value = serde_json::from_str(&row.get::<String, _>("risks_json"))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let allowed_paths: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("allowed_paths_json"))
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let payload = plan_payload(&id, version, &summary, &steps, &risks, &allowed_paths);
        let (actual_sha, rendered_json) = hash_plan(&payload)?;
        let stored_sha: Option<String> = row.get("plan_sha256");
        if stored_sha.as_deref() != Some(&actual_sha) {
            return Err(OrchestratorError::InvalidState(
                "approved coding plan seal is stale or was modified".into(),
            ));
        }
        Ok(Some(ApprovedPlanSeal {
            id,
            version,
            sha256: actual_sha,
            rendered_json,
            allowed_set: compile_allowed_paths(&allowed_paths)?,
            allowed_paths,
        }))
    }

    async fn plan_deviations(
        &self,
        worktree: &Path,
        seal: &ApprovedPlanSeal,
    ) -> Result<Vec<String>, OrchestratorError> {
        Ok(self
            .git
            .changed_paths(worktree)
            .await?
            .into_iter()
            .filter(|path| !seal.allowed_set.is_match(path))
            .collect())
    }

    async fn return_plan_for_reapproval(
        &self,
        task: &TaskRow,
        plan: &ApprovedPlanSeal,
        baseline: &str,
        worktree: &Path,
        deviations: &[String],
    ) -> Result<(), OrchestratorError> {
        self.git.reset_owned_worktree(worktree, baseline).await?;
        let now = Utc::now().to_rfc3339();
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("UPDATE task_plans SET status='pending',approved_at=NULL,plan_sha256=NULL WHERE id=?")
            .bind(&plan.id)
            .execute(&mut *tx)
            .await?;
        let changed = sqlx::query("UPDATE tasks SET current_revision=current_revision-1,status='WAITING_FOR_PLAN_APPROVAL',blocked_reason=NULL,updated_at=? WHERE id=? AND current_revision>0 AND status IN ('DEVELOPING','REVISING')")
            .bind(&now)
            .bind(&task.id)
            .execute(&mut *tx)
            .await?;
        if changed.rows_affected() != 1 {
            return Err(OrchestratorError::InvalidState(
                "plan deviation state changed before reapproval".into(),
            ));
        }
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','plan:deviation',?,?)")
            .bind(&task.id)
            .bind(task.revision)
            .bind(json!({
                "plan_id": plan.id,
                "plan_sha256": plan.sha256,
                "allowed_paths": plan.allowed_paths,
                "deviations": deviations,
            }).to_string())
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
