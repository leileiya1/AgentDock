use super::Backend;
use agentflow_contracts::{AppError, ErrorCode};
use agentflow_daemon::{DaemonRequest, DaemonResponse, request};
use serde::de::DeserializeOwned;
use std::{path::Path, time::Duration};
use tauri::State;
use tokio::process::Command;

fn daemon_error(message: impl Into<String>, detail: Option<String>) -> AppError {
    AppError {
        code: ErrorCode::Internal,
        message: message.into(),
        detail,
    }
}

/// Send every product mutation to the daemon, which is the sole workflow writer.
pub async fn mutate<T: DeserializeOwned>(
    state: &State<'_, Backend>,
    command: DaemonRequest,
) -> Result<T, AppError> {
    match request(&state.1, &command).await {
        Ok(DaemonResponse::Ok { payload }) => serde_json::from_value(payload)
            .map_err(|error| daemon_error("后台服务返回了不兼容的数据", Some(error.to_string()))),
        Ok(DaemonResponse::Error { message }) => {
            Err(daemon_error("后台服务拒绝了这项操作", Some(message)))
        }
        Err(error) => Err(daemon_error(
            "后台调度服务未连接",
            Some(format!("{error}。请重新启动 AgentFlow 后再试。")),
        )),
    }
}

/// Install or atomically upgrade the bundled daemon before the desktop opens its read client.
/// Active queues are never interrupted; an upgrade is deferred until the queue is drained.
pub async fn ensure_daemon(data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bundled = bundled_daemon()?;
    let installed = data_dir.join("bin/agentflowd");
    let ping = request(data_dir, &DaemonRequest::Ping).await.ok();
    let running_payload = match &ping {
        Some(DaemonResponse::Ok { payload }) => Some(payload),
        _ => None,
    };
    let current = same_file_contents(&bundled, &installed)
        .await
        .unwrap_or(false);
    let compatible = running_payload
        .and_then(|payload| payload.get("ipcVersion"))
        .and_then(serde_json::Value::as_u64)
        == Some(2);
    if current && compatible {
        return Ok(());
    }

    let queue_depth = running_payload
        .and_then(|payload| payload.get("queueDepth"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    if queue_depth > 0 {
        return Err(format!(
            "后台服务需要升级，但当前还有 {queue_depth} 个任务正在运行或排队；任务结束后重启 AgentFlow 即可安全升级"
        )
        .into());
    }

    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new(&bundled)
            .arg("--data-dir")
            .arg(data_dir)
            .arg("install-service")
            .output(),
    )
    .await??;
    if !output.status.success() {
        return Err(format!(
            "安装后台服务失败：{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    for _ in 0..30 {
        if matches!(
            request(data_dir, &DaemonRequest::Ping).await,
            Ok(DaemonResponse::Ok { .. })
        ) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let rollback = Command::new(&bundled)
        .arg("--data-dir")
        .arg(data_dir)
        .arg("rollback-service")
        .output()
        .await;
    let detail = match rollback {
        Ok(output) if output.status.success() => "已自动恢复上一版后台服务".to_string(),
        Ok(output) => String::from_utf8_lossy(&output.stderr).trim().to_string(),
        Err(_) => "自动回滚失败".to_string(),
    };
    Err(format!("后台服务升级后健康检查失败；{detail}").into())
}

fn bundled_daemon() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or("桌面程序路径没有父目录")?;
    [
        directory.join("agentflowd"),
        directory.join("agentflowd-aarch64-apple-darwin"),
        directory.join("agentflowd-x86_64-apple-darwin"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| "安装包中缺少 agentflowd sidecar".into())
}

async fn same_file_contents(left: &Path, right: &Path) -> Result<bool, std::io::Error> {
    let left_meta = tokio::fs::metadata(left).await?;
    let right_meta = match tokio::fs::metadata(right).await {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if left_meta.len() != right_meta.len() {
        return Ok(false);
    }
    Ok(tokio::fs::read(left).await? == tokio::fs::read(right).await?)
}
