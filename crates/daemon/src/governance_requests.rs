use super::{enqueue_task, value};
use agentflow_contracts::{BudgetLimitPatch, ExecutionNode, RollbackStrategy};
use agentflow_orchestrator::{Orchestrator, OrchestratorError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GovernanceRequest {
    PlanApprove {
        task_id: String,
        plan_id: String,
    },
    PlanReject {
        task_id: String,
        plan_id: String,
        reason: String,
    },
    BudgetUpdate {
        task_id: String,
        limits: BudgetLimitPatch,
    },
    QualityReplay {
        task_id: String,
        revision: Option<i64>,
    },
    DeliveryStart {
        task_id: String,
    },
    DeliveryRefresh {
        task_id: String,
    },
    Rollback {
        task_id: String,
        strategy: RollbackStrategy,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ExecutionNodeRequest {
    Upsert { node: ExecutionNode },
    Check { node_id: String },
    Delete { node_id: String },
}

pub async fn dispatch_governance(
    orchestrator: &Orchestrator,
    action: GovernanceRequest,
) -> Result<Value, OrchestratorError> {
    match action {
        GovernanceRequest::PlanApprove { task_id, plan_id } => {
            orchestrator.task_plan_approve(&task_id, &plan_id).await?;
            enqueue_task(orchestrator, &task_id)
                .await
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))?;
            value(orchestrator.task_get(&task_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        GovernanceRequest::PlanReject {
            task_id,
            plan_id,
            reason,
        } => {
            orchestrator
                .task_plan_reject(&task_id, &plan_id, &reason)
                .await?;
            enqueue_task(orchestrator, &task_id)
                .await
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))?;
            value(orchestrator.task_get(&task_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        GovernanceRequest::BudgetUpdate { task_id, limits } => {
            orchestrator.task_budget_update(&task_id, limits).await?;
            enqueue_task(orchestrator, &task_id)
                .await
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))?;
            value(orchestrator.task_get(&task_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        GovernanceRequest::QualityReplay { task_id, revision } => {
            value(orchestrator.task_quality_replay(&task_id, revision).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        GovernanceRequest::DeliveryStart { task_id } => {
            orchestrator.task_delivery_start(&task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        GovernanceRequest::DeliveryRefresh { task_id } => {
            orchestrator.task_delivery_refresh(&task_id).await?;
            value(orchestrator.task_get(&task_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        GovernanceRequest::Rollback { task_id, strategy } => {
            orchestrator.task_rollback(&task_id, strategy).await?;
            value(orchestrator.task_get(&task_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
    }
}

pub async fn dispatch_node(
    orchestrator: &Orchestrator,
    action: ExecutionNodeRequest,
) -> Result<Value, OrchestratorError> {
    match action {
        ExecutionNodeRequest::Upsert { node } => {
            value(orchestrator.execution_node_upsert(node).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        ExecutionNodeRequest::Check { node_id } => {
            value(orchestrator.execution_node_check(&node_id).await?)
                .map_err(|error| OrchestratorError::InvalidState(error.to_string()))
        }
        ExecutionNodeRequest::Delete { node_id } => {
            orchestrator.execution_node_delete(&node_id).await?;
            Ok(Value::Null)
        }
    }
}
