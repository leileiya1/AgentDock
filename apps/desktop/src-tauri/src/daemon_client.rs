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
            // The daemon wraps orchestrator refusals as "orchestrator error: <reason>"; the
            // reason is already user-facing copy, so show it without the internal prefix.
            let reason = message
                .strip_prefix("orchestrator error: ")
                .unwrap_or(&message)
                .to_string();
            Err(daemon_error("后台服务拒绝了这项操作", Some(reason)))
        }
        Err(error) => Err(daemon_error(
            "后台调度服务未连接",
            Some(format!("{error}。请重新启动 AgentFlow 后再试。")),
        )),
    }
}

/// What launch should do about the daemon, decided from facts gathered up front.
#[derive(Debug, PartialEq, Eq)]
enum UpgradePlan {
    /// The installed daemon is the bundled one and it answers: nothing to do.
    UpToDate,
    /// Keep the running daemon: the bundled one cannot open this (newer) database.
    KeepRunning,
    /// Install the bundled daemon.
    Install,
    /// Do not start: explain why.
    Refuse(String),
}

/// `bundled_can_open`: `Some(true/false)` from `agentflowd can-open`, `None` when the bundled
/// binary could not answer (an old build without the command).
/// `database_exists`: whether this data dir already has a database a daemon migrated.
fn plan_upgrade(
    current: bool,
    compatible: bool,
    queue_depth: i64,
    scheduler_alive: bool,
    bundled_can_open: Option<bool>,
    database_exists: bool,
) -> UpgradePlan {
    if current && compatible {
        return UpgradePlan::UpToDate;
    }
    // Never downgrade past a migration. A bundled build that cannot answer predates the
    // can-open check, so it is only trusted on a fresh machine with no database yet.
    let bundled_ok = bundled_can_open.unwrap_or(!database_exists);
    if !bundled_ok {
        if compatible {
            return UpgradePlan::KeepRunning;
        }
        return UpgradePlan::Refuse(
            "安装包里的后台服务比本机数据旧，打不开已经升级过的数据库。请安装最新版 AgentFlow（开发时先运行 bun run prepare:sidecar）".into(),
        );
    }
    if queue_depth > 0 && scheduler_alive {
        return UpgradePlan::Refuse(format!(
            "后台服务需要升级，但当前还有 {queue_depth} 个任务正在运行或排队；任务结束后重启 AgentFlow 即可安全升级"
        ));
    }
    UpgradePlan::Install
}

/// Ask a daemon binary whether it can open this data dir's database.
async fn can_open(binary: &Path, data_dir: &Path) -> Option<bool> {
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(binary)
            .arg("--data-dir")
            .arg(data_dir)
            .arg("can-open")
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    match output.status.code() {
        Some(0) => Some(true),
        Some(3) => Some(false),
        _ => None,
    }
}

/// How long a freshly installed daemon gets to answer before we call the upgrade failed.
/// launchd may throttle a restart for up to ~10 s, so 3 s produced false failures.
const HEALTH_WAIT: Duration = Duration::from_secs(20);

/// Install or atomically upgrade the bundled daemon before the desktop opens its read client.
/// Active queues are never interrupted; an upgrade is deferred until the queue is drained.
/// A bundled daemon that cannot open the existing database never replaces one that can.
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
    let queue_depth = running_payload
        .and_then(|payload| payload.get("queueDepth"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    // A daemon whose scheduler loop has stopped never drains its queue, so deferring the
    // upgrade on queue depth alone would deadlock. Older daemons omit the field; treat a
    // missing value as alive so this never regresses a healthy but older service.
    let scheduler_alive = running_payload
        .and_then(|payload| payload.get("schedulerAlive"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let bundled_can_open = if current && compatible {
        Some(true)
    } else {
        can_open(&bundled, data_dir).await
    };
    let database_exists = data_dir.join("agentflow.db").is_file();
    match plan_upgrade(
        current,
        compatible,
        queue_depth,
        scheduler_alive,
        bundled_can_open,
        database_exists,
    ) {
        UpgradePlan::UpToDate => return Ok(()),
        UpgradePlan::KeepRunning => {
            eprintln!("AgentFlow: 安装包里的后台服务比正在运行的旧，继续使用正在运行的版本");
            return Ok(());
        }
        UpgradePlan::Refuse(message) => return Err(message.into()),
        UpgradePlan::Install => {}
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
    let deadline = tokio::time::Instant::now() + HEALTH_WAIT;
    while tokio::time::Instant::now() < deadline {
        if matches!(
            request(data_dir, &DaemonRequest::Ping).await,
            Ok(DaemonResponse::Ok { .. })
        ) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    // Rolling back to a daemon that cannot open the (possibly just migrated) database would
    // leave nothing able to start, so only roll back to one that says it can.
    let previous = data_dir.join("bin/agentflowd.previous");
    if previous.is_file() && can_open(&previous, data_dir).await != Some(true) {
        return Err(format!(
            "新版后台服务 {} 秒内没有响应；上一版打不开已升级的数据，所以没有回滚。请稍后重新打开 AgentFlow，或查看 logs/agentflowd-error.log",
            HEALTH_WAIT.as_secs()
        )
        .into());
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

#[cfg(test)]
mod upgrade_tests {
    use super::{UpgradePlan, plan_upgrade};

    #[test]
    fn up_to_date_does_nothing() {
        assert_eq!(
            plan_upgrade(true, true, 3, true, None, true),
            UpgradePlan::UpToDate
        );
    }

    #[test]
    fn never_downgrades_a_healthy_newer_daemon() {
        // The bug from 2026-11-07: an old sidecar replaced a daemon that had migrated the DB.
        assert_eq!(
            plan_upgrade(false, true, 0, true, Some(false), true),
            UpgradePlan::KeepRunning
        );
        // An old sidecar that does not even know `can-open` is treated the same way.
        assert_eq!(
            plan_upgrade(false, true, 0, true, None, true),
            UpgradePlan::KeepRunning
        );
    }

    #[test]
    fn refuses_when_nothing_can_open_the_database() {
        assert!(matches!(
            plan_upgrade(false, false, 0, true, Some(false), true),
            UpgradePlan::Refuse(_)
        ));
        // Drill B2: nothing running, old sidecar that cannot answer, database already there.
        assert!(matches!(
            plan_upgrade(false, false, 0, true, None, true),
            UpgradePlan::Refuse(_)
        ));
    }

    #[test]
    fn installs_a_capable_build_and_respects_busy_queues() {
        assert_eq!(
            plan_upgrade(false, true, 0, true, Some(true), true),
            UpgradePlan::Install
        );
        assert_eq!(
            plan_upgrade(false, false, 0, true, None, false),
            UpgradePlan::Install,
            "fresh machine"
        );
        assert!(matches!(
            plan_upgrade(false, true, 2, true, Some(true), true),
            UpgradePlan::Refuse(_)
        ));
        assert_eq!(
            plan_upgrade(false, true, 2, false, Some(true), true),
            UpgradePlan::Install,
            "stuck scheduler"
        );
    }
}
