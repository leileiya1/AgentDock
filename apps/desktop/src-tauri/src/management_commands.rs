use super::*;
use crate::view_types::RunLogPage;
use serde::Deserialize;

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

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunLogArgs {
    run_id: String,
    from_line: u32,
    max_lines: u32,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectConfigApprovalArgs {
    project_id: String,
    expected_sha256: String,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct PermissionDecisionArgs {
    input: agentflow_contracts::PermissionDecisionInput,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct PermissionRuleRevokeArgs {
    project_id: String,
    rule_id: String,
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
pub(super) async fn project_prune_stale_worktrees(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<agentflow_contracts::GitCompatibilityReport, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectPruneStaleWorktrees {
            project_id: args.project_id,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn queue_task_pause(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<agentflow_contracts::QueueTaskState, AppError> {
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
) -> Result<agentflow_contracts::QueueTaskState, AppError> {
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
) -> Result<agentflow_contracts::QueueTaskState, AppError> {
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
pub(super) async fn queue_task_status(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<Option<agentflow_contracts::QueueTaskState>, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::QueueTaskStatus {
            task_id: args.task_id,
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
    args: ProjectConfigApprovalArgs,
) -> Result<agentflow_contracts::ProjectConfigTrust, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectConfigTrustApprove {
            project_id: args.project_id,
            expected_sha256: args.expected_sha256,
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

#[tauri::command]
#[specta::specta]
pub(super) async fn permission_request_list(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<Vec<agentflow_contracts::PermissionRequest>, AppError> {
    state.0.permission_requests(&args.task_id).await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn permission_decide(
    state: State<'_, Backend>,
    args: PermissionDecisionArgs,
) -> Result<agentflow_contracts::PermissionDecision, AppError> {
    daemon_mutate(&state, DaemonRequest::PermissionDecide { input: args.input }).await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn permission_rule_list(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<Vec<agentflow_contracts::PermissionRule>, AppError> {
    state.0.permission_rules(&args.project_id).await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn permission_rule_revoke(
    state: State<'_, Backend>,
    args: PermissionRuleRevokeArgs,
) -> Result<agentflow_contracts::PermissionRule, AppError> {
    daemon_mutate(&state, DaemonRequest::PermissionRuleRevoke { project_id: args.project_id, rule_id: args.rule_id }).await
}
