use agentflow_contracts::{
    AgentKind, GlobalSettings, ProjectSettings, RepairAction, StorageCleanupScope, TaskPolicy,
    TaskStatus, TaskSummary,
};
use agentflow_orchestrator::Orchestrator;
use chrono::{Duration as ChronoDuration, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

mod governance_requests;
pub use governance_requests::{ExecutionNodeRequest, GovernanceRequest};
use governance_requests::{dispatch_governance, dispatch_node};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("orchestrator error: {0}")]
    Orchestrator(#[from] agentflow_orchestrator::OrchestratorError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DaemonRequest {
    Ping,
    Enqueue {
        task_id: String,
    },
    TaskStatus {
        task_id: String,
    },
    OnboardingComplete,
    EnvSetCliPath {
        tool: String,
        path: String,
    },
    ProjectImport {
        path: String,
    },
    TaskCreate {
        project_id: String,
        title: String,
        description: String,
        developer_agent: AgentKind,
        reviewer_agent: AgentKind,
        target_branch: Option<String>,
        max_revisions: Option<i64>,
        #[serde(default)]
        allow_api_egress: bool,
        #[serde(default)]
        policy: TaskPolicy,
    },
    TaskStart {
        task_id: String,
    },
    TaskCancel {
        task_id: String,
    },
    TaskResumeWithGuidance {
        task_id: String,
        guidance: String,
    },
    TaskRepair {
        task_id: String,
        action: RepairAction,
    },
    TaskForceApprove {
        task_id: String,
    },
    TaskApprove {
        task_id: String,
        revision: i64,
        commit_sha: String,
        diff_sha256: String,
    },
    TaskReject {
        task_id: String,
        revision: i64,
        reason: String,
    },
    TaskMerge {
        task_id: String,
    },
    TaskMarkMergedExternal {
        task_id: String,
    },
    Governance {
        action: GovernanceRequest,
    },
    ExecutionNode {
        action: ExecutionNodeRequest,
    },
    ProjectSettingsUpdate {
        project_id: String,
        settings: Box<ProjectSettings>,
    },
    SettingsUpdate {
        settings: GlobalSettings,
    },
    StorageCleanup,
    TaskCleanup {
        task_id: String,
        scope: StorageCleanupScope,
    },
    TaskRestore {
        task_id: String,
    },
    TrashEmpty,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DaemonResponse {
    Ok { payload: serde_json::Value },
    Error { message: String },
}

pub fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("agentflowd.sock")
}

