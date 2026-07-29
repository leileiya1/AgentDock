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
        let seal = self.approval_seal(task).await?;
        self.verify_sealed_task_heads(task, &project, &seal).await?;
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
                vec!["pr", "view", branch, "--json", "number,url,state,mergeCommit,statusCheckRollup,headRefOid,headRefName,baseRefName,isDraft"],
            ),
            DeliveryMode::GitLabMr => (
                "glab",
                vec!["mr", "create", "--source-branch", branch, "--target-branch", &task.target_branch, "--title", &task.title, "--description", &body, "--yes"],
                vec!["mr", "view", branch, "--output", "json"],
            ),
            DeliveryMode::LocalMerge => return Ok(()),
        };
        let create_output = run_scm_allow_failure(program, &create, &project.repo).await?;
        let view_output = run_scm(program, &view, &project.repo).await.or_else(|_| {
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
        let (required_checks, ci_checks) = collect_ci_checks(
            task.policy.delivery_mode,
            branch,
            &view_output,
            &project.repo,
        )
        .await?;
        let ci_checks_json = serde_json::to_string(&ci_checks)
            .map_err(|error| OrchestratorError::InvalidState(error.to_string()))?;
        let snapshot = parse_delivery_snapshot(
            task.policy.delivery_mode,
            &view_output,
            required_checks.as_deref(),
        );
        validate_delivery_binding(task, &seal, &snapshot)?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE delivery_records SET state=?,remote_url=?,request_number=?,ci_status=?,ci_checks_json=?,merge_commit=COALESCE(?,merge_commit),approved_commit_sha=?,observed_head_sha=?,head_branch=?,base_branch=?,required_checks_json=?,updated_at=? WHERE task_id=?")
            .bind(snapshot.state.to_string()).bind(&snapshot.url).bind(snapshot.number)
            .bind(snapshot.ci_status.to_string()).bind(ci_checks_json).bind(&snapshot.merge_commit)
            .bind(&seal.commit_sha).bind(&snapshot.head_sha).bind(&snapshot.head_branch)
            .bind(&snapshot.base_branch).bind(required_checks).bind(&now).bind(&task.id)
            .execute(self.store.pool()).await?;
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
        if task.status == TaskStatus::Merged {
            return self.store.task_summary(task_id).await.map_err(Into::into);
        }
        let project = self.project(&task.project_id).await?;
        let seal = self.approval_seal(&task).await?;
        self.verify_sealed_task_heads(&task, &project, &seal).await?;
        let branch = task.branch.as_deref()
            .ok_or_else(|| OrchestratorError::InvalidState("branch missing".into()))?;
        let (program, args) = match task.policy.delivery_mode {
            DeliveryMode::GitHubPr => ("gh", vec!["pr", "view", branch, "--json", "number,url,state,mergeCommit,statusCheckRollup,headRefOid,headRefName,baseRefName,isDraft"]),
            DeliveryMode::GitLabMr => ("glab", vec!["mr", "view", branch, "--output", "json"]),
            DeliveryMode::LocalMerge => unreachable!(),
        };
        let view_output = run_scm(program, &args, &project.repo).await?;
        let (required_checks, ci_checks) = collect_ci_checks(
            task.policy.delivery_mode,
            branch,
            &view_output,
            &project.repo,
        )
        .await?;
        let ci_checks_json = serde_json::to_string(&ci_checks)
            .map_err(|error| OrchestratorError::InvalidState(error.to_string()))?;
        let snapshot = parse_delivery_snapshot(
            task.policy.delivery_mode,
            &view_output,
            required_checks.as_deref(),
        );
        if let Err(error) = validate_delivery_binding(&task, &seal, &snapshot) {
            let now = Utc::now().to_rfc3339();
            sqlx::query("UPDATE delivery_records SET state='failed',ci_status='unknown',ci_checks_json=?,approved_commit_sha=?,observed_head_sha=?,head_branch=?,base_branch=?,required_checks_json=?,updated_at=? WHERE task_id=?")
                .bind(&ci_checks_json)
                .bind(&seal.commit_sha).bind(&snapshot.head_sha).bind(&snapshot.head_branch)
                .bind(&snapshot.base_branch).bind(required_checks).bind(&now).bind(task_id)
                .execute(self.store.pool()).await?;
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','delivery:head_mismatch',?,?)")
                .bind(task_id).bind(task.revision)
                .bind(json!({"expected":seal.commit_sha,"observed":snapshot.head_sha,"error":error.to_string()}).to_string())
                .bind(&now).execute(self.store.pool()).await?;
            return Err(error);
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE delivery_records SET state=?,remote_url=?,request_number=?,ci_status=?,ci_checks_json=?,merge_commit=COALESCE(?,merge_commit),approved_commit_sha=?,observed_head_sha=?,head_branch=?,base_branch=?,required_checks_json=?,updated_at=? WHERE task_id=?")
            .bind(snapshot.state.to_string()).bind(&snapshot.url).bind(snapshot.number)
            .bind(snapshot.ci_status.to_string()).bind(&ci_checks_json).bind(&snapshot.merge_commit)
            .bind(&seal.commit_sha).bind(&snapshot.head_sha).bind(&snapshot.head_branch)
            .bind(&snapshot.base_branch).bind(required_checks).bind(&now).bind(task_id)
            .execute(self.store.pool()).await?;
        if snapshot.state == DeliveryState::Merged
            && snapshot.ci_status == CiStatus::Passed
            && matches!(task.status, TaskStatus::Approved | TaskStatus::MergeConflict)
        {
            self.store.transition(
                task_id, &[task.status], TaskStatus::Merged, None, Actor::Orchestrator,
                "delivery:merged", &json!({"url":snapshot.url,"merge_commit":snapshot.merge_commit}),
            ).await?;
            if let Some(worktree) = task.worktree_path.as_ref() {
                let _ = self.git.worktree_remove(&project.repo, worktree).await;
            }
        } else if snapshot.ci_status == CiStatus::Failed {
            sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'orchestrator','delivery:ci_failed',?,?)")
                .bind(task_id).bind(task.revision).bind(json!({"url":snapshot.url}).to_string())
                .bind(&now).execute(self.store.pool()).await?;
        }
        self.store.task_summary(task_id).await.map_err(Into::into)
    }

}

