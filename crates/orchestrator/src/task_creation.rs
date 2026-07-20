impl Orchestrator {
    pub async fn project_import(&self, path: &Path) -> Result<Project, OrchestratorError> {
        let canonical = tokio::fs::canonicalize(path).await?;
        if !self.git.is_repo(&canonical).await {
            return Err(OrchestratorError::InvalidState("PROJECT_NOT_GIT".into()));
        }
        let name = canonical
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project");
        let branch = self.git.default_branch(&canonical).await?;
        let compatibility = self.git.compatibility_report(&canonical).await?;
        let root = self.app_data.join("wt");
        Ok(self
            .store
            .import_project_identified(
                name,
                &canonical.to_string_lossy(),
                &branch,
                &root.to_string_lossy(),
                Some(&compatibility.repository_identity),
            )
            .await?)
    }

    pub async fn project_git_compatibility(
        &self,
        project_id: &str,
    ) -> Result<GitCompatibilityReport, OrchestratorError> {
        let project = self.project(project_id).await?;
        if !project.repo.exists() {
            return Err(OrchestratorError::InvalidState(
                "PROJECT_RELOCATED: re-import the repository from its new path".into(),
            ));
        }
        self.git
            .compatibility_report(&project.repo)
            .await
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn task_create(
        &self,
        project_id: &str,
        title: &str,
        description: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        target_branch: Option<&str>,
        max_revisions: Option<i64>,
    ) -> Result<TaskSummary, OrchestratorError> {
        self.task_create_with_api_egress(
            project_id,
            title,
            description,
            developer,
            reviewer,
            target_branch,
            max_revisions,
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn task_create_with_api_egress(
        &self,
        project_id: &str,
        title: &str,
        description: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        target_branch: Option<&str>,
        max_revisions: Option<i64>,
        allow_api_egress: bool,
    ) -> Result<TaskSummary, OrchestratorError> {
        let policy = TaskPolicy {
            require_plan_approval: false,
            ..TaskPolicy::default()
        };
        self.task_create_governed(
            project_id,
            title,
            description,
            developer,
            reviewer,
            target_branch,
            max_revisions,
            allow_api_egress,
            policy,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn task_create_governed(
        &self,
        project_id: &str,
        title: &str,
        description: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        target_branch: Option<&str>,
        max_revisions: Option<i64>,
        allow_api_egress: bool,
        policy: TaskPolicy,
    ) -> Result<TaskSummary, OrchestratorError> {
        validate_task_policy(&policy)?;
        self.refresh_provider_registry().await;
        let project = self.project(project_id).await?;
        if developer == reviewer {
            return Err(OrchestratorError::InvalidState("TASK_SAME_AGENT".into()));
        }
        let (developer_capable, reviewer_capable, developer_egress, reviewer_egress) = {
            let registry = self.provider_registry.read().map_err(|_| {
                OrchestratorError::Config("provider registry lock is poisoned".into())
            })?;
            (
                registry
                    .get(&developer)
                    .map_or(!developer.is_api(), |provider| {
                        provider.manifest.capabilities.development
                    }),
                registry.get(&reviewer).map_or(
                    !matches!(reviewer, AgentKind::External(_)),
                    |provider| provider.manifest.capabilities.review,
                ),
                registry.get(&developer).map_or(developer.is_api(), |provider| {
                    provider.manifest.execution_location != ExecutionLocation::Local
                        || provider.manifest.data_egress != DataEgress::None
                        || !provider.manifest.permissions.network_domains.is_empty()
                }),
                registry.get(&reviewer).map_or(reviewer.is_api(), |provider| {
                    provider.manifest.execution_location != ExecutionLocation::Local
                        || provider.manifest.data_egress != DataEgress::None
                        || !provider.manifest.permissions.network_domains.is_empty()
                }),
            )
        };
        if !developer_capable {
            return Err(OrchestratorError::InvalidState(
                "selected provider does not support development".into(),
            ));
        }
        if policy.require_plan_approval && matches!(developer, AgentKind::External(_)) {
            return Err(OrchestratorError::InvalidState(
                "external Provider planning capability is not available in protocol v1.1".into(),
            ));
        }
        if !reviewer_capable {
            return Err(OrchestratorError::InvalidState(
                "selected provider does not support review".into(),
            ));
        }
        let council_egress = project.settings.review_council.enabled
            && project
                .settings
                .review_council
                .reviewers
                .iter()
                .any(|agent| self.provider_requires_egress(agent));
        let mut egress_providers = Vec::new();
        if developer_egress {
            egress_providers.push(developer.clone());
        }
        if reviewer_egress && !egress_providers.contains(&reviewer) {
            egress_providers.push(reviewer.clone());
        }
        if project.settings.review_council.enabled {
            for agent in &project.settings.review_council.reviewers {
                if self.provider_requires_egress(agent) && !egress_providers.contains(agent) {
                    egress_providers.push(agent.clone());
                }
            }
        }
        if (developer_egress || reviewer_egress || council_egress) && !allow_api_egress {
            return Err(OrchestratorError::InvalidState(
                "API_EGRESS_APPROVAL_REQUIRED".into(),
            ));
        }
        if let Some(node_id) = policy.execution_node_id.as_deref() {
            let enabled: Option<i64> = sqlx::query_scalar(
                "SELECT enabled FROM execution_nodes WHERE id=?",
            )
            .bind(node_id)
            .fetch_optional(self.store.pool())
            .await?;
            if enabled != Some(1) {
                return Err(OrchestratorError::InvalidState(
                    "REMOTE_NODE_UNAVAILABLE".into(),
                ));
            }
        }
        let task = self
            .store
            .create_governed_task_with_api_egress(
                project_id,
                title,
                description,
                developer.clone(),
                reviewer.clone(),
                target_branch.unwrap_or(&project.default_branch),
                max_revisions.unwrap_or(3),
                allow_api_egress,
                &policy,
            )
            .await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO delivery_records(task_id,mode,state,created_at,updated_at) VALUES(?,?,'pending',?,?)")
            .bind(&task.id)
            .bind(policy.delivery_mode.to_string())
            .bind(&now)
            .bind(&now)
            .execute(self.store.pool())
            .await?;
        if allow_api_egress {
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,0,'human','privacy:api_egress_approved',?,?)")
                .bind(&task.id)
                .bind(json!({
                    "developer": developer,
                    "reviewer": reviewer,
                    "egress_providers": egress_providers,
                }).to_string())
                .bind(Utc::now().to_rfc3339())
                .execute(self.store.pool())
                .await?;
        }
        Ok(task)
    }
}

fn validate_task_policy(policy: &TaskPolicy) -> Result<(), OrchestratorError> {
    if !(-100..=100).contains(&policy.priority)
        || policy.token_budget.is_some_and(|value| value <= 0)
        || policy.cost_budget_usd.is_some_and(|value| value <= 0.0 || !value.is_finite())
        || policy.time_budget_secs.is_some_and(|value| value <= 0)
        || policy.minimum_quality_score > 100
    {
        return Err(OrchestratorError::Config(
            "task budgets must be positive and quality score must be 0..100".into(),
        ));
    }
    Ok(())
}
