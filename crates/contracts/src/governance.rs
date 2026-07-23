string_enum!(PlanStatus { Pending => "pending", Approved => "approved", Rejected => "rejected" });
string_enum!(DeliveryMode { LocalMerge => "local_merge", GitHubPr => "github_pr", GitLabMr => "gitlab_mr" });
string_enum!(DeliveryState { Pending => "pending", Open => "open", CiRunning => "ci_running", Ready => "ready", Merged => "merged", Failed => "failed", RolledBack => "rolled_back" });
string_enum!(CiStatus { Unknown => "unknown", Pending => "pending", Passed => "passed", Failed => "failed" });
string_enum!(CiCheckStatus {
    Pending => "pending",
    Passed => "passed",
    Failed => "failed",
    Cancelled => "cancelled",
    Skipped => "skipped",
    Unknown => "unknown"
});
string_enum!(RollbackStrategy { Undo => "undo", Revert => "revert" });
string_enum!(RollbackBlocker {
    TaskNotMerged => "task_not_merged",
    DeliveryRecordMissing => "delivery_record_missing",
    RemoteDeliveryUnsupported => "remote_delivery_unsupported",
    TargetBranchNotCheckedOut => "target_branch_not_checked_out",
    DirtyWorkingTree => "dirty_working_tree",
    MergeCommitMissing => "merge_commit_missing",
    PreMergeCommitMissing => "pre_merge_commit_missing",
    LaterCommitsExist => "later_commits_exist",
    MergeNotInHead => "merge_not_in_head"
});
string_enum!(RollbackRecommendationReason {
    UndoExactHead => "undo_exact_head",
    RevertPreservesLaterCommits => "revert_preserves_later_commits",
    NoSafeStrategy => "no_safe_strategy"
});
string_enum!(NodeStatus { Unknown => "unknown", Online => "online", Offline => "offline" });
string_enum!(NodeDiagnosticStatus { Passed => "passed", Failed => "failed", Skipped => "skipped" });
string_enum!(NodeDiagnosticStep {
    Dns => "dns",
    Tcp => "tcp",
    SshAuthentication => "ssh_authentication",
    WorkRoot => "work_root",
    Platform => "platform",
    Git => "git",
    ArchiveTool => "archive_tool",
    Toolchain => "toolchain"
});
string_enum!(QualityGrade { A => "A", B => "B", C => "C", D => "D" });
// Whether a budget is enforced before/during a Provider run or only reconciled
// after the Provider exits. The desktop must not present `soft` as a guarantee.
string_enum!(BudgetEnforcement { Hard => "hard", Soft => "soft", Unavailable => "unavailable" });
string_enum!(ReproducibilityLevel { #[default] FixedCommit => "fixed_commit", EnvironmentLocked => "environment_locked", Hermetic => "hermetic" });
string_enum!(QualityReplayStatus {
    Succeeded => "succeeded",
    ValidationFailed => "validation_failed",
    DriftBlocked => "drift_blocked",
    Failed => "failed"
});
string_enum!(ReproducibilityDriftKind {
    ToolVersions => "tool_versions",
    EnvironmentVariables => "environment_variables",
    SystemDependencies => "system_dependencies",
    ContainerImages => "container_images",
    GitSubmodules => "git_submodules",
    GitLfsObjects => "git_lfs_objects",
    ExternalDependencies => "external_dependencies"
});

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct TaskPolicy {
    pub require_plan_approval: bool,
    /// Scheduler order from -100 (background) to 100 (urgent).
    #[specta(type = i32)]
    pub priority: i16,
    #[specta(type = Option<i32>)]
    pub token_budget: Option<i64>,
    pub cost_budget_usd: Option<f64>,
    #[specta(type = Option<i32>)]
    pub time_budget_secs: Option<i64>,
    pub minimum_quality_score: u8,
    pub delivery_mode: DeliveryMode,
    pub execution_node_id: Option<String>,
}

impl Default for TaskPolicy {
    fn default() -> Self {
        Self {
            require_plan_approval: true,
            priority: 0,
            token_budget: Some(500_000),
            cost_budget_usd: Some(25.0),
            time_budget_secs: Some(7_200),
            minimum_quality_score: 70,
            delivery_mode: DeliveryMode::LocalMerge,
            execution_node_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub title: String,
    pub detail: String,
    pub validation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct CodingPlan {
    pub id: String,
    #[specta(type = i32)]
    pub version: i64,
    pub status: PlanStatus,
    pub summary: String,
    pub steps: Vec<PlanStep>,
    pub risks: Vec<String>,
    pub allowed_paths: Vec<String>,
    pub plan_sha256: Option<String>,
    pub created_at: String,
    pub approved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct CodingPlanHistoryEntry {
    pub plan: CodingPlan,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanVersionDiff {
    #[specta(type = i32)]
    pub from_version: i64,
    #[specta(type = i32)]
    pub to_version: i64,
    pub summary_changed: bool,
    pub added_steps: Vec<String>,
    pub removed_steps: Vec<String>,
    pub added_allowed_paths: Vec<String>,
    pub removed_allowed_paths: Vec<String>,
    pub added_risks: Vec<String>,
    pub removed_risks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanReviewContext {
    pub task_id: String,
    pub plans: Vec<CodingPlanHistoryEntry>,
    pub latest_diff: Option<PlanVersionDiff>,
    pub detected_deviations: Vec<String>,
    pub deviation_plan_id: Option<String>,
    pub deviation_detected_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanResult {
    pub schema_version: u8,
    pub task_id: String,
    pub plan_version: i64,
    pub summary: String,
    pub steps: Vec<PlanStep>,
    pub risks: Vec<String>,
    /// Repository-relative glob patterns that define the approved implementation boundary.
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetUsage {
    #[specta(type = i32)]
    pub tokens_used: i64,
    pub cost_usd: f64,
    #[specta(type = i32)]
    pub time_used_secs: i64,
    #[specta(type = Option<i32>)]
    pub token_budget: Option<i64>,
    pub cost_budget_usd: Option<f64>,
    #[specta(type = Option<i32>)]
    pub time_budget_secs: Option<i64>,
    /// Unknown usage is tracked explicitly instead of silently becoming zero.
    pub tokens_known: bool,
    pub cost_known: bool,
    #[specta(type = i32)]
    pub unknown_token_runs: i64,
    #[specta(type = i32)]
    pub unknown_cost_runs: i64,
    #[specta(type = i32)]
    pub tokens_reserved: i64,
    pub cost_reserved_usd: f64,
    pub token_enforcement: BudgetEnforcement,
    pub cost_enforcement: BudgetEnforcement,
    pub exceeded: bool,
}

/// Replacement limits submitted after a budget stop. `None` means unlimited;
/// callers must send all three values so resuming never depends on stale UI state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetLimitPatch {
    #[specta(type = Option<i32>)]
    pub token_budget: Option<i64>,
    pub cost_budget_usd: Option<f64>,
    #[specta(type = Option<i32>)]
    pub time_budget_secs: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct GitCompatibilityReport {
    pub repo_path: String,
    pub repository_identity: String,
    pub shallow: bool,
    pub sparse_checkout: bool,
    pub sparse_patterns: Vec<String>,
    pub submodules: Vec<String>,
    pub lfs_tracked: bool,
    pub lfs_available: bool,
    pub ssh_remote: bool,
    pub ssh_agent_available: bool,
    pub network_filesystem: bool,
    pub case_insensitive: bool,
    pub case_collisions: Vec<String>,
    /// Paths whose Git registration is explicitly marked `prunable`. Running `git worktree prune`
    /// removes only these stale registrations; it never deletes a live project directory.
    pub prunable_worktrees: Vec<String>,
    pub repo_readable: bool,
    pub repo_writable: bool,
    pub worktree_root_writable: bool,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReproducibilityManifest {
    pub task_id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub commit_sha: String,
    pub manifest_sha256: String,
    pub environment: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub reproducibility_level: ReproducibilityLevel,
    #[serde(default)]
    pub tool_versions: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub environment_variables: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub system_dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub container_image_digests: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub git_submodules: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub git_lfs_objects: Vec<String>,
    #[serde(default)]
    pub external_dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub limitations: Vec<String>,
    #[serde(default)]
    pub environment_sha256: String,
    pub input_sha256: String,
    pub patch_sha256: String,
    pub validation_config_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct QualityCheck {
    pub name: String,
    pub passed: bool,
    pub weight: u8,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct QualityEvaluation {
    pub task_id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub score: u8,
    pub grade: QualityGrade,
    pub passed: bool,
    pub replay: bool,
    pub checks: Vec<QualityCheck>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReproducibilityDrift {
    pub kind: ReproducibilityDriftKind,
    /// Names only. Environment values and credentials are never exposed.
    pub changed_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct QualityReplayAttempt {
    pub id: String,
    pub task_id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub status: QualityReplayStatus,
    pub reproducibility_level: ReproducibilityLevel,
    pub environment_match: bool,
    pub drift: Vec<ReproducibilityDrift>,
    pub original_quality: Option<QualityEvaluation>,
    pub replay_quality: Option<QualityEvaluation>,
    pub score_delta: Option<i32>,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub created_at: String,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryRecord {
    pub mode: DeliveryMode,
    pub state: DeliveryState,
    pub remote_url: Option<String>,
    #[specta(type = Option<i32>)]
    pub number: Option<i64>,
    pub ci_status: Option<CiStatus>,
    #[serde(default)]
    pub ci_checks: Vec<CiCheck>,
    pub merge_commit: Option<String>,
    pub pre_merge_commit: Option<String>,
    pub rollback_commit: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct CiCheck {
    pub name: String,
    pub status: CiCheckStatus,
    pub required: bool,
    pub workflow: Option<String>,
    pub description: Option<String>,
    pub failure_summary: Option<String>,
    pub details_url: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Read-only safety analysis captured immediately before a rollback choice.
/// Execution repeats the same checks so a stale dialog can never bypass Git safety.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct RollbackPreflight {
    pub task_id: String,
    pub delivery_mode: DeliveryMode,
    pub target_branch: String,
    pub current_branch: Option<String>,
    pub head_commit: Option<String>,
    pub merge_commit: Option<String>,
    pub pre_merge_commit: Option<String>,
    pub working_tree_clean: bool,
    #[specta(type = i32)]
    pub later_commit_count: i64,
    pub later_commits: Vec<RollbackCommitPreview>,
    #[specta(type = i32)]
    pub affected_file_count: i64,
    pub affected_files: Vec<String>,
    pub affected_files_truncated: bool,
    pub can_undo: bool,
    pub undo_blockers: Vec<RollbackBlocker>,
    pub can_revert: bool,
    pub revert_blockers: Vec<RollbackBlocker>,
    pub recommended_strategy: Option<RollbackStrategy>,
    pub recommendation_reason: RollbackRecommendationReason,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct RollbackCommitPreview {
    pub sha: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskGovernance {
    pub manifest: Option<ReproducibilityManifest>,
    pub quality: Option<QualityEvaluation>,
    pub original_quality: Option<QualityEvaluation>,
    pub latest_replay: Option<QualityReplayAttempt>,
    pub budget: BudgetUsage,
    pub delivery: Option<DeliveryRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionNode {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub work_root: String,
    pub enabled: bool,
    pub status: NodeStatus,
    pub platform: Option<String>,
    pub git_version: Option<String>,
    pub problem: Option<String>,
    pub last_checked_at: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<ExecutionNodeDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionNodeDiagnostic {
    pub step: NodeDiagnosticStep,
    pub status: NodeDiagnosticStatus,
    pub blocking: bool,
    pub summary: String,
    pub detail: Option<String>,
    pub duration_ms: u32,
    pub checked_at: String,
}

pub fn plan_result_schema() -> Value {
    let mut value = serde_json::to_value(schemars::schema_for!(PlanResult))
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(root) = value.as_object_mut() {
        root.insert("additionalProperties".into(), Value::Bool(false));
        if let Some(properties) = root.get_mut("properties").and_then(Value::as_object_mut) {
            properties.insert(
                "schema_version".into(),
                serde_json::json!({"type":"integer", "const":1}),
            );
            if let Some(version) = properties.get_mut("plan_version").and_then(Value::as_object_mut) {
                version.insert("minimum".into(), Value::from(1));
            }
            if let Some(summary) = properties.get_mut("summary").and_then(Value::as_object_mut) {
                summary.insert("minLength".into(), Value::from(1));
                summary.insert("maxLength".into(), Value::from(4000));
            }
        }
        if let Some(step) = root.get_mut("$defs").and_then(Value::as_object_mut)
            .and_then(|defs| defs.get_mut("PlanStep")).and_then(Value::as_object_mut) {
            step.insert("additionalProperties".into(), Value::Bool(false));
        }
    }
    require_all_object_properties(&mut value);
    value
}