pub fn default_data_dir() -> Result<PathBuf, DaemonError> {
    let user_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| DaemonError::Protocol("HOME is unavailable".into()))?;
    #[cfg(target_os = "macos")]
    {
        Ok(user_home.join("Library/Application Support/com.agentflow.desktop"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(user_home.join(".local/share/agentflow"))
    }
}

pub async fn request(
    data_dir: &Path,
    request: &DaemonRequest,
) -> Result<DaemonResponse, DaemonError> {
    let mut stream = UnixStream::connect(socket_path(data_dir)).await?;
    let mut bytes =
        serde_json::to_vec(request).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    serde_json::from_str(&line).map_err(|error| DaemonError::Protocol(error.to_string()))
}

pub async fn serve(data_dir: PathBuf, shutdown: CancellationToken) -> Result<(), DaemonError> {
    tokio::fs::create_dir_all(&data_dir).await?;

    // Acquire ownership before opening the authoritative Orchestrator. Recovery mutates running
    // rows and may terminate orphaned process groups, so a second daemon must never reach it.
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(data_dir.join("agentflowd.lock"))?;
    lock_file
        .try_lock_exclusive()
        .map_err(|_| DaemonError::Protocol("agentflowd is already running".into()))?;

    let path = socket_path(&data_dir);
    if path.exists() {
        if UnixStream::connect(&path).await.is_ok() {
            return Err(DaemonError::Protocol(
                "agentflowd is already running".into(),
            ));
        }
        tokio::fs::remove_file(&path).await?;
    }
    let listener = UnixListener::bind(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
    }

    let orchestrator = Arc::new(Orchestrator::open(&data_dir).await?);
    sqlx::query("UPDATE daemon_queue SET state='QUEUED',updated_at=? WHERE state='RUNNING'")
        .bind(Utc::now().to_rfc3339())
        .execute(orchestrator.store.pool())
        .await?;

    let scheduler_shutdown = shutdown.clone();
    let scheduler_orchestrator = Arc::clone(&orchestrator);
    let scheduler =
        tokio::spawn(
            async move { scheduler_loop(scheduler_orchestrator, scheduler_shutdown).await },
        );
    let maintenance_shutdown = shutdown.clone();
    let maintenance_orchestrator = Arc::clone(&orchestrator);
    let maintenance = tokio::spawn(async move {
        maintenance_loop(maintenance_orchestrator, maintenance_shutdown).await
    });

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let orchestrator = Arc::clone(&orchestrator);
                let shutdown = shutdown.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(stream, orchestrator, shutdown).await {
                        tracing::warn!(%error, "daemon IPC request failed");
                    }
                });
            }
        }
    }
    let _ = scheduler.await;
    let _ = maintenance.await;
    if path.exists() {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

async fn maintenance_loop(orchestrator: Arc<Orchestrator>, shutdown: CancellationToken) {
    let mut tick = tokio::time::interval(Duration::from_secs(6 * 60 * 60));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = tick.tick() => {
                match orchestrator.storage_cleanup(true).await {
                    Ok(result) if result.bytes_reclaimed > 0 || result.tasks_purged > 0 => {
                        tracing::info!(
                            bytes_reclaimed=result.bytes_reclaimed,
                            tasks_purged=result.tasks_purged,
                            "automatic storage cleanup completed"
                        );
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!(%error, "automatic storage cleanup failed"),
                }
            }
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    orchestrator: Arc<Orchestrator>,
    shutdown: CancellationToken,
) -> Result<(), DaemonError> {
    let (read, mut write) = stream.into_split();
    let mut line = String::new();
    BufReader::new(read).read_line(&mut line).await?;
    let request: DaemonRequest =
        serde_json::from_str(&line).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    let response = match dispatch(request, &orchestrator, &shutdown).await {
        Ok(payload) => DaemonResponse::Ok { payload },
        Err(error) => DaemonResponse::Error {
            message: error.to_string(),
        },
    };
    let mut bytes =
        serde_json::to_vec(&response).map_err(|error| DaemonError::Protocol(error.to_string()))?;
    bytes.push(b'\n');
    write.write_all(&bytes).await?;
    Ok(())
}

async fn dispatch(
    request: DaemonRequest,
    orchestrator: &Orchestrator,
    shutdown: &CancellationToken,
) -> Result<serde_json::Value, DaemonError> {
    match request {
        DaemonRequest::Ping => Ok(json!({
            "pid": std::process::id(),
            "version": env!("CARGO_PKG_VERSION"),
            "ipcVersion": 2,
            "queueDepth": queue_depth(orchestrator).await?
        })),
        DaemonRequest::Enqueue { task_id } => {
            enqueue_task(orchestrator, &task_id).await?;
            Ok(json!({"taskId": task_id, "queued": true}))
        }
        DaemonRequest::TaskStatus { task_id } => {
            let task = orchestrator.task_get(&task_id).await?;
            value(task.summary)
        }
        DaemonRequest::OnboardingComplete => {
            orchestrator.onboarding_complete().await?;
            Ok(Value::Null)
        }
        DaemonRequest::EnvSetCliPath { tool, path } => value(
            orchestrator
                .env_set_cli_path(&tool, Path::new(&path))
                .await?,
        ),
        DaemonRequest::ProjectImport { path } => {
            value(orchestrator.project_import(Path::new(&path)).await?)
        }
        DaemonRequest::TaskCreate {
            project_id,
            title,
            description,
            developer_agent,
            reviewer_agent,
            target_branch,
            max_revisions,
            allow_api_egress,
            policy,
        } => {
            let task = orchestrator
                .task_create_governed(
                    &project_id,
                    &title,
                    &description,
                    developer_agent,
                    reviewer_agent,
                    target_branch.as_deref(),
                    max_revisions,
                    allow_api_egress,
                    policy,
                )
                .await?;
            value(orchestrator.task_get(&task.id).await?)
        }
        DaemonRequest::TaskStart { task_id } => {
            orchestrator.task_start(&task_id).await?;
            enqueue_task(orchestrator, &task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskCancel { task_id } => {
            orchestrator.cancel(&task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskResumeWithGuidance { task_id, guidance } => {
            orchestrator
                .resume_with_guidance(&task_id, &guidance)
                .await?;
            enqueue_task(orchestrator, &task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskRepair { task_id, action } => {
            orchestrator.task_repair_apply(&task_id, action).await?;
            enqueue_task(orchestrator, &task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskForceApprove { task_id } => {
            orchestrator.force_approve(&task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskApprove {
            task_id,
            revision,
            commit_sha,
            diff_sha256,
        } => {
            orchestrator
                .approve(&task_id, revision, &commit_sha, &diff_sha256)
                .await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskReject {
            task_id,
            revision,
            reason,
        } => {
            orchestrator.reject(&task_id, revision, &reason).await?;
            enqueue_task(orchestrator, &task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskMerge { task_id } => {
            orchestrator.merge(&task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::TaskMarkMergedExternal { task_id } => {
            orchestrator.mark_merged_external(&task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
        }
        DaemonRequest::Governance { action } => dispatch_governance(orchestrator, action)
            .await
            .map_err(Into::into),
        DaemonRequest::ExecutionNode { action } => dispatch_node(orchestrator, action)
            .await
            .map_err(Into::into),
        DaemonRequest::ProjectSettingsUpdate {
            project_id,
            settings,
        } => value(
            orchestrator
                .project_settings_update(&project_id, &settings)
                .await?,
        ),
        DaemonRequest::SettingsUpdate { settings } => {
            value(orchestrator.settings_update(&settings).await?)
        }
        DaemonRequest::StorageCleanup => value(orchestrator.storage_cleanup(false).await?),
        DaemonRequest::TaskCleanup { task_id, scope } => {
            value(orchestrator.task_cleanup(&task_id, scope).await?)
        }
        DaemonRequest::TaskRestore { task_id } => value(orchestrator.task_restore(&task_id).await?),
        DaemonRequest::TrashEmpty => value(orchestrator.trash_empty().await?),
        DaemonRequest::Shutdown => {
            shutdown.cancel();
            Ok(json!({"shuttingDown": true}))
        }
    }
}

fn value<T: Serialize>(value: T) -> Result<Value, DaemonError> {
    serde_json::to_value(value).map_err(|error| DaemonError::Protocol(error.to_string()))
}

async fn enqueue_task(orchestrator: &Orchestrator, task_id: &str) -> Result<(), DaemonError> {
    let task = orchestrator.task_get(task_id).await?;
    if !is_runnable(task.summary.status) {
        return Err(DaemonError::Protocol(format!(
            "task {task_id} is not runnable from {}",
            task.summary.status
        )));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO daemon_queue(task_id,state,attempts,not_before,last_error,enqueued_at,updated_at) VALUES(?,'QUEUED',0,NULL,NULL,?,?) ON CONFLICT(task_id) DO UPDATE SET state='QUEUED',not_before=NULL,last_error=NULL,updated_at=excluded.updated_at")
        .bind(task_id)
        .bind(&now)
        .bind(&now)
        .execute(orchestrator.store.pool())
        .await?;
    Ok(())
}

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
                let limit = scheduler_limit(&orchestrator.settings_get().await?);
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
    let row = sqlx::query("SELECT task_id FROM daemon_queue WHERE state='QUEUED' AND (not_before IS NULL OR not_before<=?) ORDER BY enqueued_at LIMIT 1")
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

#[cfg(test)]
mod tests;
