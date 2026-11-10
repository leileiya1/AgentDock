//! Read-only recovery facts and versioned plan previews. Resource release is owner-only and
//! requires OS proof; a UI retry or expired lease never constitutes that proof.
use super::*;

struct ResourceProof {
    state: TeamResourceState,
    detail: String,
    leases: Vec<agentflow_process_supervisor::ProcessLease>,
}
async fn inspect_resources(
    dir: &Path,
    intent: &str,
    managed_probes: bool,
) -> Result<ResourceProof, OrchestratorError> {
    let mut paths = vec![
        dir.join("process-lease.json"),
        dir.join("probe/process-lease.json"),
    ];
    let mut unknown = false;
    match tokio::fs::read_dir(dir.join("checks")).await {
        Ok(mut entries) => {
            while let Some(entry) = entries.next_entry().await? {
                paths.push(entry.path().join("process-lease.json"));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => unknown = true,
    }
    if intent == "starting"
        && !dir.join("process-lease.json").exists()
        && !dir.join("process-outcome.json").exists()
    {
        unknown = true;
    }
    if let Some(check_dir) = intent.strip_prefix("runner:") {
        let check = Path::new(check_dir);
        if !check.starts_with(dir.join("checks"))
            || (!check.join("process-lease.json").exists()
                && !check.join("process-outcome.json").exists())
        {
            unknown = true;
        }
    }
    if let Some(probe_dir) = intent.strip_prefix("probe:") {
        let probe = Path::new(probe_dir);
        if probe != dir.join("probe")
            || (!probe.join("process-lease.json").exists()
                && !probe.join("process-outcome.json").exists())
        {
            unknown = true;
        }
    }
    let mut present = false;
    let mut leases = Vec::new();
    let has_outcome = paths.iter().any(|p| {
        p.parent()
            .is_some_and(|parent| parent.join("process-outcome.json").exists())
    });
    for path in paths {
        if !path.exists() {
            continue;
        }
        match agentflow_process_supervisor::read_process_lease(&path).await {
            Ok(lease) => {
                match agentflow_process_supervisor::inspect_process_group_lease(&lease).await {
                    Ok(Some(true)) => present = true,
                    Ok(Some(false)) => {}
                    Ok(None) | Err(_) => unknown = true,
                }
                leases.push(lease);
            }
            Err(_) => unknown = true,
        }
    }
    if intent == "prepared"
        && !managed_probes
        && leases.is_empty()
        && !dir.join("process-outcome.json").exists()
    {
        unknown = true; // Old prepared attempts could have an unregistered model probe.
    }
    let state = if unknown {
        TeamResourceState::Unknown
    } else if present {
        TeamResourceState::Present
    } else if leases.is_empty() && !has_outcome && matches!(intent, "reserved" | "prepared") {
        TeamResourceState::NotStarted
    } else {
        TeamResourceState::Stopped
    };
    let detail = match state {
        TeamResourceState::Unknown => {
            "无法核实旧进程组身份或启动结果；保留席位、预算历史和工作区，禁止旧结果发布"
        }
        TeamResourceState::Present => {
            "操作系统仍可观察到受管进程组；仅能按记录的身份回收，不能直接释放席位"
        }
        TeamResourceState::Stopped => {
            "已检查 CLI 及检查器的全部已记录进程组，未发现仍存活的受管资源"
        }
        TeamResourceState::NotStarted => "持久化启动意图尚未进入进程启动；没有已记录的子进程",
    }
    .into();
    Ok(ResourceProof {
        state,
        detail,
        leases,
    })
}

impl Orchestrator {
    pub(super) async fn team_resource_proof_unreaped(
        &self,
        claim: &TeamClaim,
    ) -> Result<bool, OrchestratorError> {
        let intent: String =
            sqlx::query_scalar("SELECT start_intent FROM team_attempt_contexts WHERE run_id=?")
                .bind(&claim.identity.attempt_id)
                .fetch_one(self.store.pool())
                .await?;
        Ok(matches!(
            inspect_resources(
                &claim.run_dir,
                &intent,
                claim.input["resourceProtocolVersion"] == 2
            )
            .await?
            .state,
            TeamResourceState::Present | TeamResourceState::Unknown
        ))
    }
    pub async fn team_recovery_get(
        &self,
        task: &str,
    ) -> Result<TeamRecoveryView, OrchestratorError> {
        let view = self
            .team_get(task)
            .await?
            .ok_or_else(|| err("not a team task"))?;
        let rows = sqlx::query("SELECT r.id,r.agent,r.run_dir,c.identity_json,c.start_intent,c.failure_class,c.cancel_requested,c.input_json FROM agent_runs r JOIN team_attempt_contexts c ON c.run_id=r.id WHERE r.task_id=? AND r.status='RUNNING' ORDER BY r.created_at")
            .bind(task).fetch_all(self.store.pool()).await?;
        let mut resources = Vec::new();
        for row in rows {
            let identity: TeamIdentity = decode(row.get("identity_json"))?;
            let proof = inspect_resources(
                Path::new(&row.get::<String, _>("run_dir")),
                &row.get::<String, _>("start_intent"),
                decode::<Value>(row.get("input_json"))?["resourceProtocolVersion"] == 2,
            )
            .await?;
            resources.push(TeamResourceInspection {
                attempt_id: identity.attempt_id,
                work_item_id: identity.work_item_id,
                member_id: identity.member_id,
                provider: row
                    .get::<Option<String>, _>("agent")
                    .map(parse)
                    .transpose()?,
                failure_class: row.get("failure_class"),
                cancel_requested: row.get::<i64, _>("cancel_requested") != 0,
                state: proof.state,
                detail: proof.detail,
            });
        }
        let row = self.task(task).await?;
        let project = self.team_project(task).await?;
        let target_current = self
            .git
            .resolve(&project.repo, &row.target_branch)
            .await
            .ok();
        let unfinished = !matches!(view.phase, TeamPhase::Delivered | TeamPhase::Cancelled);
        let target_changed = unfinished
            && row
                .base_commit
                .as_ref()
                .zip(target_current.as_ref())
                .is_some_and(|(a, b)| a != b);
        let config_changed = self.team_config_drift(task).await?;
        let mut notices = Vec::new();
        if !resources.is_empty() {
            notices.push(
                "旧尝试占用的资源尚未全部释放。先检查并对账；恢复和换成员不会绕过占用。".into(),
            );
        }
        if resources
            .iter()
            .any(|r| r.state == TeamResourceState::Unknown)
        {
            notices.push("有结果未知的进程；本页不提供强制释放。补齐进程身份或确认其生命周期后再对账，租约过期不算停止证明。".into());
        }
        if unfinished && view.attempts_used >= view.config.max_attempts {
            notices
                .push("累计尝试额度已用尽；恢复不会重置额度。现有成果和失败记录继续保留。".into());
        }
        if unfinished && view.reworks_used >= view.config.max_reworks {
            notices.push(
                "产品返工额度已用尽；只能完成仍有额度的核对、验证和审查，不能通过换计划重开返工。"
                    .into(),
            );
        }
        if target_changed {
            notices.push(
                "目标分支已变化。空闲后更新基线，保留有效源码成果并重新集成、实测和独立审查。"
                    .into(),
            );
        }
        if target_current.is_none() {
            notices.push("无法读取交付目标分支；检查仓库和分支状态。".into());
        }
        if config_changed {
            notices.push(
                "项目配置与任务快照不一致。恢复原配置，或按新授权新建任务；旧证据不能用于新策略。"
                    .into(),
            );
        }
        for item in view
            .items
            .iter()
            .filter(|w| w.state == TeamWorkState::Blocked)
        {
            if let Some(class) = view
                .attempts
                .iter()
                .rev()
                .find(|a| a.identity.work_item_id == item.spec.id)
                .and_then(|a| a.failure_class.as_deref())
            {
                let guidance = match class {
                    "auth" => "在设置中重新登录该 Provider，再恢复此项",
                    "protocol_repair" => "检查 CLI 兼容性；恢复将创建新尝试，保留旧输出和累计消耗",
                    "waiting_input" => "查看成员提出的问题，补齐关键事实后修订计划或工作规范",
                    "product_rework" => "查看真实失败证据；修复仍受累计返工上限约束",
                    "crash_recovery" => "旧资源已对账，可从检查点创建新尝试；未接受旧输出",
                    _ => "查看该项失败详情，处理原因后恢复",
                };
                notices.push(format!("{}：{guidance}", item.spec.title));
            }
        }
        Ok(TeamRecoveryView {
            task_id: task.into(),
            phase: view.phase,
            resources,
            attempts_remaining: view.config.max_attempts.saturating_sub(view.attempts_used),
            reworks_remaining: view.config.max_reworks.saturating_sub(view.reworks_used),
            target_base: row.base_commit,
            target_current,
            target_changed,
            config_changed,
            notices,
            inspected_at: Utc::now().to_rfc3339(),
        })
    }

    /// `online` excludes a live supervisor. Startup recovery runs before workers are admitted.
    pub(super) async fn team_reconcile_claim(
        &self,
        claim: &TeamClaim,
        intent: &str,
        online: bool,
    ) -> Result<(), OrchestratorError> {
        if online
            && self
                .active_cancellations
                .read()
                .map_err(|_| err("resource guard poisoned"))?
                .contains_key(&format!(
                    "team/{}/{}",
                    claim.identity.task_id, claim.identity.attempt_id
                ))
        {
            return Ok(());
        }
        let proof = inspect_resources(
            &claim.run_dir,
            intent,
            claim.input["resourceProtocolVersion"] == 2,
        )
        .await?;
        for lease in &proof.leases {
            if agentflow_process_supervisor::inspect_process_group_lease(lease).await? == Some(true)
            {
                agentflow_process_supervisor::terminate_process_lease(
                    lease,
                    Duration::from_millis(200),
                )
                .await?;
            }
        }
        let after = inspect_resources(
            &claim.run_dir,
            intent,
            claim.input["resourceProtocolVersion"] == 2,
        )
        .await?;
        if matches!(
            after.state,
            TeamResourceState::Unknown | TeamResourceState::Present
        ) {
            sqlx::query("UPDATE team_attempt_contexts SET cancel_requested=1,failure_class='result_unknown' WHERE run_id=? AND EXISTS(SELECT 1 FROM agent_runs WHERE id=? AND status='RUNNING')")
                .bind(&claim.identity.attempt_id).bind(&claim.identity.attempt_id).execute(self.store.pool()).await?;
            self.team_set_phase(
                &claim.identity.task_id,
                TeamPhase::Blocked,
                Some(&after.detail),
            )
            .await?;
            return Ok(());
        }
        self.store
            .team_finish(
                claim,
                false,
                None,
                "old resources reconciled; unaccepted output retained, fresh attempt required",
                Some("crash_recovery"),
            )
            .await?;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("UPDATE team_attempt_contexts SET failure_class='crash_recovery',start_intent='finished' WHERE run_id=? AND EXISTS(SELECT 1 FROM agent_runs WHERE id=? AND status!='RUNNING')")
            .bind(&claim.identity.attempt_id).bind(&claim.identity.attempt_id).execute(&mut *tx).await?;
        // Fenced attempts finish CANCELLED, so team_finish intentionally cannot alter their work.
        // Restore only the exact unchanged epoch/spec, and never resurrect user cancellation.
        sqlx::query("UPDATE team_work_items SET state=CASE WHEN (SELECT phase FROM team_tasks WHERE task_id=?) IN ('planning','running') THEN 'pending' ELSE 'blocked' END,detail='旧资源已对账；可从检查点创建新尝试' WHERE task_id=? AND id=? AND epoch=? AND spec_hash=? AND state IN ('active','blocked') AND EXISTS(SELECT 1 FROM team_tasks WHERE task_id=? AND phase NOT IN ('cancelled','delivered','delivering')) AND NOT EXISTS(SELECT 1 FROM agent_runs WHERE task_id=? AND team_work_item_id=? AND status='RUNNING')")
            .bind(&claim.identity.task_id).bind(&claim.identity.task_id).bind(&claim.identity.work_item_id).bind(claim.identity.lease_epoch).bind(&claim.identity.spec_hash).bind(&claim.identity.task_id).bind(&claim.identity.task_id).bind(&claim.identity.work_item_id).execute(&mut *tx).await?;
        sqlx::query("INSERT OR IGNORE INTO events(task_id,run_id,actor,event_type,payload_json,created_at,team_event_key) VALUES(?,?,'orchestrator','team:resources_reconciled',?,?,?)")
            .bind(&claim.identity.task_id).bind(&claim.identity.attempt_id).bind(json!({"identity":claim.identity,"proof":after.state,"oldOutputAccepted":false}).to_string()).bind(Utc::now().to_rfc3339()).bind(format!("{}:resources_reconciled",claim.identity.attempt_id)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn team_reconcile_resources(
        &self,
        task: &str,
    ) -> Result<(), OrchestratorError> {
        let rows=sqlx::query("SELECT r.id,r.agent,r.run_dir,c.identity_json,c.spec_json,c.input_json,c.start_intent FROM agent_runs r JOIN team_attempt_contexts c ON c.run_id=r.id WHERE r.task_id=? AND r.status='RUNNING' AND (c.cancel_requested=1 OR c.failure_class='result_unknown')")
            .bind(task).fetch_all(self.store.pool()).await?;
        let had_candidates = !rows.is_empty();
        for row in rows {
            let claim = TeamClaim {
                identity: decode(row.get("identity_json"))?,
                spec: decode(row.get("spec_json"))?,
                input: decode(row.get("input_json"))?,
                provider: row
                    .get::<Option<String>, _>("agent")
                    .map(parse)
                    .transpose()?,
                run_dir: PathBuf::from(row.get::<String, _>("run_dir")),
            };
            self.team_reconcile_claim(&claim, &row.get::<String, _>("start_intent"), true)
                .await?;
        }
        if had_candidates {
            sqlx::query("UPDATE team_tasks SET detail='旧资源已对账；可从检查点恢复，累计额度保持不变' WHERE task_id=? AND phase='blocked' AND NOT EXISTS(SELECT 1 FROM agent_runs WHERE task_id=? AND status='RUNNING')")
                .bind(task).bind(task).execute(self.store.pool()).await?;
        }
        Ok(())
    }
}
