impl Orchestrator {
    /// Evaluates a canonical operation without trusting Provider prose. If human input is
    /// required, the request and task pause are committed atomically before this returns.
    pub async fn permission_request(
        &self,
        input: PermissionRequestInput,
    ) -> Result<PermissionRequest, OrchestratorError> {
        let task = self.task(&input.task_id).await?;
        let project = self.project(&task.project_id).await?;
        let operation_root = task.worktree_path.as_deref().unwrap_or(&project.repo);
        let canonical_root = std::fs::canonicalize(operation_root)?;
        let operation = normalize_permission_operation(&input.operation, &canonical_root)?;
        let action_type = canonical_permission_action(input.action_type, &operation, &canonical_root);
        let operation_sha256 = permission_operation_sha(action_type, &operation)?;
        let policy_sha256 = self.permission_policy_sha(&task.id).await?;
        let (risk_level, grantable) = classify_permission(
            action_type,
            input.role,
            &operation,
            task.worktree_path.as_deref(),
        );
        let now = Utc::now();

        if let Some(existing) = self
            .pending_permission_request(&task.id, &operation_sha256, &policy_sha256)
            .await?
        {
            sqlx::query("UPDATE permission_requests SET request_count=request_count+1 WHERE id=?")
                .bind(&existing.id)
                .execute(self.store.pool())
                .await?;
            return self.permission_request_get(&existing.id).await;
        }

        let id = Uuid::now_v7().to_string();
        let expires_at = (now + chrono::Duration::minutes(PERMISSION_TTL_MINUTES)).to_rfc3339();
        let summary = permission_summary(action_type, &operation);
        let reason = safe_permission_reason(&input.reason);
        let resume_status = task.status.to_string();
        let retry_revision = if matches!(task.status, TaskStatus::Developing | TaskStatus::Revising) {
            task.revision.saturating_sub(1)
        } else {
            task.revision
        };
        let status = if grantable {
            PermissionRequestStatus::Pending
        } else {
            PermissionRequestStatus::Denied
        };
        let prior_denials: i64 = if grantable {
            0
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM permission_requests WHERE task_id=? AND operation_sha256=? AND status='denied'")
                .bind(&task.id).bind(&operation_sha256).fetch_one(self.store.pool()).await?
        };
        let mut transaction = self.store.pool().begin().await?;
        sqlx::query("INSERT INTO permission_requests(id,project_id,task_id,revision,run_id,provider_id,role,action_type,summary,reason,operation_json,risk_level,grantable,operation_sha256,policy_sha256,status,resume_status,retry_revision,requested_at,expires_at,decided_at,provider_resume_token) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(&id).bind(&task.project_id).bind(&task.id).bind(task.revision)
            .bind(&input.run_id).bind(input.provider_id.to_string()).bind(input.role.to_string())
            .bind(action_type.to_string()).bind(&summary).bind(&reason)
            .bind(serde_json::to_string(&operation).map_err(|error| OrchestratorError::Config(error.to_string()))?)
            .bind(risk_level.to_string()).bind(i64::from(grantable)).bind(&operation_sha256)
            .bind(&policy_sha256).bind(status.to_string()).bind(&resume_status).bind(retry_revision)
            .bind(now.to_rfc3339()).bind(&expires_at)
            .bind((!grantable).then(|| now.to_rfc3339()))
            // Resume tokens are Provider capabilities, not audit facts. Until protected token
            // storage is available we deliberately checkpoint-restart instead of persisting one.
            .bind(Option::<String>::None)
            .execute(&mut *transaction).await?;
        if grantable && matches!(task.status, TaskStatus::Developing | TaskStatus::Revising) {
            sqlx::query("UPDATE tasks SET status='BLOCKED',blocked_reason='permission_required',blocked_detail=?,updated_at=? WHERE id=? AND current_revision=?")
                .bind(&summary).bind(now.to_rfc3339()).bind(&task.id).bind(task.revision)
                .execute(&mut *transaction).await?;
        } else if !grantable && matches!(task.status, TaskStatus::Developing | TaskStatus::Revising) {
            if prior_denials >= 2 {
                sqlx::query("UPDATE tasks SET status='BLOCKED',blocked_reason='run_failed',blocked_detail=?,updated_at=? WHERE id=?")
                    .bind("Provider 重复请求不可授权的系统能力；任务已安全停止").bind(now.to_rfc3339()).bind(&task.id)
                    .execute(&mut *transaction).await?;
            } else {
                let ready = if task.status == TaskStatus::Developing { TaskStatus::ReadyForDevelopment } else { TaskStatus::ReadyForRevision };
                sqlx::query("UPDATE tasks SET status=?,current_revision=?,blocked_reason=NULL,blocked_detail=?,updated_at=? WHERE id=?")
                    .bind(ready.to_string()).bind(retry_revision).bind("该能力不可授权，请改用不访问系统或秘密的方案")
                    .bind(now.to_rfc3339()).bind(&task.id).execute(&mut *transaction).await?;
            }
        }
        let event_type = if grantable { "permission:requested" } else { "permission:denied" };
        sqlx::query("INSERT INTO events(task_id,run_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,?,'orchestrator',?,?,?)")
            .bind(&task.id).bind(&input.run_id).bind(task.revision).bind(event_type)
            .bind(json!({"request_id":id,"action_type":action_type,"risk_level":risk_level,"grantable":grantable,"operation_sha256":operation_sha256,"policy_sha256":policy_sha256}).to_string())
            .bind(now.to_rfc3339()).execute(&mut *transaction).await?;
        transaction.commit().await?;
        self.permission_request_get(&id).await
    }

    /// Checks stored grants immediately before an operation. Once grants are consumed in the
    /// same transaction as the audit event, preventing double execution after a crash/retry.
    pub async fn permission_authorize(
        &self,
        task_id: &str,
        provider: &AgentKind,
        role: RunRole,
        action: PermissionActionType,
        operation: &PermissionOperation,
    ) -> Result<PermissionAuthorization, OrchestratorError> {
        let task = self.task(task_id).await?;
        let project = self.project(&task.project_id).await?;
        let operation_root = task.worktree_path.as_deref().unwrap_or(&project.repo);
        let canonical_root = std::fs::canonicalize(operation_root)?;
        let operation = normalize_permission_operation(operation, &canonical_root)?;
        let action = canonical_permission_action(action, &operation, &canonical_root);
        let sha = permission_operation_sha(action, &operation)?;
        let (risk, grantable) = classify_permission(action, role, &operation, task.worktree_path.as_deref());
        if !grantable {
            return Ok(PermissionAuthorization::Denied);
        }
        if default_permission_allowed(action, role, risk, &operation, task.worktree_path.as_deref()) {
            return Ok(PermissionAuthorization::AllowedByDefault);
        }
        let now = Utc::now().to_rfc3339();
        let policy_sha = self.permission_policy_sha(task_id).await?;
        let identity = self.project_identity(&task.project_id).await?;
        let mut transaction = self.store.pool().begin().await?;
        let once = sqlx::query("SELECT d.id FROM permission_decisions d JOIN permission_requests r ON r.id=d.request_id WHERE r.task_id=? AND r.operation_sha256=? AND r.policy_sha256=? AND r.provider_id=? AND r.role=? AND d.decision='approve' AND d.scope='once' AND d.consumed_at IS NULL AND d.expires_at>? ORDER BY d.created_at LIMIT 1")
            .bind(task_id).bind(&sha).bind(&policy_sha).bind(provider.to_string()).bind(role.to_string()).bind(&now)
            .fetch_optional(&mut *transaction).await?;
        if let Some(row) = once {
            let id: String = row.get("id");
            sqlx::query("UPDATE permission_decisions SET consumed_at=? WHERE id=? AND consumed_at IS NULL")
                .bind(&now).bind(&id).execute(&mut *transaction).await?;
            record_permission_match(&mut transaction, task_id, "once", &id, &sha, &now).await?;
            transaction.commit().await?;
            return Ok(PermissionAuthorization::AllowedByGrant);
        }
        let task_grant = sqlx::query("SELECT d.id FROM permission_decisions d JOIN permission_requests r ON r.id=d.request_id WHERE r.task_id=? AND r.operation_sha256=? AND r.policy_sha256=? AND r.provider_id=? AND r.role=? AND d.decision='approve' AND d.scope='task' AND d.expires_at>? ORDER BY d.created_at DESC LIMIT 1")
            .bind(task_id).bind(&sha).bind(&policy_sha).bind(provider.to_string()).bind(role.to_string()).bind(&now)
            .fetch_optional(&mut *transaction).await?;
        if let Some(row) = task_grant {
            let id: String = row.get("id");
            record_permission_match(&mut transaction, task_id, "task", &id, &sha, &now).await?;
            transaction.commit().await?;
            return Ok(PermissionAuthorization::AllowedByGrant);
        }
        let rules = sqlx::query("SELECT id,operation_json FROM permission_rules WHERE project_id=? AND project_identity=? AND provider_id=? AND role=? AND action_type=? AND enabled=1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?)")
            .bind(&task.project_id).bind(identity).bind(provider.to_string()).bind(role.to_string())
            .bind(action.to_string()).bind(&now).fetch_all(&mut *transaction).await?;
        for row in rules {
            let rule_operation: PermissionOperation = serde_json::from_str(&row.get::<String,_>("operation_json"))
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
            if rule_operation == operation {
                let id: String = row.get("id");
                sqlx::query("UPDATE permission_rules SET last_matched_at=? WHERE id=? AND enabled=1")
                    .bind(&now).bind(&id).execute(&mut *transaction).await?;
                record_permission_match(&mut transaction, task_id, "project_rule", &id, &sha, &now).await?;
                transaction.commit().await?;
                return Ok(PermissionAuthorization::AllowedByGrant);
            }
        }
        transaction.commit().await?;
        Ok(PermissionAuthorization::PermissionRequired)
    }

    pub async fn permission_decide(
        &self,
        input: PermissionDecisionInput,
    ) -> Result<PermissionDecision, OrchestratorError> {
        let request = self.permission_request_get(&input.request_id).await?;
        if request.status == PermissionRequestStatus::Expired {
            return Err(permission_error(PermissionErrorCode::Expired));
        }
        if request.status != PermissionRequestStatus::Pending {
            return Err(permission_error(PermissionErrorCode::RequestStale));
        }
        if !request.grantable && input.decision == PermissionDecisionKind::Approve {
            return Err(permission_error(PermissionErrorCode::NotGrantable));
        }
        if request.operation_sha256 != input.operation_sha256 || request.policy_sha256 != input.policy_sha256 {
            return Err(permission_error(PermissionErrorCode::RequestStale));
        }
        if request.expires_at <= Utc::now().to_rfc3339() {
            self.expire_permission_request(&request.id).await?;
            return Err(permission_error(PermissionErrorCode::Expired));
        }
        let current_task = self.task(&request.task_id).await?;
        let current_summary = self.store.task_summary(&request.task_id).await?;
        let current_policy = self.permission_policy_sha(&request.task_id).await?;
        let current_operation = permission_operation_sha(request.action_type, &request.operation)?;
        if current_task.revision != request.revision || current_policy != request.policy_sha256 || current_operation != request.operation_sha256 {
            return Err(permission_error(PermissionErrorCode::RequestStale));
        }
        if input.scope == PermissionGrantScope::ProjectRule
            && (request.risk_level == PermissionRiskLevel::High || rule_is_too_broad(&request.operation))
        {
            return Err(permission_error(PermissionErrorCode::RuleTooBroad));
        }
        let now = Utc::now();
        let expires_at = match input.scope {
            PermissionGrantScope::Once => Some((now + chrono::Duration::minutes(PERMISSION_TTL_MINUTES)).to_rfc3339()),
            PermissionGrantScope::Task => Some((now + chrono::Duration::hours(TASK_GRANT_TTL_HOURS)).to_rfc3339()),
            PermissionGrantScope::ProjectRule => None,
        };
        let decision_id = Uuid::now_v7().to_string();
        let project_identity = if input.decision == PermissionDecisionKind::Approve
            && input.scope == PermissionGrantScope::ProjectRule
        {
            Some(self.project_identity(&request.project_id).await?)
        } else {
            None
        };
        let mut transaction = self.store.pool().begin().await?;
        let status = match input.decision {
            PermissionDecisionKind::Approve => PermissionRequestStatus::Approved,
            PermissionDecisionKind::Deny => PermissionRequestStatus::Denied,
            PermissionDecisionKind::CancelTask => PermissionRequestStatus::Cancelled,
        };
        sqlx::query("UPDATE permission_requests SET status=?,decided_at=? WHERE id=? AND status='pending'")
            .bind(status.to_string()).bind(now.to_rfc3339()).bind(&request.id)
            .execute(&mut *transaction).await?;
        sqlx::query("INSERT INTO permission_decisions(id,request_id,operation_sha256,policy_sha256,decision,scope,expires_at,approved_by,guidance,created_at) VALUES(?,?,?,?,?,?,?,'human',?,?)")
            .bind(&decision_id).bind(&request.id).bind(&request.operation_sha256).bind(&request.policy_sha256)
            .bind(input.decision.to_string()).bind(input.scope.to_string()).bind(&expires_at)
            .bind(input.guidance.as_deref().map(safe_permission_reason)).bind(now.to_rfc3339())
            .execute(&mut *transaction).await?;
        if input.decision == PermissionDecisionKind::Approve && input.scope == PermissionGrantScope::ProjectRule {
            let identity = project_identity.ok_or_else(|| {
                OrchestratorError::InvalidState("PERMISSION_PROJECT_IDENTITY_MISSING".into())
            })?;
            let rule_id = Uuid::now_v7().to_string();
            let rule_sha = permission_rule_sha(&identity, &request)?;
            sqlx::query("INSERT INTO permission_rules(id,project_id,project_identity,provider_id,role,action_type,operation_json,rule_sha256,created_by,created_at) VALUES(?,?,?,?,?,?,?,?,'human',?)")
                .bind(rule_id).bind(&request.project_id).bind(identity).bind(request.provider_id.to_string())
                .bind(request.role.to_string()).bind(request.action_type.to_string())
                .bind(serde_json::to_string(&request.operation).map_err(|error| OrchestratorError::Config(error.to_string()))?)
                .bind(rule_sha).bind(now.to_rfc3339()).execute(&mut *transaction).await?;
        }
        match input.decision {
            PermissionDecisionKind::Approve if current_summary.status == TaskStatus::Blocked
                && current_summary.blocked_reason == Some(BlockedReason::PermissionRequired) => {
                let resume_status: String = sqlx::query_scalar("SELECT resume_status FROM permission_requests WHERE id=?")
                    .bind(&request.id).fetch_one(&mut *transaction).await?;
                let retry_revision: i64 = sqlx::query_scalar("SELECT retry_revision FROM permission_requests WHERE id=?")
                    .bind(&request.id).fetch_one(&mut *transaction).await?;
                let ready = if resume_status == TaskStatus::Developing.to_string() { TaskStatus::ReadyForDevelopment } else { TaskStatus::ReadyForRevision };
                sqlx::query("UPDATE tasks SET status=?,blocked_reason=NULL,blocked_detail=NULL,current_revision=?,updated_at=? WHERE id=?")
                    .bind(ready.to_string()).bind(retry_revision).bind(now.to_rfc3339()).bind(&request.task_id)
                    .execute(&mut *transaction).await?;
            }
            PermissionDecisionKind::CancelTask => {
                sqlx::query("UPDATE tasks SET status='CANCELLED',blocked_reason=NULL,blocked_detail=NULL,updated_at=? WHERE id=?")
                    .bind(now.to_rfc3339()).bind(&request.task_id).execute(&mut *transaction).await?;
            }
            PermissionDecisionKind::Deny
                if current_summary.status == TaskStatus::Blocked
                    && current_summary.blocked_reason == Some(BlockedReason::PermissionRequired) => {
                    let resume_status: String = sqlx::query_scalar("SELECT resume_status FROM permission_requests WHERE id=?").bind(&request.id).fetch_one(&mut *transaction).await?;
                    let retry_revision: i64 = sqlx::query_scalar("SELECT retry_revision FROM permission_requests WHERE id=?").bind(&request.id).fetch_one(&mut *transaction).await?;
                    let ready = if resume_status == TaskStatus::Developing.to_string() { TaskStatus::ReadyForDevelopment } else { TaskStatus::ReadyForRevision };
                    sqlx::query("UPDATE tasks SET status=?,current_revision=?,blocked_reason=NULL,blocked_detail=?,updated_at=? WHERE id=?")
                        .bind(ready.to_string()).bind(retry_revision)
                        .bind(input.guidance.as_deref().map(safe_permission_reason).unwrap_or_else(|| "权限请求已拒绝；请改用受限方案".into()))
                        .bind(now.to_rfc3339()).bind(&request.task_id).execute(&mut *transaction).await?;
            }
            _ => {}
        }
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'human','permission:decided',?,?)")
            .bind(&request.task_id).bind(request.revision)
            .bind(json!({"request_id":request.id,"decision":input.decision,"scope":input.scope,"operation_sha256":request.operation_sha256,"policy_sha256":request.policy_sha256,"expires_at":expires_at}).to_string())
            .bind(now.to_rfc3339()).execute(&mut *transaction).await?;
        transaction.commit().await?;
        self.permission_decision_get(&decision_id).await
    }

