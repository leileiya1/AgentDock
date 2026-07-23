struct ProjectConfigSnapshot {
    config: ProjectConfig,
    sha256: Option<String>,
    path: PathBuf,
}

impl Orchestrator {
    async fn config_snapshot(
        &self,
        project: &ProjectRow,
    ) -> Result<ProjectConfigSnapshot, OrchestratorError> {
        let path = project.repo.join(".agentflow/project.toml");
        if !path.exists() {
            return Ok(ProjectConfigSnapshot {
                config: ProjectConfig {
                    schema_version: 1,
                    ..Default::default()
                },
                sha256: None,
                path,
            });
        }
        let bytes = tokio::fs::read(&path).await?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let config = toml::from_str(text)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        Ok(ProjectConfigSnapshot {
            config,
            sha256: Some(format!("{:x}", Sha256::digest(&bytes))),
            path,
        })
    }

    /// Loads repository configuration only when the exact bytes were approved locally.
    async fn load_trusted_config(
        &self,
        project: &ProjectRow,
    ) -> Result<ProjectConfig, OrchestratorError> {
        let snapshot = self.config_snapshot(project).await?;
        let Some(actual_sha) = snapshot.sha256 else {
            return Ok(snapshot.config);
        };
        let trusted_sha: Option<String> = sqlx::query_scalar(
            "SELECT config_sha256 FROM project_config_trust WHERE project_id=?",
        )
        .bind(&project.id)
        .fetch_optional(self.store.pool())
        .await?;
        if trusted_sha.as_deref() != Some(&actual_sha) {
            return Err(OrchestratorError::UntrustedProjectConfig { sha256: actual_sha });
        }
        Ok(snapshot.config)
    }

    pub async fn project_config_trust_get(
        &self,
        project_id: &str,
    ) -> Result<ProjectConfigTrust, OrchestratorError> {
        let project = self.project(project_id).await?;
        let snapshot = self.config_snapshot(&project).await?;
        let approval: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT config_sha256,approved_at,approved_summary_json FROM project_config_trust WHERE project_id=?",
        )
        .bind(project_id)
        .fetch_optional(self.store.pool())
        .await?;
        let trusted = match (&snapshot.sha256, &approval) {
            (None, _) => true,
            (Some(actual), Some((approved, _, _))) => actual == approved,
            _ => false,
        };
        let current_summary = safe_config_summary(&snapshot.config);
        let approved_summary = approval
            .as_ref()
            .and_then(|(_, _, summary)| summary.as_deref())
            .and_then(|summary| serde_json::from_str::<Value>(summary).ok());
        let changes = if trusted {
            Vec::new()
        } else {
            config_summary_changes(approved_summary.as_ref(), &current_summary)
        };
        let byte_only_change = !trusted
            && approval.is_some()
            && changes.is_empty()
            && snapshot.sha256.is_some();
        Ok(ProjectConfigTrust {
            exists: snapshot.sha256.is_some(),
            path: snapshot.path.to_string_lossy().into_owned(),
            sha256: snapshot.sha256,
            trusted,
            validation_steps: snapshot
                .config
                .validate
                .steps
                .iter()
                .map(|step| step.name.clone())
                .collect(),
            extra_allowed_commands: snapshot
                .config
                .agents
                .extra_allowed_commands
                .iter()
                .map(|command| safe_config_text(command))
                .collect(),
            validation_commands: snapshot
                .config
                .validate
                .steps
                .iter()
                .map(|step| ProjectConfigCommand {
                    name: step.name.clone(),
                    argv: step.argv.iter().map(|arg| safe_config_text(arg)).collect(),
                    timeout_secs: u32::try_from(step.timeout_secs).unwrap_or(u32::MAX),
                })
                .collect(),
            environment_allowlist: snapshot.config.reproducibility.env_allowlist.clone(),
            external_dependencies: snapshot
                .config
                .reproducibility
                .external_dependencies
                .keys()
                .cloned()
                .collect(),
            container_images: snapshot
                .config
                .reproducibility
                .container_images
                .iter()
                .map(|(image, digest)| format!("{image}@{digest}"))
                .collect(),
            lock_environment: snapshot.config.reproducibility.lock_environment,
            hermetic: snapshot.config.reproducibility.hermetic,
            previous_approved_sha256: approval.as_ref().map(|(sha, _, _)| sha.clone()),
            changes,
            byte_only_change,
            approved_at: approval.map(|(_, at, _)| at),
        })
    }

    pub async fn project_config_trust_approve(
        &self,
        project_id: &str,
        expected_sha256: &str,
    ) -> Result<ProjectConfigTrust, OrchestratorError> {
        let project = self.project(project_id).await?;
        let snapshot = self.config_snapshot(&project).await?;
        let sha = snapshot.sha256.ok_or_else(|| {
            OrchestratorError::Config("project.toml does not exist; no approval is needed".into())
        })?;
        // Bind the approval to the exact snapshot shown in the confirmation dialog.
        // Without this comparison a file edit between preview and click could approve
        // commands the user never reviewed (a classic time-of-check/time-of-use race).
        if sha != expected_sha256 {
            return Err(OrchestratorError::InvalidState(
                "PROJECT_CONFIG_CHANGED_DURING_APPROVAL".into(),
            ));
        }
        let summary = serde_json::to_string(&safe_config_summary(&snapshot.config))
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO project_config_trust(project_id,config_sha256,approved_at,approved_summary_json) VALUES(?,?,?,?) \
             ON CONFLICT(project_id) DO UPDATE SET config_sha256=excluded.config_sha256,approved_at=excluded.approved_at,approved_summary_json=excluded.approved_summary_json",
        )
        .bind(project_id)
        .bind(&sha)
        .bind(&now)
        .bind(summary)
        .execute(self.store.pool())
        .await?;
        sqlx::query("INSERT INTO events(actor,event_type,payload_json,created_at) VALUES('human','project_config:approve',?,?)")
            .bind(json!({"project_id":project_id,"sha256":sha}).to_string())
            .bind(&now)
            .execute(self.store.pool())
            .await?;
        self.project_config_trust_get(project_id).await
    }

    pub async fn project_config_trust_revoke(
        &self,
        project_id: &str,
    ) -> Result<ProjectConfigTrust, OrchestratorError> {
        sqlx::query("DELETE FROM project_config_trust WHERE project_id=?")
            .bind(project_id)
            .execute(self.store.pool())
            .await?;
        sqlx::query("INSERT INTO events(actor,event_type,payload_json,created_at) VALUES('human','project_config:revoke',?,?)")
            .bind(json!({"project_id":project_id}).to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        self.project_config_trust_get(project_id).await
    }
}

