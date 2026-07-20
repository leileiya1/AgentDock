impl Orchestrator {
    pub async fn task_delivery_start(
        &self,
        task_id: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if !matches!(task.status, TaskStatus::Approved | TaskStatus::MergeConflict) {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let quality = self.latest_quality(task_id, task.revision).await?;
        if quality.as_ref().is_none_or(|value| !value.passed) {
            return Err(OrchestratorError::QualityGate(
                "latest deterministic evaluation did not pass".into(),
            ));
        }
        if task.policy.delivery_mode != DeliveryMode::LocalMerge
            && self
                .delivery_record(task_id)
                .await?
                .is_some_and(|record| record.remote_url.is_some())
        {
            // Creating the same change request twice is never a valid retry. Once
            // an URL is recorded, this action becomes an idempotent status refresh.
            return self.task_delivery_refresh(task_id).await;
        }
        match task.policy.delivery_mode {
            DeliveryMode::LocalMerge => self.merge(task_id).await,
            DeliveryMode::GitHubPr | DeliveryMode::GitLabMr => {
                self.open_change_request(&task).await?;
                self.store.task_summary(task_id).await.map_err(Into::into)
            }
        }
    }

    async fn open_change_request(&self, task: &TaskRow) -> Result<(), OrchestratorError> {
        let project = self.project(&task.project_id).await?;
        let branch = task.branch.as_deref()
            .ok_or_else(|| OrchestratorError::InvalidState("branch missing".into()))?;
        self.git.push_branch(&project.repo, "origin", branch).await?;
        let body = format!(
            "AgentFlow TASK-{}\n\n{}\n\nQuality gate: passed. Revision: r{}.",
            task.seq, task.description, task.revision
        );
        let (program, create, view) = match task.policy.delivery_mode {
            DeliveryMode::GitHubPr => (
                "gh",
                vec!["pr", "create", "--base", &task.target_branch, "--head", branch, "--title", &task.title, "--body", &body],
                vec!["pr", "view", branch, "--json", "number,url,state,mergeCommit,statusCheckRollup"],
            ),
            DeliveryMode::GitLabMr => (
                "glab",
                vec!["mr", "create", "--source-branch", branch, "--target-branch", &task.target_branch, "--title", &task.title, "--description", &body, "--yes"],
                vec!["mr", "view", branch, "--output", "json"],
            ),
            DeliveryMode::LocalMerge => return Ok(()),
        };
        let create_output = run_scm_allow_failure(program, &create).await?;
        let view_output = run_scm(program, &view).await.or_else(|_| {
            if create_output.status.success() {
                Ok(String::from_utf8_lossy(&create_output.stdout).to_string())
            } else {
                Err(OrchestratorError::InvalidState(
                    agentflow_process_supervisor::redact(
                        String::from_utf8_lossy(&create_output.stderr).chars().take(1200).collect(),
                    ),
                ))
            }
        })?;
        let snapshot = parse_delivery_snapshot(task.policy.delivery_mode, &view_output);
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE delivery_records SET state=?,remote_url=?,request_number=?,ci_status=?,merge_commit=COALESCE(?,merge_commit),updated_at=? WHERE task_id=?")
            .bind(snapshot.state.to_string()).bind(&snapshot.url).bind(snapshot.number)
            .bind(snapshot.ci_status.to_string()).bind(&snapshot.merge_commit)
            .bind(&now).bind(&task.id).execute(self.store.pool()).await?;
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'human','delivery:change_request_opened',?,?)")
            .bind(&task.id).bind(task.revision)
            .bind(json!({"mode":task.policy.delivery_mode,"url":snapshot.url,"number":snapshot.number,"ci_status":snapshot.ci_status}).to_string())
            .bind(now).execute(self.store.pool()).await?;
        Ok(())
    }

    pub async fn task_delivery_refresh(
        &self,
        task_id: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.policy.delivery_mode == DeliveryMode::LocalMerge {
            return self.store.task_summary(task_id).await.map_err(Into::into);
        }
        let branch = task.branch.as_deref()
            .ok_or_else(|| OrchestratorError::InvalidState("branch missing".into()))?;
        let (program, args) = match task.policy.delivery_mode {
            DeliveryMode::GitHubPr => ("gh", vec!["pr", "view", branch, "--json", "number,url,state,mergeCommit,statusCheckRollup"]),
            DeliveryMode::GitLabMr => ("glab", vec!["mr", "view", branch, "--output", "json"]),
            DeliveryMode::LocalMerge => unreachable!(),
        };
        let snapshot = parse_delivery_snapshot(task.policy.delivery_mode, &run_scm(program, &args).await?);
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE delivery_records SET state=?,remote_url=?,request_number=?,ci_status=?,merge_commit=COALESCE(?,merge_commit),updated_at=? WHERE task_id=?")
            .bind(snapshot.state.to_string()).bind(&snapshot.url).bind(snapshot.number)
            .bind(snapshot.ci_status.to_string()).bind(&snapshot.merge_commit)
            .bind(&now).bind(task_id).execute(self.store.pool()).await?;
        if snapshot.state == DeliveryState::Merged
            && snapshot.ci_status == CiStatus::Passed
            && matches!(task.status, TaskStatus::Approved | TaskStatus::MergeConflict)
        {
            let project = self.project(&task.project_id).await?;
            if let Some(worktree) = task.worktree_path.as_ref() {
                let _ = self.git.worktree_remove(&project.repo, worktree).await;
            }
            self.store.transition(
                task_id, &[task.status], TaskStatus::Merged, None, Actor::Orchestrator,
                "delivery:merged", &json!({"url":snapshot.url,"merge_commit":snapshot.merge_commit}),
            ).await?;
        } else if snapshot.ci_status == CiStatus::Failed {
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','delivery:ci_failed',?,?)")
                .bind(task_id).bind(task.revision).bind(json!({"url":snapshot.url}).to_string())
                .bind(&now).execute(self.store.pool()).await?;
        }
        self.store.task_summary(task_id).await.map_err(Into::into)
    }

    pub async fn task_rollback(
        &self,
        task_id: &str,
        strategy: RollbackStrategy,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::Merged {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let project = self.project(&task.project_id).await?;
        if self.git.default_branch(&project.repo).await? != task.target_branch
            || !self.git.is_clean(&project.repo).await?
        {
            return Err(OrchestratorError::RollbackUnsafe(
                "target checkout must be clean and on the target branch".into(),
            ));
        }
        let delivery = self.delivery_record(task_id).await?
            .ok_or_else(|| OrchestratorError::RollbackUnsafe("delivery record missing".into()))?;
        let merge = delivery.merge_commit.as_deref()
            .ok_or_else(|| OrchestratorError::RollbackUnsafe("merge commit missing".into()))?;
        let rollback_commit = match strategy {
            RollbackStrategy::Undo => {
                let before = delivery.pre_merge_commit.as_deref().ok_or_else(|| {
                    OrchestratorError::RollbackUnsafe("pre-merge commit missing".into())
                })?;
                if self.git.resolve(&project.repo, "HEAD").await? != merge {
                    return Err(OrchestratorError::RollbackUnsafe(
                        "later commits exist; use revert instead".into(),
                    ));
                }
                self.git.reset_branch_head(&project.repo, before).await?;
                before.to_string()
            }
            RollbackStrategy::Revert => {
                self.git.revert_merge(
                    &project.repo,
                    merge,
                    &format!("[agentflow] rollback TASK-{}: {}", task.seq, task.title),
                ).await?
            }
        };
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE delivery_records SET state='rolled_back',rollback_commit=?,updated_at=? WHERE task_id=?")
            .bind(&rollback_commit).bind(&now).bind(task_id).execute(self.store.pool()).await?;
        self.store.transition(
            task_id, &[TaskStatus::Merged], TaskStatus::RolledBack, None, Actor::Human,
            "human:rollback", &json!({"strategy":strategy,"merge_commit":merge,"rollback_commit":rollback_commit}),
        ).await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
}

struct DeliverySnapshot {
    state: DeliveryState,
    url: Option<String>,
    number: Option<i64>,
    ci_status: CiStatus,
    merge_commit: Option<String>,
}

fn parse_delivery_snapshot(mode: DeliveryMode, text: &str) -> DeliverySnapshot {
    let value = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
    let state_text = value.get("state").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let merged = state_text == "merged";
    let (ci_status, checks_present) = match mode {
        DeliveryMode::GitHubPr => {
            let checks = value.get("statusCheckRollup").and_then(Value::as_array).cloned().unwrap_or_default();
            let failed = checks.iter().any(|check| matches!(
                check.get("conclusion").and_then(Value::as_str),
                Some("FAILURE" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED")
            ));
            let pending = checks.iter().any(|check| {
                check.get("status").and_then(Value::as_str).is_some_and(|status| status != "COMPLETED")
            });
            (if failed { CiStatus::Failed } else if pending { CiStatus::Pending } else if checks.is_empty() { CiStatus::Unknown } else { CiStatus::Passed }, !checks.is_empty())
        }
        DeliveryMode::GitLabMr => {
            let status = value.pointer("/head_pipeline/status").and_then(Value::as_str)
                .or_else(|| value.pointer("/headPipeline/status").and_then(Value::as_str))
                .or_else(|| value.pointer("/pipeline/status").and_then(Value::as_str));
            let ci = match status.unwrap_or("").to_ascii_lowercase().as_str() {
                "success" | "passed" => CiStatus::Passed,
                "failed" | "canceled" => CiStatus::Failed,
                "running" | "pending" | "created" => CiStatus::Pending,
                _ => CiStatus::Unknown,
            };
            (ci, status.is_some())
        }
        DeliveryMode::LocalMerge => (CiStatus::Passed, true),
    };
    let state = if merged && ci_status == CiStatus::Passed {
        DeliveryState::Merged
    } else if ci_status == CiStatus::Failed {
        DeliveryState::Failed
    } else if ci_status == CiStatus::Pending || !checks_present {
        DeliveryState::CiRunning
    } else {
        DeliveryState::Ready
    };
    DeliverySnapshot {
        state,
        url: value.get("url").and_then(Value::as_str).map(str::to_string)
            .or_else(|| value.get("web_url").and_then(Value::as_str).map(str::to_string))
            .or_else(|| extract_http_url(text)),
        number: value.get("number").and_then(Value::as_i64)
            .or_else(|| value.get("iid").and_then(Value::as_i64)),
        ci_status,
        merge_commit: value.pointer("/mergeCommit/oid").and_then(Value::as_str).map(str::to_string)
            .or_else(|| value.get("merge_commit_sha").and_then(Value::as_str).map(str::to_string)),
    }
}

async fn run_scm(program: &str, args: &[&str]) -> Result<String, OrchestratorError> {
    let output = run_scm_allow_failure(program, args).await?;
    if !output.status.success() {
        return Err(OrchestratorError::InvalidState(
            agentflow_process_supervisor::redact(
                format!("{program}: {}", String::from_utf8_lossy(&output.stderr)).chars().take(1600).collect(),
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn run_scm_allow_failure(
    program: &str,
    args: &[&str],
) -> Result<std::process::Output, OrchestratorError> {
    tokio::time::timeout(
        Duration::from_secs(120),
        Command::new(program).args(args).stdin(std::process::Stdio::null()).output(),
    ).await.map_err(|_|OrchestratorError::InvalidState(format!("{program} timed out")))?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                OrchestratorError::ScmCliNotFound(program.into())
            } else {
                OrchestratorError::Io(error)
            }
        })
}

fn extract_http_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|value| value.trim_matches(|ch: char| matches!(ch, ',' | ')' | '(' | '"')))
        .find(|value| value.starts_with("https://") || value.starts_with("http://"))
        .map(str::to_string)
}

#[cfg(test)]
mod delivery_tests {
    use super::*;

    #[test]
    fn github_snapshot_maps_ci_and_merge_state() {
        let value = r#"{"number":7,"url":"https://github.test/pull/7","state":"MERGED","mergeCommit":{"oid":"abc"},"statusCheckRollup":[{"status":"COMPLETED","conclusion":"SUCCESS"}]}"#;
        let snapshot = parse_delivery_snapshot(DeliveryMode::GitHubPr, value);
        assert_eq!(snapshot.state, DeliveryState::Merged);
        assert_eq!(snapshot.ci_status, CiStatus::Passed);
        assert_eq!(snapshot.number, Some(7));
        assert_eq!(snapshot.merge_commit.as_deref(), Some("abc"));
    }

    #[test]
    fn github_failed_check_blocks_delivery() {
        let value = r#"{"state":"OPEN","statusCheckRollup":[{"status":"COMPLETED","conclusion":"FAILURE"}]}"#;
        let snapshot = parse_delivery_snapshot(DeliveryMode::GitHubPr, value);
        assert_eq!(snapshot.state, DeliveryState::Failed);
        assert_eq!(snapshot.ci_status, CiStatus::Failed);
    }

    #[test]
    fn github_without_configured_checks_never_bypasses_ci_gate() {
        let snapshot = parse_delivery_snapshot(
            DeliveryMode::GitHubPr,
            r#"{"state":"OPEN","statusCheckRollup":[]}"#,
        );
        assert_eq!(snapshot.state, DeliveryState::CiRunning);
        assert_eq!(snapshot.ci_status, CiStatus::Unknown);
    }

    #[test]
    fn gitlab_snapshot_supports_glab_fields_and_pipeline_states() {
        let passed = parse_delivery_snapshot(
            DeliveryMode::GitLabMr,
            r#"{"iid":9,"web_url":"https://gitlab.test/merge_requests/9","state":"merged","head_pipeline":{"status":"success"},"merge_commit_sha":"def"}"#,
        );
        assert_eq!(passed.state, DeliveryState::Merged);
        assert_eq!(passed.ci_status, CiStatus::Passed);
        assert_eq!(passed.number, Some(9));
        assert_eq!(passed.url.as_deref(), Some("https://gitlab.test/merge_requests/9"));
        let failed = parse_delivery_snapshot(
            DeliveryMode::GitLabMr,
            r#"{"state":"opened","headPipeline":{"status":"failed"}}"#,
        );
        assert_eq!(failed.state, DeliveryState::Failed);
        assert_eq!(failed.ci_status, CiStatus::Failed);
    }
}
