use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use serde_json::Value;
use specta::Type;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Type)]
        pub enum $name { $(#[serde(rename = $value)] $variant),+ }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let value = match self { $(Self::$variant => $value),+ };
                f.write_str(value)
            }
        }
        impl std::str::FromStr for $name {
            type Err = String;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value { $($value => Ok(Self::$variant),)+ _ => Err(format!("invalid {}: {value}", stringify!($name))) }
            }
        }
    };
}

// Provider identifiers are isolated because this is the public extension boundary.
include!("providers.rs");
// Settings grow as built-in Providers are added, so keep them out of the core DTO file.
include!("settings.rs");
// Recovery/update DTOs are isolated from the core task view to keep this contract readable.
include!("operations.rs");
// Planning, budgets, reproducibility, delivery and execution-node DTOs share one governance API.
include!("governance.rs");

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    #[specta(type = i32)]
    pub seq: i64,
    pub name: String,
    pub repo_path: String,
    pub default_branch: String,
    pub worktree_root: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub project_id: String,
    #[specta(type = i32)]
    pub seq: i64,
    pub title: String,
    pub status: TaskStatus,
    pub blocked_reason: Option<BlockedReason>,
    #[specta(type = i32)]
    pub current_revision: i64,
    pub developer_agent: AgentKind,
    pub reviewer_agent: AgentKind,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    #[serde(flatten)]
    pub summary: TaskSummary,
    pub description: String,
    pub target_branch: String,
    pub base_commit: Option<String>,
    pub branch: Option<String>,
    #[specta(type = i32)]
    pub max_revisions: i64,
    pub blocked_detail: Option<String>,
    pub revisions: Vec<RevisionInfo>,
    pub policy: TaskPolicy,
    pub plan: Option<CodingPlan>,
    pub budget: BudgetUsage,
    pub delivery: Option<DeliveryRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct RevisionInfo {
    #[specta(type = i32)]
    pub revision: i64,
    pub commit_sha: Option<String>,
    pub stat: Option<DiffStat>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffStat {
    #[specta(type = i32)]
    pub files: i64,
    #[specta(type = i32)]
    pub insertions: i64,
    #[specta(type = i32)]
    pub deletions: i64,
    pub flagged: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub task_id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub role: RunRole,
    pub agent: Option<AgentKind>,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub cost_usd: Option<f64>,
    #[specta(type = Option<i32>)]
    pub tokens_in: Option<i64>,
    #[specta(type = Option<i32>)]
    pub tokens_out: Option<i64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffPayload {
    pub base_commit: String,
    pub commit_sha: String,
    pub diff_sha256: String,
    pub truncated: bool,
    pub files: Vec<FileDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub binary: bool,
    pub flagged: bool,
    #[specta(type = i32)]
    pub insertions: i64,
    #[specta(type = i32)]
    pub deletions: i64,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub commit_sha: String,
    pub decision: ReviewDecision,
    pub summary: Option<String>,
    #[serde(default)]
    pub reviewer_agents: Vec<AgentKind>,
    pub issues: Vec<ReviewIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    pub id: String,
    pub severity: Severity,
    pub file: Option<String>,
    #[specta(type = Option<i32>)]
    pub line_start: Option<i64>,
    #[specta(type = Option<i32>)]
    pub line_end: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub suggested_action: Option<String>,
    pub resolved: bool,
    #[serde(default)]
    pub reported_by: Vec<AgentKind>,
    #[specta(type = i32)]
    pub agreement_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub compatible: bool,
    pub problem: Option<String>,
    pub authenticated: Option<bool>,
    /// Normalized source such as `account`, `api_key`, or `oauth_token`.
    pub auth_method: Option<String>,
    pub auth_problem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct EnvReport {
    pub git: ToolStatus,
    pub claude_code: ToolStatus,
    pub codex: ToolStatus,
    pub gemini_cli: ToolStatus,
    pub qwen_code: ToolStatus,
    pub grok_cli: ToolStatus,
    pub kimi_cli: ToolStatus,
    pub minimax_cli: ToolStatus,
    pub openai_api: ProviderStatus,
    pub anthropic_api: ProviderStatus,
    pub deepseek_api: ProviderStatus,
    pub grok_api: ProviderStatus,
    pub minimax_api: ProviderStatus,
    pub kimi_api: ProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingReport {
    pub first_run: bool,
    pub daemon_running: bool,
    pub ready: bool,
    pub data_dir: String,
    pub env: EnvReport,
    pub recommended_developer: Option<AgentKind>,
    pub recommended_reviewer: Option<AgentKind>,
    pub notices: Vec<String>,
    pub storage: StorageReport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageReport {
    pub data_dir: String,
    #[specta(type = f64)]
    pub total_bytes: u64,
    #[specta(type = f64)]
    pub database_bytes: u64,
    #[specta(type = f64)]
    pub task_runtime_bytes: u64,
    #[specta(type = f64)]
    pub artifact_bytes: u64,
    #[specta(type = f64)]
    pub log_bytes: u64,
    #[specta(type = f64)]
    pub cache_bytes: u64,
    #[specta(type = f64)]
    pub trash_bytes: u64,
    #[specta(type = u32)]
    pub trash_entries: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    #[specta(type = f64)]
    pub files_removed: u64,
    #[specta(type = f64)]
    pub bytes_reclaimed: u64,
    pub tasks_trashed: u32,
    pub tasks_purged: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrashEntry {
    pub task_id: String,
    pub title: String,
    pub trashed_at: String,
    pub purge_after: String,
    #[specta(type = f64)]
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub configured: bool,
    pub available: bool,
    pub model: String,
    pub base_url: String,
    pub key_env: String,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    #[specta(type = i32)]
    pub id: i64,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    #[specta(type = Option<i32>)]
    pub revision: Option<i64>,
    pub actor: Actor,
    pub event_type: String,
    #[specta(type = specta_typescript::Unknown)]
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub ts: String,
    pub stream: EventStream,
    pub kind: AgentEventKind,
    pub summary: String,
    pub text: Option<String>,
}

string_enum!(EventStream { Stdout => "stdout", Stderr => "stderr" });
string_enum!(AgentEventKind { System => "system", AssistantText => "assistant_text", ToolUse => "tool_use", ToolResult => "tool_result", Result => "result", Raw => "raw" });

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentResult {
    pub schema_version: u8,
    pub task_id: String,
    pub revision: i64,
    pub status: DevelopmentStatus,
    pub summary: String,
    pub question: Option<String>,
    pub changed_files: Option<Vec<String>>,
    pub notes: Option<String>,
}
string_enum!(DevelopmentStatus { Completed => "completed", Failed => "failed", NeedsClarification => "needs_clarification" });

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewResult {
    pub schema_version: u8,
    pub task_id: String,
    pub revision: i64,
    pub commit_sha: String,
    pub decision: ReviewDecision,
    pub summary: String,
    pub issues: Vec<ReviewIssueResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewIssueResult {
    pub severity: Severity,
    pub file: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
}

string_enum!(ErrorCode {
    EnvCliNotFound => "ENV_CLI_NOT_FOUND", EnvCliIncompatible => "ENV_CLI_INCOMPATIBLE",
    CliInstallFailed => "CLI_INSTALL_FAILED", ApiCredentialFailed => "API_CREDENTIAL_FAILED",
    ApiEgressApprovalRequired => "API_EGRESS_APPROVAL_REQUIRED",
    ProjectNotGit => "PROJECT_NOT_GIT", ProjectAlreadyImported => "PROJECT_ALREADY_IMPORTED",
    TaskInvalidState => "TASK_INVALID_STATE", TaskSameAgent => "TASK_SAME_AGENT",
    RunSpawnFailed => "RUN_SPAWN_FAILED", ResultInvalidSchema => "RESULT_INVALID_SCHEMA",
    DiffStale => "DIFF_STALE", MergePreconditionFailed => "MERGE_PRECONDITION_FAILED",
    MergeConflict => "MERGE_CONFLICT", WorktreeMissing => "WORKTREE_MISSING", DbError => "DB_ERROR",
    PlanApprovalRequired => "PLAN_APPROVAL_REQUIRED", BudgetExceeded => "BUDGET_EXCEEDED",
    QualityGateFailed => "QUALITY_GATE_FAILED", ScmCliNotFound => "SCM_CLI_NOT_FOUND",
    CiFailed => "CI_FAILED", RemoteNodeUnavailable => "REMOTE_NODE_UNAVAILABLE",
    RollbackUnsafe => "ROLLBACK_UNSAFE", IoError => "IO_ERROR", Internal => "INTERNAL"
});

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct GlobalSettings {
    pub max_concurrent_runs: Option<u32>,
    #[specta(type = Option<u32>)]
    pub developer_timeout_secs: Option<u64>,
    #[specta(type = Option<u32>)]
    pub reviewer_timeout_secs: Option<u64>,
    #[specta(type = Option<u32>)]
    pub idle_timeout_secs: Option<u64>,
    pub storage: StoragePolicy,
    pub notifications: NotificationSettings,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            max_concurrent_runs: Some(2),
            developer_timeout_secs: Some(1_800),
            reviewer_timeout_secs: Some(900),
            idle_timeout_secs: Some(300),
            storage: StoragePolicy::default(),
            notifications: NotificationSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct StoragePolicy {
    pub auto_cleanup: bool,
    pub raw_logs_days: u32,
    pub trash_days: u32,
    #[specta(type = f64)]
    pub cache_max_bytes: u64,
}

impl Default for StoragePolicy {
    fn default() -> Self {
        Self {
            auto_cleanup: true,
            raw_logs_days: 14,
            trash_days: 7,
            cache_max_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub on_attention: bool,
    pub on_completion: bool,
    pub on_fallback: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            on_attention: true,
            on_completion: true,
            on_fallback: true,
        }
    }
}

pub fn development_result_schema() -> Value {
    let mut value = serde_json::to_value(schemars::schema_for!(DevelopmentResult))
        .unwrap_or_else(|_| serde_json::json!({}));
    harden_schema(&mut value, false);
    value
}

pub fn review_result_schema() -> Value {
    let mut value = serde_json::to_value(schemars::schema_for!(ReviewResult))
        .unwrap_or_else(|_| serde_json::json!({}));
    harden_schema(&mut value, true);
    value
}

fn harden_schema(value: &mut Value, review: bool) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    root.insert("additionalProperties".into(), Value::Bool(false));
    if let Some(properties) = root.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert("schema_version".into(), serde_json::json!({"const":1}));
        if let Some(revision) = properties
            .get_mut("revision")
            .and_then(Value::as_object_mut)
        {
            revision.insert("minimum".into(), Value::from(1));
        }
        if let Some(summary) = properties.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("maxLength".into(), Value::from(4000));
            if !review {
                summary.insert("minLength".into(), Value::from(1));
            }
        }
        if review {
            if let Some(commit) = properties
                .get_mut("commit_sha")
                .and_then(Value::as_object_mut)
            {
                commit.insert("minLength".into(), Value::from(7));
            }
        } else {
            for key in ["question", "notes"] {
                set_optional_type(properties, key, "string");
            }
            set_optional_type(properties, "changed_files", "array");
        }
    }
    if review
        && let Some(issue) = root
            .get_mut("$defs")
            .and_then(Value::as_object_mut)
            .and_then(|defs| defs.get_mut("ReviewIssueResult"))
            .and_then(Value::as_object_mut)
    {
        issue.insert("additionalProperties".into(), Value::Bool(false));
        if let Some(properties) = issue.get_mut("properties").and_then(Value::as_object_mut) {
            for key in ["file", "description", "suggested_action"] {
                set_optional_type(properties, key, "string");
            }
            for key in ["line_start", "line_end"] {
                set_optional_type(properties, key, "integer");
                if let Some(field) = properties.get_mut(key).and_then(Value::as_object_mut) {
                    field.insert("minimum".into(), Value::from(1));
                }
            }
            if let Some(title) = properties.get_mut("title").and_then(Value::as_object_mut) {
                title.insert("minLength".into(), Value::from(1));
                title.insert("maxLength".into(), Value::from(200));
            }
            for (key, max) in [("description", 4000), ("suggested_action", 2000)] {
                if let Some(field) = properties.get_mut(key).and_then(Value::as_object_mut) {
                    field.insert("maxLength".into(), Value::from(max));
                }
            }
        }
    }
}

fn set_optional_type(properties: &mut serde_json::Map<String, Value>, key: &str, kind: &str) {
    if let Some(field) = properties.get_mut(key).and_then(Value::as_object_mut) {
        field.insert("type".into(), Value::String(kind.into()));
    }
}
