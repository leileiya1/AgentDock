impl Orchestrator {
    pub async fn task_quality_replay(
        &self,
        task_id: &str,
        revision: Option<i64>,
    ) -> Result<QualityEvaluation, OrchestratorError> {
        let mut task = self.task(task_id).await?;
        task.revision = revision.unwrap_or(task.revision);
        if task.revision <= 0 {
            return Err(OrchestratorError::InvalidState(
                "replay requires a committed revision".into(),
            ));
        }
        let project = self.project(&task.project_id).await?;
        let sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(task_id)
        .bind(task.revision)
        .fetch_one(self.store.pool())
        .await?;
        let steps_path = self
            .task_dir(task_id)
            .join("artifacts")
            .join(format!("r{}-validation-config.json", task.revision));
        let steps: Vec<ValidateStep> = serde_json::from_slice(&tokio::fs::read(steps_path).await?)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let reproducibility_path = self.task_dir(task_id).join("artifacts").join(format!(
            "r{}-reproducibility-config.json",
            task.revision
        ));
        let reproducibility = if reproducibility_path.exists() {
            serde_json::from_slice(&tokio::fs::read(reproducibility_path).await?)
                .map_err(|error| OrchestratorError::Config(error.to_string()))?
        } else {
            ReproducibilityConfig::default()
        };
        let replay_config = ProjectConfig {
            schema_version: 1,
            validate: ValidateConfig {
                steps: steps.clone(),
            },
            reproducibility,
            ..ProjectConfig::default()
        };
        let manifest = self
            .reproducibility_manifest(task_id, task.revision)
            .await?
            .ok_or_else(|| {
                OrchestratorError::InvalidState("replay manifest is missing".into())
            })?;
        let replay_root = self.app_data.join("replays");
        tokio::fs::create_dir_all(&replay_root).await?;
        let replay_path = replay_root.join(Uuid::now_v7().to_string());
        self.git
            .worktree_add_detached(&project.repo, &replay_path, &sha)
            .await?;
        let current = self
            .capture_reproducibility_environment(&replay_path, &replay_config)
            .await?;
        let drift = reproducibility_drift(&manifest, &current);
        if manifest.reproducibility_level != ReproducibilityLevel::FixedCommit
            && !drift.is_empty()
        {
            let _ = self.git.worktree_remove(&project.repo, &replay_path).await;
            return Err(OrchestratorError::InvalidState(format!(
                "REPRODUCIBILITY_DRIFT: {}",
                drift.join("; ")
            )));
        }
        let report_result = self.execute_validation(&task, &replay_path, &steps).await;
        let _ = self.git.worktree_remove(&project.repo, &replay_path).await;
        let report = report_result?;
        let artifact = self.task_dir(task_id).join("artifacts").join(format!(
            "r{}-replay-{}.json",
            task.revision,
            Utc::now().format("%Y%m%dT%H%M%SZ")
        ));
        tokio::fs::write(
            artifact,
            serde_json::to_vec_pretty(&report)
                .map_err(|error| OrchestratorError::Config(error.to_string()))?,
        )
        .await?;
        let quality = self.evaluate_quality(&task, &report, true).await?;
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'human','quality:replayed',?,?)")
            .bind(task_id).bind(task.revision)
            .bind(json!({
                "score":quality.score,
                "passed":quality.passed,
                "reproducibility_level":manifest.reproducibility_level,
                "environment_match":drift.is_empty(),
                "drift":drift,
            }).to_string())
            .bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
        Ok(quality)
    }
}

fn aggregate_budget_mode(modes: &[String]) -> BudgetEnforcement {
    if modes.iter().any(|mode| mode == "unavailable") {
        BudgetEnforcement::Unavailable
    } else if modes.iter().any(|mode| mode == "soft") {
        BudgetEnforcement::Soft
    } else if modes.iter().any(|mode| mode == "hard") {
        BudgetEnforcement::Hard
    } else {
        BudgetEnforcement::Unavailable
    }
}

async fn latest_run_input(
    store: &Store,
    task_id: &str,
    revision: i64,
) -> Result<Vec<u8>, OrchestratorError> {
    let run_dir: Option<String> = sqlx::query_scalar(
        "SELECT run_dir FROM agent_runs WHERE task_id=? AND revision=? AND role='developer' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(revision)
    .fetch_optional(store.pool())
    .await?;
    let Some(run_dir) = run_dir else {
        return Ok(Vec::new());
    };
    Ok(store
        .read_protected_file(&Path::new(&run_dir).join("input.md"))
        .await
        .unwrap_or_default())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
