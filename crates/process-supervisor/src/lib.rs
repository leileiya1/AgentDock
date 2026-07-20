use agentflow_contracts::{AgentEvent, AgentEventKind, EventStream};
use chrono::Utc;
use regex::Regex;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{Mutex, mpsc},
};
use tokio_util::sync::CancellationToken;

mod lease;
pub use lease::{
    LeaseState, ProcessLease, inspect_process_lease, read_process_lease, terminate_process_lease,
};

const MAX_LOG_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub env_denylist: Vec<String>,
    pub timeout: Duration,
    pub idle_timeout: Duration,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    /// Written immediately after spawn so a restarted daemon can identify and clean up orphans.
    pub lease_path: PathBuf,
}
#[derive(Debug, Clone)]
pub struct ProcessOutcome {
    pub pid: u32,
    pub started_at: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub log_truncated: bool,
}
#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("process I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("process had no {0} pipe")]
    MissingPipe(&'static str),
    #[error("process wait task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub async fn run(
    spec: ProcessSpec,
    cancel: CancellationToken,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<ProcessOutcome, SupervisorError> {
    if let Some(parent) = spec.stdout_path.parent() {
        tokio::fs::create_dir_all(parent).await?
    }
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in &spec.env_denylist {
        cmd.env_remove(key);
    }
    cmd.envs(spec.env.clone());
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    let pid = child.id().unwrap_or(0);
    let started_at = Utc::now().to_rfc3339();
    let lease = lease::new_lease(pid, started_at.clone(), &spec.program);
    if let Err(error) = lease::write_process_lease(&spec.lease_path, &lease).await {
        let _ = terminate_tree(&mut child, pid, Some(&lease)).await;
        let _ = child.wait().await;
        return Err(error.into());
    }
    let stdout = child
        .stdout
        .take()
        .ok_or(SupervisorError::MissingPipe("stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(SupervisorError::MissingPipe("stderr"))?;
    let activity = Arc::new(Mutex::new(Instant::now()));
    let out_task = tokio::spawn(pump(
        stdout,
        spec.stdout_path,
        EventStream::Stdout,
        event_tx.clone(),
        activity.clone(),
    ));
    let err_task = tokio::spawn(pump(
        stderr,
        spec.stderr_path,
        EventStream::Stderr,
        event_tx,
        activity.clone(),
    ));
    let absolute = tokio::time::sleep(spec.timeout);
    tokio::pin!(absolute);
    let mut idle = tokio::time::interval(Duration::from_secs(1));
    let mut timed_out = false;
    let mut cancelled = false;
    let status = loop {
        tokio::select! {
            result=child.wait()=>break result?,
            _=&mut absolute=>{timed_out=true;terminate_tree(&mut child,pid,Some(&lease)).await?;break child.wait().await?},
            _=cancel.cancelled()=>{cancelled=true;terminate_tree(&mut child,pid,Some(&lease)).await?;break child.wait().await?},
            _=idle.tick()=>{if activity.lock().await.elapsed()>=spec.idle_timeout{timed_out=true;terminate_tree(&mut child,pid,Some(&lease)).await?;break child.wait().await?}}
        }
    };
    let a = out_task.await??;
    let b = err_task.await??;
    let _ = tokio::fs::remove_file(&spec.lease_path).await;
    Ok(ProcessOutcome {
        pid,
        started_at,
        exit_code: status.code(),
        timed_out,
        cancelled,
        log_truncated: a || b,
    })
}

async fn pump<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    path: PathBuf,
    stream: EventStream,
    tx: mpsc::Sender<AgentEvent>,
    activity: Arc<Mutex<Instant>>,
) -> Result<bool, std::io::Error> {
    let mut reader = BufReader::new(reader);
    let mut file = File::create(path).await?;
    let mut line = Vec::new();
    let mut written = 0u64;
    let mut truncated = false;
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line).await?;
        if n == 0 {
            break;
        }
        *activity.lock().await = Instant::now();
        if written < MAX_LOG_BYTES {
            let remaining = (MAX_LOG_BYTES - written) as usize;
            let part = &line[..line.len().min(remaining)];
            file.write_all(part).await?;
            written += part.len() as u64;
            if part.len() < line.len() {
                truncated = true
            }
        } else {
            truncated = true
        }
        let raw = String::from_utf8_lossy(&line).trim().to_string();
        let (kind, summary) = classify_and_summarize(&raw);
        let event = AgentEvent {
            ts: Utc::now().to_rfc3339(),
            stream,
            kind,
            summary: redact(summary),
            text: Some(redact(raw)),
        };
        let _ = tx.send(event).await;
    }
    Ok(truncated)
}
/// Convert provider-specific JSONL into a small stable vocabulary for the UI. The complete,
/// redacted line remains in `AgentEvent.text`, so summarisation never destroys diagnostics.
fn classify_and_summarize(line: &str) -> (AgentEventKind, String) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return (AgentEventKind::Raw, compact(line, 240));
    };
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("thread.started") => (AgentEventKind::System, "Codex 会话已启动".into()),
        Some("turn.started") => (AgentEventKind::System, "开始处理任务".into()),
        Some("turn.completed") => (AgentEventKind::Result, "本轮处理完成".into()),
        Some("item.started") | Some("item.completed") => summarize_codex_item(&value),
        Some("assistant") => summarize_claude_message(&value),
        Some("user") | Some("tool_result") => (AgentEventKind::ToolResult, "工具已返回结果".into()),
        Some("system")
            if value.get("subtype").and_then(serde_json::Value::as_str) == Some("init") =>
        {
            let model = value
                .get("model")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("默认模型");
            (
                AgentEventKind::System,
                format!("Claude Code 已启动（{model}）"),
            )
        }
        Some("rate_limit_event") => {
            let status = value
                .pointer("/rate_limit_info/status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("已更新");
            (AgentEventKind::System, format!("额度状态：{status}"))
        }
        Some("result") => summarize_result(&value),
        Some("tool_use") | Some("command_execution") => {
            (AgentEventKind::ToolUse, "正在调用工具".into())
        }
        _ => (AgentEventKind::Raw, json_event_label(&value)),
    }
}

