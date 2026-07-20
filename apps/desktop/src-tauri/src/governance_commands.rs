use super::Backend;
use crate::{daemon_client::mutate as daemon_mutate, error::app_error};
use agentflow_contracts::{
    AppError, BudgetLimitPatch, ExecutionNode, QualityEvaluation, RollbackStrategy, TaskDetail,
    TaskGovernance,
};
use agentflow_daemon::{DaemonRequest, ExecutionNodeRequest, GovernanceRequest};
use serde::Deserialize;
use specta::Type;
use tauri::State;

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanActionArgs {
    task_id: String,
    plan_id: String,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanRejectArgs {
    task_id: String,
    plan_id: String,
    reason: String,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GovernanceArgs {
    task_id: String,
    revision: Option<i32>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BudgetUpdateArgs {
    task_id: String,
    limits: BudgetLimitPatch,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RollbackArgs {
    task_id: String,
    strategy: RollbackStrategy,
}

#[derive(Deserialize, Type)]
pub(crate) struct NodeUpsertArgs {
    node: ExecutionNode,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeIdArgs {
    node_id: String,
}

macro_rules! task_governance_mutation {
    ($name:ident, $args:ty, $map:expr) => {
        #[tauri::command]
        #[specta::specta]
        pub(crate) async fn $name(
            state: State<'_, Backend>,
            args: $args,
        ) -> Result<TaskDetail, AppError> {
            daemon_mutate(&state, DaemonRequest::Governance { action: ($map)(args) }).await
        }
    };
}

task_governance_mutation!(task_plan_approve, PlanActionArgs, |args: PlanActionArgs| GovernanceRequest::PlanApprove { task_id: args.task_id, plan_id: args.plan_id });
task_governance_mutation!(task_plan_reject, PlanRejectArgs, |args: PlanRejectArgs| GovernanceRequest::PlanReject { task_id: args.task_id, plan_id: args.plan_id, reason: args.reason });
task_governance_mutation!(task_budget_update, BudgetUpdateArgs, |args: BudgetUpdateArgs| GovernanceRequest::BudgetUpdate { task_id: args.task_id, limits: args.limits });
task_governance_mutation!(task_delivery_start, super::TaskIdArgs, |args: super::TaskIdArgs| GovernanceRequest::DeliveryStart { task_id: args.task_id });
task_governance_mutation!(task_delivery_refresh, super::TaskIdArgs, |args: super::TaskIdArgs| GovernanceRequest::DeliveryRefresh { task_id: args.task_id });
task_governance_mutation!(task_rollback, RollbackArgs, |args: RollbackArgs| GovernanceRequest::Rollback { task_id: args.task_id, strategy: args.strategy });

#[tauri::command]
#[specta::specta]
pub(crate) async fn task_governance_get(
    state: State<'_, Backend>,
    args: GovernanceArgs,
) -> Result<TaskGovernance, AppError> {
    state.0.task_governance_get(&args.task_id, args.revision.map(i64::from)).await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn task_quality_replay(
    state: State<'_, Backend>,
    args: GovernanceArgs,
) -> Result<QualityEvaluation, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::Governance {
            action: GovernanceRequest::QualityReplay {
                task_id: args.task_id,
                revision: args.revision.map(i64::from),
            },
        },
    ).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execution_node_list(
    state: State<'_, Backend>,
) -> Result<Vec<ExecutionNode>, AppError> {
    state.0.execution_node_list().await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execution_node_upsert(
    state: State<'_, Backend>,
    args: NodeUpsertArgs,
) -> Result<ExecutionNode, AppError> {
    daemon_mutate(&state, DaemonRequest::ExecutionNode {
        action: ExecutionNodeRequest::Upsert { node: args.node },
    }).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execution_node_check(
    state: State<'_, Backend>,
    args: NodeIdArgs,
) -> Result<ExecutionNode, AppError> {
    daemon_mutate(&state, DaemonRequest::ExecutionNode {
        action: ExecutionNodeRequest::Check { node_id: args.node_id },
    }).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execution_node_delete(
    state: State<'_, Backend>,
    args: NodeIdArgs,
) -> Result<(), AppError> {
    daemon_mutate(&state, DaemonRequest::ExecutionNode {
        action: ExecutionNodeRequest::Delete { node_id: args.node_id },
    }).await
}
