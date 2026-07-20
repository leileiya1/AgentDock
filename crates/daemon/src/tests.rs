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
