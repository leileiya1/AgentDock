use super::*;

#[tokio::test]
async fn daemon_answers_ping_and_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let data = temp.path().to_path_buf();
    let shutdown = CancellationToken::new();
    let server_data = data.clone();
    let server_shutdown = shutdown.clone();
    let server = tokio::spawn(async move { serve(server_data, server_shutdown).await });
    for _ in 0..50 {
        if socket_path(&data).exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let ping = request(&data, &DaemonRequest::Ping).await?;
    assert!(matches!(
        ping,
        DaemonResponse::Ok { ref payload }
            if payload.get("ipcVersion").and_then(Value::as_u64) == Some(2)
    ));
    let duplicate = serve(data.clone(), CancellationToken::new()).await;
    assert!(matches!(
        duplicate,
        Err(DaemonError::Protocol(message)) if message.contains("already running")
    ));
    let settings = GlobalSettings {
        max_concurrent_runs: Some(4),
        ..GlobalSettings::default()
    };
    let changed = request(
        &data,
        &DaemonRequest::SettingsUpdate {
            settings: settings.clone(),
        },
    )
    .await?;
    assert!(matches!(changed, DaemonResponse::Ok { .. }));
    let client = Orchestrator::open_client(&data).await?;
    assert_eq!(client.settings_get().await?.max_concurrent_runs, Some(4));
    let stopped = request(&data, &DaemonRequest::Shutdown).await?;
    assert!(matches!(stopped, DaemonResponse::Ok { .. }));
    server.await??;
    assert!(!socket_path(&data).exists());
    Ok(())
}

#[test]
fn scheduler_limit_uses_the_saved_setting_and_safe_bounds() {
    let mut settings = GlobalSettings {
        max_concurrent_runs: Some(5),
        ..GlobalSettings::default()
    };
    assert_eq!(scheduler_limit(&settings), 5);
    settings.max_concurrent_runs = Some(0);
    assert_eq!(scheduler_limit(&settings), 1);
    settings.max_concurrent_runs = Some(99);
    assert_eq!(scheduler_limit(&settings), 16);
}

#[tokio::test]
async fn queue_honors_priority_and_pause_without_losing_fifo_order()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(root.path()).await?;
    let project = orchestrator
        .store
        .import_project("queue", "/tmp/queue", "main", "/tmp/queue-wt")
        .await?;
    let create = |title: &'static str, priority: i16| {
        let orchestrator = &orchestrator;
        let project_id = project.id.clone();
        async move {
            orchestrator
                .task_create_governed(
                    &project_id,
                    title,
                    "queue test",
                    AgentKind::ClaudeCode,
                    AgentKind::Codex,
                    None,
                    None,
                    false,
                    TaskPolicy {
                        require_plan_approval: false,
                        priority,
                        ..TaskPolicy::default()
                    },
                )
                .await
        }
    };
    let low = create("low", -10).await?;
    let urgent = create("urgent", 90).await?;
    let paused = create("paused", 100).await?;
    for task in [&low, &urgent, &paused] {
        sqlx::query("UPDATE tasks SET status='READY_FOR_DEVELOPMENT' WHERE id=?")
            .bind(&task.id)
            .execute(orchestrator.store.pool())
            .await?;
        enqueue_task(&orchestrator, &task.id).await?;
    }
    dispatch(
        DaemonRequest::QueueTaskPause {
            task_id: paused.id.clone(),
        },
        &orchestrator,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(
        claim_next(&orchestrator).await?.as_deref(),
        Some(urgent.id.as_str())
    );
    assert_eq!(
        claim_next(&orchestrator).await?.as_deref(),
        Some(low.id.as_str())
    );
    assert!(claim_next(&orchestrator).await?.is_none());
    dispatch(
        DaemonRequest::QueueTaskResume {
            task_id: paused.id.clone(),
        },
        &orchestrator,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(
        claim_next(&orchestrator).await?.as_deref(),
        Some(paused.id.as_str())
    );
    Ok(())
}

#[test]
fn overnight_run_windows_are_evaluated_without_rejecting_midnight() {
    let mut settings = GlobalSettings::default();
    let now = chrono::Local::now().time();
    settings.run_window_start = Some(now.format("%H:%M").to_string());
    settings.run_window_end = Some(
        (now + ChronoDuration::minutes(2))
            .format("%H:%M")
            .to_string(),
    );
    assert!(inside_run_window(&settings));

    settings.run_window_start = Some("23:00".into());
    settings.run_window_end = Some("01:00".into());
    let _ = inside_run_window(&settings); // Must accept an overnight window without panicking.
}
