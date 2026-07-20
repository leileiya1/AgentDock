use super::*;

#[tokio::test]
async fn later_reviews_resolve_fixed_issues_and_keep_recurring_ones_open()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("p", "/tmp/review-lifecycle", "main", "/tmp/wt")
        .await?;
    let created = orchestrator
        .task_create(
            &project.id,
            "review lifecycle",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            None,
        )
        .await?;
    sqlx::query("UPDATE tasks SET current_revision=2 WHERE id=?")
        .bind(&created.id)
        .execute(orchestrator.store.pool())
        .await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,created_at) VALUES('run-r1',?,1,'reviewer','claude_code','SUCCEEDED','/tmp/run-r1',900,300,?)")
        .bind(&created.id)
        .bind(&now)
        .execute(orchestrator.store.pool())
        .await?;
    sqlx::query("INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,summary,raw_path,created_at) VALUES('review-r1',?,1,'run-r1','abcdef1','request_changes','two issues','/tmp/result',?)")
        .bind(&created.id)
        .bind(&now)
        .execute(orchestrator.store.pool())
        .await?;
    for (id, title) in [("fixed", "missing test"), ("recurring", "unsafe write")] {
        sqlx::query("INSERT INTO review_issues(id,review_id,severity,file,title) VALUES(?,'review-r1','high','src/main.rs',?)")
            .bind(id)
            .bind(title)
            .execute(orchestrator.store.pool())
            .await?;
    }

    let task = orchestrator.task(&created.id).await?;
    let recurring = HashSet::from([review_issue_key_parts(Some("src/main.rs"), "unsafe write")]);
    orchestrator
        .reconcile_review_issues(&task, &recurring)
        .await?;

    let fixed: (i64, Option<i64>) =
        sqlx::query_as("SELECT resolved,resolved_by_revision FROM review_issues WHERE id='fixed'")
            .fetch_one(orchestrator.store.pool())
            .await?;
    let still_open: i64 =
        sqlx::query_scalar("SELECT resolved FROM review_issues WHERE id='recurring'")
            .fetch_one(orchestrator.store.pool())
            .await?;
    assert_eq!(fixed, (1, Some(2)));
    assert_eq!(still_open, 0);

    sqlx::query("UPDATE tasks SET current_revision=3 WHERE id=?")
        .bind(&created.id)
        .execute(orchestrator.store.pool())
        .await?;
    let task = orchestrator.task(&created.id).await?;
    orchestrator
        .reconcile_review_issues(&task, &HashSet::new())
        .await?;
    let resolved_by: Option<i64> =
        sqlx::query_scalar("SELECT resolved_by_revision FROM review_issues WHERE id='recurring'")
            .fetch_one(orchestrator.store.pool())
            .await?;
    assert_eq!(resolved_by, Some(3));
    Ok(())
}
