use super::*;
impl Orchestrator {
    pub async fn team_get(&self, task_id: &str) -> Result<Option<TeamView>, OrchestratorError> {
        let row = sqlx::query("SELECT * FROM team_tasks WHERE task_id=?")
            .bind(task_id)
            .fetch_optional(self.store.pool())
            .await?;
        let Some(row) = row else { return Ok(None) };
        let config: TeamConfig = decode(row.get("config_json"))?;
        let revision = row.get::<i64, _>("plan_revision") as u32;
        let plan_row =
            sqlx::query("SELECT plan_json,hash FROM team_plans WHERE task_id=? AND revision=?")
                .bind(task_id)
                .bind(revision)
                .fetch_optional(self.store.pool())
                .await?;
        let plan: Option<TeamPlan> = plan_row
            .as_ref()
            .map(|r| decode(r.get("plan_json")))
            .transpose()?;
        let plan_hash = plan_row
            .as_ref()
            .map(|r| r.get::<String, _>("hash"))
            .unwrap_or_default();
        let work = sqlx::query("SELECT * FROM team_work_items WHERE task_id=? ORDER BY id")
            .bind(task_id)
            .fetch_all(self.store.pool())
            .await?;
        let mut items = Vec::new();
        for r in work {
            items.push(TeamWorkView {
                spec: decode(r.get("spec_json"))?,
                spec_hash: r.get("spec_hash"),
                state: parse(r.get("state"))?,
                epoch: r.get::<i64, _>("epoch") as u32,
                artifact_id: r.get("artifact_id"),
                detail: r.get("detail"),
            });
        }
        let runs=sqlx::query("SELECT r.*,c.identity_json,c.worktree,c.input_base_sha,c.failure_class FROM agent_runs r JOIN team_attempt_contexts c ON c.run_id=r.id WHERE r.task_id=? ORDER BY r.created_at,r.id").bind(task_id).fetch_all(self.store.pool()).await?;
        let mut attempts = Vec::new();
        for r in runs {
            let provider: Option<String> = r.get("agent");
            if let Some(provider) = provider {
                attempts.push(TeamAttemptView {
                    identity: decode(r.get("identity_json"))?,
                    provider: parse(provider)?,
                    status: parse(r.get("status"))?,
                    worktree: r.get("worktree"),
                    input_base_sha: r.get("input_base_sha"),
                    failure_class: r.get("failure_class"),
                    cost_usd: r.get("cost_usd"),
                    tokens_in: r.get("tokens_in"),
                    tokens_out: r.get("tokens_out"),
                });
            }
        }
        let artifacts = self.team_artifacts(task_id).await?;
        let candidate = match row.get::<Option<String>, _>("current_candidate_id") {
            Some(id) => sqlx::query_scalar::<_, String>(
                "SELECT candidate_json FROM team_candidates WHERE id=? AND task_id=?",
            )
            .bind(id)
            .bind(task_id)
            .fetch_optional(self.store.pool())
            .await?
            .map(decode)
            .transpose()?,
            None => None,
        };
        let evidence = self.team_evidence(task_id).await?;
        let mut criteria = Vec::new();
        for c in plan
            .as_ref()
            .map(|p| p.criteria.clone())
            .unwrap_or_default()
        {
            let relevant: Vec<_> = evidence
                .iter()
                .filter(|e| e.criterion_id == c.id)
                .cloned()
                .collect();
            let status = rules::criterion_status(&c, &hash(&c)?, candidate.as_ref(), &relevant);
            let stale = !relevant.is_empty()
                && !relevant.iter().any(|e| {
                    candidate.as_ref().is_some_and(|n| e.candidate_id == n.id)
                        && e.criterion_hash == hash(&c).unwrap_or_default()
                });
            criteria.push(TeamCriterionView {
                criterion: c,
                status,
                stale,
                evidence: relevant,
            });
        }
        let config_drift = self.team_config_drift(task_id).await?;
        if config_drift {
            for criterion in &mut criteria {
                criterion.stale = !criterion.evidence.is_empty();
                criterion.status = CriterionStatus::NotRun;
            }
        }
        let mut view=TeamView{task_id:task_id.into(),phase:parse(row.get("phase"))?,original_goal:row.get("goal"),config,requirements:decode(row.get("requirements_json"))?,plan,plan_revision:revision,items,attempts,artifacts,candidate,criteria,gate_blockers:Vec::new(),attempts_used:row.get::<i64,_>("attempts_used") as u32,reworks_used:row.get::<i64,_>("reworks_used") as u32,detail:row.get("detail"),report_path:row.get("report_path"),limitations:vec![
            "本地单 Git 项目；团队协议 v1 只接内置 Claude Code/Codex CLI，旧 sidecar/API 不具备本轮团队工具协议".into(),
            "团队预算限制受管尝试次数和单次时长；未知 Token/费用保留未知，不承诺订阅或硬费用上限".into(),
            "读写范围含后端产物检查；CLI 沙箱与内部再分派限制按实际适配器保证记录，不能替代操作系统容器隔离".into(),
            "真实实测通道为本地命令 Runner（可执行 API/浏览器测试程序）；无内置浏览器会话产品能力".into(),
        ]};
        let coverage:i64=sqlx::query_scalar("SELECT COUNT(*) FROM team_audits a JOIN agent_runs r ON r.id=a.attempt_id WHERE a.task_id=? AND r.status='SUCCEEDED' AND a.kind='coverage' AND plan_hash=? AND approved=1")
            .bind(task_id).bind(&plan_hash).fetch_one(self.store.pool()).await?;
        let reviews = self.team_valid_reviews(&view, &plan_hash).await?;
        let required = self.team_required_reviews(task_id, &view).await?;
        let issues: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_issues WHERE task_id=? AND resolved_by_candidate_id IS NULL",
        )
        .bind(task_id)
        .fetch_one(self.store.pool())
        .await?;
        view.gate_blockers = rules::final_gate(
            &view,
            coverage > 0,
            reviews.len(),
            required,
            issues as usize,
        );
        if config_drift {
            view.gate_blockers.push(
                "project configuration differs from the task snapshot; current evidence is invalid"
                    .into(),
            );
        }
        Ok(Some(view))
    }
    pub(super) async fn team_artifacts(
        &self,
        task: &str,
    ) -> Result<Vec<TeamArtifact>, OrchestratorError> {
        sqlx::query_scalar::<_, String>(
            "SELECT artifact_json FROM team_artifacts WHERE task_id=? ORDER BY created_at,id",
        )
        .bind(task)
        .fetch_all(self.store.pool())
        .await?
        .into_iter()
        .map(|s| decode(s).map_err(Into::into))
        .collect()
    }
    pub(super) async fn team_evidence(
        &self,
        task: &str,
    ) -> Result<Vec<TeamEvidence>, OrchestratorError> {
        sqlx::query_scalar::<_,String>("SELECT e.evidence_json FROM team_evidence e JOIN agent_runs r ON r.id=e.attempt_id WHERE e.task_id=? AND r.status='SUCCEEDED' ORDER BY e.created_at,e.id").bind(task).fetch_all(self.store.pool()).await?.into_iter().map(|s|decode(s).map_err(Into::into)).collect()
    }
    pub(super) async fn team_valid_reviews(
        &self,
        view: &TeamView,
        plan_hash: &str,
    ) -> Result<Vec<(String, AgentKind)>, OrchestratorError> {
        let Some(candidate) = &view.candidate else {
            return Ok(Vec::new());
        };
        let project = self.team_project(&view.task_id).await?;
        let council = &project.settings.review_council;
        let strict_cross_vendor = view.config.cross_vendor_review
            || self.team_required_reviews(&view.task_id, view).await? > 1;
        let rows=sqlx::query("SELECT a.member_id,a.provider FROM team_audits a JOIN agent_runs r ON r.id=a.attempt_id WHERE a.task_id=? AND r.status='SUCCEEDED' AND a.kind='review' AND plan_hash=? AND candidate_id=? AND approved=1 ORDER BY a.created_at")
            .bind(&view.task_id).bind(plan_hash).bind(&candidate.id).fetch_all(self.store.pool()).await?;
        let mut seen = HashSet::new();
        let mut families = HashSet::new();
        let mut result = Vec::new();
        for row in rows {
            let id: String = row.get("member_id");
            let provider: AgentKind = parse(row.get("provider"))?;
            if let Some(member) = view
                .config
                .members
                .iter()
                .find(|m| m.id == id && m.roles.contains(&TeamRole::Reviewer))
                && rules::independent(
                    member,
                    &provider,
                    TeamWorkKind::Review,
                    candidate,
                    strict_cross_vendor,
                )
                && (!council.enabled
                    || council.reviewers.is_empty()
                    || council.reviewers.contains(&provider))
                && seen.insert(id.clone())
                && families.insert(rules::family(&provider))
            {
                result.push((id, provider));
            }
        }
        Ok(result)
    }
    pub(super) async fn team_required_reviews(
        &self,
        task_id: &str,
        view: &TeamView,
    ) -> Result<usize, OrchestratorError> {
        let project = self.team_project(task_id).await?;
        let paths: Vec<_> = view
            .candidate
            .as_ref()
            .map(|c| {
                view.artifacts
                    .iter()
                    .filter(|a| c.artifact_ids.contains(&a.id))
                    .flat_map(|a| a.changed_files.clone())
                    .collect()
            })
            .unwrap_or_default();
        let sensitive = paths
            .iter()
            .any(|p| is_security_sensitive(p) || is_test_path(p));
        let council = &project.settings.review_council;
        Ok(if sensitive || council.enabled {
            usize::from(council.minimum_successful_reviews.max(2))
        } else {
            1
        })
    }
    pub async fn team_assert_gate(&self, task_id: &str) -> Result<(), OrchestratorError> {
        if let Some(view) = self.team_get(task_id).await? {
            if !view.gate_blockers.is_empty() {
                return Err(err(format!("TEAM_GATE: {}", view.gate_blockers.join("; "))));
            }
            let candidate = view
                .candidate
                .as_ref()
                .ok_or_else(|| err("candidate missing"))?;
            // Reconcile immutable evidence contents at the final boundary, not just filenames.
            for c in view.criteria.iter().filter(|c| c.criterion.required) {
                for check in &c.criterion.checks {
                    let e = c
                        .evidence
                        .iter()
                        .rev()
                        .find(|e| {
                            e.candidate_id == candidate.id
                                && e.criterion_hash == hash(&c.criterion).unwrap_or_default()
                                && e.check_id == check.id
                                && e.check_hash == hash(check).unwrap_or_default()
                                && e.status == CriterionStatus::Passed
                        })
                        .ok_or_else(|| err("current executable evidence missing"))?;
                    let bytes = self
                        .store
                        .read_protected_file(Path::new(&e.raw_path))
                        .await?;
                    if hex_digest(&bytes) != e.raw_sha256 {
                        return Err(err("evidence digest mismatch"));
                    }
                    if candidate.contributor_member_ids.contains(&e.member_id) {
                        return Err(err("tester became a source contributor"));
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) async fn team_report(&self, task: &str) -> Result<(), OrchestratorError> {
        let Some(view) = self.team_get(task).await? else {
            return Ok(());
        };
        let dir = self.task_dir(task).join("artifacts");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join("team-report.json");
        let tmp = dir.join("team-report.tmp");
        tokio::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&view).map_err(|e| err(e.to_string()))?,
        )
        .await?;
        tokio::fs::rename(&tmp, &path).await?;
        sqlx::query("UPDATE team_tasks SET report_path=? WHERE task_id=?")
            .bind(path.to_string_lossy().as_ref())
            .bind(task)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }
}

impl Orchestrator {
    pub async fn team_evidence_read(
        &self,
        task: &str,
        evidence_id: &str,
    ) -> Result<String, OrchestratorError> {
        let raw: String =
            sqlx::query_scalar("SELECT evidence_json FROM team_evidence WHERE id=? AND task_id=?")
                .bind(evidence_id)
                .bind(task)
                .fetch_one(self.store.pool())
                .await?;
        let evidence: TeamEvidence = decode(raw)?;
        let path = tokio::fs::canonicalize(&evidence.raw_path).await?;
        let root = tokio::fs::canonicalize(self.task_dir(task)).await?;
        if !path.starts_with(root) {
            return Err(err("evidence path escapes its task"));
        }
        let bytes = self.store.read_protected_file(&path).await?;
        if hex_digest(&bytes) != evidence.raw_sha256 {
            return Err(err("evidence content digest mismatch"));
        }
        let text = String::from_utf8(bytes).map_err(|e| err(e.to_string()))?;
        let redacted = agentflow_process_supervisor::redact(text);
        Ok(if redacted.chars().count() > 100_000 {
            redacted.chars().take(100_000).collect::<String>()
                + "\n[display truncated; full evidence hash retained]"
        } else {
            redacted
        })
    }
}

impl Orchestrator {
    pub(super) async fn team_config_drift(&self, task: &str) -> Result<bool, OrchestratorError> {
        let snapshot: String =
            sqlx::query_scalar("SELECT project_settings_json FROM team_tasks WHERE task_id=?")
                .bind(task)
                .fetch_one(self.store.pool())
                .await?;
        let snapshot: Value = decode(snapshot)?;
        let project = self.team_project(task).await?;
        let current = team_config_file_hash(&project.repo).await?;
        Ok(snapshot
            .get("_teamProjectConfigSha")
            .and_then(Value::as_str)
            != Some(current.as_str()))
    }
}