    pub async fn permission_requests(&self, task_id: &str) -> Result<Vec<PermissionRequest>, OrchestratorError> {
        self.permission_expire_pending().await?;
        let ids = sqlx::query_scalar::<_, String>("SELECT id FROM permission_requests WHERE task_id=? ORDER BY requested_at DESC")
            .bind(task_id).fetch_all(self.store.pool()).await?;
        let mut values = Vec::with_capacity(ids.len());
        for id in ids { values.push(self.permission_request_get(&id).await?); }
        Ok(values)
    }

    pub async fn permission_expire_pending(&self) -> Result<u64, OrchestratorError> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query("SELECT id,task_id,revision,operation_sha256 FROM permission_requests WHERE status='pending' AND expires_at<=?")
            .bind(&now).fetch_all(self.store.pool()).await?;
        if rows.is_empty() { return Ok(0); }
        let mut transaction = self.store.pool().begin().await?;
        for row in &rows {
            let id: String = row.get("id");
            let task_id: String = row.get("task_id");
            sqlx::query("UPDATE permission_requests SET status='expired',decided_at=? WHERE id=? AND status='pending'")
                .bind(&now).bind(&id).execute(&mut *transaction).await?;
            sqlx::query("UPDATE tasks SET blocked_detail='权限请求已过期；请重新运行或补充安全方案',updated_at=? WHERE id=? AND blocked_reason='permission_required'")
                .bind(&now).bind(&task_id).execute(&mut *transaction).await?;
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'system','permission:expired',?,?)")
                .bind(&task_id).bind(row.get::<i64,_>("revision"))
                .bind(json!({"request_id":id,"operation_sha256":row.get::<String,_>("operation_sha256")}).to_string())
                .bind(&now).execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(rows.len() as u64)
    }

    pub async fn permission_effective_for_run(
        &self,
        task_id: &str,
        provider: &AgentKind,
        role: RunRole,
    ) -> Result<EffectivePermissions, OrchestratorError> {
        let task = self.task(task_id).await?;
        let mut effective = EffectivePermissions {
            worktree_read: true,
            worktree_write: role == RunRole::Developer,
            sandbox_guarantee: if role == RunRole::Developer {
                SandboxGuarantee::WorktreeRestricted
            } else {
                SandboxGuarantee::ReadOnly
            },
            ..Default::default()
        };
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query("SELECT r.action_type,r.operation_json FROM permission_decisions d JOIN permission_requests r ON r.id=d.request_id WHERE r.task_id=? AND r.provider_id=? AND r.role=? AND d.decision='approve' AND d.scope IN ('once','task') AND d.expires_at>? AND (d.scope='task' OR d.consumed_at IS NULL) ORDER BY d.created_at")
            .bind(task_id).bind(provider.to_string()).bind(role.to_string()).bind(&now)
            .fetch_all(self.store.pool()).await?;
        for row in rows {
            let action: PermissionActionType = parse(row.get("action_type"))?;
            let operation: PermissionOperation = serde_json::from_str(&row.get::<String,_>("operation_json"))
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
            if self.permission_authorize(task_id, provider, role, action, &operation).await?
                == PermissionAuthorization::AllowedByGrant
            {
                extend_effective_permissions(&mut effective, action, &operation);
            }
        }
        let identity = self.project_identity(&task.project_id).await?;
        let rules = sqlx::query("SELECT action_type,operation_json FROM permission_rules WHERE project_id=? AND project_identity=? AND provider_id=? AND role=? AND enabled=1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?) ORDER BY rule_sha256")
            .bind(&task.project_id).bind(identity).bind(provider.to_string()).bind(role.to_string()).bind(&now)
            .fetch_all(self.store.pool()).await?;
        for row in rules {
            let action = parse(row.get("action_type"))?;
            let operation: PermissionOperation = serde_json::from_str(&row.get::<String,_>("operation_json"))
                .map_err(|error| OrchestratorError::Config(error.to_string()))?;
            extend_effective_permissions(&mut effective, action, &operation);
        }
        effective.command_argv.sort(); effective.command_argv.dedup();
        effective.external_paths.sort_by(|a,b| a.path.cmp(&b.path)); effective.external_paths.dedup();
        effective.network_domains.sort(); effective.network_domains.dedup();
        effective.environment_names.sort(); effective.environment_names.dedup();
        Ok(effective)
    }

    pub async fn permission_request_task_id(&self, request_id: &str) -> Result<String, OrchestratorError> {
        Ok(sqlx::query_scalar("SELECT task_id FROM permission_requests WHERE id=?")
            .bind(request_id).fetch_one(self.store.pool()).await?)
    }

    pub async fn permission_rules(&self, project_id: &str) -> Result<Vec<PermissionRule>, OrchestratorError> {
        self.project(project_id).await?;
        let rows = sqlx::query("SELECT * FROM permission_rules WHERE project_id=? ORDER BY created_at DESC")
            .bind(project_id).fetch_all(self.store.pool()).await?;
        rows.into_iter().map(permission_rule_from_row).collect()
    }

    pub async fn permission_rule_revoke(&self, project_id: &str, rule_id: &str) -> Result<PermissionRule, OrchestratorError> {
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE permission_rules SET enabled=0,revoked_at=? WHERE id=? AND project_id=? AND enabled=1")
            .bind(&now).bind(rule_id).bind(project_id).execute(self.store.pool()).await?;
        if changed.rows_affected() != 1 { return Err(OrchestratorError::InvalidState("PERMISSION_RULE_NOT_ACTIVE".into())); }
        sqlx::query("INSERT INTO events(actor,event_type,payload_json,created_at) VALUES('human','permission:rule_revoked',?,?)")
            .bind(json!({"project_id":project_id,"rule_id":rule_id}).to_string()).bind(&now)
            .execute(self.store.pool()).await?;
        let row = sqlx::query("SELECT * FROM permission_rules WHERE id=?").bind(rule_id).fetch_one(self.store.pool()).await?;
        permission_rule_from_row(row)
    }

    async fn permission_policy_sha(&self, task_id: &str) -> Result<String, OrchestratorError> {
        let task = self.task(task_id).await?;
        let identity = self.project_identity(&task.project_id).await?;
        let config_sha: Option<String> = sqlx::query_scalar("SELECT config_sha256 FROM project_config_trust WHERE project_id=?")
            .bind(&task.project_id).fetch_optional(self.store.pool()).await?.flatten();
        let rule_shas = sqlx::query_scalar::<_, String>("SELECT rule_sha256 FROM permission_rules WHERE project_id=? AND project_identity=? AND enabled=1 AND revoked_at IS NULL ORDER BY rule_sha256")
            .bind(&task.project_id).bind(&identity).fetch_all(self.store.pool()).await?;
        seal_json(&json!({"version":PERMISSION_POLICY_VERSION,"project_identity":identity,"task_id":task.id,"revision":task.revision,"config_sha256":config_sha,"rule_sha256":rule_shas}))
    }

    async fn project_identity(&self, project_id: &str) -> Result<String, OrchestratorError> {
        let row = sqlx::query("SELECT repo_identity,repo_path FROM projects WHERE id=?")
            .bind(project_id).fetch_one(self.store.pool()).await?;
        Ok(row.get::<Option<String>,_>("repo_identity").unwrap_or_else(|| row.get("repo_path")))
    }

    async fn pending_permission_request(&self, task_id: &str, operation_sha: &str, policy_sha: &str) -> Result<Option<PermissionRequest>, OrchestratorError> {
        let id = sqlx::query_scalar::<_,String>("SELECT id FROM permission_requests WHERE task_id=? AND operation_sha256=? AND policy_sha256=? AND status='pending' AND expires_at>? ORDER BY requested_at DESC LIMIT 1")
            .bind(task_id).bind(operation_sha).bind(policy_sha).bind(Utc::now().to_rfc3339())
            .fetch_optional(self.store.pool()).await?;
        match id { Some(id) => Ok(Some(self.permission_request_get(&id).await?)), None => Ok(None) }
    }

    async fn permission_request_get(&self, id: &str) -> Result<PermissionRequest, OrchestratorError> {
        let row = sqlx::query("SELECT * FROM permission_requests WHERE id=?").bind(id).fetch_one(self.store.pool()).await?;
        permission_request_from_row(row)
    }

    async fn permission_decision_get(&self, id: &str) -> Result<PermissionDecision, OrchestratorError> {
        let row = sqlx::query("SELECT * FROM permission_decisions WHERE id=?").bind(id).fetch_one(self.store.pool()).await?;
        Ok(PermissionDecision { id: row.get("id"), request_id: row.get("request_id"), operation_sha256: row.get("operation_sha256"), policy_sha256: row.get("policy_sha256"), decision: parse(row.get("decision"))?, scope: parse(row.get("scope"))?, expires_at: row.get("expires_at"), approved_by: row.get("approved_by"), guidance: row.get("guidance"), created_at: row.get("created_at"), consumed_at: row.get("consumed_at") })
    }

    async fn expire_permission_request(&self, id: &str) -> Result<(), OrchestratorError> {
        sqlx::query("UPDATE permission_requests SET status='expired',decided_at=? WHERE id=? AND status='pending'")
            .bind(Utc::now().to_rfc3339()).bind(id).execute(self.store.pool()).await?;
        Ok(())
    }
}

