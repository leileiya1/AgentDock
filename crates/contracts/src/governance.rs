string_enum!(PlanStatus { Pending => "pending", Approved => "approved", Rejected => "rejected" });
string_enum!(DeliveryMode { LocalMerge => "local_merge", GitHubPr => "github_pr", GitLabMr => "gitlab_mr" });
string_enum!(DeliveryState { Pending => "pending", Open => "open", CiRunning => "ci_running", Ready => "ready", Merged => "merged", Failed => "failed", RolledBack => "rolled_back" });
string_enum!(CiStatus { Unknown => "unknown", Pending => "pending", Passed => "passed", Failed => "failed" });
string_enum!(RollbackStrategy { Undo => "undo", Revert => "revert" });
string_enum!(NodeStatus { Unknown => "unknown", Online => "online", Offline => "offline" });
string_enum!(QualityGrade { A => "A", B => "B", C => "C", D => "D" });

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct TaskPolicy {
    pub require_plan_approval: bool,
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
    pub created_at: String,
    pub approved_at: Option<String>,
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReproducibilityManifest {
    pub task_id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub commit_sha: String,
    pub manifest_sha256: String,
    pub environment: std::collections::BTreeMap<String, String>,
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
pub struct DeliveryRecord {
    pub mode: DeliveryMode,
    pub state: DeliveryState,
    pub remote_url: Option<String>,
    #[specta(type = Option<i32>)]
    pub number: Option<i64>,
    pub ci_status: Option<CiStatus>,
    pub merge_commit: Option<String>,
    pub pre_merge_commit: Option<String>,
    pub rollback_commit: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskGovernance {
    pub manifest: Option<ReproducibilityManifest>,
    pub quality: Option<QualityEvaluation>,
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
}

pub fn plan_result_schema() -> Value {
    let mut value = serde_json::to_value(schemars::schema_for!(PlanResult))
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(root) = value.as_object_mut() {
        root.insert("additionalProperties".into(), Value::Bool(false));
        if let Some(properties) = root.get_mut("properties").and_then(Value::as_object_mut) {
            properties.insert("schema_version".into(), serde_json::json!({"const":1}));
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
    value
}
