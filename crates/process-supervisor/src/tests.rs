use super::*;

#[cfg(unix)]
#[tokio::test]
async fn cancellation_terminates_the_leased_process_group() -> Result<(), Box<dyn std::error::Error>>
{
    let root = tempfile::tempdir()?;
    let lease_path = root.path().join("process-lease.json");
    let child_pid_path = root.path().join("child.pid");
    let mut env = HashMap::new();
    env.insert(
        "CHILD_PID_FILE".into(),
        child_pid_path.to_string_lossy().into_owned(),
    );
    let spec = ProcessSpec {
        program: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            "sleep 30 & echo $! > \"$CHILD_PID_FILE\"; wait".into(),
        ],
        cwd: root.path().into(),
        env,
        env_denylist: Vec::new(),
        timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        stdout_path: root.path().join("stdout.log"),
        stderr_path: root.path().join("stderr.log"),
        lease_path: lease_path.clone(),
    };
    let cancellation = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(8);
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let run = tokio::spawn(super::run(spec, cancellation.clone(), tx));
    for _ in 0..100 {
        if lease_path.exists() && child_pid_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let lease = read_process_lease(&lease_path).await?;
    assert_eq!(inspect_process_lease(&lease), LeaseState::Alive);
    cancellation.cancel();
    let outcome = run.await??;
    drain.await?;
    assert!(outcome.cancelled);
    assert_eq!(inspect_process_lease(&lease), LeaseState::Exited);

    let child_pid: u32 = tokio::fs::read_to_string(child_pid_path)
        .await?
        .trim()
        .parse()?;
    let system = sysinfo::System::new_all();
    let child_is_live = system
        .process(sysinfo::Pid::from_u32(child_pid))
        .is_some_and(|process| {
            !matches!(
                process.status(),
                sysinfo::ProcessStatus::Dead | sysinfo::ProcessStatus::Zombie
            )
        });
    assert!(
        !child_is_live,
        "grandchild survived process-group cancellation"
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn provider_survives_supervisor_crash_and_records_exit_and_logs()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let lease_path = root.path().join("process-lease.json");
    let outcome_path = root.path().join("process-outcome.json");
    let spec = ProcessSpec {
        program: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            "printf 'before-crash\\n'; sleep 0.25; printf 'after-crash\\n'".into(),
        ],
        cwd: root.path().into(),
        env: HashMap::new(),
        env_denylist: Vec::new(),
        timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(10),
        stdout_path: root.path().join("stdout.log"),
        stderr_path: root.path().join("stderr.log"),
        lease_path: lease_path.clone(),
    };
    let (tx, mut rx) = mpsc::channel(8);
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let supervisor = tokio::spawn(run(spec, CancellationToken::new(), tx));
    for _ in 0..100 {
        if lease_path.exists()
            && tokio::fs::read_to_string(root.path().join("stdout.log"))
                .await
                .unwrap_or_default()
                .contains("before-crash")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let lease = read_process_lease(&lease_path).await?;
    supervisor.abort();
    let _ = supervisor.await;

    for _ in 0..100 {
        if outcome_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(read_process_exit_code(&outcome_path).await?, 0);
    assert_eq!(inspect_process_lease(&lease), LeaseState::Exited);
    let output = tokio::fs::read_to_string(root.path().join("stdout.log")).await?;
    assert!(output.contains("before-crash"));
    assert!(output.contains("after-crash"));
    drop(drain);
    Ok(())
}

#[test]
fn codex_events_become_short_process_summaries() {
    let (kind, summary) = classify_and_summarize(
        r#"{"type":"item.completed","item":{"type":"agent_message","text":"实现完成，正在运行测试。"}}"#,
    );
    assert_eq!(kind, AgentEventKind::AssistantText);
    assert_eq!(summary, "实现完成，正在运行测试。");

    let (kind, summary) = classify_and_summarize(
        r#"{"type":"item.completed","item":{"type":"agent_message","text":"{\"schema_version\":1,\"summary\":\"已完成改动并通过测试。\"}"}}"#,
    );
    assert_eq!(kind, AgentEventKind::Result);
    assert_eq!(summary, "已完成改动并通过测试。");

    let (kind, summary) = classify_and_summarize(
        r#"{"type":"item.completed","item":{"type":"file_change","changes":[{"path":"/tmp/src/main.rs"},{"path":"/tmp/README.md"}],"status":"completed"}}"#,
    );
    assert_eq!(kind, AgentEventKind::ToolUse);
    assert_eq!(summary, "已修改：main.rs、README.md");
}

#[test]
fn claude_events_hide_transport_json() {
    let (kind, summary) = classify_and_summarize(
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/tmp/review-input.md"}}]}}"#,
    );
    assert_eq!(kind, AgentEventKind::ToolUse);
    assert_eq!(summary, "调用 Read：/tmp/review-input.md");

    let (kind, summary) = classify_and_summarize(
        r#"{"type":"result","result":"{\"summary\":\"审查通过，没有阻断问题。\"}"}"#,
    );
    assert_eq!(kind, AgentEventKind::Result);
    assert_eq!(summary, "审查通过，没有阻断问题。");
}

#[test]
fn secrets_are_redacted() {
    let s =
        redact("Authorization: Bearer abc password=hunter2 ghp_abcdefghijklmnopqrstuvwxyz".into());
    assert!(!s.contains("hunter2"));
    assert!(!s.contains("abcdefghijklmnopqrstuvwxyz"));
}