async fn record_permission_match(transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>, task_id: &str, scope: &str, source_id: &str, operation_sha: &str, now: &str) -> Result<(), OrchestratorError> {
    sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'orchestrator','permission:auto_matched',?,?)")
        .bind(task_id).bind(json!({"scope":scope,"source_id":source_id,"operation_sha256":operation_sha}).to_string())
        .bind(now).execute(&mut **transaction).await?;
    Ok(())
}

fn permission_request_from_row(row: sqlx::sqlite::SqliteRow) -> Result<PermissionRequest, OrchestratorError> {
    Ok(PermissionRequest { id: row.get("id"), project_id: row.get("project_id"), task_id: row.get("task_id"), revision: row.get("revision"), run_id: row.get("run_id"), provider_id: parse(row.get("provider_id"))?, role: parse(row.get("role"))?, action_type: parse(row.get("action_type"))?, summary: row.get("summary"), reason: row.get("reason"), operation: serde_json::from_str(&row.get::<String,_>("operation_json")).map_err(|error| OrchestratorError::Config(error.to_string()))?, risk_level: parse(row.get("risk_level"))?, grantable: row.get::<i64,_>("grantable") != 0, operation_sha256: row.get("operation_sha256"), policy_sha256: row.get("policy_sha256"), status: parse(row.get("status"))?, matched_rule_id: row.get("matched_rule_id"), request_count: row.get("request_count"), requested_at: row.get("requested_at"), expires_at: row.get("expires_at"), decided_at: row.get("decided_at"), provider_resume_token: row.get("provider_resume_token") })
}