fn summarize_codex_item(value: &serde_json::Value) -> (AgentEventKind, String) {
    let event_type = value.get("type").and_then(serde_json::Value::as_str);
    let item = value.get("item").unwrap_or(&serde_json::Value::Null);
    match item.get("type").and_then(serde_json::Value::as_str) {
        Some("agent_message") => {
            let text = item
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Agent 已更新进展");
            if let Some(summary) = embedded_summary(text) {
                (AgentEventKind::Result, compact(&summary, 240))
            } else {
                (AgentEventKind::AssistantText, compact(text, 240))
            }
        }
        Some("command_execution") if event_type == Some("item.started") => {
            let command = item
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("未知命令");
            (
                AgentEventKind::ToolUse,
                format!("运行命令：{}", compact(command, 190)),
            )
        }
        Some("command_execution") => {
            let status = item
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("completed");
            let exit = item.get("exit_code").and_then(serde_json::Value::as_i64);
            let suffix = exit
                .map(|code| format!("，退出码 {code}"))
                .unwrap_or_default();
            (AgentEventKind::ToolResult, format!("命令{status}{suffix}"))
        }
        Some("file_change") if event_type == Some("item.completed") => {
            let paths = item
                .get("changes")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|change| change.get("path").and_then(serde_json::Value::as_str))
                .map(short_path)
                .collect::<Vec<_>>();
            let shown = paths.iter().take(4).cloned().collect::<Vec<_>>().join("、");
            let more = if paths.len() > 4 {
                format!(" 等 {} 个文件", paths.len())
            } else {
                String::new()
            };
            (AgentEventKind::ToolUse, format!("已修改：{shown}{more}"))
        }
        Some("file_change") => (AgentEventKind::Raw, "正在准备文件改动".into()),
        Some("web_search") if event_type == Some("item.started") => {
            let query = item
                .get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("相关资料");
            (
                AgentEventKind::ToolUse,
                format!("搜索：{}", compact(query, 190)),
            )
        }
        Some(kind) => (AgentEventKind::Raw, format!("Codex 内部事件：{kind}")),
        None => (AgentEventKind::Raw, "Codex 内部事件".into()),
    }
}

