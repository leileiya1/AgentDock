use super::*;
impl Orchestrator {
    /// Executes one authorized, versioned check. The model does not write this verdict.
    pub(super) async fn team_run_check(
        &self,
        claim: &TeamClaim,
        check: &TeamCheck,
        workspace: &Path,
        label: &str,
    ) -> Result<(CriterionStatus, String, PathBuf, String), OrchestratorError> {
        let view = self
            .team_get(&claim.identity.task_id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        if !rules::command_allowed(check, &view.config) {
            return Err(err("check exceeds task command authorization"));
        }
        let dir = claim.run_dir.join("checks").join(label);
        tokio::fs::create_dir_all(&dir).await?;
        let cancel = CancellationToken::new();
        let watcher = self.team_watch_claim(claim.identity.clone(), cancel.clone());
        let key = format!(
            "team/{}/{}",
            claim.identity.task_id, claim.identity.attempt_id
        );
        self.register_cancellation(&key, cancel.clone());
        let mut env = HashMap::new();
        for name in ["PATH", "HOME", "LANG", "LC_ALL", "SYSTEMROOT", "TMPDIR"] {
            if let Ok(value) = std::env::var(name) {
                env.insert(name.into(), value);
            }
        }
        env.insert("TMPDIR".into(), dir.to_string_lossy().into_owned());
        env.insert(
            "XDG_CACHE_HOME".into(),
            dir.join("cache").to_string_lossy().into_owned(),
        );
        env.insert("PYTHONDONTWRITEBYTECODE".into(), "1".into());
        let spec = agentflow_process_supervisor::ProcessSpec {
            program: check.argv[0].clone().into(),
            args: check.argv[1..].to_vec(),
            cwd: workspace.into(),
            env,
            clear_environment: true,
            env_denylist: Vec::new(),
            timeout: Duration::from_secs(u64::from(check.timeout_secs)),
            idle_timeout: Duration::from_secs(u64::from(check.timeout_secs)),
            stdout_path: dir.join("stdout.log"),
            stderr_path: dir.join("stderr.log"),
            lease_path: dir.join("process-lease.json"),
        };
        // Persist the active Runner lease location before spawning. Recovery also scans check
        // directories: a terminated model process does not prove its test service has stopped.
        sqlx::query("UPDATE team_attempt_contexts SET start_intent=? WHERE run_id=?")
            .bind(format!("runner:{}", dir.display()))
            .bind(&claim.identity.attempt_id)
            .execute(self.store.pool())
            .await?;
        let (tx, mut rx) = mpsc::channel(256);
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        let outcome = agentflow_process_supervisor::run(spec, cancel, tx).await;
        watcher.abort();
        self.unregister_cancellation(&key);
        let _ = drain.await;
        let stdout = tokio::fs::read_to_string(dir.join("stdout.log"))
            .await
            .unwrap_or_default();
        let stderr = tokio::fs::read_to_string(dir.join("stderr.log"))
            .await
            .unwrap_or_default();
        let (status, detail) = match &outcome {
            Err(e) => (
                CriterionStatus::Blocked,
                format!("environment/spawn failure: {e}"),
            ),
            Ok(o) if o.cancelled || o.timed_out => (
                CriterionStatus::Blocked,
                format!(
                    "Runner stopped: cancelled={}, timeout={}",
                    o.cancelled, o.timed_out
                ),
            ),
            Ok(o)
                if o.exit_code == Some(check.expected_exit)
                    && check
                        .stdout_contains
                        .as_ref()
                        .is_none_or(|s| stdout.contains(s)) =>
            {
                (
                    CriterionStatus::Passed,
                    "actual exit/output assertions passed".into(),
                )
            }
            Ok(_) if stderr.contains("EADDRINUSE") || (stderr.contains("listen") && stderr.contains("EPERM")) => (
                CriterionStatus::Blocked,
                "test environment port is unavailable or listening is forbidden; product failure is not established".into(),
            ),
            Ok(_) if check.argv.get(1).is_some_and(|arg|arg=="-e") && Path::new(&check.argv[0]).file_name().is_some_and(|name|name=="node") && stderr.lines().next().is_some_and(|line|line.starts_with("[eval]:")) && stderr.contains("SyntaxError:") => (CriterionStatus::Blocked,"test package has an inline JavaScript syntax error; product failure is not established".into()),
            Ok(o) => (
                CriterionStatus::Failed,
                format!(
                    "expected exit {} and output {:?}, observed exit {:?}",
                    check.expected_exit, check.stdout_contains, o.exit_code
                ),
            ),
        };
        let environment = json!({"schemaVersion":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"sourceTree":self.git.resolve(workspace,"HEAD^{tree}").await?,"checkHash":hash(check)?,"environment":"minimal inherited PATH/HOME; isolated cache/tmp; credential env withheld","runnerVersion":env!("CARGO_PKG_VERSION")});
        let raw = json!({"check":check,"environment":environment,"outcome":outcome.as_ref().ok(),"error":outcome.as_ref().err().map(ToString::to_string),"stdout":stdout,"stderr":stderr,"status":status,"detail":detail});
        let path = dir.join("evidence.json");
        let bytes = serde_json::to_vec_pretty(&raw).map_err(|e| err(e.to_string()))?;
        let digest = hex_digest(&bytes);
        tokio::fs::write(&path, bytes).await?;
        self.store.protect_file(&path).await?;
        self.store.protect_file(&dir.join("stdout.log")).await?;
        self.store.protect_file(&dir.join("stderr.log")).await?;
        Ok((
            status,
            detail,
            path,
            format!("{}:{digest}", hash(&environment)?),
        ))
    }
    pub(super) async fn team_verify(
        &self,
        claim: &TeamClaim,
        submission: &TeamSubmission,
        workspace: &Path,
        base: &str,
    ) -> Result<(), OrchestratorError> {
        let view = self
            .team_get(&claim.identity.task_id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        let candidate = view
            .candidate
            .as_ref()
            .ok_or_else(|| err("no integrated candidate"))?;
        let member = view
            .config
            .members
            .iter()
            .find(|m| m.id == claim.identity.member_id)
            .ok_or_else(|| err("tester missing"))?;
        if !rules::independent(
            member,
            claim
                .provider
                .as_ref()
                .ok_or_else(|| err("provider missing"))?,
            TeamWorkKind::Verify,
            candidate,
            view.config.cross_vendor_review,
        ) {
            return Err(err("tester contributed to this candidate"));
        }
        let plan = view.plan.as_ref().ok_or_else(|| err("plan missing"))?;
        let known: HashSet<_> = plan
            .criteria
            .iter()
            .flat_map(|c| c.checks.iter().map(|s| s.id.as_str()))
            .collect();
        if submission
            .check_ids
            .iter()
            .any(|id| !known.contains(id.as_str()))
        {
            return Err(err("TEAM_PROTOCOL: unknown check reference"));
        }
        let mut failed_owners = Vec::new();
        let mut failed_details = Vec::new();
        let mut environment_blocked = false;
        for criterion in &plan.criteria {
            for check in &criterion.checks {
                if !submission.check_ids.contains(&check.id) {
                    continue;
                }
                if !self.store.team_identity_valid(&claim.identity).await? {
                    return Err(err("verification cancelled"));
                }
                let (status, detail, path, digests) = self
                    .team_run_check(claim, check, workspace, &check.id)
                    .await?;
                let (environment_hash, raw_sha256) = digests
                    .split_once(':')
                    .ok_or_else(|| err("invalid evidence digest"))?;
                let evidence = TeamEvidence {
                    id: Uuid::now_v7().to_string(),
                    criterion_id: criterion.id.clone(),
                    criterion_hash: hash(criterion)?,
                    candidate_id: candidate.id.clone(),
                    candidate_tree: candidate.tree_sha.clone(),
                    attempt_id: claim.identity.attempt_id.clone(),
                    member_id: claim.identity.member_id.clone(),
                    check_id: check.id.clone(),
                    check_hash: hash(check)?,
                    status,
                    source: "runner_rule".into(),
                    environment_hash: environment_hash.into(),
                    raw_path: path.to_string_lossy().into_owned(),
                    raw_sha256: raw_sha256.into(),
                    detail: detail.clone(),
                };
                sqlx::query("INSERT INTO team_evidence(id,task_id,candidate_id,criterion_id,attempt_id,evidence_json,created_at) VALUES(?,?,?,?,?,?,?)").bind(&evidence.id).bind(&claim.identity.task_id).bind(&candidate.id).bind(&criterion.id).bind(&claim.identity.attempt_id).bind(encode(&evidence)?).bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
                if status == CriterionStatus::Failed && criterion.required {
                    failed_owners.push(criterion.owner_work_item_id.clone());
                    failed_details.push(format!("{} / {}: {detail}", criterion.id, check.id));
                }
                if status == CriterionStatus::Blocked {
                    environment_blocked = true;
                }
            }
        }
        if self.git.resolve(workspace, "HEAD").await? != base
            || self.git.has_changes(workspace).await?
        {
            return Err(err(
                "OUTPUT_SCOPE: verification modified the candidate; evidence cannot validate modified source",
            ));
        }
        // The verification operation completed even if product assertions failed. Its per-check
        // facts remain queryable; work acceptance still requires all required checks to pass.
        self.store
            .team_finish(
                claim,
                true,
                None,
                &agentflow_process_supervisor::redact(format!(
                    "Runner executions recorded individually. CLI environment observations: {}",
                    submission.environment_notes.join("; ")
                )),
                None,
            )
            .await?;
        let after = self
            .team_get(&claim.identity.task_id)
            .await?
            .ok_or_else(|| err("team missing"))?;
        let all = after
            .criteria
            .iter()
            .filter(|c| c.criterion.required)
            .all(|c| c.status == CriterionStatus::Passed && !c.stale);
        if all && submission.issues.is_empty() && submission.approved {
            sqlx::query("UPDATE team_issues SET resolved_by_candidate_id=? WHERE task_id=? AND criterion_id IS NOT NULL AND resolved_by_candidate_id IS NULL").bind(&candidate.id).bind(&claim.identity.task_id).execute(self.store.pool()).await?;
        } else if !failed_owners.is_empty() {
            failed_owners.sort();
            failed_owners.dedup();
            self.team_request_rework(
                &claim.identity.task_id,
                &failed_owners,
                &failed_details.join("; "),
                Some("runner_assertions"),
            )
            .await?;
        } else {
            let reason = if environment_blocked {
                "test environment or test package blocked; source is not proven failed"
            } else {
                "required criteria remain not_run/inconclusive or tester found unresolved omissions"
            };
            sqlx::query(
                "UPDATE team_work_items SET state='blocked',detail=? WHERE task_id=? AND id=?",
            )
            .bind(format!("{reason}: {}", submission.issues.join("; ")))
            .bind(&claim.identity.task_id)
            .bind(&claim.spec.id)
            .execute(self.store.pool())
            .await?;
        }
        Ok(())
    }
}