fn permission_rule_from_row(row: sqlx::sqlite::SqliteRow) -> Result<PermissionRule, OrchestratorError> {
    Ok(PermissionRule { id: row.get("id"), project_id: row.get("project_id"), project_identity: row.get("project_identity"), provider_id: parse(row.get("provider_id"))?, role: parse(row.get("role"))?, action_type: parse(row.get("action_type"))?, operation: serde_json::from_str(&row.get::<String,_>("operation_json")).map_err(|error| OrchestratorError::Config(error.to_string()))?, rule_sha256: row.get("rule_sha256"), enabled: row.get::<i64,_>("enabled") != 0, created_by: row.get("created_by"), created_at: row.get("created_at"), expires_at: row.get("expires_at"), last_matched_at: row.get("last_matched_at"), revoked_at: row.get("revoked_at") })
}

pub fn permission_operation_sha(action: PermissionActionType, operation: &PermissionOperation) -> Result<String, OrchestratorError> {
    seal_json(&json!({"action_type":action,"operation":operation}))
}

fn permission_rule_sha(identity: &str, request: &PermissionRequest) -> Result<String, OrchestratorError> {
    seal_json(&json!({"project_identity":identity,"provider_id":request.provider_id,"role":request.role,"action_type":request.action_type,"operation":request.operation}))
}