struct DeliverySnapshot {
    state: DeliveryState,
    url: Option<String>,
    number: Option<i64>,
    ci_status: CiStatus,
    merge_commit: Option<String>,
    head_sha: Option<String>,
    head_branch: Option<String>,
    base_branch: Option<String>,
    draft: Option<bool>,
}

fn parse_delivery_snapshot(
    mode: DeliveryMode,
    text: &str,
    required_checks_json: Option<&str>,
) -> DeliverySnapshot {
    let value = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
    let state_text = value.get("state").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let merged = state_text == "merged";
    let ci_status = match mode {
        DeliveryMode::GitHubPr => {
            let checks = required_checks_json
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .and_then(|checks| checks.as_array().cloned())
                .unwrap_or_default();
            if checks.is_empty() {
                CiStatus::Unknown
            } else if checks.iter().any(|check| {
                matches!(
                    check.get("bucket").and_then(Value::as_str),
                    Some("fail" | "cancel")
                )
            }) {
                CiStatus::Failed
            } else if checks.iter().any(|check| {
                check.get("bucket").and_then(Value::as_str) == Some("pending")
            }) {
                CiStatus::Pending
            } else if checks.iter().all(|check| {
                check.get("bucket").and_then(Value::as_str) == Some("pass")
            }) {
                CiStatus::Passed
            } else {
                // `skipping`, missing buckets and future GitHub values are not explicit success.
                CiStatus::Unknown
            }
        }
        DeliveryMode::GitLabMr => {
            let status = value.pointer("/head_pipeline/status").and_then(Value::as_str)
                .or_else(|| value.pointer("/headPipeline/status").and_then(Value::as_str))
                .or_else(|| value.pointer("/pipeline/status").and_then(Value::as_str));
            match status.unwrap_or("").to_ascii_lowercase().as_str() {
                "success" | "passed" => CiStatus::Passed,
                "failed" | "canceled" => CiStatus::Failed,
                "running" | "pending" | "created" => CiStatus::Pending,
                _ => CiStatus::Unknown,
            }
        }
        DeliveryMode::LocalMerge => CiStatus::Passed,
    };
    let state = if merged && ci_status == CiStatus::Passed {
        DeliveryState::Merged
    } else if ci_status == CiStatus::Failed {
        DeliveryState::Failed
    } else if matches!(ci_status, CiStatus::Pending | CiStatus::Unknown) {
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
        head_sha: value.get("headRefOid").and_then(Value::as_str).map(str::to_string)
            .or_else(|| value.pointer("/diff_refs/head_sha").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.pointer("/diffRefs/headSha").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.get("sha").and_then(Value::as_str).map(str::to_string)),
        head_branch: value.get("headRefName").and_then(Value::as_str).map(str::to_string)
            .or_else(|| value.get("source_branch").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.get("sourceBranch").and_then(Value::as_str).map(str::to_string)),
        base_branch: value.get("baseRefName").and_then(Value::as_str).map(str::to_string)
            .or_else(|| value.get("target_branch").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.get("targetBranch").and_then(Value::as_str).map(str::to_string)),
        draft: value.get("isDraft").and_then(Value::as_bool)
            .or_else(|| value.get("draft").and_then(Value::as_bool))
            .or_else(|| value.get("work_in_progress").and_then(Value::as_bool)),
    }
}

