const PERMISSION_POLICY_VERSION: &str = "agentflow.permission.v1";
const PERMISSION_TTL_MINUTES: i64 = 30;
const TASK_GRANT_TTL_HOURS: i64 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAuthorization {
    AllowedByDefault,
    AllowedByGrant,
    PermissionRequired,
    Denied,
}
