#[cfg(test)]
mod recovery_isolation_tests {
    use super::*;

    async fn git(repo: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .await?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    }

    async fn init_repo(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let repo = root.join("repo");
        tokio::fs::create_dir_all(&repo).await?;
        git(&repo, &["init", "-q"]).await?;
        git(&repo, &["branch", "-M", "main"]).await?;
        git(&repo, &["config", "user.name", "AgentFlow Test"]).await?;
        git(&repo, &["config", "user.email", "test@agentflow.local"]).await?;
        tokio::fs::write(repo.join("shared.txt"), "base\n").await?;
        git(&repo, &["add", "shared.txt"]).await?;
        git(&repo, &["commit", "-q", "-m", "base"]).await?;
        Ok(repo)
    }

    async fn started_task(
        orchestrator: &Orchestrator,
        project_id: &str,
        title: &str,
    ) -> Result<TaskSummary, Box<dyn std::error::Error>> {
        let task = orchestrator
            .task_create(
                project_id,
                title,
                "exercise recovery isolation",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                None,
                Some(2),
            )
            .await?;
        orchestrator.task_start(&task.id).await?;
        Ok(task)
    }

    #[tokio::test]
    async fn one_corrupt_task_is_quarantined_without_bricking_startup_or_other_tasks()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(&data).await?;
        let project = orchestrator.project_import(&repo).await?;
        let broken = started_task(&orchestrator, &project.id, "broken task").await?;
        let healthy = started_task(&orchestrator, &project.id, "healthy task").await?;

        // Fault injection for the broken task: a durable revision row whose commit does not
        // match the worktree HEAD makes recover_orphaned_stage fail deterministically.
        let broken_row = orchestrator.task(&broken.id).await?;
        orchestrator
            .enter_development_stage(
                &broken_row,
                TaskStatus::ReadyForDevelopment,
                TaskStatus::Developing,
                1,
            )
            .await?;
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,created_at) VALUES(?,?,1,?,?)")
            .bind(Uuid::now_v7().to_string())
            .bind(&broken.id)
            .bind("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef")
            .bind(Utc::now().to_rfc3339())
            .execute(orchestrator.store.pool())
            .await?;
        // The healthy task is a plain orphaned development stage that recovery must still fix.
        let healthy_row = orchestrator.task(&healthy.id).await?;
        orchestrator
            .enter_development_stage(
                &healthy_row,
                TaskStatus::ReadyForDevelopment,
                TaskStatus::Developing,
                1,
            )
            .await?;
        drop(orchestrator);

        let recovered = Orchestrator::open(&data).await?;
        let broken_detail = recovered.task_get(&broken.id).await?;
        assert_eq!(broken_detail.summary.status, TaskStatus::Blocked);
        assert_eq!(
            broken_detail.summary.blocked_reason,
            Some(BlockedReason::RecoveryFailed)
        );
        let resume_status: Option<String> =
            sqlx::query_scalar("SELECT repair_resume_status FROM tasks WHERE id=?")
                .bind(&broken.id)
                .fetch_one(recovered.store.pool())
                .await?;
        assert_eq!(resume_status.as_deref(), Some("DEVELOPING"));
        assert!(
            recovered
                .events_list(&broken.id, None, None)
                .await?
                .iter()
                .any(|event| event.event_type == "recovery:task_failed")
        );
        let healthy_detail = recovered.task_get(&healthy.id).await?;
        assert_eq!(
            healthy_detail.summary.status,
            TaskStatus::ReadyForDevelopment
        );
        Ok(())
    }

    #[tokio::test]
    async fn corrupt_start_saga_payload_is_quarantined_instead_of_failing_open()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let data = root.path().join("data");
        let repo = init_repo(root.path()).await?;
        let orchestrator = Orchestrator::open(&data).await?;
        let project = orchestrator.project_import(&repo).await?;
        let task = started_task(&orchestrator, &project.id, "saga payload").await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO task_operations(id,task_id,kind,phase,status,payload_json,created_at,updated_at) VALUES(?,?,'task_start','intent','RUNNING','not json',?,?)")
            .bind(Uuid::now_v7().to_string())
            .bind(&task.id)
            .bind(&now)
            .bind(&now)
            .execute(orchestrator.store.pool())
            .await?;
        drop(orchestrator);

        let recovered = Orchestrator::open(&data).await?;
        let detail = recovered.task_get(&task.id).await?;
        assert_eq!(detail.summary.status, TaskStatus::Blocked);
        assert_eq!(
            detail.summary.blocked_reason,
            Some(BlockedReason::RecoveryFailed)
        );
        assert!(
            recovered
                .events_list(&task.id, None, None)
                .await?
                .iter()
                .any(|event| event.event_type == "recovery:task_failed")
        );
        Ok(())
    }
}