fn validate_delivery_binding(
    task: &TaskRow,
    seal: &ApprovalSeal,
    snapshot: &DeliverySnapshot,
) -> Result<(), OrchestratorError> {
    let expected_branch = task
        .branch
        .as_deref()
        .ok_or_else(|| OrchestratorError::MergePrecondition("task branch missing".into()))?;
    if snapshot.head_sha.as_deref() != Some(seal.commit_sha.as_str()) {
        return Err(OrchestratorError::MergePrecondition(
            "change request head SHA does not equal the approved commit".into(),
        ));
    }
    if snapshot.head_branch.as_deref() != Some(expected_branch) {
        return Err(OrchestratorError::MergePrecondition(
            "change request head branch does not match the task branch".into(),
        ));
    }
    if snapshot.base_branch.as_deref() != Some(task.target_branch.as_str()) {
        return Err(OrchestratorError::MergePrecondition(
            "change request base branch does not match the approved target".into(),
        ));
    }
    if snapshot.draft != Some(false) {
        return Err(OrchestratorError::MergePrecondition(
            "change request draft state is unknown or still draft".into(),
        ));
    }
    Ok(())
}

async fn collect_ci_checks(
    mode: DeliveryMode,
    branch: &str,
    view_output: &str,
    cwd: &Path,
) -> Result<(Option<String>, Vec<CiCheck>), OrchestratorError> {
    match mode {
        DeliveryMode::GitHubPr => {
            let required = github_checks_json(branch, cwd, true).await?;
            let all = github_checks_json(branch, cwd, false).await?;
            let mut checks = parse_github_ci_checks(&all, &required);
            enrich_github_failure_summaries(&mut checks, cwd).await;
            Ok((Some(required), checks))
        }
        DeliveryMode::GitLabMr => Ok((None, gitlab_ci_checks(view_output, cwd).await)),
        DeliveryMode::LocalMerge => Ok((None, Vec::new())),
    }
}

