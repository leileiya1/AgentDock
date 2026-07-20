impl Orchestrator {
    async fn task_policy(&self, task_id: &str) -> Result<TaskPolicy, OrchestratorError> {
        let row = sqlx::query(
            "SELECT require_plan_approval,priority,token_budget,cost_budget_usd,time_budget_secs,\
             minimum_quality_score,delivery_mode,execution_node_id FROM task_policies WHERE task_id=?",
        )
        .bind(task_id)
        .fetch_optional(self.store.pool())
        .await?;
        let Some(row) = row else {
            return Ok(TaskPolicy {
                require_plan_approval: false,
                ..TaskPolicy::default()
            });
        };
        Ok(TaskPolicy {
            require_plan_approval: row.get::<i64, _>("require_plan_approval") != 0,
            priority: row.get::<i64, _>("priority").clamp(-100, 100) as i16,
            token_budget: row.get("token_budget"),
            cost_budget_usd: row.get("cost_budget_usd"),
            time_budget_secs: row.get("time_budget_secs"),
            minimum_quality_score: row
                .get::<i64, _>("minimum_quality_score")
                .clamp(0, 100) as u8,
            delivery_mode: parse(row.get("delivery_mode"))?,
            execution_node_id: row.get("execution_node_id"),
        })
    }

    async fn latest_plan(&self, task_id: &str) -> Result<Option<CodingPlan>, OrchestratorError> {
        let row = sqlx::query(
            "SELECT id,version,status,summary,steps_json,risks_json,allowed_paths_json,plan_sha256,created_at,approved_at \
             FROM task_plans WHERE task_id=? ORDER BY version DESC LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(self.store.pool())
        .await?;
        row.map(|row| {
            Ok(CodingPlan {
                id: row.get("id"),
                version: row.get("version"),
                status: parse(row.get("status"))?,
                summary: row.get("summary"),
                steps: serde_json::from_str(&row.get::<String, _>("steps_json"))
                    .map_err(|error| OrchestratorError::Config(error.to_string()))?,
                risks: serde_json::from_str(&row.get::<String, _>("risks_json"))
                    .map_err(|error| OrchestratorError::Config(error.to_string()))?,
                allowed_paths: serde_json::from_str(&row.get::<String, _>("allowed_paths_json"))
                    .map_err(|error| OrchestratorError::Config(error.to_string()))?,
                plan_sha256: row.get("plan_sha256"),
                created_at: row.get("created_at"),
                approved_at: row.get("approved_at"),
            })
        })
        .transpose()
    }

    pub async fn budget_usage(&self, task_id: &str) -> Result<BudgetUsage, OrchestratorError> {
        let policy = self.task_policy(task_id).await?;
        let rows = sqlx::query(
            "SELECT status,cost_usd,tokens_in,tokens_out,started_at,finished_at,\
             token_budget_mode,cost_budget_mode,reserved_tokens,reserved_cost_usd \
             FROM agent_runs WHERE task_id=?",
        )
        .bind(task_id)
        .fetch_all(self.store.pool())
        .await?;
        let now = Utc::now();
        let mut tokens = 0_i64;
        let mut cost = 0.0_f64;
        let mut seconds = 0_i64;
        let mut unknown_token_runs = 0_i64;
        let mut unknown_cost_runs = 0_i64;
        let mut tokens_reserved = 0_i64;
        let mut cost_reserved_usd = 0.0_f64;
        let mut token_modes = Vec::new();
        let mut cost_modes = Vec::new();
        for row in rows {
            let tokens_in = row.get::<Option<i64>, _>("tokens_in");
            let tokens_out = row.get::<Option<i64>, _>("tokens_out");
            match (tokens_in, tokens_out) {
                (Some(input), Some(output)) => {
                    tokens = tokens.saturating_add(input).saturating_add(output);
                }
                _ => unknown_token_runs = unknown_token_runs.saturating_add(1),
            }
            match row.get::<Option<f64>, _>("cost_usd") {
                Some(value) if value.is_finite() && value >= 0.0 => cost += value,
                _ => unknown_cost_runs = unknown_cost_runs.saturating_add(1),
            }
            let status: String = row.get("status");
            if status == "RUNNING" {
                tokens_reserved = tokens_reserved.saturating_add(
                    row.get::<Option<i64>, _>("reserved_tokens").unwrap_or(0),
                );
                cost_reserved_usd += row
                    .get::<Option<f64>, _>("reserved_cost_usd")
                    .unwrap_or(0.0);
            }
            token_modes.push(row.get::<String, _>("token_budget_mode"));
            cost_modes.push(row.get::<String, _>("cost_budget_mode"));
            let started = row
                .get::<Option<String>, _>("started_at")
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value).ok())
                .map(|value| value.with_timezone(&Utc));
            let finished = row
                .get::<Option<String>, _>("finished_at")
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value).ok())
                .map(|value| value.with_timezone(&Utc));
            if let Some(started) = started {
                seconds = seconds.saturating_add(
                    finished
                        .unwrap_or(now)
                        .signed_duration_since(started)
                        .num_seconds()
                        .max(0),
                );
            }
        }
        let exceeded = policy
            .token_budget
            .is_some_and(|limit| tokens.saturating_add(tokens_reserved) >= limit)
            || policy
                .cost_budget_usd
                .is_some_and(|limit| cost + cost_reserved_usd >= limit)
            || policy.time_budget_secs.is_some_and(|limit| seconds >= limit);
        Ok(BudgetUsage {
            tokens_used: tokens,
            cost_usd: cost,
            time_used_secs: seconds,
            token_budget: policy.token_budget,
            cost_budget_usd: policy.cost_budget_usd,
            time_budget_secs: policy.time_budget_secs,
            tokens_known: unknown_token_runs == 0,
            cost_known: unknown_cost_runs == 0,
            unknown_token_runs,
            unknown_cost_runs,
            tokens_reserved,
            cost_reserved_usd,
            token_enforcement: aggregate_budget_mode(&token_modes),
            cost_enforcement: aggregate_budget_mode(&cost_modes),
            exceeded,
        })
    }

    async fn remaining_time_budget(&self, task_id: &str) -> Result<Option<u64>, OrchestratorError> {
        let usage = self.budget_usage(task_id).await?;
        Ok(usage.time_budget_secs.map(|limit| {
            limit
                .saturating_sub(usage.time_used_secs)
                .max(1) as u64
        }))
    }

    async fn enforce_budget(&self, task: &TaskRow) -> Result<bool, OrchestratorError> {
        let usage = self.budget_usage(&task.id).await?;
        if !usage.exceeded {
            return Ok(false);
        }
        // A Provider process cannot be resumed in-place. Running stages return to
        // their deterministic scheduler boundary; development also releases the
        // provisional revision number because no revision commit exists yet.
        let (resume, revision) = match task.status {
            TaskStatus::Developing => (
                TaskStatus::ReadyForDevelopment,
                Some(task.revision.saturating_sub(1)),
            ),
            TaskStatus::Revising => (
                TaskStatus::ReadyForRevision,
                Some(task.revision.saturating_sub(1)),
            ),
            TaskStatus::Reviewing => (TaskStatus::ReadyForReview, None),
            status => (status, None),
        };
        sqlx::query("UPDATE tasks SET blocked_detail=?,repair_resume_status=?,current_revision=COALESCE(?,current_revision) WHERE id=?")
            .bind(format!(
                "预算已用：{} tokens / ${:.4} / {} 秒",
                usage.tokens_used, usage.cost_usd, usage.time_used_secs
            ))
            .bind(resume.to_string())
            .bind(revision)
            .bind(&task.id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                &task.id,
                &[task.status],
                TaskStatus::Blocked,
                Some(BlockedReason::BudgetExceeded),
                Actor::System,
                "budget:exceeded",
                &serde_json::to_value(&usage).unwrap_or(Value::Null),
            )
            .await?;
        Ok(true)
    }

    pub async fn task_budget_update(
        &self,
        task_id: &str,
        limits: BudgetLimitPatch,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::Blocked
            || self.store.task_summary(task_id).await?.blocked_reason
                != Some(BlockedReason::BudgetExceeded)
        {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        if limits.token_budget.is_some_and(|value| value <= 0)
            || limits.time_budget_secs.is_some_and(|value| value <= 0)
            || limits.cost_budget_usd.is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(OrchestratorError::InvalidState(
                "budget limits must be positive or unlimited".into(),
            ));
        }
        let usage = self.budget_usage(task_id).await?;
        if limits.token_budget.is_some_and(|value| value <= usage.tokens_used)
            || limits.cost_budget_usd.is_some_and(|value| value <= usage.cost_usd)
            || limits.time_budget_secs.is_some_and(|value| value <= usage.time_used_secs)
        {
            return Err(OrchestratorError::InvalidState(
                "BUDGET_EXCEEDED: new limits must exceed current usage".into(),
            ));
        }
        let resume_text: Option<String> = sqlx::query_scalar(
            "SELECT repair_resume_status FROM tasks WHERE id=?",
        )
        .bind(task_id)
        .fetch_one(self.store.pool())
        .await?;
        let fallback = if task.revision == 0 {
            TaskStatus::ReadyForDevelopment
        } else {
            TaskStatus::ReadyForRevision
        };
        let resume = resume_text
            .map(parse)
            .transpose()?
            .unwrap_or(fallback);
        if !matches!(
            resume,
            TaskStatus::Planning
                | TaskStatus::ReadyForDevelopment
                | TaskStatus::ReadyForRevision
                | TaskStatus::Validating
                | TaskStatus::ReadyForReview
        ) {
            return Err(OrchestratorError::InvalidState(
                "budget checkpoint cannot be resumed safely".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE task_policies SET token_budget=?,cost_budget_usd=?,time_budget_secs=?,updated_at=? WHERE task_id=?",
        )
        .bind(limits.token_budget)
        .bind(limits.cost_budget_usd)
        .bind(limits.time_budget_secs)
        .bind(&now)
        .bind(task_id)
        .execute(self.store.pool())
        .await?;
        sqlx::query("UPDATE tasks SET blocked_detail=NULL,repair_resume_status=NULL WHERE id=?")
            .bind(task_id)
            .execute(self.store.pool())
            .await?;
        self.store
            .transition(
                task_id,
                &[TaskStatus::Blocked],
                resume,
                None,
                Actor::Human,
                "human:budget_extended",
                &json!({
                    "token_budget": limits.token_budget,
                    "cost_budget_usd": limits.cost_budget_usd,
                    "time_budget_secs": limits.time_budget_secs,
                    "resume_status": resume,
                }),
            )
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }

    async fn delivery_record(
        &self,
        task_id: &str,
    ) -> Result<Option<DeliveryRecord>, OrchestratorError> {
        let row = sqlx::query("SELECT mode,state,remote_url,request_number,ci_status,merge_commit,pre_merge_commit,rollback_commit,updated_at FROM delivery_records WHERE task_id=?")
            .bind(task_id).fetch_optional(self.store.pool()).await?;
        row.map(|row| {
            Ok(DeliveryRecord {
                mode: parse(row.get("mode"))?,
                state: parse(row.get("state"))?,
                remote_url: row.get("remote_url"),
                number: row.get("request_number"),
                ci_status: parse_opt(row.get("ci_status"))?,
                merge_commit: row.get("merge_commit"),
                pre_merge_commit: row.get("pre_merge_commit"),
                rollback_commit: row.get("rollback_commit"),
                updated_at: row.get("updated_at"),
            })
        }).transpose()
    }

    pub async fn task_governance_get(
        &self,
        task_id: &str,
        revision: Option<i64>,
    ) -> Result<TaskGovernance, OrchestratorError> {
        let task = self.task(task_id).await?;
        let revision = revision.unwrap_or(task.revision);
        Ok(TaskGovernance {
            manifest: self.reproducibility_manifest(task_id, revision).await?,
            quality: self.latest_quality(task_id, revision).await?,
            budget: self.budget_usage(task_id).await?,
            delivery: self.delivery_record(task_id).await?,
        })
    }

    async fn reproducibility_manifest(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Option<ReproducibilityManifest>, OrchestratorError> {
        let json: Option<String> = sqlx::query_scalar(
            "SELECT manifest_json FROM reproducibility_manifests WHERE task_id=? AND revision=?",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_optional(self.store.pool())
        .await?;
        json.map(|value| {
            serde_json::from_str(&value).map_err(|error| OrchestratorError::Config(error.to_string()))
        }).transpose()
    }

    async fn latest_quality(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Option<QualityEvaluation>, OrchestratorError> {
        let json: Option<String> = sqlx::query_scalar(
            "SELECT evaluation_json FROM quality_evaluations WHERE task_id=? AND revision=? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_optional(self.store.pool())
        .await?;
        json.map(|value| {
            serde_json::from_str(&value).map_err(|error| OrchestratorError::Config(error.to_string()))
        }).transpose()
    }

    async fn record_reproducibility_manifest(
        &self,
        task: &TaskRow,
        config: &ProjectConfig,
    ) -> Result<ReproducibilityManifest, OrchestratorError> {
        let commit_sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_one(self.store.pool())
        .await?;
        let artifact_dir = self.task_dir(&task.id).join("artifacts");
        let patch = tokio::fs::read(artifact_dir.join(format!("r{}.patch", task.revision)))
            .await
            .unwrap_or_default();
        let input = latest_run_input(&self.store, &task.id, task.revision).await?;
        let validation = serde_json::to_vec(&config.validate.steps)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        tokio::fs::create_dir_all(&artifact_dir).await?;
        tokio::fs::write(
            artifact_dir.join(format!("r{}-validation-config.json", task.revision)),
            &validation,
        )
        .await?;
        tokio::fs::write(
            artifact_dir.join(format!("r{}-reproducibility-config.json", task.revision)),
            serde_json::to_vec(&json!({
                "lock_environment": config.reproducibility.lock_environment,
                "hermetic": config.reproducibility.hermetic,
                "env_allowlist": config.reproducibility.env_allowlist,
                "external_dependencies": config.reproducibility.external_dependencies,
                "container_images": config.reproducibility.container_images,
            }))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?,
        )
        .await?;
        let worktree = required_path(&task.worktree_path)?;
        let capture = self
            .capture_reproducibility_environment(&worktree, config)
            .await?;
        let mut environment = std::collections::BTreeMap::new();
        environment.insert("orchestrator_os".into(), std::env::consts::OS.into());
        environment.insert("orchestrator_arch".into(), std::env::consts::ARCH.into());
        environment.insert(
            "orchestrator_git".into(),
            capture
                .tool_versions
                .get("git")
                .cloned()
                .unwrap_or_else(|| "unavailable".into()),
        );
        environment.insert("developer_provider".into(), task.developer.to_string());
        environment.insert("reviewer_provider".into(), task.reviewer.to_string());
        if let Some(node_id) = task.policy.execution_node_id.as_deref() {
            let node = self.execution_node_get(node_id).await?;
            environment.insert("validation_location".into(), "remote".into());
            environment.insert("execution_node_id".into(), node.id);
            environment.insert("execution_node_name".into(), node.name);
            environment.insert(
                "validation_platform".into(),
                node.platform.unwrap_or_else(|| "unknown".into()),
            );
            environment.insert(
                "validation_git".into(),
                node.git_version.unwrap_or_else(|| "unknown".into()),
            );
        } else {
            environment.insert("validation_location".into(), "local".into());
            environment.insert("validation_platform".into(), format!(
                "{} {}", std::env::consts::OS, std::env::consts::ARCH
            ));
        }
        let created_at = Utc::now().to_rfc3339();
        let environment_sha256 = capture.sha256();
        let unsigned = json!({
            "task_id": task.id,
            "revision": task.revision,
            "commit_sha": commit_sha,
            "environment": environment,
            "reproducibility_level": capture.level,
            "environment_sha256": environment_sha256,
            "input_sha256": sha256_hex(&input),
            "patch_sha256": sha256_hex(&patch),
            "validation_config_sha256": sha256_hex(&validation),
        });
        let manifest_sha256 = sha256_hex(&serde_json::to_vec(&unsigned).unwrap_or_default());
        let manifest = ReproducibilityManifest {
            task_id: task.id.clone(),
            revision: task.revision,
            commit_sha,
            manifest_sha256,
            environment,
            reproducibility_level: capture.level,
            tool_versions: capture.tool_versions,
            environment_variables: capture.environment_variables,
            system_dependencies: capture.system_dependencies,
            container_image_digests: capture.container_image_digests,
            git_submodules: capture.git_submodules,
            git_lfs_objects: capture.git_lfs_objects,
            external_dependencies: capture.external_dependencies,
            limitations: capture.limitations,
            environment_sha256,
            input_sha256: sha256_hex(&input),
            patch_sha256: sha256_hex(&patch),
            validation_config_sha256: sha256_hex(&validation),
            created_at,
        };
        if let Some(existing) = self
            .reproducibility_manifest(&task.id, task.revision)
            .await?
        {
            if existing.manifest_sha256 != manifest.manifest_sha256 {
                return Err(OrchestratorError::InvalidState(
                    "reproducibility manifest is immutable and no longer matches".into(),
                ));
            }
            return Ok(existing);
        }
        let value = serde_json::to_string(&manifest)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        sqlx::query("INSERT INTO reproducibility_manifests(id,task_id,revision,commit_sha,manifest_sha256,manifest_json,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(Uuid::now_v7().to_string()).bind(&task.id).bind(task.revision)
            .bind(&manifest.commit_sha).bind(&manifest.manifest_sha256).bind(value)
            .bind(&manifest.created_at).execute(self.store.pool()).await?;
        Ok(manifest)
    }

    async fn evaluate_quality(
        &self,
        task: &TaskRow,
        report: &TestReport,
        replay: bool,
    ) -> Result<QualityEvaluation, OrchestratorError> {
        let review: Option<String> = sqlx::query_scalar(
            "SELECT decision FROM reviews WHERE task_id=? AND revision=? ORDER BY is_aggregate DESC,created_at DESC LIMIT 1",
        )
        .bind(&task.id).bind(task.revision).fetch_optional(self.store.pool()).await?;
        let high_issues: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM review_issues i JOIN reviews r ON r.id=i.review_id WHERE r.task_id=? AND r.revision=? AND i.resolved=0 AND i.severity IN ('critical','high')",
        )
        .bind(&task.id).bind(task.revision).fetch_one(self.store.pool()).await?;
        let stat: Option<String> = sqlx::query_scalar(
            "SELECT diff_stat_json FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(&task.id).bind(task.revision).fetch_optional(self.store.pool()).await?;
        let flagged = stat
            .and_then(|value| serde_json::from_str::<DiffStat>(&value).ok())
            .map_or(0, |value| value.flagged.len());
        let checks = vec![
            QualityCheck { name: "validation".into(), passed: report.passed, weight: 50, detail: format!("{} validation steps", report.steps.len()) },
            QualityCheck { name: "independent_review".into(), passed: review.as_deref() == Some("pass"), weight: 25, detail: review.unwrap_or_else(|| "not reviewed".into()) },
            QualityCheck { name: "high_risk_issues".into(), passed: high_issues == 0, weight: 15, detail: format!("{high_issues} unresolved critical/high issues") },
            QualityCheck { name: "control_plane_changes".into(), passed: flagged == 0, weight: 10, detail: format!("{flagged} flagged files") },
        ];
        let score = checks.iter().filter(|check| check.passed).map(|check| check.weight).sum();
        let grade = match score { 90..=100 => QualityGrade::A, 80..=89 => QualityGrade::B, 70..=79 => QualityGrade::C, _ => QualityGrade::D };
        let passed = score >= task.policy.minimum_quality_score
            && checks.iter().take(3).all(|check| check.passed);
        let evaluation = QualityEvaluation {
            task_id: task.id.clone(), revision: task.revision, score, grade, passed, replay,
            checks, created_at: Utc::now().to_rfc3339(),
        };
        sqlx::query("INSERT INTO quality_evaluations(id,task_id,revision,score,passed,replay,evaluation_json,created_at) VALUES(?,?,?,?,?,?,?,?)")
            .bind(Uuid::now_v7().to_string()).bind(&task.id).bind(task.revision)
            .bind(i64::from(score)).bind(i64::from(passed)).bind(i64::from(replay))
            .bind(serde_json::to_string(&evaluation).map_err(|error|OrchestratorError::Config(error.to_string()))?)
            .bind(&evaluation.created_at).execute(self.store.pool()).await?;
        Ok(evaluation)
    }

    async fn stored_test_report(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<TestReport, OrchestratorError> {
        let path = self
            .task_dir(task_id)
            .join("artifacts")
            .join(format!("r{revision}-tests.json"));
        let bytes = tokio::fs::read(path).await?;
        serde_json::from_slice(&bytes)
            .map_err(|error| OrchestratorError::Config(error.to_string()))
    }

}
