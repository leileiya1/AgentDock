use agentflow_agent_adapters::{
    AgentAdapter, AgentRunRequest, ApiProviderAdapter, ClaudeCodeAdapter, CodexAdapter,
    CollectedResult, ExternalProviderAdapter, GeminiCliAdapter, PermissionTier, QwenCodeAdapter,
    UnavailableProviderAdapter, api_provider_status,
};
use agentflow_contracts::*;
use agentflow_git_engine::{Git, GitError, summarize};
use agentflow_persistence::{PersistenceError, Store};
use agentflow_provider_protocol::{PROTOCOL_VERSION, ProtocolClient, ProviderRegistry};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, RwLock},
    time::SystemTime,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{io::AsyncWriteExt, process::Command, sync::mpsc};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    Git(#[from] agentflow_git_engine::GitError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("adapter: {0}")]
    Adapter(#[from] agentflow_agent_adapters::AdapterError),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("validation infrastructure: {0}")]
    ValidationInfra(String),
    #[error("remote execution node unavailable: {0}")]
    RemoteNodeUnavailable(String),
    #[error("quality gate failed: {0}")]
    QualityGate(String),
    #[error("SCM CLI unavailable: {0}")]
    ScmCliNotFound(String),
    #[error("unsafe rollback: {0}")]
    RollbackUnsafe(String),
    #[error("stale diff")]
    DiffStale,
    #[error("merge precondition: {0}")]
    MergePrecondition(String),
    #[error("invalid project config: {0}")]
    Config(String),
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub schema_version: u8,
    #[serde(default)]
    pub validate: ValidateConfig,
    #[serde(default)]
    pub review: ReviewConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
}
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ValidateConfig {
    #[serde(default)]
    pub steps: Vec<ValidateStep>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidateStep {
    pub name: String,
    pub argv: Vec<String>,
    #[serde(default = "default_step_timeout")]
    pub timeout_secs: u64,
}
fn default_step_timeout() -> u64 {
    600
}
#[derive(Debug, Clone, Deserialize)]
pub struct ReviewConfig {
    #[serde(default = "default_excludes")]
    pub exclude_globs: Vec<String>,
    #[serde(default = "default_patch_bytes")]
    pub max_patch_bytes: usize,
}
impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            exclude_globs: default_excludes(),
            max_patch_bytes: default_patch_bytes(),
        }
    }
}
fn default_excludes() -> Vec<String> {
    [
        "*.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "bun.lockb",
        "dist/**",
        "*.min.*",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}
fn default_patch_bytes() -> usize {
    262_144
}

fn cli_descriptor(id: AgentKind, display_name: &str, status: &ToolStatus) -> ProviderDescriptor {
    let available = status.found && status.compatible && status.authenticated != Some(false);
    ProviderDescriptor {
        id,
        display_name: display_name.into(),
        source: ProviderSource::Builtin,
        protocol_version: PROTOCOL_VERSION.into(),
        capabilities: ProviderCapabilities {
            development: true,
            review: true,
            streaming: true,
            structured_output: true,
            sandbox: true,
            resume: true,
        },
        execution_location: ExecutionLocation::Local,
        data_egress: DataEgress::None,
        permissions: ProviderPermissions {
            worktree_read: true,
            worktree_write: true,
            network_domains: Vec::new(),
            commands: Vec::new(),
        },
        trust: ProviderTrust::Builtin,
        available,
        problem: status
            .problem
            .clone()
            .or_else(|| status.auth_problem.clone()),
    }
}

fn api_descriptor(
    id: AgentKind,
    display_name: &str,
    status: &ProviderStatus,
) -> ProviderDescriptor {
    ProviderDescriptor {
        id,
        display_name: display_name.into(),
        source: ProviderSource::Builtin,
        protocol_version: PROTOCOL_VERSION.into(),
        capabilities: ProviderCapabilities {
            development: false,
            review: true,
            streaming: true,
            structured_output: true,
            sandbox: true,
            resume: false,
        },
        execution_location: ExecutionLocation::Remote,
        data_egress: DataEgress::Diff,
        permissions: ProviderPermissions {
            worktree_read: false,
            worktree_write: false,
            network_domains: vec!["provider-api".into()],
            commands: Vec::new(),
        },
        trust: ProviderTrust::Builtin,
        available: status.available,
        problem: status.problem.clone(),
    }
}
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AgentsConfig {
    #[serde(default)]
    pub extra_allowed_commands: Vec<String>,
}

#[derive(Debug, Clone)]
struct TaskRow {
    id: String,
    project_id: String,
    seq: i64,
    title: String,
    description: String,
    status: TaskStatus,
    blocked_detail: Option<String>,
    developer: AgentKind,
    reviewer: AgentKind,
    target_branch: String,
    base_commit: Option<String>,
    branch: Option<String>,
    worktree_path: Option<PathBuf>,
    revision: i64,
    max_revisions: i64,
    api_egress_approved: bool,
    policy: TaskPolicy,
}
#[derive(Debug, Clone)]
struct ProjectRow {
    seq: i64,
    repo: PathBuf,
    default_branch: String,
    worktree_root: PathBuf,
    settings: ProjectSettings,
}

#[derive(Clone)]
pub struct Orchestrator {
    pub store: Store,
    git: Git,
    app_data: PathBuf,
    provider_registry: Arc<RwLock<ProviderRegistry>>,
    active_cancellations: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

// Same-module includes preserve private invariants while keeping each workflow concern reviewable.
include!("lifecycle.rs");
include!("task_creation.rs");
include!("planning.rs");
include!("development.rs");
include!("agent_run.rs");
include!("history.rs");
include!("review_council.rs");
include!("review.rs");
include!("result_repair.rs");
include!("repair.rs");
include!("governance.rs");
include!("delivery.rs");
include!("execution_nodes.rs");
include!("telemetry.rs");
include!("storage.rs");
include!("task_queries.rs");
include!("support.rs");
#[cfg(test)]
include!("test_support.rs");
include!("tests.rs");
include!("failure_tests.rs");
#[cfg(test)]
mod governance_tests;
#[cfg(test)]
mod privacy_tests;
#[cfg(test)]
mod repair_tests;
#[cfg(test)]
mod review_council_tests;
#[cfg(test)]
mod review_issue_tests;
#[cfg(test)]
mod settings_tests;
