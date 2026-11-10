// Opt-in team protocol. Legacy TaskDetail and sidecar protocol remain unchanged.
string_enum!(TeamRole { Planner => "planner", Developer => "developer", Tester => "tester", Reviewer => "reviewer" });
string_enum!(TeamWorkKind { Plan => "plan", Coverage => "coverage", Implement => "implement", Integrate => "integrate", Verify => "verify", Review => "review" });
string_enum!(TeamWorkState { Pending => "pending", Active => "active", Submitted => "submitted", Accepted => "output_accepted", Blocked => "blocked", Cancelled => "cancelled", Superseded => "superseded" });
string_enum!(TeamPhase { Draft => "draft", Planning => "planning", PlanApproval => "plan_approval", Running => "running", Paused => "paused", Blocked => "blocked", ReadyForDelivery => "ready_for_delivery", Delivering => "delivering", Delivered => "delivered", Cancelled => "cancelled" });
string_enum!(CriterionStatus { NotRun => "not_run", Passed => "passed", Failed => "failed", Blocked => "blocked", Inconclusive => "inconclusive" });

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamMember {
    pub id: String,
    pub name: String,
    pub roles: Vec<TeamRole>,
    /// Ordered, task-owned profile snapshot. Replacement preserves this logical seat.
    pub providers: Vec<AgentKind>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamConfig {
    pub team_capacity: u32,
    pub max_concurrent_attempts: u32,
    pub planner_member_id: String,
    pub members: Vec<TeamMember>,
    pub max_attempts: u32,
    pub verification_reserve: u32,
    pub max_reworks: u32,
    pub max_plan_revisions: u32,
    pub attempt_timeout_secs: u32,
    /// Actual OS/sandbox guarantees come from the adapter, not this path policy.
    pub write_globs: Vec<String>,
    /// Command prefixes explicitly authorized by the user. No implicit shell.
    pub allowed_commands: Vec<Vec<String>>,
    pub require_plan_approval: bool,
    pub cross_vendor_review: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamCreateInput {
    pub project_id: String,
    pub title: String,
    pub goal: String,
    pub requirements: Vec<String>,
    pub target_branch: Option<String>,
    pub config: TeamConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamRequirement {
    pub id: String,
    pub original_text: String,
    pub source: String,
    pub required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamCheck {
    pub id: String,
    pub argv: Vec<String>,
    pub expected_exit: i32,
    pub stdout_contains: Option<String>,
    pub timeout_secs: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamCriterion {
    pub id: String,
    pub text: String,
    pub requirement_ids: Vec<String>,
    pub required: bool,
    pub expected: String,
    /// A versioned executable test package. Each check is run separately by the Runner.
    pub checks: Vec<TeamCheck>,
    pub owner_work_item_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamWorkSpec {
    pub id: String,
    pub title: String,
    pub instructions: String,
    pub kind: TeamWorkKind,
    pub depends_on: Vec<String>,
    pub member_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub write_globs: Vec<String>,
    pub local_checks: Vec<TeamCheck>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamPlan {
    pub schema_version: u32,
    pub rationale: String,
    /// User-owned requirements are immutable; a plan refers to their IDs.
    pub items: Vec<TeamWorkSpec>,
    pub criteria: Vec<TeamCriterion>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamIdentity {
    pub task_id: String,
    pub work_item_id: String,
    pub attempt_id: String,
    pub member_id: String,
    pub spec_hash: String,
    pub input_manifest_hash: String,
    pub lease_epoch: u32,
}
/// Carried in DevelopmentResult.notes as JSON. Transport success never accepts an artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamSubmission {
    pub schema_version: u32,
    pub identity: TeamIdentity,
    pub summary: String,
    pub plan: Option<TeamPlan>,
    pub approved: bool,
    pub changed_files: Vec<String>,
    /// Tester selects exact check IDs; absent checks remain not_run.
    pub check_ids: Vec<String>,
    pub issues: Vec<String>,
    /// CLI-local tool limits are observations, not product findings or passing evidence.
    #[serde(default)]
    pub environment_notes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamWorkView {
    pub spec: TeamWorkSpec,
    pub spec_hash: String,
    pub state: TeamWorkState,
    pub epoch: u32,
    pub artifact_id: Option<String>,
    pub detail: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamAttemptView {
    pub identity: TeamIdentity,
    pub provider: AgentKind,
    pub status: RunStatus,
    pub worktree: Option<String>,
    pub input_base_sha: Option<String>,
    pub failure_class: Option<String>,
    pub cost_usd: Option<f64>,
    #[specta(type = Option<i32>)]
    pub tokens_in: Option<i64>,
    #[specta(type = Option<i32>)]
    pub tokens_out: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamArtifact {
    pub id: String,
    pub work_item_id: String,
    pub attempt_id: String,
    pub member_id: String,
    pub provider: AgentKind,
    pub input_base_sha: String,
    pub result_sha: String,
    pub patch_sha256: String,
    pub changed_files: Vec<String>,
    pub included_artifact_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamCandidate {
    pub id: String,
    pub commit_sha: String,
    pub tree_sha: String,
    pub base_sha: String,
    pub artifact_ids: Vec<String>,
    pub contributor_member_ids: Vec<String>,
    pub contributor_providers: Vec<AgentKind>,
    pub plan_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamEvidence {
    pub id: String,
    pub criterion_id: String,
    pub criterion_hash: String,
    pub candidate_id: String,
    pub candidate_tree: String,
    pub attempt_id: String,
    pub member_id: String,
    pub check_id: String,
    pub check_hash: String,
    pub status: CriterionStatus,
    pub source: String,
    pub environment_hash: String,
    pub raw_path: String,
    pub raw_sha256: String,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamCriterionView {
    pub criterion: TeamCriterion,
    pub status: CriterionStatus,
    pub stale: bool,
    pub evidence: Vec<TeamEvidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamView {
    pub task_id: String,
    pub phase: TeamPhase,
    pub original_goal: String,
    pub config: TeamConfig,
    pub requirements: Vec<TeamRequirement>,
    pub plan: Option<TeamPlan>,
    pub plan_revision: u32,
    pub items: Vec<TeamWorkView>,
    pub attempts: Vec<TeamAttemptView>,
    pub artifacts: Vec<TeamArtifact>,
    pub candidate: Option<TeamCandidate>,
    pub criteria: Vec<TeamCriterionView>,
    pub gate_blockers: Vec<String>,
    pub attempts_used: u32,
    pub reworks_used: u32,
    pub detail: Option<String>,
    pub report_path: Option<String>,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TeamAction {
    /// Reap only quarantined/cancelled attempts whose old resources can be proved stopped.
    ReconcileResources,
    ReviseAcceptance { expected_revision:u32, criteria:Vec<TeamCriterion>, reason:String },
    RefreshTarget,
    Start,
    ApprovePlan {
        expected_revision: u32,
    },
    Pause,
    Resume,
    Cancel,
    CancelWork {
        work_item_id: String,
    },
    RetryWork {
        work_item_id: String,
    },
    ReplaceMember {
        member: TeamMember,
    },
    RevisePlan {
        expected_revision: u32,
        plan: TeamPlan,
    },
    Deliver,
}

string_enum!(TeamResourceState { NotStarted => "not_started", Present => "present", Stopped => "stopped", Unknown => "unknown" });
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamResourceInspection {
    pub attempt_id: String,
    pub work_item_id: String,
    pub member_id: String,
    pub provider: Option<AgentKind>,
    pub failure_class: Option<String>,
    pub cancel_requested: bool,
    pub state: TeamResourceState,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamRecoveryView {
    pub task_id: String,
    pub phase: TeamPhase,
    pub resources: Vec<TeamResourceInspection>,
    pub attempts_remaining: u32,
    pub reworks_remaining: u32,
    pub target_base: Option<String>,
    pub target_current: Option<String>,
    pub target_changed: bool,
    pub config_changed: bool,
    pub notices: Vec<String>,
    pub inspected_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamPlanPreviewInput {
    pub task_id: String,
    pub expected_revision: u32,
    pub plan: TeamPlan,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamPlanPreview {
    pub base_revision: u32,
    pub proposal_hash: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub affected: Vec<String>,
    pub preserved: Vec<String>,
    pub blocking_attempt_ids: Vec<String>,
    pub acceptance_changed: bool,
    pub blockers: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamReportExportInput {
    pub task_id: String,
    pub parent_directory: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamReportExport {
    pub directory: String,
    pub candidate_id: Option<String>,
    pub evidence_count: u32,
    pub report_sha256: String,
    pub manifest_sha256: String,
}
