const HISTORY_MAX_BYTES: usize = 8 * 1024;

#[derive(Debug, Serialize)]
struct HistorySnapshot {
    schema_version: u8,
    task_id: String,
    next_revision: i64,
    revisions: Vec<RevisionMemory>,
}

#[derive(Debug, Serialize)]
struct RevisionMemory {
    revision: i64,
    commit_sha: Option<String>,
    developer_summary: Option<String>,
    validation_summary: Option<String>,
    review: Option<ReviewMemory>,
    human_guidance: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReviewMemory {
    decision: String,
    summary: Option<String>,
    issues: Vec<IssueMemory>,
}

#[derive(Debug, Serialize)]
struct IssueMemory {
    severity: String,
    file: Option<String>,
    line_start: Option<i64>,
    title: String,
    detail: String,
    suggested_action: Option<String>,
}

impl Orchestrator {
    /// Persist AgentFlow-owned cross-revision context instead of relying on a Provider's
    /// private chat memory. The digest is deterministic, reviewable and safe to pass to any CLI.
    async fn write_history(
        &self,
        task: &TaskRow,
        wt: &Path,
    ) -> Result<Option<String>, OrchestratorError> {
        if task.revision <= 1 {
            return Ok(None);
        }
        let snapshot = self.build_history_snapshot(task).await?;
        let digest = render_history_digest(&snapshot);
        let input_dir = wt.join(".agentflow-in");
        write_atomic(&input_dir.join("history.md"), digest.as_bytes()).await?;
        let json = serde_json::to_vec_pretty(&snapshot)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        write_atomic(&input_dir.join("history.json"), &json).await?;
        Ok(Some(digest))
    }

    async fn build_history_snapshot(
        &self,
        task: &TaskRow,
    ) -> Result<HistorySnapshot, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT revision,commit_sha FROM task_revisions \
             WHERE task_id=? AND revision<? ORDER BY revision",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_all(self.store.pool())
        .await?;
        let mut revisions = Vec::with_capacity(rows.len());
        for row in rows {
            let revision: i64 = row.get("revision");
            let commit: Option<String> = row.get("commit_sha");
            revisions.push(
                self.revision_memory(task, revision, commit)
                    .await?,
            );
        }
        Ok(HistorySnapshot {
            schema_version: 1,
            task_id: task.id.clone(),
            next_revision: task.revision,
            revisions,
        })
    }

    async fn revision_memory(
        &self,
        task: &TaskRow,
        revision: i64,
        commit: Option<String>,
    ) -> Result<RevisionMemory, OrchestratorError> {
        Ok(RevisionMemory {
            revision,
            commit_sha: commit,
            developer_summary: self.developer_summary(&task.id, revision).await?,
            validation_summary: self.test_summary(&task.id, revision).await,
            review: self.review_memory(&task.id, revision).await?,
            human_guidance: self.human_guidance(&task.id, revision).await?,
        })
    }

