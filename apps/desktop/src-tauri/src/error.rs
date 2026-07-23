use agentflow_contracts::{AppError, ErrorCode};
use agentflow_orchestrator::OrchestratorError;

pub(super) fn app_error(error: OrchestratorError) -> AppError {
    let detail = match &error {
        OrchestratorError::MergePrecondition(value)
        | OrchestratorError::RemoteNodeUnavailable(value)
        | OrchestratorError::QualityGate(value)
        | OrchestratorError::ScmCliNotFound(value)
        | OrchestratorError::RollbackUnsafe(value) => Some(value.clone()),
        // §65: surface the egress provider list (everything after the code) as the error detail.
        OrchestratorError::InvalidState(value)
            if value.starts_with("API_EGRESS_APPROVAL_REQUIRED:") =>
        {
            value
                .split_once(':')
                .map(|(_, rest)| format!("会外发到：{}", rest.trim()))
        }
        _ => None,
    };
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
        OrchestratorError::InvalidState(value)
            if value.starts_with("API_EGRESS_APPROVAL_REQUIRED") =>
        {
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
        OrchestratorError::InvalidState(value) if value == "PERMISSION_REQUEST_STALE" => ErrorCode::PermissionRequestStale,
        OrchestratorError::InvalidState(value) if value == "PERMISSION_PATH_ESCAPE" => ErrorCode::PermissionPathEscape,
        OrchestratorError::InvalidState(value) if value == "PERMISSION_RULE_TOO_BROAD" => ErrorCode::PermissionRuleTooBroad,
        OrchestratorError::InvalidState(value) if value == "PERMISSION_NOT_GRANTABLE" => ErrorCode::PermissionNotGrantable,
        OrchestratorError::InvalidState(value) if value == "PERMISSION_EXPIRED" => ErrorCode::PermissionExpired,
        OrchestratorError::InvalidState(value) if value == "PERMISSION_DENIED" => ErrorCode::PermissionDenied,
        OrchestratorError::InvalidState(_) => ErrorCode::TaskInvalidState,
        _ => ErrorCode::Internal,
    };
    AppError {
        code,
        message: error.to_string(),
        detail,
    }
}