async fn github_checks_json(
    branch: &str,
    cwd: &Path,
    required_only: bool,
) -> Result<String, OrchestratorError> {
    let mut args = vec!["pr", "checks", branch];
    if required_only {
        args.push("--required");
    }
    args.extend([
        "--json",
        "name,state,bucket,link,workflow,description,startedAt,completedAt",
    ]);
    let output = run_scm_allow_failure(
        "gh",
        &args,
        cwd,
    )
    .await?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if serde_json::from_str::<Value>(&stdout)
        .ok()
        .is_some_and(|value| value.is_array())
    {
        // gh exits 8 while checks are pending and 1 when checks fail; both still provide the
        // authoritative JSON snapshot that the state mapper must inspect.
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    if stderr.contains("no checks reported") || stderr.contains("no required checks") {
        return Ok("[]".into());
    }
    Err(OrchestratorError::InvalidState(
        agentflow_process_supervisor::redact(
            format!("gh pr checks: {stderr}").chars().take(1600).collect(),
        ),
    ))
}

fn parse_github_ci_checks(all_json: &str, required_json: &str) -> Vec<CiCheck> {
    let required: HashSet<(String, Option<String>)> = serde_json::from_str::<Vec<Value>>(required_json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|check| {
            Some((
                check.get("name")?.as_str()?.to_string(),
                check.get("workflow").and_then(Value::as_str).map(str::to_string),
            ))
        })
        .collect();
    serde_json::from_str::<Vec<Value>>(all_json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|check| {
            let name = check.get("name")?.as_str()?.to_string();
            let workflow = check
                .get("workflow")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let status = ci_check_status(
                check.get("bucket").and_then(Value::as_str),
                check.get("state").and_then(Value::as_str),
            );
            let description = check
                .get("description")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.trim().chars().take(500).collect());
            let failure_summary = matches!(status, CiCheckStatus::Failed | CiCheckStatus::Cancelled)
                .then(|| description.clone())
                .flatten();
            Some(CiCheck {
                required: required.contains(&(name.clone(), workflow.clone()))
                    || required.iter().any(|(required_name, _)| required_name == &name),
                name,
                status,
                workflow,
                description,
                failure_summary,
                details_url: check.get("link").and_then(Value::as_str).map(str::to_string),
                started_at: check.get("startedAt").and_then(Value::as_str).map(str::to_string),
                completed_at: check.get("completedAt").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect()
}

fn ci_check_status(bucket: Option<&str>, state: Option<&str>) -> CiCheckStatus {
    match bucket.unwrap_or("").to_ascii_lowercase().as_str() {
        "pass" => CiCheckStatus::Passed,
        "fail" => CiCheckStatus::Failed,
        "cancel" => CiCheckStatus::Cancelled,
        "pending" => CiCheckStatus::Pending,
        "skipping" => CiCheckStatus::Skipped,
        _ => match state.unwrap_or("").to_ascii_lowercase().as_str() {
            "success" | "passed" => CiCheckStatus::Passed,
            "failure" | "failed" | "timed_out" => CiCheckStatus::Failed,
            "cancelled" | "canceled" => CiCheckStatus::Cancelled,
            "queued" | "waiting" | "pending" | "in_progress" | "running" => {
                CiCheckStatus::Pending
            }
            "skipped" | "manual" => CiCheckStatus::Skipped,
            _ => CiCheckStatus::Unknown,
        },
    }
}

async fn enrich_github_failure_summaries(checks: &mut [CiCheck], cwd: &Path) {
    let mut summaries: HashMap<String, Option<String>> = HashMap::new();
    for check in checks.iter_mut().filter(|check| {
        matches!(check.status, CiCheckStatus::Failed | CiCheckStatus::Cancelled)
            && check.failure_summary.is_none()
    }) {
        let Some(run_id) = check.details_url.as_deref().and_then(github_run_id) else {
            continue;
        };
        let summary = if let Some(cached) = summaries.get(&run_id) {
            cached.clone()
        } else {
            let output = run_scm_allow_failure("gh", &["run", "view", &run_id, "--log-failed"], cwd).await;
            let summary = output.ok().and_then(|output| {
                summarize_failed_log(&String::from_utf8_lossy(&output.stdout), &check.name)
            });
            summaries.insert(run_id.clone(), summary.clone());
            summary
        };
        check.failure_summary = summary;
    }
}

fn github_run_id(link: &str) -> Option<String> {
    link.split("/actions/runs/")
        .nth(1)?
        .split('/')
        .next()
        .filter(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}

fn summarize_failed_log(log: &str, check_name: &str) -> Option<String> {
    let check_name = check_name.to_ascii_lowercase();
    let meaningful = |line: &&str| {
        let lower = line.to_ascii_lowercase();
        lower.contains("error")
            || lower.contains("failed")
            || lower.contains("failure")
            || lower.contains("panic")
    };
    let all: Vec<&str> = log.lines().filter(meaningful).collect();
    let scoped: Vec<&str> = all
        .iter()
        .copied()
        .filter(|line| line.to_ascii_lowercase().contains(&check_name))
        .collect();
    let source = if scoped.is_empty() { &all } else { &scoped };
    let summary = source
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (!summary.is_empty()).then(|| {
        agentflow_process_supervisor::redact(summary.chars().take(700).collect())
    })
}

async fn gitlab_ci_checks(view_output: &str, cwd: &Path) -> Vec<CiCheck> {
    let value = serde_json::from_str::<Value>(view_output).unwrap_or(Value::Null);
    let pipeline = value
        .get("head_pipeline")
        .or_else(|| value.get("headPipeline"))
        .or_else(|| value.get("pipeline"));
    let pipeline_id = pipeline.and_then(|value| value.get("id")).and_then(Value::as_i64);
    let mut jobs = pipeline
        .and_then(|value| value.get("jobs"))
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if jobs.is_empty() && let Some(pipeline_id) = pipeline_id {
        let endpoint = format!("projects/:fullpath/pipelines/{pipeline_id}/jobs");
        if let Ok(output) = run_scm_allow_failure("glab", &["api", &endpoint], cwd).await
            && output.status.success()
        {
            jobs = serde_json::from_slice::<Vec<Value>>(&output.stdout).unwrap_or_default();
        }
    }
    let checks: Vec<CiCheck> = jobs.into_iter().filter_map(parse_gitlab_job).collect();
    if !checks.is_empty() {
        return checks;
    }
    pipeline.and_then(parse_gitlab_pipeline).into_iter().collect()
}

fn parse_gitlab_job(job: Value) -> Option<CiCheck> {
    let name = job.get("name")?.as_str()?.to_string();
    let status_text = job.get("status").and_then(Value::as_str);
    let status = ci_check_status(None, status_text);
    let failure_reason = job
        .get("failure_reason")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('_', " "));
    Some(CiCheck {
        name,
        status,
        required: !job.get("allow_failure").and_then(Value::as_bool).unwrap_or(false),
        workflow: job.get("stage").and_then(Value::as_str).map(str::to_string),
        description: failure_reason.clone(),
        failure_summary: matches!(status, CiCheckStatus::Failed | CiCheckStatus::Cancelled)
            .then_some(failure_reason)
            .flatten(),
        details_url: job.get("web_url").and_then(Value::as_str).map(str::to_string),
        started_at: job.get("started_at").and_then(Value::as_str).map(str::to_string),
        completed_at: job.get("finished_at").and_then(Value::as_str).map(str::to_string),
    })
}

fn parse_gitlab_pipeline(pipeline: &Value) -> Option<CiCheck> {
    let status_text = pipeline.get("status").and_then(Value::as_str)?;
    let status = ci_check_status(None, Some(status_text));
    Some(CiCheck {
        name: "GitLab Pipeline".into(),
        status,
        required: true,
        workflow: None,
        description: None,
        failure_summary: None,
        details_url: pipeline
            .get("web_url")
            .or_else(|| pipeline.get("webUrl"))
            .and_then(Value::as_str)
            .map(str::to_string),
        started_at: pipeline.get("created_at").and_then(Value::as_str).map(str::to_string),
        completed_at: pipeline.get("updated_at").and_then(Value::as_str).map(str::to_string),
    })
}

async fn run_scm(program: &str, args: &[&str], cwd: &Path) -> Result<String, OrchestratorError> {
    let output = run_scm_allow_failure(program, args, cwd).await?;
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
    cwd: &Path,
) -> Result<std::process::Output, OrchestratorError> {
    tokio::time::timeout(
        Duration::from_secs(120),
        Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .output(),
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

    #[tokio::test]
    async fn scm_commands_run_inside_the_project_repository() -> anyhow::Result<()> {
        let repository = tempfile::tempdir()?;
        let init = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(repository.path())
            .output()
            .await?;
        anyhow::ensure!(init.status.success(), "git init failed");

        let actual = run_scm(
            "git",
            &["rev-parse", "--show-toplevel"],
            repository.path(),
        )
        .await?;
        assert_eq!(
            Path::new(actual.trim()).canonicalize()?,
            repository.path().canonicalize()?,
        );
        Ok(())
    }

    #[test]
    fn github_snapshot_maps_ci_and_merge_state() {
        let value = r#"{"number":7,"url":"https://github.test/pull/7","state":"MERGED","mergeCommit":{"oid":"abc"},"headRefOid":"approved","headRefName":"agentflow/TASK-7","baseRefName":"main","isDraft":false}"#;
        let snapshot = parse_delivery_snapshot(
            DeliveryMode::GitHubPr,
            value,
            Some(r#"[{"name":"test","state":"SUCCESS","bucket":"pass"}]"#),
        );
        assert_eq!(snapshot.state, DeliveryState::Merged);
        assert_eq!(snapshot.ci_status, CiStatus::Passed);
        assert_eq!(snapshot.number, Some(7));
        assert_eq!(snapshot.merge_commit.as_deref(), Some("abc"));
    }

    #[test]
    fn github_failed_check_blocks_delivery() {
        let value = r#"{"state":"OPEN","headRefOid":"approved","headRefName":"agentflow/TASK-7","baseRefName":"main"}"#;
        let snapshot = parse_delivery_snapshot(
            DeliveryMode::GitHubPr,
            value,
            Some(r#"[{"name":"test","state":"FAILURE","bucket":"fail"}]"#),
        );
        assert_eq!(snapshot.state, DeliveryState::Failed);
        assert_eq!(snapshot.ci_status, CiStatus::Failed);
    }

    #[test]
    fn github_without_configured_checks_never_bypasses_ci_gate() {
        let snapshot = parse_delivery_snapshot(
            DeliveryMode::GitHubPr,
            r#"{"state":"OPEN","headRefOid":"approved","headRefName":"agentflow/TASK-7","baseRefName":"main"}"#,
            Some("[]"),
        );
        assert_eq!(snapshot.state, DeliveryState::CiRunning);
        assert_eq!(snapshot.ci_status, CiStatus::Unknown);
    }

    #[test]
    fn gitlab_snapshot_supports_glab_fields_and_pipeline_states() {
        let passed = parse_delivery_snapshot(
            DeliveryMode::GitLabMr,
            r#"{"iid":9,"web_url":"https://gitlab.test/merge_requests/9","state":"merged","head_pipeline":{"status":"success"},"merge_commit_sha":"def","sha":"approved","source_branch":"agentflow/TASK-9","target_branch":"main"}"#,
            None,
        );
        assert_eq!(passed.state, DeliveryState::Merged);
        assert_eq!(passed.ci_status, CiStatus::Passed);
        assert_eq!(passed.number, Some(9));
        assert_eq!(passed.url.as_deref(), Some("https://gitlab.test/merge_requests/9"));
        let failed = parse_delivery_snapshot(
            DeliveryMode::GitLabMr,
            r#"{"state":"opened","headPipeline":{"status":"failed"},"sha":"approved","source_branch":"agentflow/TASK-9","target_branch":"main"}"#,
            None,
        );
        assert_eq!(failed.state, DeliveryState::Failed);
        assert_eq!(failed.ci_status, CiStatus::Failed);
    }

    #[test]
    fn github_unknown_or_partially_registered_required_checks_never_pass() {
        let view = r#"{"state":"OPEN","headRefOid":"approved","headRefName":"agentflow/TASK-7","baseRefName":"main"}"#;
        let pending = parse_delivery_snapshot(
            DeliveryMode::GitHubPr,
            view,
            Some(r#"[{"name":"test","state":"QUEUED","bucket":"pending"}]"#),
        );
        assert_eq!(pending.ci_status, CiStatus::Pending);
        assert_eq!(pending.state, DeliveryState::CiRunning);
        let future_or_skipped = parse_delivery_snapshot(
            DeliveryMode::GitHubPr,
            view,
            Some(r#"[{"name":"test","state":"SKIPPED","bucket":"skipping"}]"#),
        );
        assert_eq!(future_or_skipped.ci_status, CiStatus::Unknown);
        assert_eq!(future_or_skipped.state, DeliveryState::CiRunning);
    }

    #[test]
    fn github_check_details_mark_required_rows_and_keep_remote_logs() {
        let required = r#"[{"name":"test","workflow":"CI","state":"FAILURE","bucket":"fail","link":"https://github.test/actions/runs/123/job/456"}]"#;
        let all = r#"[{"name":"test","workflow":"CI","description":"unit tests failed","state":"FAILURE","bucket":"fail","link":"https://github.test/actions/runs/123/job/456","startedAt":"2026-07-20T10:00:00Z","completedAt":"2026-07-20T10:02:00Z"},{"name":"lint","workflow":"CI","state":"SUCCESS","bucket":"pass","link":"https://github.test/actions/runs/123/job/789"}]"#;
        let checks = parse_github_ci_checks(all, required);
        assert_eq!(checks.len(), 2);
        assert!(checks[0].required);
        assert_eq!(checks[0].status, CiCheckStatus::Failed);
        assert_eq!(checks[0].failure_summary.as_deref(), Some("unit tests failed"));
        assert_eq!(checks[0].details_url.as_deref(), Some("https://github.test/actions/runs/123/job/456"));
        assert!(!checks[1].required);
        assert_eq!(checks[1].status, CiCheckStatus::Passed);
    }

    #[test]
    fn github_failed_log_summary_is_short_and_error_focused() {
        let log = "test\tsetup\tstarting\ntest\trun\terror: expected 2, received 3\ntest\trun\tFAILURE: 1 test failed\nother\tstep\tpanic should not win scoped lines\n";
        let Some(summary) = summarize_failed_log(log, "test") else {
            panic!("expected a failed log summary")
        };
        assert!(summary.contains("expected 2"));
        assert!(summary.contains("1 test failed"));
        assert!(!summary.contains("starting"));
        assert!(!summary.contains("other"));
        assert_eq!(github_run_id("https://github.com/a/b/actions/runs/123/job/9").as_deref(), Some("123"));
    }

    #[test]
    fn gitlab_jobs_expose_stage_failure_and_log_link() {
        let Some(check) = parse_gitlab_job(serde_json::json!({
            "name": "integration",
            "stage": "test",
            "status": "failed",
            "allow_failure": false,
            "failure_reason": "script_failure",
            "web_url": "https://gitlab.test/jobs/42",
            "started_at": "2026-07-20T10:00:00Z",
            "finished_at": "2026-07-20T10:03:00Z"
        })) else {
            panic!("expected a parsed GitLab job")
        };
        assert_eq!(check.status, CiCheckStatus::Failed);
        assert!(check.required);
        assert_eq!(check.workflow.as_deref(), Some("test"));
        assert_eq!(check.failure_summary.as_deref(), Some("script failure"));
        assert_eq!(check.details_url.as_deref(), Some("https://gitlab.test/jobs/42"));
    }

    #[test]
    fn delivery_binding_rejects_old_head_wrong_branches_and_drafts() {
        let task = TaskRow {
            id: "task".into(), project_id: "project".into(), seq: 7,
            title: "delivery".into(), description: "test".into(),
            status: TaskStatus::Approved, blocked_detail: None,
            developer: AgentKind::Codex, reviewer: AgentKind::ClaudeCode,
            target_branch: "main".into(), base_commit: Some("base".into()),
            branch: Some("agentflow/TASK-7".into()), worktree_path: None,
            revision: 1, max_revisions: 2, api_egress_approved: false,
            policy: TaskPolicy::default(),
        };
        let seal = ApprovalSeal { commit_sha: "approved".into() };
        let exact = DeliverySnapshot {
            state: DeliveryState::Ready, url: None, number: Some(7),
            ci_status: CiStatus::Passed, merge_commit: None,
            head_sha: Some("approved".into()),
            head_branch: Some("agentflow/TASK-7".into()),
            base_branch: Some("main".into()), draft: Some(false),
        };
        assert!(validate_delivery_binding(&task, &seal, &exact).is_ok());
        let mut old_head = exact;
        old_head.head_sha = Some("unapproved".into());
        assert!(matches!(
            validate_delivery_binding(&task, &seal, &old_head),
            Err(OrchestratorError::MergePrecondition(_))
        ));
        old_head.head_sha = Some("approved".into());
        old_head.base_branch = Some("release".into());
        assert!(validate_delivery_binding(&task, &seal, &old_head).is_err());
        old_head.base_branch = Some("main".into());
        old_head.draft = Some(true);
        assert!(validate_delivery_binding(&task, &seal, &old_head).is_err());
    }

    #[tokio::test]
    #[ignore = "requires a logged-in gh CLI and AGENTFLOW_LIVE_GITHUB_BRANCH"]
    async fn live_github_snapshot_uses_the_real_head_draft_and_required_checks()
    -> Result<(), Box<dyn std::error::Error>> {
        let branch = std::env::var("AGENTFLOW_LIVE_GITHUB_BRANCH")?;
        let approved_sha = std::env::var("AGENTFLOW_LIVE_GITHUB_APPROVED_SHA")?;
        let target = std::env::var("AGENTFLOW_LIVE_GITHUB_TARGET").unwrap_or_else(|_| "main".into());
        let view = run_scm(
            "gh",
            &["pr", "view", &branch, "--json", "number,url,state,mergeCommit,statusCheckRollup,headRefOid,headRefName,baseRefName,isDraft"],
            Path::new("."),
        ).await?;
        let required = github_checks_json(&branch, Path::new("."), true).await?;
        let snapshot = parse_delivery_snapshot(DeliveryMode::GitHubPr, &view, Some(&required));
        assert_eq!(snapshot.head_sha.as_deref(), Some(approved_sha.as_str()));
        assert_eq!(snapshot.head_branch.as_deref(), Some(branch.as_str()));
        assert_eq!(snapshot.base_branch.as_deref(), Some(target.as_str()));
        let task = TaskRow {
            id: "live".into(), project_id: "live".into(), seq: 1,
            title: "live".into(), description: "live".into(), status: TaskStatus::Approved,
            blocked_detail: None, developer: AgentKind::Codex, reviewer: AgentKind::ClaudeCode,
            target_branch: target, base_commit: None, branch: Some(branch), worktree_path: None,
            revision: 1, max_revisions: 1, api_egress_approved: false, policy: TaskPolicy::default(),
        };
        let binding = validate_delivery_binding(&task, &ApprovalSeal { commit_sha: approved_sha }, &snapshot);
        assert_eq!(binding.is_ok(), snapshot.draft == Some(false));
        if required == "[]" {
            assert_eq!(snapshot.ci_status, CiStatus::Unknown);
            assert_ne!(snapshot.state, DeliveryState::Ready);
        }
        Ok(())
    }
}
