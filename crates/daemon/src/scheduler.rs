async fn scheduler_loop(
    orchestrator: Arc<Orchestrator>,
    shutdown: CancellationToken,
) -> Result<(), DaemonError> {
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut running = JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                // Dropping a workflow future alone can orphan its CLI child. Tell every active
                // run to terminate its process group, then wait for the cleanup paths to finish.
                let interrupted = orchestrator.interrupt_active_runs();
                while let Some(joined) = running.join_next().await {
                    if let Err(error) = joined {
                        tracing::warn!(%error, "scheduler worker stopped during shutdown");
                    }
                }
                orchestrator.requeue_interrupted_tasks(&interrupted).await?;
                for task_id in interrupted {
                    sqlx::query("UPDATE daemon_queue SET state='QUEUED',updated_at=? WHERE task_id=?")
                        .bind(Utc::now().to_rfc3339())
                        .bind(task_id)
                        .execute(orchestrator.store.pool())
                        .await?;
                }
                return Ok(());
            },
            joined = running.join_next(), if !running.is_empty() => {
                if let Some(Err(error)) = joined {
                    tracing::warn!(%error, "scheduler worker panicked");
                }
            },
            _ = tick.tick() => {
                let settings = orchestrator.settings_get().await?;
                if settings.scheduler_paused || !inside_run_window(&settings) || global_budget_exhausted(&orchestrator, &settings).await? {
                    continue;
                }
                let limit = scheduler_limit(&settings);
                while running.len() < limit {
                    let Some(task_id) = claim_next(&orchestrator).await? else { break };
                    let worker = Arc::clone(&orchestrator);
                    running.spawn(async move {
                        let result = worker.drive_task(&task_id).await;
                        if let Err(error) = finish_queue_item(&worker, &task_id, result).await {
                            tracing::error!(task_id, %error, "failed to finish daemon queue item");
                        }
                    });
                }
            }
        }
    }
}

fn scheduler_limit(settings: &GlobalSettings) -> usize {
    settings.max_concurrent_runs.unwrap_or(2).clamp(1, 16) as usize
}

async fn claim_next(orchestrator: &Orchestrator) -> Result<Option<String>, DaemonError> {
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query("SELECT task_id FROM daemon_queue WHERE state='QUEUED' AND paused=0 AND (not_before IS NULL OR not_before<=?) ORDER BY priority DESC,enqueued_at LIMIT 1")
        .bind(&now)
        .fetch_optional(orchestrator.store.pool())
        .await?;
    let Some(row) = row else { return Ok(None) };
    let task_id: String = row.get("task_id");
    let updated = sqlx::query(
        "UPDATE daemon_queue SET state='RUNNING',updated_at=? WHERE task_id=? AND state='QUEUED'",
    )
    .bind(&now)
    .bind(&task_id)
    .execute(orchestrator.store.pool())
    .await?;
    Ok((updated.rows_affected() == 1).then_some(task_id))
}

fn inside_run_window(settings: &GlobalSettings) -> bool {
    let (Some(start), Some(end)) = (&settings.run_window_start, &settings.run_window_end) else {
        return true;
    };
    let Ok(start) = chrono::NaiveTime::parse_from_str(start, "%H:%M") else {
        return false;
    };
    let Ok(end) = chrono::NaiveTime::parse_from_str(end, "%H:%M") else {
        return false;
    };
    let now = chrono::Local::now().time();
    if start < end {
        (start..end).contains(&now)
    } else {
        now >= start || now < end
    }
}

async fn global_budget_exhausted(
    orchestrator: &Orchestrator,
    settings: &GlobalSettings,
) -> Result<bool, sqlx::Error> {
    let Some(limit) = settings.global_daily_cost_usd else {
        return Ok(false);
    };
    let day = Utc::now()
        .date_naive()
        .format("%Y-%m-%dT00:00:00Z")
        .to_string();
    let used: f64 = sqlx::query_scalar("SELECT COALESCE(SUM(COALESCE(cost_usd,reserved_cost_usd,0)),0) FROM agent_runs WHERE created_at>=? AND status IN ('RUNNING','SUCCEEDED','FAILED','TIMED_OUT')")
        .bind(day)
        .fetch_one(orchestrator.store.pool())
        .await?;
    Ok(used >= limit)
}

async fn finish_queue_item(
    orchestrator: &Orchestrator,
    task_id: &str,
    result: Result<TaskSummary, agentflow_orchestrator::OrchestratorError>,
) -> Result<(), DaemonError> {
    let now = Utc::now();
    match result {
        Ok(summary) => {
            sqlx::query("UPDATE daemon_queue SET state='COMPLETED',last_error=NULL,updated_at=? WHERE task_id=?")
                .bind(now.to_rfc3339())
                .bind(task_id)
                .execute(orchestrator.store.pool())
                .await?;
            tracing::info!(task_id, status=%summary.status, "daemon task drive completed");
            orchestrator.notify_task(&summary).await;
        }
        Err(error) => {
            let attempts: i64 =
                sqlx::query_scalar("SELECT attempts + 1 FROM daemon_queue WHERE task_id=?")
                    .bind(task_id)
                    .fetch_one(orchestrator.store.pool())
                    .await?;
            let delay = 2_i64.saturating_pow(attempts.min(6) as u32);
            let next = now + ChronoDuration::seconds(delay);
            let state = if attempts >= 5 { "FAILED" } else { "QUEUED" };
            sqlx::query("UPDATE daemon_queue SET state=?,attempts=?,not_before=?,last_error=?,updated_at=? WHERE task_id=?")
                .bind(state)
                .bind(attempts)
                .bind(next.to_rfc3339())
                .bind(error.to_string())
                .bind(now.to_rfc3339())
                .bind(task_id)
                .execute(orchestrator.store.pool())
                .await?;
            if state == "FAILED" {
                orchestrator
                    .notify_daemon_failure(task_id, &error.to_string())
                    .await;
            }
        }
    }
    Ok(())
}

async fn queue_depth(orchestrator: &Orchestrator) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM daemon_queue WHERE state IN ('QUEUED','RUNNING')")
        .fetch_one(orchestrator.store.pool())
        .await
}

fn is_runnable(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Planning
            | TaskStatus::ReadyForDevelopment
            | TaskStatus::ReadyForRevision
            | TaskStatus::Validating
            | TaskStatus::ReadyForReview
    )
}