fn seal_json(value: &Value) -> Result<String, OrchestratorError> {
    let bytes = serde_json::to_vec(value).map_err(|error| OrchestratorError::Config(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn normalize_permission_operation(operation: &PermissionOperation, project_root: &Path) -> Result<PermissionOperation, OrchestratorError> {
    let cwd = normalize_absolute_path(&operation.cwd, project_root)?;
    let mut paths = operation.paths.iter().map(|entry| {
        let lexical = lexical_absolute_path(&entry.path, Path::new(&cwd))?;
        let normalized = normalize_absolute_path(&entry.path, Path::new(&cwd))?;
        if lexical.starts_with(project_root) && !Path::new(&normalized).starts_with(project_root) {
            return Err(permission_error(PermissionErrorCode::PathEscape));
        }
        Ok(PermissionPath { outside_worktree: !Path::new(&normalized).starts_with(project_root), path: normalized, access: entry.access })
    }).collect::<Result<Vec<_>, OrchestratorError>>()?;
    paths.sort_by(|a,b| a.path.cmp(&b.path).then(a.access.to_string().cmp(&b.access.to_string())));
    paths.dedup();
    let mut domains = operation.network_domains.iter().map(|value| normalize_domain(value)).collect::<Result<Vec<_>,_>>()?;
    domains.sort(); domains.dedup();
    let mut environment_names = operation.environment_names.iter().map(|value| value.trim().to_ascii_uppercase()).collect::<Vec<_>>();
    if environment_names.iter().any(|value| value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')) {
        return Err(OrchestratorError::InvalidState("PERMISSION_INVALID_ENVIRONMENT_NAME".into()));
    }
    environment_names.sort(); environment_names.dedup();
    let argv = operation.argv.iter().map(|value| {
        if permission_arg_is_sensitive(value) { "[REDACTED_SECRET_ARGUMENT]".into() } else { value.clone() }
    }).collect();
    Ok(PermissionOperation { argv, cwd, paths, network_domains: domains, environment_names, attributes: Default::default() })
}

fn normalize_absolute_path(value: &str, base: &Path) -> Result<String, OrchestratorError> {
    let raw = Path::new(value);
    let joined = if raw.is_absolute() { raw.to_path_buf() } else { base.join(raw) };
    let mut clean = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::RootDir | std::path::Component::Prefix(_) | std::path::Component::Normal(_) => clean.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return Err(permission_error(PermissionErrorCode::PathEscape)),
        }
    }
    if !clean.is_absolute() { return Err(permission_error(PermissionErrorCode::PathEscape)); }
    let mut existing = clean.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| permission_error(PermissionErrorCode::PathEscape))?;
    }
    let canonical = std::fs::canonicalize(existing)?;
    let suffix = clean.strip_prefix(existing).map_err(|_| permission_error(PermissionErrorCode::PathEscape))?;
    Ok(canonical.join(suffix).to_string_lossy().into_owned())
}

