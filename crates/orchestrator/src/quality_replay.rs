impl Orchestrator {
    pub async fn task_quality_replay(
        &self,
        task_id: &str,
        revision: Option<i64>,
    ) -> Result<QualityReplayAttempt, OrchestratorError> {
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
        if !steps_path.is_file() {
            return Err(OrchestratorError::InvalidState(
                "replay validation snapshot is missing for this historical revision".into(),
            ));
        }
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
        let original_quality = self.original_quality(task_id, task.revision).await?;
        let attempt_id = Uuid::now_v7().to_string();
        let created_at = Utc::now().to_rfc3339();
        let replay_root = self.app_data.join("replays");
        tokio::fs::create_dir_all(&replay_root).await?;
        let replay_path = replay_root.join(Uuid::now_v7().to_string());
        self.git
            .worktree_add_detached(&project.repo, &replay_path, &sha)
            .await?;
        let current = match self
            .capture_reproducibility_environment(&replay_path, &replay_config)
            .await
        {
            Ok(current) => current,
            Err(error) => {
                let _ = self.git.worktree_remove(&project.repo, &replay_path).await;
                let attempt = QualityReplayAttempt {
                    id: attempt_id,
                    task_id: task_id.to_string(),
                    revision: task.revision,
                    status: QualityReplayStatus::Failed,
                    reproducibility_level: manifest.reproducibility_level,
                    environment_match: false,
                    drift: Vec::new(),
                    original_quality,
                    replay_quality: None,
                    score_delta: None,
                    error_code: Some("ENVIRONMENT_CAPTURE_FAILED".into()),
                    error_detail: Some(replay_error_detail(&error)),
                    created_at,
                    finished_at: Utc::now().to_rfc3339(),
                };
                self.record_quality_replay_attempt(&attempt).await?;
                return Ok(attempt);
            }
        };
        let drift = reproducibility_drift(&manifest, &current);
        if manifest.reproducibility_level != ReproducibilityLevel::FixedCommit
            && !drift.is_empty()
        {
            let _ = self.git.worktree_remove(&project.repo, &replay_path).await;
            let attempt = QualityReplayAttempt {
                id: attempt_id,
                task_id: task_id.to_string(),
                revision: task.revision,
                status: QualityReplayStatus::DriftBlocked,
                reproducibility_level: manifest.reproducibility_level,
                environment_match: false,
                drift,
                original_quality,
                replay_quality: None,
                score_delta: None,
                error_code: Some("REPRODUCIBILITY_DRIFT".into()),
                error_detail: None,
                created_at,
                finished_at: Utc::now().to_rfc3339(),
            };
            self.record_quality_replay_attempt(&attempt).await?;
            self.record_replay_event(&attempt).await?;
            return Ok(attempt);
        }
        let report_result = self.execute_validation(&task, &replay_path, &steps).await;
        let _ = self.git.worktree_remove(&project.repo, &replay_path).await;
        let report = match report_result {
            Ok(report) => report,
            Err(error) => {
                let attempt = QualityReplayAttempt {
                    id: attempt_id,
                    task_id: task_id.to_string(),
                    revision: task.revision,
                    status: QualityReplayStatus::Failed,
                    reproducibility_level: manifest.reproducibility_level,
                    environment_match: drift.is_empty(),
                    drift,
                    original_quality,
                    replay_quality: None,
                    score_delta: None,
                    error_code: Some("VALIDATION_INFRA_FAILED".into()),
                    error_detail: Some(replay_error_detail(&error)),
                    created_at,
                    finished_at: Utc::now().to_rfc3339(),
                };
                self.record_quality_replay_attempt(&attempt).await?;
                self.record_replay_event(&attempt).await?;
                return Ok(attempt);
            }
        };
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
        let score_delta = original_quality
            .as_ref()
            .map(|original| i32::from(quality.score) - i32::from(original.score));
        let attempt = QualityReplayAttempt {
            id: attempt_id,
            task_id: task_id.to_string(),
            revision: task.revision,
            status: if quality.passed {
                QualityReplayStatus::Succeeded
            } else {
                QualityReplayStatus::ValidationFailed
            },
            reproducibility_level: manifest.reproducibility_level,
            environment_match: drift.is_empty(),
            drift,
            original_quality,
            replay_quality: Some(quality),
            score_delta,
            error_code: None,
            error_detail: None,
            created_at,
            finished_at: Utc::now().to_rfc3339(),
        };
        self.record_quality_replay_attempt(&attempt).await?;
        self.record_replay_event(&attempt).await?;
        Ok(attempt)
    }

    async fn record_quality_replay_attempt(
        &self,
        attempt: &QualityReplayAttempt,
    ) -> Result<(), OrchestratorError> {
        let value = serde_json::to_string(attempt)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        sqlx::query("INSERT INTO quality_replay_attempts(id,task_id,revision,status,attempt_json,created_at) VALUES(?,?,?,?,?,?)")
            .bind(&attempt.id)
            .bind(&attempt.task_id)
            .bind(attempt.revision)
            .bind(attempt.status.to_string())
            .bind(value)
            .bind(&attempt.created_at)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    async fn record_replay_event(
        &self,
        attempt: &QualityReplayAttempt,
    ) -> Result<(), OrchestratorError> {
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'human','quality:replayed',?,?)")
            .bind(&attempt.task_id)
            .bind(attempt.revision)
            .bind(json!({
                "status":attempt.status,
                "score":attempt.replay_quality.as_ref().map(|quality| quality.score),
                "passed":attempt.replay_quality.as_ref().map(|quality| quality.passed),
                "reproducibility_level":attempt.reproducibility_level,
                "environment_match":attempt.environment_match,
                "drift":attempt.drift,
            }).to_string())
            .bind(&attempt.finished_at)
            .execute(self.store.pool()).await?;
        Ok(())
    }
}

fn replay_error_detail(error: &OrchestratorError) -> String {
    agentflow_process_supervisor::redact(error.to_string().chars().take(800).collect())
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
