use super::*;
use crate::view_types::RunLogPage;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Type)]
pub(super) struct DatabaseRestoreArgs {
    path: String,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct QueuePriorityArgs {
    task_id: String,
    priority: i16,
}

#[derive(Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct QueueMutationResult {
    task_id: String,
    paused: Option<bool>,
    #[specta(type = Option<i32>)]
    priority: Option<i16>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunLogArgs {
    run_id: String,
    from_line: u32,
    max_lines: u32,
}

#[tauri::command]
#[specta::specta]
pub(super) async fn database_backup_list(
    state: State<'_, Backend>,
) -> Result<Vec<agentflow_contracts::DatabaseBackupInfo>, AppError> {
    state.0.database_backup_list().await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn database_backup_create(
    state: State<'_, Backend>,
) -> Result<agentflow_contracts::DatabaseBackupInfo, AppError> {
    daemon_mutate(&state, DaemonRequest::DatabaseBackupCreate).await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn database_backup_restore(
    state: State<'_, Backend>,
    args: DatabaseRestoreArgs,
) -> Result<agentflow_contracts::DatabaseRestoreResult, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::DatabaseBackupRestore { path: args.path },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_git_compatibility(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<agentflow_contracts::GitCompatibilityReport, AppError> {
    state
        .0
        .project_git_compatibility(&args.project_id)
        .await
        .map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn queue_task_pause(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<QueueMutationResult, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::QueueTaskPause {
            task_id: args.task_id,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn queue_task_resume(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<QueueMutationResult, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::QueueTaskResume {
            task_id: args.task_id,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn queue_task_priority(
    state: State<'_, Backend>,
    args: QueuePriorityArgs,
) -> Result<QueueMutationResult, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::QueueTaskPriority {
            task_id: args.task_id,
            priority: args.priority,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_config_trust_get(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<agentflow_contracts::ProjectConfigTrust, AppError> {
    state
        .0
        .project_config_trust_get(&args.project_id)
        .await
        .map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_config_trust_approve(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<agentflow_contracts::ProjectConfigTrust, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectConfigTrustApprove {
            project_id: args.project_id,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_config_trust_revoke(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<agentflow_contracts::ProjectConfigTrust, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectConfigTrustRevoke {
            project_id: args.project_id,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn run_log_tail(
    state: State<'_, Backend>,
    args: RunLogArgs,
) -> Result<RunLogPage, AppError> {
    state
        .0
        .run_log_tail(
            &args.run_id,
            args.from_line as usize,
            args.max_lines as usize,
        )
        .await
        .map(|(lines, next_from_line, eof)| RunLogPage {
            lines,
            next_from_line: next_from_line as u32,
            eof,
        })
        .map_err(app_error)
}