fn safe_config_summary(config: &ProjectConfig) -> Value {
    let external_dependencies = config
        .reproducibility
        .external_dependencies
        .iter()
        .map(|(name, state)| {
            let digest = format!("{:x}", Sha256::digest(state.as_bytes()));
            (name.clone(), format!("sha256:{}", &digest[..12]))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    json!({
        "schema_version": config.schema_version,
        "validate": {
            "steps": config.validate.steps.iter().map(|step| json!({
                "name": step.name,
                "argv": step.argv.iter().map(|arg| safe_config_text(arg)).collect::<Vec<_>>(),
                "timeout_secs": step.timeout_secs,
            })).collect::<Vec<_>>()
        },
        "review": {
            "exclude_globs": config.review.exclude_globs,
            "max_patch_bytes": config.review.max_patch_bytes,
        },
        "agents": {
            "extra_allowed_commands": config.agents.extra_allowed_commands.iter()
                .map(|command| safe_config_text(command)).collect::<Vec<_>>()
        },
        "reproducibility": {
            "lock_environment": config.reproducibility.lock_environment,
            "hermetic": config.reproducibility.hermetic,
            "env_allowlist": config.reproducibility.env_allowlist,
            "external_dependencies": external_dependencies,
            "container_images": config.reproducibility.container_images,
        }
    })
}

fn safe_config_text(value: &str) -> String {
    let redacted = agentflow_process_supervisor::redact(value.to_string());
    let lower = redacted.to_ascii_lowercase();
    for marker in ["password=", "token=", "secret=", "api_key=", "apikey="] {
        if let Some(index) = lower.find(marker) {
            return format!("{}***", &redacted[..index + marker.len()]);
        }
    }
    redacted.chars().take(500).collect()
}

fn config_summary_changes(previous: Option<&Value>, current: &Value) -> Vec<ProjectConfigChange> {
    let mut before = std::collections::BTreeMap::new();
    let mut after = std::collections::BTreeMap::new();
    if let Some(previous) = previous {
        flatten_config_summary("", previous, &mut before);
    }
    flatten_config_summary("", current, &mut after);
    before
        .keys()
        .chain(after.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| {
            let old = before.get(path);
            let new = after.get(path);
            (old != new).then(|| ProjectConfigChange {
                path: (*path).clone(),
                kind: match (old, new) {
                    (None, Some(_)) => ProjectConfigChangeKind::Added,
                    (Some(_), None) => ProjectConfigChangeKind::Removed,
                    _ => ProjectConfigChangeKind::Changed,
                },
                before: old.cloned(),
                after: new.cloned(),
                high_risk: config_path_is_high_risk(path),
            })
        })
        .collect()
}

fn flatten_config_summary(
    prefix: &str,
    value: &Value,
    output: &mut std::collections::BTreeMap<String, String>,
) {
    if let Value::Object(object) = value {
        for (key, value) in object {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten_config_summary(&path, value, output);
        }
    } else {
        let rendered = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
        output.insert(prefix.to_string(), rendered.chars().take(800).collect());
    }
}

fn config_path_is_high_risk(path: &str) -> bool {
    path.starts_with("validate.steps")
        || path.starts_with("agents.extra_allowed_commands")
        || path.starts_with("reproducibility.env_allowlist")
        || path.starts_with("reproducibility.external_dependencies")
        || path.starts_with("reproducibility.container_images")
        || path.ends_with("lock_environment")
        || path.ends_with("hermetic")
}
