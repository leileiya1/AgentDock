impl Orchestrator {
    async fn refresh_provider_registry(&self) {
        let root = self.app_data.join("providers");
        let Ok(discovered) = ProviderRegistry::discover(&root).await else {
            return;
        };
        if let Ok(mut registry) = self.provider_registry.write() {
            *registry = discovered;
        }
    }

    pub async fn project_list(&self) -> Result<Vec<Project>, OrchestratorError> {
        self.store.projects().await.map_err(Into::into)
    }
    pub async fn task_list(&self, project_id: &str) -> Result<Vec<TaskSummary>, OrchestratorError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM tasks WHERE project_id=? AND deleted_at IS NULL ORDER BY seq",
        )
        .bind(project_id)
        .fetch_all(self.store.pool())
        .await?;
        let mut tasks = Vec::with_capacity(ids.len());
        for id in ids {
            tasks.push(self.store.task_summary(&id).await?);
        }
        Ok(tasks)
    }
    pub async fn task_get(&self, task_id: &str) -> Result<TaskDetail, OrchestratorError> {
        let task = self.task(task_id).await?;
        let summary = self.store.task_summary(task_id).await?;
        let rows = sqlx::query("SELECT revision,commit_sha,diff_stat_json,created_at FROM task_revisions WHERE task_id=? ORDER BY revision")
            .bind(task_id).fetch_all(self.store.pool()).await?;
        let revisions = rows
            .into_iter()
            .map(|r| RevisionInfo {
                revision: r.get("revision"),
                commit_sha: r.get("commit_sha"),
                stat: r
                    .get::<Option<String>, _>("diff_stat_json")
                    .and_then(|v| serde_json::from_str(&v).ok()),
                created_at: r.get("created_at"),
            })
            .collect();
        let acceptance_criteria = self.task_acceptance_criteria(task_id).await?;
        Ok(TaskDetail {
            summary,
            description: task.description,
            target_branch: task.target_branch,
            base_commit: task.base_commit,
            branch: task.branch,
            max_revisions: task.max_revisions,
            acceptance_criteria,
            blocked_detail: task.blocked_detail,
            revisions,
            policy: task.policy.clone(),
            plan: self.latest_plan(task_id).await?,
            budget: self.budget_usage(task_id).await?,
            delivery: self.delivery_record(task_id).await?,
        })
    }

    async fn task_acceptance_criteria(
        &self,
        task_id: &str,
    ) -> Result<Vec<AcceptanceCriterion>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT id,kind,text,position FROM task_acceptance_criteria WHERE task_id=? ORDER BY position",
        )
        .bind(task_id)
        .fetch_all(self.store.pool())
        .await?;
        rows.into_iter()
            .map(|row| {
                let kind = AcceptanceCriterionKind::from_str(&row.get::<String, _>("kind"))
                    .map_err(OrchestratorError::Config)?;
                Ok(AcceptanceCriterion {
                    id: row.get("id"),
                    kind,
                    text: row.get("text"),
                    position: row.get("position"),
                })
            })
            .collect()
    }

    async fn acceptance_criteria_markdown(
        &self,
        task_id: &str,
    ) -> Result<String, OrchestratorError> {
        let criteria = self.task_acceptance_criteria(task_id).await?;
        if criteria.is_empty() {
            return Ok("（未单独设置结构化验收条件）".into());
        }
        Ok(criteria
            .iter()
            .map(|criterion| {
                let kind = match criterion.kind {
                    AcceptanceCriterionKind::Build => "构建",
                    AcceptanceCriterionKind::Test => "测试",
                    AcceptanceCriterionKind::Behavior => "行为",
                    AcceptanceCriterionKind::Manual => "人工",
                };
                format!("- [{}] {}", kind, criterion.text)
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
    pub async fn events_list(
        &self,
        task_id: &str,
        after_id: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<TaskEvent>, OrchestratorError> {
        self.store
            .events(task_id, after_id.unwrap_or(0), limit.unwrap_or(100))
            .await
            .map_err(Into::into)
    }
    pub async fn diff_get(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<DiffPayload, OrchestratorError> {
        let task = self.task(task_id).await?;
        let project = self.project(&task.project_id).await?;
        let config = self.load_trusted_config(&project).await?;
        let sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_one(self.store.pool())
        .await?;
        self.git
            .diff(
                &required_path(&task.worktree_path)?,
                task.base_commit
                    .as_deref()
                    .ok_or_else(|| OrchestratorError::InvalidState("base commit missing".into()))?,
                &sha,
                &config.review.exclude_globs,
                // 桌面端查看用大预算，逐文件展示完整逐行内容；送审仍用 config 的小预算控制 token。
                agentflow_git_engine::UI_DIFF_MAX_BYTES,
            )
            .await
            .map_err(Into::into)
    }
    pub async fn reject(
        &self,
        task_id: &str,
        revision: i64,
        reason: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        if reason.trim().is_empty() {
            return Err(OrchestratorError::InvalidState(
                "reject reason is required".into(),
            ));
        }
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::WaitingForHumanApproval || task.revision != revision {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_one(self.store.pool())
        .await?;
        let patch = self
            .git
            .full_patch(
                &required_path(&task.worktree_path)?,
                task.base_commit.as_deref().unwrap_or(""),
                &sha,
            )
            .await?;
        let hash = format!("{:x}", Sha256::digest(patch));
        sqlx::query("INSERT INTO approvals(id,task_id,revision,commit_sha,diff_sha256,action,reason,created_at) VALUES(?,?,?,?,?,'reject',?,?)").bind(Uuid::now_v7().to_string()).bind(task_id).bind(revision).bind(sha).bind(hash).bind(reason).bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
        sqlx::query("UPDATE tasks SET blocked_detail=? WHERE id=?")
            .bind(reason)
            .bind(task_id)
            .execute(self.store.pool())
            .await?;
        let to = if task.revision >= task.max_revisions {
            TaskStatus::Blocked
        } else {
            TaskStatus::ReadyForRevision
        };
        let blocked = (to == TaskStatus::Blocked).then_some(BlockedReason::MaxRevisions);
        self.store
            .transition(
                task_id,
                &[TaskStatus::WaitingForHumanApproval],
                to,
                blocked,
                Actor::Human,
                "human:reject",
                &json!({"reason":reason}),
            )
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn resume_with_guidance(
        &self,
        task_id: &str,
        guidance: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::Blocked {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let blocked_reason = self.store.task_summary(task_id).await?.blocked_reason;
        sqlx::query("UPDATE tasks SET blocked_detail=?,blocked_reason=NULL WHERE id=?")
            .bind(guidance)
            .bind(task_id)
            .execute(self.store.pool())
            .await?;
        let to = if task.revision == 0 && task.policy.require_plan_approval {
            // A failed planner has not produced an approved plan. Resume the planning checkpoint;
            // skipping straight to development would bypass the user's plan gate (P1-01).
            TaskStatus::Planning
        } else if task.revision == 0 {
            TaskStatus::ReadyForDevelopment
        } else if blocked_reason == Some(BlockedReason::ReviewFailed) {
            // The revision commit and validation evidence are already durable. A reviewer process
            // or result-contract failure must retry that read-only checkpoint, not create a new
            // development revision that can only end as a misleading "no changes" failure.
            TaskStatus::ReadyForReview
        } else {
            TaskStatus::ReadyForRevision
        };
        self.store
            .transition(
                task_id,
                &[TaskStatus::Blocked],
                to,
                None,
                Actor::Human,
                "human:resume_with_guidance",
                &json!({"guidance":guidance}),
            )
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn force_approve(&self, task_id: &str) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::Blocked || task.revision == 0 {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        self.store
            .transition(
                task_id,
                &[TaskStatus::Blocked],
                TaskStatus::WaitingForHumanApproval,
                None,
                Actor::Human,
                "human:force_approve",
                &json!({}),
            )
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn cancel(&self, task_id: &str) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if matches!(
            task.status,
            TaskStatus::Merged | TaskStatus::RolledBack | TaskStatus::Cancelled
        ) {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        // Make cancellation authoritative before signalling the child. Development/review loops
        // re-check this state and must never fall back to another Provider after cancellation.
        self.store
            .transition(
                task_id,
                &[task.status],
                TaskStatus::Cancelled,
                None,
                Actor::Human,
                "human:cancel",
                &json!({}),
            )
            .await?;
        self.cancel_active_run(task_id);
        // Removing a worktree while the CLI is still writing races with the child. Wait for
        // the supervisor to finish its bounded TERM/KILL sequence; retain the worktree if it
        // cannot stop so diagnostics and user files remain recoverable.
        for _ in 0..48 {
            let running: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND status='RUNNING'",
            )
            .bind(task_id)
            .fetch_one(self.store.pool())
            .await?;
            if running == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND status='RUNNING'",
        )
        .bind(task_id)
        .fetch_one(self.store.pool())
        .await?;
        if running == 0 {
            let project = self.project(&task.project_id).await?;
            if let Some(wt) = task.worktree_path.as_ref() {
                let _ = self.git.worktree_remove(&project.repo, wt).await;
            }
        }
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn merge(&self, task_id: &str) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if !matches!(
            task.status,
            TaskStatus::Approved | TaskStatus::MergeConflict
        ) {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let project = self.project(&task.project_id).await?;
        let seal = self.approval_seal(&task).await?;
        self.verify_sealed_task_heads(&task, &project, &seal)
            .await?;
        if self.git.default_branch(&project.repo).await? != task.target_branch {
            return Err(OrchestratorError::MergePrecondition(
                "main checkout is not on target branch".into(),
            ));
        }
        if !self.git.is_clean(&project.repo).await? {
            return Err(OrchestratorError::MergePrecondition(
                "main checkout has uncommitted changes".into(),
            ));
        }
        let pre_merge_commit = self.git.resolve(&project.repo, "HEAD").await?;
        self.store
            .transition(
                task_id,
                &[task.status],
                TaskStatus::Merging,
                None,
                Actor::Human,
                "human:merge",
                &json!({}),
            )
            .await?;
        let message = format!("[agentflow] merge TASK-{}: {}", task.seq, task.title);
        let branch = task
            .branch
            .as_deref()
            .ok_or_else(|| OrchestratorError::InvalidState("branch missing".into()))?;
        match self.git.merge(&project.repo, branch, &message).await {
            Ok(()) => {
                let merge_commit = self.git.resolve(&project.repo, "HEAD").await?;
                self.store
                    .transition(
                        task_id,
                        &[TaskStatus::Merging],
                        TaskStatus::Merged,
                        None,
                        Actor::Orchestrator,
                        "merge:succeeded",
                        &json!({"pre_merge_commit":pre_merge_commit,"merge_commit":merge_commit}),
                    )
                    .await?;
                if let Some(wt) = task.worktree_path.as_ref() {
                    let _ = self.git.worktree_remove(&project.repo, wt).await;
                }
                sqlx::query("UPDATE delivery_records SET state='merged',ci_status='passed',pre_merge_commit=?,merge_commit=?,updated_at=? WHERE task_id=?")
                    .bind(&pre_merge_commit).bind(&merge_commit).bind(Utc::now().to_rfc3339())
                    .bind(task_id).execute(self.store.pool()).await?;
            }
            Err(e) => {
                let _ = self.git.abort_merge(&project.repo).await;
                self.store
                    .transition(
                        task_id,
                        &[TaskStatus::Merging],
                        TaskStatus::MergeConflict,
                        None,
                        Actor::Orchestrator,
                        "merge:conflict",
                        &json!({"error":e.to_string()}),
                    )
                    .await?;
            }
        }
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn mark_merged_external(
        &self,
        task_id: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if !matches!(
            task.status,
            TaskStatus::Approved | TaskStatus::MergeConflict
        ) {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let project = self.project(&task.project_id).await?;
        let seal = self.approval_seal(&task).await?;
        let merge_commit = self.git.resolve(&project.repo, "HEAD").await?;
        if !self
            .git
            .is_ancestor(&project.repo, &seal.commit_sha, &merge_commit)
            .await?
        {
            return Err(OrchestratorError::MergePrecondition(
                "approved commit is not contained in the target branch".into(),
            ));
        }
        self.store
            .transition(
                task_id,
                &[task.status],
                TaskStatus::Merged,
                None,
                Actor::Human,
                "human:mark_merged_external",
                &json!({}),
            )
            .await?;
        if let Some(wt) = task.worktree_path.as_ref() {
            let _ = self.git.worktree_remove(&project.repo, wt).await;
        }
        sqlx::query("UPDATE delivery_records SET state='merged',ci_status='passed',merge_commit=COALESCE(?,merge_commit),updated_at=? WHERE task_id=?")
            .bind(Some(merge_commit)).bind(Utc::now().to_rfc3339()).bind(task_id)
            .execute(self.store.pool()).await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn run_list(&self, task_id: &str) -> Result<Vec<RunSummary>, OrchestratorError> {
        let rows=sqlx::query("SELECT id,task_id,revision,role,agent,status,exit_code,cost_usd,tokens_in,tokens_out,started_at,finished_at FROM agent_runs WHERE task_id=? ORDER BY created_at").bind(task_id).fetch_all(self.store.pool()).await?;
        rows.into_iter()
            .map(|r| {
                Ok(RunSummary {
                    id: r.get("id"),
                    task_id: r.get("task_id"),
                    revision: r.get("revision"),
                    role: parse(r.get("role"))?,
                    agent: parse_opt(r.get("agent"))?,
                    status: parse(r.get("status"))?,
                    exit_code: r.get("exit_code"),
                    cost_usd: r.get("cost_usd"),
                    tokens_in: r.get("tokens_in"),
                    tokens_out: r.get("tokens_out"),
                    started_at: r.get("started_at"),
                    finished_at: r.get("finished_at"),
                })
            })
            .collect()
    }
    pub async fn review_get(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Option<Review>, OrchestratorError> {
        let row = sqlx::query("SELECT id,revision,commit_sha,decision,summary,reviewer_agent,member_review_ids_json,reviewer_agents_json FROM reviews WHERE task_id=? AND revision=? ORDER BY is_aggregate DESC,created_at DESC LIMIT 1")
            .bind(task_id).bind(revision).fetch_optional(self.store.pool()).await?;
        let Some(row) = row else { return Ok(None) };
        let review_id: String = row.get("id");
        let issue_rows = sqlx::query("SELECT id,severity,file,line_start,line_end,title,description,suggested_action,resolved,reported_by_json,agreement_count,severity_disagreement FROM review_issues WHERE review_id=?")
            .bind(&review_id).fetch_all(self.store.pool()).await?;
        let issues = issue_rows
            .into_iter()
            .map(|issue| {
                Ok(ReviewIssue {
                    id: issue.get("id"),
                    severity: parse(issue.get("severity"))?,
                    file: issue.get("file"),
                    line_start: issue.get("line_start"),
                    line_end: issue.get("line_end"),
                    title: issue.get("title"),
                    description: issue.get("description"),
                    suggested_action: issue.get("suggested_action"),
                    resolved: issue.get::<i64, _>("resolved") != 0,
                    reported_by: serde_json::from_str(&issue.get::<String, _>("reported_by_json"))
                        .unwrap_or_default(),
                    agreement_count: issue.get("agreement_count"),
                    severity_disagreement: issue.get::<i64, _>("severity_disagreement") != 0,
                })
            })
            .collect::<Result<Vec<_>, OrchestratorError>>()?;
        // 委员会每位成员的独立投票：聚合行记录了成员 review 行 id，逐一取回其 agent/decision/summary。
        // 单一审查（无成员 id）时为空。让前端能展示「谁投了什么」及裁决依据，而无需新增数据库列。
        let member_ids: Vec<String> = row
            .get::<Option<String>, _>("member_review_ids_json")
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        let mut member_votes = Vec::new();
        for member_id in member_ids {
            if let Some(member) =
                sqlx::query("SELECT reviewer_agent,decision,summary FROM reviews WHERE id=?")
                    .bind(&member_id)
                    .fetch_optional(self.store.pool())
                    .await?
                && let Some(agent) = member
                    .get::<Option<String>, _>("reviewer_agent")
                    .and_then(|value| value.parse().ok())
            {
                member_votes.push(CouncilMemberVote {
                    agent,
                    decision: parse(member.get("decision"))?,
                    summary: member.get("summary"),
                });
            }
        }
        Ok(Some(Review {
            id: review_id,
            revision: row.get("revision"),
            commit_sha: row.get("commit_sha"),
            decision: parse(row.get("decision"))?,
            summary: row.get("summary"),
            reviewer_agents: serde_json::from_str(&row.get::<String, _>("reviewer_agents_json"))
                .unwrap_or_else(|_| {
                    row.get::<Option<String>, _>("reviewer_agent")
                        .and_then(|value| value.parse().ok())
                        .into_iter()
                        .collect()
                }),
            member_votes,
            issues,
        }))
    }
    /// One page of a run log addressed by absolute line number. `from_line: None` returns the
    /// most recent page, which is what a viewer should show first: seeding from line 0 leaves a
    /// hole between the file's opening lines and the live tail, and hides the final result event.
    pub async fn run_log_page(
        &self,
        run_id: &str,
        from_line: Option<usize>,
        max_lines: usize,
    ) -> Result<RunLogWindow, OrchestratorError> {
        let take = max_lines.clamp(1, 1000);
        let total = self.run_log_line_count(run_id).await?;
        let start = from_line.unwrap_or_else(|| total.saturating_sub(take));
        let (lines, next_from_line, eof) = self.run_log_tail(run_id, start, take).await?;
        Ok(RunLogWindow {
            lines,
            from_line: start,
            next_from_line,
            eof,
            total_lines: total,
        })
    }

    pub async fn run_log_tail(
        &self,
        run_id: &str,
        from_line: usize,
        max_lines: usize,
    ) -> Result<(Vec<AgentEvent>, usize, bool), OrchestratorError> {
        let run_dir: String = sqlx::query_scalar("SELECT run_dir FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_one(self.store.pool())
            .await?;
        let path = Path::new(&run_dir).join("agent-events.jsonl");
        let take = max_lines.clamp(1, 1000);
        // Following a live run is the hot path: the desktop polls every run several times a
        // second. Resuming from a cached byte offset keeps that cost proportional to the new
        // output instead of to the whole (unbounded) log.
        if let Some(result) = self.tail_live_log(run_id, &path, from_line, take).await {
            return Ok(result);
        }
        let text = self
            .read_run_file(&path)
            .await
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let all = text.lines().collect::<Vec<_>>();
        let end = from_line.saturating_add(take).min(all.len());
        let lines = all
            .iter()
            .skip(from_line)
            .take(take)
            .map(|line| parse_log_line(line))
            .collect::<Vec<_>>();
        // Advance over malformed rows as well. Otherwise one bad JSONL line pins every live
        // subscriber to the same cursor forever and hides all later valid output.
        let next = end;
        self.seed_log_cursor(run_id, &path, &text, next).await;
        Ok((lines, next, next >= all.len()))
    }

    /// Records where the full read stopped so the next sequential tail can resume incrementally.
    /// Only meaningful while the log is still plaintext on disk; encrypted logs are left alone.
    async fn seed_log_cursor(&self, run_id: &str, path: &Path, text: &str, next: usize) {
        let Ok(metadata) = tokio::fs::metadata(path).await else {
            return;
        };
        let byte = text
            .split_inclusive('\n')
            .take(next)
            .map(|line| line.len() as u64)
            .sum::<u64>();
        // If the decrypted text is not the file's own bytes, offsets would be wrong.
        if byte > metadata.len() {
            return;
        }
        self.remember_log_cursor(
            run_id,
            LogCursor {
                line: next,
                byte,
                len: metadata.len(),
            },
        );
    }

    /// Incremental tail for a plaintext (still-running) log. Returns `None` whenever the fast
    /// path does not apply — an encrypted/finished log, a rewritten file, or a cursor that is
    /// not the one we cached — so the caller falls back to the exact full-file behaviour.
    async fn tail_live_log(
        &self,
        run_id: &str,
        path: &Path,
        from_line: usize,
        take: usize,
    ) -> Option<(Vec<AgentEvent>, usize, bool)> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let cached = self
            .run_log_cursors
            .read()
            .ok()
            .and_then(|cursors| cursors.get(run_id).copied())
            .filter(|cursor| cursor.line == from_line)?;
        let len = tokio::fs::metadata(path).await.ok()?.len();
        // A shorter file means it was replaced (or protected in place); re-read from scratch.
        if len < cached.len || cached.byte > len {
            self.forget_log_cursor(run_id);
            return None;
        }
        if len == cached.byte {
            return Some((Vec::new(), from_line, true));
        }
        let mut file = tokio::fs::File::open(path).await.ok()?;
        // Protected logs are a single encrypted blob; byte offsets are meaningless there.
        let mut magic = [0_u8; 6];
        if file.read_exact(&mut magic).await.is_ok() && &magic == b"AFENC1" {
            self.forget_log_cursor(run_id);
            return None;
        }
        file.seek(std::io::SeekFrom::Start(cached.byte))
            .await
            .ok()?;
        let mut appended = Vec::new();
        file.read_to_end(&mut appended).await.ok()?;
        let text = String::from_utf8_lossy(&appended).into_owned();

        let mut events = Vec::new();
        let mut consumed_bytes = 0_u64;
        let mut consumed_lines = 0_usize;
        for line in text.split_inclusive('\n') {
            // A partially written trailing line must not advance the cursor past it.
            if !line.ends_with('\n') {
                break;
            }
            if consumed_lines == take {
                break;
            }
            consumed_bytes += line.len() as u64;
            consumed_lines += 1;
            events.push(parse_log_line(line.trim_end_matches(['\n', '\r'])));
        }
        let next = from_line + consumed_lines;
        let byte = cached.byte + consumed_bytes;
        self.remember_log_cursor(
            run_id,
            LogCursor {
                line: next,
                byte,
                len,
            },
        );
        Some((events, next, byte >= len))
    }

    fn remember_log_cursor(&self, run_id: &str, cursor: LogCursor) {
        if let Ok(mut cursors) = self.run_log_cursors.write() {
            // Bounded so a long-lived daemon cannot accumulate an entry per historical run.
            if cursors.len() > 256 && !cursors.contains_key(run_id) {
                cursors.clear();
            }
            cursors.insert(run_id.to_owned(), cursor);
        }
    }

    fn forget_log_cursor(&self, run_id: &str) {
        if let Ok(mut cursors) = self.run_log_cursors.write() {
            cursors.remove(run_id);
        }
    }
    pub async fn run_log_line_count(&self, run_id: &str) -> Result<usize, OrchestratorError> {
        let run_dir: String = sqlx::query_scalar("SELECT run_dir FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_one(self.store.pool())
            .await?;
        let bytes = self
            .read_run_file(&Path::new(&run_dir).join("agent-events.jsonl"))
            .await
            .unwrap_or_default();
        Ok(bytes.iter().filter(|byte| **byte == b'\n').count())
    }
    pub async fn project_settings_get(
        &self,
        project_id: &str,
    ) -> Result<ProjectSettings, OrchestratorError> {
        Ok(self.project(project_id).await?.settings)
    }
    pub async fn project_settings_update(
        &self,
        project_id: &str,
        settings: &ProjectSettings,
    ) -> Result<ProjectSettings, OrchestratorError> {
        validate_project_settings(settings)?;
        sqlx::query("UPDATE projects SET settings_json=?,updated_at=? WHERE id=?")
            .bind(
                serde_json::to_string(settings)
                    .map_err(|e| OrchestratorError::Config(e.to_string()))?,
            )
            .bind(Utc::now().to_rfc3339())
            .bind(project_id)
            .execute(self.store.pool())
            .await?;
        Ok(settings.clone())
    }
    pub async fn settings_get(&self) -> Result<GlobalSettings, OrchestratorError> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key='global'")
                .fetch_optional(self.store.pool())
                .await?;
        Ok(normalize_global_settings(
            value
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default(),
        ))
    }
    pub async fn settings_update(
        &self,
        settings: &GlobalSettings,
    ) -> Result<GlobalSettings, OrchestratorError> {
        let settings = normalize_global_settings(settings.clone());
        validate_global_settings(&settings)?;
        sqlx::query("INSERT INTO settings(key,value_json) VALUES('global',?) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json")
            .bind(serde_json::to_string(&settings).map_err(|e|OrchestratorError::Config(e.to_string()))?)
            .execute(self.store.pool()).await?;
        Ok(settings)
    }
    async fn task(&self, id: &str) -> Result<TaskRow, OrchestratorError> {
        let r = sqlx::query("SELECT * FROM tasks WHERE id=? AND deleted_at IS NULL")
            .bind(id)
            .fetch_one(self.store.pool())
            .await?;
        let policy = self.task_policy(id).await?;
        Ok(TaskRow {
            id: r.get("id"),
            project_id: r.get("project_id"),
            seq: r.get("seq"),
            title: r.get("title"),
            description: r.get("description"),
            status: parse(r.get("status"))?,
            blocked_detail: r.get("blocked_detail"),
            developer: parse(r.get("developer_agent"))?,
            reviewer: parse(r.get("reviewer_agent"))?,
            target_branch: r.get("target_branch"),
            base_commit: r.get("base_commit"),
            branch: r.get("branch"),
            worktree_path: r
                .get::<Option<String>, _>("worktree_path")
                .map(PathBuf::from),
            revision: r.get("current_revision"),
            max_revisions: r.get("max_revisions"),
            api_egress_approved: r
                .get::<Option<String>, _>("api_egress_approved_at")
                .is_some(),
            policy,
        })
    }
    async fn project(&self, id: &str) -> Result<ProjectRow, OrchestratorError> {
        let r = sqlx::query("SELECT * FROM projects WHERE id=?")
            .bind(id)
            .fetch_one(self.store.pool())
            .await?;
        Ok(ProjectRow {
            id: r.get("id"),
            seq: r.get("seq"),
            repo: r.get::<String, _>("repo_path").into(),
            default_branch: r.get("default_branch"),
            worktree_root: r.get::<String, _>("worktree_root").into(),
            settings: serde_json::from_str(&r.get::<String, _>("settings_json"))
                .unwrap_or_default(),
        })
    }
    fn task_dir(&self, task_id: &str) -> PathBuf {
        self.app_data.join("projects/tasks").join(task_id)
    }
    fn run_dir(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id)
            .join("runs")
            .join(Uuid::now_v7().to_string())
    }
}

/// Turns one JSONL line into an event. A line that does not parse becomes a `Raw` event instead
/// of disappearing: the viewer positions live batches by absolute line number, so silently
/// dropping a line would shift every later line and make deduplication impossible — and a
/// Provider's malformed output is itself worth showing during diagnosis.
fn parse_log_line(line: &str) -> AgentEvent {
    serde_json::from_str(line).unwrap_or_else(|_| AgentEvent {
        ts: String::new(),
        stream: EventStream::Stderr,
        kind: AgentEventKind::Raw,
        summary: "无法解析的 Provider 输出".into(),
        text: Some(line.chars().take(2000).collect()),
    })
}