fn summarize_claude_message(value: &serde_json::Value) -> (AgentEventKind, String) {
    let content = value
        .pointer("/message/content")
        .and_then(serde_json::Value::as_array);
    for item in content.into_iter().flatten() {
        match item.get("type").and_then(serde_json::Value::as_str) {
            Some("text") => {
                if let Some(text) = item.get("text").and_then(serde_json::Value::as_str)
                    && !text.trim().is_empty()
                {
                    return (AgentEventKind::AssistantText, compact(text, 240));
                }
            }
            Some("tool_use") => {
                let name = item
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("工具");
                let detail = tool_detail(item.get("input"));
                return (
                    AgentEventKind::ToolUse,
                    compact(&format!("调用 {name}{detail}"), 240),
                );
            }
            _ => {}
        }
    }
    (AgentEventKind::Raw, "Claude 正在分析".into())
}

fn summarize_result(value: &serde_json::Value) -> (AgentEventKind, String) {
    let result = value
        .get("result")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if let Some(summary) = embedded_summary(result) {
        return (AgentEventKind::Result, compact(&summary, 240));
    }
    if value.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
        return (
            AgentEventKind::Result,
            format!("运行失败：{}", compact(result, 210)),
        );
    }
    let summary = if result.trim().is_empty() {
        "运行完成".into()
    } else {
        compact(result, 240)
    };
    (AgentEventKind::Result, summary)
}

fn embedded_summary(text: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return value
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }
    text.char_indices().find_map(|(index, ch)| {
        (ch == '{')
            .then(|| serde_json::from_str::<serde_json::Value>(&text[index..]).ok())
            .flatten()
            .and_then(|value| {
                value
                    .get("summary")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
    })
}

fn tool_detail(input: Option<&serde_json::Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    for key in ["file_path", "command", "pattern", "query"] {
        if let Some(value) = input.get(key).and_then(serde_json::Value::as_str) {
            return format!("：{}", compact(value, 180));
        }
    }
    String::new()
}

fn short_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn json_event_label(value: &serde_json::Value) -> String {
    let kind = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    format!("内部事件：{kind}")
}

fn compact(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }
    compact
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}
pub fn redact(mut text: String) -> String {
    for pattern in [
        r"AKIA[0-9A-Z]{16}",
        r"ghp_[A-Za-z0-9]{20,}",
        r"sk-[A-Za-z0-9_-]{16,}",
        r"(?i)Authorization:\s*Bearer\s+\S+",
        r"(?i)password\s*=\s*\S+",
    ] {
        if let Ok(re) = Regex::new(pattern) {
            text = re.replace_all(&text, "[REDACTED]").into_owned();
        }
    }
    text
}

async fn terminate_tree(
    _child: &mut Child,
    pid: u32,
    lease: Option<&ProcessLease>,
) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        if let Some(lease) = lease {
            let _ = terminate_process_lease(lease, Duration::from_secs(2)).await?;
        } else if pid > 0 {
            let fallback = lease::new_lease(pid, String::new(), PathBuf::new().as_path());
            let _ = terminate_process_lease(&fallback, Duration::from_secs(2)).await?;
        }
    }
    #[cfg(windows)]
    {
        _child.kill().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_terminates_the_leased_process_group()
    -> Result<(), Box<dyn std::error::Error>> {
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
        let s = redact(
            "Authorization: Bearer abc password=hunter2 ghp_abcdefghijklmnopqrstuvwxyz".into(),
        );
        assert!(!s.contains("hunter2"));
        assert!(!s.contains("abcdefghijklmnopqrstuvwxyz"));
    }
}