fn lexical_absolute_path(value: &str, base: &Path) -> Result<PathBuf, OrchestratorError> {
    let raw = Path::new(value);
    let joined = if raw.is_absolute() { raw.to_path_buf() } else { base.join(raw) };
    if joined.components().any(|part| matches!(part, std::path::Component::ParentDir)) {
        return Err(permission_error(PermissionErrorCode::PathEscape));
    }
    Ok(joined)
}

fn canonical_permission_action(action: PermissionActionType, operation: &PermissionOperation, root: &Path) -> PermissionActionType {
    if action == PermissionActionType::WorktreeWrite
        && operation.paths.iter().any(|entry| is_control_plane_path(&entry.path, root))
    {
        PermissionActionType::ControlPlaneWrite
    } else {
        action
    }
}

fn is_control_plane_path(path: &str, root: &Path) -> bool {
    let relative = Path::new(path).strip_prefix(root).unwrap_or(Path::new(path));
    let text = relative.to_string_lossy().replace('\\', "/");
    text == "AGENTS.md" || text == "CLAUDE.md" || text == "package.json"
        || text == ".gitlab-ci.yml" || text.starts_with(".agentflow/")
        || text.starts_with(".github/workflows/") || text.starts_with(".git/hooks/")
        || text.starts_with(".husky/")
}

