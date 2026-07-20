use super::{Backend, TaskIdArgs};
use crate::{daemon_client::mutate as daemon_mutate, error::app_error};
use agentflow_contracts::{AppError, RepairAction, RepairReport, TaskDetail};
use agentflow_daemon::DaemonRequest;
use serde::Deserialize;
use specta::Type;
use tauri::State;

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskRepairArgs {
    task_id: String,
    action: RepairAction,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn task_repair_inspect(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<RepairReport, AppError> {
    state
        .0
        .task_repair_inspect(&args.task_id)
        .await
        .map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn task_repair_apply(
    state: State<'_, Backend>,
    args: TaskRepairArgs,
) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskRepair {
            task_id: args.task_id,
            action: args.action,
        },
    )
    .await
}
