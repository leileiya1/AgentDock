use agentflow_contracts::{
    AgentEvent, AgentEventKind, AgentKind, ApiProviderSettings, DevelopmentResult,
    DevelopmentStatus, EventStream, PlanResult, ProviderStatus, ReviewDecision, ReviewResult,
    RunRole, ToolStatus, development_result_schema, plan_result_schema, review_result_schema,
};
use agentflow_process_supervisor::{ProcessOutcome, ProcessSpec};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use thiserror::Error;
use tokio::{process::Command, sync::mpsc};
use tokio_util::sync::CancellationToken;

mod credentials;
mod dynamic;
use credentials::cli_credential_env;
pub use dynamic::{ExternalProviderAdapter, UnavailableProviderAdapter};

#[derive(Debug, Clone, Default)]
pub struct CliEnv {
    pub explicit_path: Option<PathBuf>,
}
#[derive(Debug, Clone)]
pub struct AgentCapabilities {
    pub streams_events: bool,
    pub native_output_schema: bool,
    pub supports_resume: bool,
    pub read_only_mode: bool,
    pub supports_development: bool,
    pub supports_review: bool,
}
#[derive(Debug, Clone)]
pub struct AgentInstallation {
    pub path: PathBuf,
    pub version: String,
    pub capabilities: AgentCapabilities,
}
#[derive(Debug, Clone, Copy)]
pub enum PermissionTier {
    Normal,
    /// Contract/result repair may inspect the worktree but cannot change it.
    ReadOnly,
    Yolo,
}
#[derive(Debug, Clone)]
pub struct AgentRunRequest {
    /// Authoritative identity supplied by the orchestrator. Providers must echo it
    /// in structured results instead of trying to recover it from the prompt.
    pub task_id: String,
    pub revision: i64,
    /// Present for review runs so a Provider can bind its verdict to one commit.
    pub commit_sha: Option<String>,
    pub worktree: PathBuf,
    pub run_dir: PathBuf,
    pub role: RunRole,
    pub input_file: String,
    pub timeout: Duration,
    pub idle_timeout: Duration,
    pub permission: PermissionTier,
    /// Optional opaque Provider session token. Artifact history remains authoritative;
    /// this is supplied only when the user explicitly enables session reuse.
    pub resume_session_id: Option<String>,
    pub extra_allowed_commands: Vec<String>,
    pub env_denylist: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct RunningAgent {
    pub outcome: ProcessOutcome,
    pub run_dir: PathBuf,
    pub role: RunRole,
}
#[derive(Debug, Clone)]
pub enum CollectedResult {
    Plan(PlanResult),
    Development(DevelopmentResult),
    Review(ReviewResult),
}
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("CLI not found: {0}")]
    NotFound(String),
    #[error("CLI incompatible: {0}")]
    Incompatible(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("process error: {0}")]
    Process(#[from] agentflow_process_supervisor::SupervisorError),
    #[error("invalid result: {0}")]
    InvalidResult(String),
    #[error("unsupported role {0}")]
    UnsupportedRole(RunRole),
    #[error("provider {provider} failed ({status:?}): {message}")]
    Provider {
        provider: AgentKind,
        status: Option<u16>,
        message: String,
        retryable: bool,
    },
}

#[async_trait]
pub trait AgentProvider: Send + Sync {
    fn kind(&self) -> AgentKind;
    async fn detect(&self, env: &CliEnv) -> Result<AgentInstallation, AdapterError>;
    fn capabilities(&self) -> AgentCapabilities;
    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError>;
    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError>;
}

pub use AgentProvider as AgentAdapter;

#[derive(Debug, Clone)]
pub struct ClaudeCodeAdapter {
    executable: PathBuf,
}
impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self {
            executable: "claude".into(),
        }
    }
}
impl ClaudeCodeAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct CodexAdapter {
    executable: PathBuf,
    schema_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GeminiCliAdapter {
    executable: PathBuf,
}

impl GeminiCliAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QwenCodeAdapter {
    executable: PathBuf,
    schema_path: PathBuf,
}

impl QwenCodeAdapter {
    pub fn new(executable: impl Into<PathBuf>, schema_path: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            schema_path: schema_path.into(),
        }
    }
}
impl CodexAdapter {
    pub fn new(executable: impl Into<PathBuf>, schema_path: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            schema_path: schema_path.into(),
        }
    }
}

// Kept as same-module includes so shared adapter types remain private while each concern stays small.
include!("cli_providers.rs");
include!("api_provider.rs");
include!("support.rs");
include!("tests.rs");