fn normalize_domain(value: &str) -> Result<String, OrchestratorError> {
    let lower = value.trim().to_ascii_lowercase();
    let without_scheme = lower.split_once("://").map_or(lower.as_str(), |(_, rest)| rest);
    let authority = without_scheme.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains('*') || authority.contains('@') || authority.chars().any(char::is_whitespace) {
        return Err(OrchestratorError::InvalidState("PERMISSION_INVALID_DOMAIN".into()));
    }
    Ok(authority.trim_end_matches('.').into())
}

fn classify_permission(action: PermissionActionType, role: RunRole, operation: &PermissionOperation, worktree: Option<&Path>) -> (PermissionRiskLevel, bool) {
    let mut forbidden = matches!(action, PermissionActionType::SystemChange | PermissionActionType::SecretAccess);
    let command = operation.argv.first().map(String::as_str).unwrap_or_default();
    forbidden |= matches!(command, "sudo" | "security" | "launchctl" | "systemctl")
        || operation.argv.iter().any(|arg| arg == "--no-preserve-root");
    forbidden |= operation.argv.iter().any(|arg| arg == "[REDACTED_SECRET_ARGUMENT]")
        || operation.environment_names.iter().any(|name| {
            let name = name.to_ascii_uppercase();
            name.contains("TOKEN") || name.contains("SECRET") || name.contains("PASSWORD")
                || name.contains("API_KEY") || name.contains("PRIVATE_KEY")
        });
    let mutates = matches!(action, PermissionActionType::WorktreeWrite | PermissionActionType::WorktreeDelete | PermissionActionType::ControlPlaneWrite | PermissionActionType::DependencyInstall | PermissionActionType::GitMutation | PermissionActionType::SystemChange);
    if matches!(role, RunRole::Planner | RunRole::Reviewer | RunRole::Validator) && mutates { forbidden = true; }
    if forbidden { return (PermissionRiskLevel::Forbidden, false); }
    let outside = operation.paths.iter().any(|path| path.outside_worktree || worktree.is_some_and(|root| !Path::new(&path.path).starts_with(root)));
    if outside || matches!(action, PermissionActionType::GitMutation | PermissionActionType::ControlPlaneWrite) { return (PermissionRiskLevel::High, true); }
    if matches!(action, PermissionActionType::DependencyInstall | PermissionActionType::NetworkAccess | PermissionActionType::EnvironmentRead | PermissionActionType::CommandExecute | PermissionActionType::WorktreeDelete) { return (PermissionRiskLevel::Medium, true); }
    (PermissionRiskLevel::Low, true)
}

