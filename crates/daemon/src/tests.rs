use super::*;

#[tokio::test]
async fn daemon_answers_ping_and_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let data = temp.path().to_path_buf();
    // The daemon crate sees persistence as a release dependency even in this
    // integration test, so seed its explicit file-key fallback and never touch
    // the developer's login Keychain.
    std::fs::write(data.join("local-data.key"), [7_u8; 32])?;
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
async fn permission_approval_is_daemon_owned_and_requeues_checkpoint()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("local-data.key"), [11_u8; 32])?;
    let repo = root.path().join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    let orchestrator = Orchestrator::open(root.path()).await?;
    let project = orchestrator
        .store
        .import_project(
            "permission",
            &repo.to_string_lossy(),
            "main",
            &root.path().join("wt").to_string_lossy(),
        )
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "permission",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(2),
        )
        .await?;
    sqlx::query(
        "UPDATE tasks SET status='DEVELOPING',current_revision=1,worktree_path=? WHERE id=?",
    )
    .bind(repo.to_string_lossy().as_ref())
    .bind(&task.id)
    .execute(orchestrator.store.pool())
    .await?;
    let permission = orchestrator
        .permission_request(agentflow_contracts::PermissionRequestInput {
            task_id: task.id.clone(),
            run_id: Some("run-probe".into()),
            provider_id: AgentKind::Codex,
            role: agentflow_contracts::RunRole::Developer,
            action_type: agentflow_contracts::PermissionActionType::NetworkAccess,
            reason: "test daemon ownership".into(),
            operation: agentflow_contracts::PermissionOperation {
                argv: Vec::new(),
                cwd: repo.to_string_lossy().into_owned(),
                paths: Vec::new(),
                network_domains: vec!["example.test:443".into()],
                environment_names: Vec::new(),
                attributes: Default::default(),
            },
            provider_resume_token: None,
        })
        .await?;
    let response = dispatch(
        DaemonRequest::PermissionDecide {
            input: agentflow_contracts::PermissionDecisionInput {
                request_id: permission.id,
                operation_sha256: permission.operation_sha256,
                policy_sha256: permission.policy_sha256,
                decision: agentflow_contracts::PermissionDecisionKind::Approve,
                scope: agentflow_contracts::PermissionGrantScope::Once,
                guidance: None,
            },
        },
        &orchestrator,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(
        response.get("decision").and_then(Value::as_str),
        Some("approve")
    );
    let queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM daemon_queue WHERE task_id=? AND state='QUEUED'")
            .bind(&task.id)
            .fetch_one(orchestrator.store.pool())
            .await?;
    assert_eq!(queued, 1);
    Ok(())
}

#[tokio::test]
async fn queue_honors_priority_and_pause_without_losing_fifo_order()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("local-data.key"), [9_u8; 32])?;
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

#[tokio::test]
async fn queue_state_survives_daemon_restart_and_rejects_missing_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let data = root.path().to_path_buf();
    std::fs::write(data.join("local-data.key"), [11_u8; 32])?;

    let orchestrator = Orchestrator::open(&data).await?;
    let project = orchestrator
        .store
        .import_project(
            "restart queue",
            "/tmp/restart-queue",
            "main",
            "/tmp/restart-queue-wt",
        )
        .await?;
    let task = orchestrator
        .task_create_governed(
            &project.id,
            "persist queue state",
            "daemon restart test",
            AgentKind::ClaudeCode,
            AgentKind::Codex,
            None,
            None,
            false,
            TaskPolicy {
                require_plan_approval: false,
                ..TaskPolicy::default()
            },
        )
        .await?;
    sqlx::query("UPDATE tasks SET status='READY_FOR_DEVELOPMENT' WHERE id=?")
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    enqueue_task(&orchestrator, &task.id).await?;
    let settings = GlobalSettings {
        scheduler_paused: true,
        ..GlobalSettings::default()
    };
    orchestrator.settings_update(&settings).await?;
    drop(orchestrator);

    async fn start_daemon(
        data: &Path,
    ) -> (
        CancellationToken,
        tokio::task::JoinHandle<Result<(), DaemonError>>,
    ) {
        let shutdown = CancellationToken::new();
        let server_data = data.to_path_buf();
        let server_shutdown = shutdown.clone();
        let server = tokio::spawn(async move { serve(server_data, server_shutdown).await });
        for _ in 0..50 {
            if socket_path(data).exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        (shutdown, server)
    }

    fn require_ok(response: DaemonResponse, operation: &str) -> Value {
        match response {
            DaemonResponse::Ok { payload } => payload,
            DaemonResponse::Error { message } => panic!("{operation} failed: {message}"),
        }
    }

    let (_shutdown, server) = start_daemon(&data).await;
    require_ok(
        request(
            &data,
            &DaemonRequest::QueueTaskPriority {
                task_id: task.id.clone(),
                priority: 50,
            },
        )
        .await?,
        "change priority",
    );
    require_ok(
        request(
            &data,
            &DaemonRequest::QueueTaskPause {
                task_id: task.id.clone(),
            },
        )
        .await?,
        "pause queue",
    );
    let before = require_ok(
        request(
            &data,
            &DaemonRequest::QueueTaskStatus {
                task_id: task.id.clone(),
            },
        )
        .await?,
        "read queue status",
    );
    let before: QueueTaskState = serde_json::from_value(before)?;
    assert!(before.paused);
    assert_eq!(before.priority, 50);
    assert_eq!(before.position, None);
    assert_eq!(before.waiting_reason, Some(QueueWaitingReason::Paused));

    request(&data, &DaemonRequest::Shutdown).await?;
    server.await??;

    let (_shutdown, restarted) = start_daemon(&data).await;
    let after = require_ok(
        request(
            &data,
            &DaemonRequest::QueueTaskStatus {
                task_id: task.id.clone(),
            },
        )
        .await?,
        "read queue status after restart",
    );
    let after: QueueTaskState = serde_json::from_value(after)?;
    assert!(after.paused);
    assert_eq!(after.priority, 50);
    assert_eq!(after.waiting_reason, Some(QueueWaitingReason::Paused));
    let client = Orchestrator::open_client(&data).await?;
    assert_eq!(client.task_get(&task.id).await?.policy.priority, 50);
    drop(client);

    let missing = request(
        &data,
        &DaemonRequest::QueueTaskPriority {
            task_id: "missing-task".into(),
            priority: 50,
        },
    )
    .await?;
    assert!(matches!(
        missing,
        DaemonResponse::Error { message } if message.contains("not queued")
    ));
    request(&data, &DaemonRequest::Shutdown).await?;
    restarted.await??;
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
