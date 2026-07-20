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
        let approval: Option<(String, String)> = sqlx::query_as(
            "SELECT config_sha256,approved_at FROM project_config_trust WHERE project_id=?",
        )
        .bind(project_id)
        .fetch_optional(self.store.pool())
        .await?;
        let trusted = match (&snapshot.sha256, &approval) {
            (None, _) => true,
            (Some(actual), Some((approved, _))) => actual == approved,
            _ => false,
        };
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
            extra_allowed_commands: snapshot.config.agents.extra_allowed_commands,
            approved_at: approval.map(|(_, at)| at),
        })
    }

    pub async fn project_config_trust_approve(
        &self,
        project_id: &str,
    ) -> Result<ProjectConfigTrust, OrchestratorError> {
        let project = self.project(project_id).await?;
        let snapshot = self.config_snapshot(&project).await?;
        let sha = snapshot.sha256.ok_or_else(|| {
            OrchestratorError::Config("project.toml does not exist; no approval is needed".into())
        })?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO project_config_trust(project_id,config_sha256,approved_at) VALUES(?,?,?) \
             ON CONFLICT(project_id) DO UPDATE SET config_sha256=excluded.config_sha256,approved_at=excluded.approved_at",
        )
        .bind(project_id)
        .bind(&sha)
        .bind(&now)
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
