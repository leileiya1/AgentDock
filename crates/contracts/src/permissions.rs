use std::collections::BTreeMap;

string_enum!(PermissionActionType {
    WorktreeRead => "worktree_read", WorktreeWrite => "worktree_write",
    WorktreeDelete => "worktree_delete", ControlPlaneWrite => "control_plane_write",
    CommandExecute => "command_execute", DependencyInstall => "dependency_install",
    NetworkAccess => "network_access", EnvironmentRead => "environment_read",
    SecretAccess => "secret_access", ProcessControl => "process_control",
    GitRead => "git_read", GitMutation => "git_mutation", SystemChange => "system_change",
    ExternalPath => "external_path"
});
string_enum!(PermissionRiskLevel { Low => "low", Medium => "medium", High => "high", Forbidden => "forbidden" });
string_enum!(PermissionPathAccess { Read => "read", Write => "write", Delete => "delete" });
string_enum!(PermissionRequestStatus {
    Pending => "pending", Approved => "approved", Denied => "denied",
    Cancelled => "cancelled", Expired => "expired"
});
string_enum!(PermissionDecisionKind { Approve => "approve", Deny => "deny", CancelTask => "cancel_task" });
string_enum!(PermissionGrantScope { Once => "once", Task => "task", ProjectRule => "project_rule" });
string_enum!(PermissionErrorCode {
    RequestStale => "PERMISSION_REQUEST_STALE", PathEscape => "PERMISSION_PATH_ESCAPE",
    RuleTooBroad => "PERMISSION_RULE_TOO_BROAD", NotGrantable => "PERMISSION_NOT_GRANTABLE",
    Expired => "PERMISSION_EXPIRED", Denied => "PERMISSION_DENIED",
    Required => "PERMISSION_REQUIRED", ManifestExceeded => "PERMISSION_MANIFEST_EXCEEDED",
    RoleReadOnly => "PERMISSION_ROLE_READ_ONLY"
});
string_enum!(SandboxGuarantee { #[default] None => "none", ReadOnly => "read_only", WorktreeRestricted => "worktree_restricted" });

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPath {
    pub path: String,
    pub access: PermissionPathAccess,
    pub outside_worktree: bool,
}

/// Canonical, value-free operation used for policy matching and sealing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOperation {
    pub argv: Vec<String>,
    pub cwd: String,
    pub paths: Vec<PermissionPath>,
    pub network_domains: Vec<String>,
    pub environment_names: Vec<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub id: String,
    pub project_id: String,
    pub task_id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub run_id: Option<String>,
    pub provider_id: AgentKind,
    pub role: RunRole,
    pub action_type: PermissionActionType,
    pub summary: String,
    pub reason: String,
    pub operation: PermissionOperation,
    pub risk_level: PermissionRiskLevel,
    pub grantable: bool,
    pub operation_sha256: String,
    pub policy_sha256: String,
    pub status: PermissionRequestStatus,
    pub matched_rule_id: Option<String>,
    #[specta(type = i32)]
    pub request_count: i64,
    pub requested_at: String,
    pub expires_at: String,
    pub decided_at: Option<String>,
    pub provider_resume_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestInput {
    pub task_id: String,
    pub run_id: Option<String>,
    pub provider_id: AgentKind,
    pub role: RunRole,
    pub action_type: PermissionActionType,
    pub reason: String,
    pub operation: PermissionOperation,
    pub provider_resume_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecisionInput {
    pub request_id: String,
    pub operation_sha256: String,
    pub policy_sha256: String,
    pub decision: PermissionDecisionKind,
    pub scope: PermissionGrantScope,
    pub guidance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecision {
    pub id: String,
    pub request_id: String,
    pub operation_sha256: String,
    pub policy_sha256: String,
    pub decision: PermissionDecisionKind,
    pub scope: PermissionGrantScope,
    pub expires_at: Option<String>,
    pub approved_by: String,
    pub guidance: Option<String>,
    pub created_at: String,
    pub consumed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub id: String,
    pub project_id: String,
    pub project_identity: String,
    pub provider_id: AgentKind,
    pub role: RunRole,
    pub action_type: PermissionActionType,
    pub operation: PermissionOperation,
    pub rule_sha256: String,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_matched_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePermissions {
    pub worktree_read: bool,
    pub worktree_write: bool,
    pub command_argv: Vec<Vec<String>>,
    pub external_paths: Vec<PermissionPath>,
    pub network_domains: Vec<String>,
    pub environment_names: Vec<String>,
    pub sandbox_guarantee: SandboxGuarantee,
    pub expires_at: Option<String>,
}
