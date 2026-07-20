use agentflow_contracts::{AppError, ErrorCode};
use agentflow_orchestrator::OrchestratorError;

pub(super) fn app_error(error: OrchestratorError) -> AppError {
    let code = match &error {
        OrchestratorError::DiffStale => ErrorCode::DiffStale,
        OrchestratorError::MergePrecondition(_) => ErrorCode::MergePreconditionFailed,
        OrchestratorError::RemoteNodeUnavailable(_) => ErrorCode::RemoteNodeUnavailable,
        OrchestratorError::QualityGate(_) => ErrorCode::QualityGateFailed,
        OrchestratorError::ScmCliNotFound(_) => ErrorCode::ScmCliNotFound,
        OrchestratorError::RollbackUnsafe(_) => ErrorCode::RollbackUnsafe,
        OrchestratorError::Persistence(_) | OrchestratorError::Sqlx(_) => ErrorCode::DbError,
        OrchestratorError::Io(_) => ErrorCode::IoError,
        OrchestratorError::Git(_) => ErrorCode::Internal,
        OrchestratorError::Adapter(_) => ErrorCode::RunSpawnFailed,
        OrchestratorError::InvalidState(value) if value == "TASK_SAME_AGENT" => {
            ErrorCode::TaskSameAgent
        }
        OrchestratorError::InvalidState(value) if value == "PROJECT_NOT_GIT" => {
            ErrorCode::ProjectNotGit
        }
        OrchestratorError::InvalidState(value) if value == "API_EGRESS_APPROVAL_REQUIRED" => {
            ErrorCode::ApiEgressApprovalRequired
        }
        OrchestratorError::InvalidState(value) if value.starts_with("PLAN_APPROVAL_REQUIRED") => {
            ErrorCode::PlanApprovalRequired
        }
        OrchestratorError::InvalidState(value) if value.starts_with("BUDGET_EXCEEDED") => {
            ErrorCode::BudgetExceeded
        }
        OrchestratorError::InvalidState(value) if value.starts_with("REMOTE_NODE_UNAVAILABLE") => {
            ErrorCode::RemoteNodeUnavailable
        }
        OrchestratorError::InvalidState(value) if value.starts_with("CI_FAILED") => {
            ErrorCode::CiFailed
        }
        OrchestratorError::InvalidState(value) if value.starts_with("SCM_CLI_NOT_FOUND") => {
            ErrorCode::ScmCliNotFound
        }
        OrchestratorError::InvalidState(value) if value.starts_with("ROLLBACK_UNSAFE") => {
            ErrorCode::RollbackUnsafe
        }
        OrchestratorError::InvalidState(_) => ErrorCode::TaskInvalidState,
        _ => ErrorCode::Internal,
    };
    AppError {
        code,
        message: error.to_string(),
        detail: None,
    }
}