    async fn developer_summary(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Option<String>, OrchestratorError> {
        let run_dir = sqlx::query_scalar::<_, String>(
            "SELECT run_dir FROM agent_runs WHERE task_id=? AND revision=? \
             AND role='developer' AND status='SUCCEEDED' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_optional(self.store.pool())
        .await?;
        let Some(run_dir) = run_dir else {
            return Ok(None);
        };
        let result = tokio::fs::read_to_string(Path::new(&run_dir).join("result.json"))
            .await
            .ok()
            .and_then(|text| serde_json::from_str::<DevelopmentResult>(&text).ok());
        Ok(result.map(|value| value.summary))
    }

    async fn test_summary(&self, task_id: &str, revision: i64) -> Option<String> {
        let path = self
            .task_dir(task_id)
            .join("artifacts")
            .join(format!("r{revision}-tests.json"));
        let value: Value = serde_json::from_slice(&tokio::fs::read(path).await.ok()?).ok()?;
        if value
            .get("steps")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            return Some("未配置自动验证（未执行）".into());
        }
        if value.get("passed").and_then(Value::as_bool) == Some(true) {
            return Some("通过".into());
        }
        let failures = value
            .get("steps")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|step| step.get("exit_code").and_then(Value::as_i64) != Some(0))
            .map(|step| {
                let name = step.get("name").and_then(Value::as_str).unwrap_or("未命名步骤");
                let exit = step
                    .get("exit_code")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "超时".into());
                let detail = step
                    .get("stderr_tail")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .or_else(|| step.get("stdout_tail").and_then(Value::as_str))
                    .unwrap_or("");
                format!("{name} (exit {exit}): {}", compact_text(detail, 700))
            })
            .collect::<Vec<_>>();
        Some(if failures.is_empty() {
            "未通过（没有可用的失败详情）".into()
        } else {
            failures.join("；")
        })
    }

    async fn review_memory(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Option<ReviewMemory>, OrchestratorError> {
        let review = sqlx::query(
            "SELECT id,decision,summary FROM reviews WHERE task_id=? AND revision=? \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_optional(self.store.pool())
        .await?;
        let Some(review) = review else {
            return Ok(None);
        };
        let review_id: String = review.get("id");
        let decision: String = review.get("decision");
        let summary: Option<String> = review.get("summary");
        let issues = sqlx::query(
            "SELECT severity,file,line_start,title,description,suggested_action \
             FROM review_issues WHERE review_id=? AND resolved=0 ORDER BY rowid",
        )
        .bind(review_id)
        .fetch_all(self.store.pool())
        .await?;
        let issues = issues
            .into_iter()
            .map(|issue| IssueMemory {
                severity: issue.get("severity"),
                file: issue.get("file"),
                line_start: issue.get("line_start"),
                title: issue.get("title"),
                detail: issue.get::<Option<String>, _>("description").unwrap_or_default(),
                suggested_action: issue.get("suggested_action"),
            })
            .collect();
        Ok(Some(ReviewMemory {
            decision,
            summary,
            issues,
        }))
    }

    async fn human_guidance(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<Vec<String>, OrchestratorError> {
        let rows = sqlx::query(
            "SELECT event_type,payload_json FROM events WHERE task_id=? AND revision=? \
             AND event_type IN ('human:reject','human:resume_with_guidance') ORDER BY id",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_all(self.store.pool())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let event_type: String = row.get("event_type");
                let payload: String = row.get("payload_json");
                let value: Value = serde_json::from_str(&payload).ok()?;
                let key = if event_type == "human:reject" {
                    "reason"
                } else {
                    "guidance"
                };
                value.get(key).and_then(Value::as_str).map(str::to_string)
            })
            .collect())
    }
}

async fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
    let temporary = path.with_extension("tmp");
    tokio::fs::write(&temporary, contents).await?;
    tokio::fs::rename(temporary, path).await
}

fn render_history_digest(snapshot: &HistorySnapshot) -> String {
    let sections = snapshot
        .revisions
        .iter()
        .map(render_revision)
        .collect::<Vec<_>>();
    bound_history_sections(sections)
}

fn render_revision(memory: &RevisionMemory) -> String {
    let mut out = format!(
        "## r{}\n\n- 提交：{}\n",
        memory.revision,
        memory.commit_sha.as_deref().unwrap_or("未生成")
    );
    if let Some(summary) = &memory.developer_summary {
        out.push_str(&format!("- 开发摘要：{}\n", compact_text(summary, 900)));
    }
    if let Some(summary) = &memory.validation_summary {
        out.push_str(&format!("- 验证：{}\n", compact_text(summary, 1_200)));
    }
    if let Some(review) = &memory.review {
        out.push_str(&format!(
            "- 审查结论：{}；{}\n",
            review.decision,
            compact_text(review.summary.as_deref().unwrap_or("无摘要"), 900)
        ));
        for issue in &review.issues {
            let location = match (&issue.file, issue.line_start) {
                (Some(file), Some(line)) => format!(" {file}:{line}"),
                (Some(file), None) => format!(" {file}"),
                _ => String::new(),
            };
            out.push_str(&format!(
                "  - [{}]{} {}：{}\n",
                issue.severity,
                location,
                issue.title,
                compact_text(&issue.detail, 450)
            ));
            if let Some(action) = &issue.suggested_action {
                out.push_str(&format!("    建议：{}\n", compact_text(action, 450)));
            }
        }
    }
    for guidance in &memory.human_guidance {
        out.push_str(&format!("- 人工反馈：{}\n", compact_text(guidance, 900)));
    }
    out.push('\n');
    out
}

fn compact_text(text: &str, max_bytes: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_utf8(&compact, max_bytes, "…")
}

fn bound_history_sections(mut sections: Vec<String>) -> String {
    const HEADER: &str = "# AgentFlow 跨轮历史\n\n这是编排器从结构化结果、测试、审查和人工反馈生成的事实摘要；以当前 worktree 和提交为准。\n\n";
    while sections.len() > 1
        && HEADER.len() + sections.iter().map(String::len).sum::<usize>() > HISTORY_MAX_BYTES
    {
        sections.remove(0);
    }
    truncate_utf8(
        &(HEADER.to_owned() + &sections.concat()),
        HISTORY_MAX_BYTES,
        "\n\n[历史摘要已按 8KB 上限截断]\n",
    )
}

fn truncate_utf8(text: &str, max_bytes: usize, marker: &str) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes.saturating_sub(marker.len()).min(text.len());
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}{}", &text[..end], marker)
}