#[cfg(test)]
mod run_log_tail_tests {
    use super::*;

    async fn seed_run(
        orchestrator: &Orchestrator,
        run_dir: &Path,
        lines: usize,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let project = orchestrator
            .store
            .import_project("logs", "/tmp/log-tail", "main", "/tmp/log-tail-wt")
            .await?;
        let task = orchestrator
            .task_create(
                &project.id,
                "log tail",
                "exercise incremental tailing",
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                None,
                None,
            )
            .await?;
        tokio::fs::create_dir_all(run_dir).await?;
        let mut text = String::new();
        for index in 0..lines {
            text.push_str(&format!(
                "{}\n",
                json!({
                    "ts": "2026-07-27T00:00:00Z",
                    "stream": "stdout",
                    "kind": "assistant_text",
                    "summary": format!("line {index}"),
                    "text": null
                })
            ));
        }
        tokio::fs::write(run_dir.join("agent-events.jsonl"), &text).await?;
        let run_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES(?,?,1,'developer','claude_code','RUNNING',?,900,300,?)")
            .bind(&run_id)
            .bind(&task.id)
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(&now)
            .execute(orchestrator.store.pool())
            .await?;
        Ok(run_id)
    }

    #[tokio::test]
    async fn incremental_tailing_matches_a_full_read_and_follows_appends()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let run_dir = dir.path().join("run");
        let run_id = seed_run(&orchestrator, &run_dir, 5).await?;

        // Cold read seeds the cursor.
        let (first, next, at_end) = orchestrator.run_log_tail(&run_id, 0, 3).await?;
        assert_eq!(first.len(), 3);
        assert_eq!(next, 3);
        assert!(!at_end);
        assert_eq!(first[0].summary, "line 0");

        // Warm read resumes from the cached byte offset and must agree with the cold path.
        let (second, next, at_end) = orchestrator.run_log_tail(&run_id, next, 10).await?;
        assert_eq!(second.len(), 2);
        assert_eq!(next, 5);
        assert!(at_end);
        assert_eq!(second[1].summary, "line 4");

        // Nothing new yet.
        let (empty, still, at_end) = orchestrator.run_log_tail(&run_id, next, 10).await?;
        assert!(empty.is_empty());
        assert_eq!(still, 5);
        assert!(at_end);

        // Appended output is picked up without re-reading earlier lines.
        let appended = format!(
            "{}\n",
            json!({
                "ts": "2026-07-27T00:00:01Z",
                "stream": "stdout",
                "kind": "assistant_text",
                "summary": "line 5",
                "text": null
            })
        );
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(run_dir.join("agent-events.jsonl"))
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, appended.as_bytes()).await?;
        drop(file);
        let (third, next, at_end) = orchestrator.run_log_tail(&run_id, next, 10).await?;
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].summary, "line 5");
        assert_eq!(next, 6);
        assert!(at_end);
        Ok(())
    }

    #[tokio::test]
    async fn a_partially_written_trailing_line_is_not_consumed()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(dir.path()).await?;
        let run_dir = dir.path().join("run");
        let run_id = seed_run(&orchestrator, &run_dir, 2).await?;
        let (_, next, _) = orchestrator.run_log_tail(&run_id, 0, 10).await?;
        assert_eq!(next, 2);

        // A half-flushed line must not advance the cursor past itself.
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(run_dir.join("agent-events.jsonl"))
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, b"{\"ts\":\"2026").await?;
        drop(file);
        let (events, still, _) = orchestrator.run_log_tail(&run_id, next, 10).await?;
        assert!(events.is_empty());
        assert_eq!(still, 2, "a partial line must not be consumed");
        Ok(())
    }
}
