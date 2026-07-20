/// Stable Provider identifier used by tasks, persistence and the external protocol.
///
/// Known variants keep ergonomic Rust matching. `External` deliberately serializes as the raw
/// provider id so new sidecars can be installed without changing the AgentFlow core or database.
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema, Type)]
#[schemars(with = "String")]
#[specta(type = String)]
pub enum AgentKind {
    ClaudeCode,
    Codex,
    GeminiCli,
    QwenCode,
    GrokCli,
    KimiCli,
    MiniMaxCli,
    OpenAiApi,
    AnthropicApi,
    DeepSeekApi,
    GrokApi,
    MiniMaxApi,
    KimiApi,
    External(String),
}

impl AgentKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::GeminiCli => "gemini_cli",
            Self::QwenCode => "qwen_code",
            Self::GrokCli => "grok_cli",
            Self::KimiCli => "kimi_cli",
            Self::MiniMaxCli => "minimax_cli",
            Self::OpenAiApi => "openai_api",
            Self::AnthropicApi => "anthropic_api",
            Self::DeepSeekApi => "deepseek_api",
            Self::GrokApi => "grok_api",
            Self::MiniMaxApi => "minimax_api",
            Self::KimiApi => "kimi_api",
            Self::External(id) => id,
        }
    }

    pub fn is_api(&self) -> bool {
        matches!(
            self,
            Self::OpenAiApi
                | Self::AnthropicApi
                | Self::DeepSeekApi
                | Self::GrokApi
                | Self::MiniMaxApi
                | Self::KimiApi
        )
    }
}

impl std::fmt::Display for AgentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AgentKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "claude_code" => Self::ClaudeCode,
            "codex" => Self::Codex,
            "gemini_cli" => Self::GeminiCli,
            "qwen_code" => Self::QwenCode,
            "grok_cli" => Self::GrokCli,
            "kimi_cli" => Self::KimiCli,
            "minimax_cli" => Self::MiniMaxCli,
            "openai_api" => Self::OpenAiApi,
            "anthropic_api" => Self::AnthropicApi,
            "deepseek_api" => Self::DeepSeekApi,
            "grok_api" => Self::GrokApi,
            "minimax_api" => Self::MiniMaxApi,
            "kimi_api" => Self::KimiApi,
            custom if valid_provider_id(custom) => Self::External(custom.into()),
            _ => return Err(format!("invalid provider id: {value}")),
        })
    }
}

impl Serialize for AgentKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgentKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

fn valid_provider_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    (2..=64).contains(&value.len())
        && bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.')
        })
}
string_enum!(RunRole { Planner => "planner", Developer => "developer", Reviewer => "reviewer", Validator => "validator" });
string_enum!(TaskStatus {
    Draft => "DRAFT", Planning => "PLANNING", WaitingForPlanApproval => "WAITING_FOR_PLAN_APPROVAL",
    ReadyForDevelopment => "READY_FOR_DEVELOPMENT", Developing => "DEVELOPING",
    Validating => "VALIDATING", ReadyForReview => "READY_FOR_REVIEW", Reviewing => "REVIEWING",
    ReadyForRevision => "READY_FOR_REVISION", Revising => "REVISING",
    WaitingForHumanApproval => "WAITING_FOR_HUMAN_APPROVAL", Approved => "APPROVED",
    Merging => "MERGING", MergeConflict => "MERGE_CONFLICT", Merged => "MERGED", RolledBack => "ROLLED_BACK",
    Blocked => "BLOCKED", Cancelled => "CANCELLED"
});
string_enum!(RunStatus { Running => "RUNNING", Succeeded => "SUCCEEDED", Failed => "FAILED", TimedOut => "TIMED_OUT", Cancelled => "CANCELLED", Interrupted => "INTERRUPTED" });
string_enum!(BlockedReason {
    NoChanges => "no_changes", NeedsClarification => "needs_clarification", RunFailed => "run_failed",
    ValidationInfra => "validation_infra", ReviewBlock => "review_block", ReviewFailed => "review_failed",
    MaxRevisions => "max_revisions", WorktreeMissing => "worktree_missing",
    CommitGuard => "commit_guard", BudgetExceeded => "budget_exceeded",
    RemoteNodeUnavailable => "remote_node_unavailable", CiFailed => "ci_failed",
    QualityGate => "quality_gate"
});
string_enum!(ReviewDecision { Pass => "pass", RequestChanges => "request_changes", Block => "block" });
string_enum!(Severity { Critical => "critical", High => "high", Medium => "medium", Low => "low" });
string_enum!(Actor { Orchestrator => "orchestrator", Agent => "agent", Human => "human", System => "system" });
string_enum!(StorageCleanupScope {
    Logs => "logs", Runtime => "runtime", Everything => "everything"
});

string_enum!(ProviderSource { Builtin => "builtin", External => "external" });
string_enum!(ExecutionLocation { Local => "local", Remote => "remote", Hybrid => "hybrid" });
string_enum!(DataEgress { None => "none", Metadata => "metadata", Diff => "diff", FullFiles => "full_files" });
string_enum!(ProviderTrust { Builtin => "builtin", Verified => "verified", Quarantined => "quarantined" });

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPermissions {
    pub worktree_read: bool,
    pub worktree_write: bool,
    #[serde(default)]
    pub network_domains: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub development: bool,
    pub review: bool,
    pub streaming: bool,
    pub structured_output: bool,
    pub sandbox: bool,
    pub resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: AgentKind,
    pub display_name: String,
    pub source: ProviderSource,
    pub protocol_version: String,
    pub capabilities: ProviderCapabilities,
    pub execution_location: ExecutionLocation,
    pub data_egress: DataEgress,
    pub permissions: ProviderPermissions,
    pub trust: ProviderTrust,
    pub available: bool,
    pub problem: Option<String>,
}

impl ProviderDescriptor {
    pub fn requires_egress_approval(&self) -> bool {
        self.execution_location != ExecutionLocation::Local
            || self.data_egress != DataEgress::None
            || !self.permissions.network_domains.is_empty()
    }
}