fn default_permission_allowed(action: PermissionActionType, role: RunRole, risk: PermissionRiskLevel, operation: &PermissionOperation, worktree: Option<&Path>) -> bool {
    if risk != PermissionRiskLevel::Low { return false; }
    match action {
        PermissionActionType::WorktreeRead | PermissionActionType::GitRead => true,
        PermissionActionType::WorktreeWrite => role == RunRole::Developer && operation.paths.iter().all(|path| worktree.is_some_and(|root| Path::new(&path.path).starts_with(root))),
        PermissionActionType::ProcessControl => true,
        _ => false,
    }
}

fn rule_is_too_broad(operation: &PermissionOperation) -> bool {
    operation.argv.iter().any(|arg| arg == "*" || arg == "**")
        || operation.paths.iter().any(|path| path.path == "/" || path.path.ends_with("/**"))
        || operation.network_domains.iter().any(|domain| domain.contains('*'))
        || operation.environment_names.iter().any(|name| name == "*")
}

fn permission_summary(action: PermissionActionType, operation: &PermissionOperation) -> String {
    match action {
        PermissionActionType::CommandExecute | PermissionActionType::DependencyInstall => format!("运行命令 {}", operation.argv.first().map(String::as_str).unwrap_or("(空命令)")),
        PermissionActionType::NetworkAccess => format!("访问网络 {}", operation.network_domains.first().map(String::as_str).unwrap_or("(未指定域名)")),
        PermissionActionType::ExternalPath => format!("访问工作树外路径 {}", operation.paths.first().map(|p| p.path.as_str()).unwrap_or("(未指定路径)")),
        _ => format!("请求 {} 权限", action),
    }
}

fn extend_effective_permissions(effective: &mut EffectivePermissions, action: PermissionActionType, operation: &PermissionOperation) {
    match action {
        PermissionActionType::CommandExecute | PermissionActionType::DependencyInstall => effective.command_argv.push(operation.argv.clone()),
        PermissionActionType::ExternalPath => effective.external_paths.extend(operation.paths.clone()),
        PermissionActionType::NetworkAccess => effective.network_domains.extend(operation.network_domains.clone()),
        PermissionActionType::EnvironmentRead => effective.environment_names.extend(operation.environment_names.clone()),
        _ => {}
    }
}

fn safe_permission_reason(value: &str) -> String {
    agentflow_process_supervisor::redact(value.chars().take(500).collect())
}

fn permission_arg_is_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("sk-") || lower.starts_with("bearer ")
        || ["--token=", "--password=", "--secret=", "--api-key=", "authorization:"]
            .iter().any(|marker| lower.contains(marker))
}

fn permission_error(code: PermissionErrorCode) -> OrchestratorError {
    OrchestratorError::InvalidState(code.to_string())
}
