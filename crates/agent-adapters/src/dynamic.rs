use super::{
    AdapterError, AgentCapabilities, AgentInstallation, AgentProvider, AgentRunRequest, CliEnv,
    CollectedResult, PermissionTier, RunningAgent, read_development, read_review,
};
use agentflow_contracts::{AgentEvent, AgentKind, RunRole};
use agentflow_process_supervisor::ProcessOutcome;
use agentflow_provider_protocol::{
    ProtocolClient, ProtocolPermission, ProtocolResult, ProtocolRunRequest,
    ResolvedProviderManifest,
};
use async_trait::async_trait;
use serde_json::json;
use std::{path::Path, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Bridges a dynamically installed sidecar into the same interface used by built-in Providers.
/// Vendor-specific command-line flags and response formats never leak past this boundary.
#[derive(Debug, Clone)]
pub struct ExternalProviderAdapter {
    provider: ResolvedProviderManifest,
}

/// Keeps persisted tasks diagnosable when an external package was removed after task creation.
#[derive(Debug, Clone)]
pub struct UnavailableProviderAdapter {
    kind: AgentKind,
}

impl UnavailableProviderAdapter {
    pub fn new(kind: AgentKind) -> Self {
        Self { kind }
    }

    fn error(&self) -> AdapterError {
        AdapterError::NotFound(format!(
            "Provider package '{}' is not installed or enabled",
            self.kind
        ))
    }
}

impl ExternalProviderAdapter {
    pub fn new(provider: ResolvedProviderManifest) -> Self {
        Self { provider }
    }

    fn provider_error(&self, error: impl std::fmt::Display) -> AdapterError {
        AdapterError::Provider {
            provider: self.provider.manifest.id.clone(),
            status: None,
            message: error.to_string(),
            retryable: false,
        }
    }
}

#[async_trait]
impl AgentProvider for ExternalProviderAdapter {
    fn kind(&self) -> AgentKind {
        self.provider.manifest.id.clone()
    }

    async fn detect(&self, _env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        let (handshake, health) = ProtocolClient::new(self.provider.clone())
            .probe()
            .await
            .map_err(|error| self.provider_error(error))?;
        if !matches!(
            health.status,
            agentflow_provider_protocol::HealthStatus::Ready
                | agentflow_provider_protocol::HealthStatus::Degraded
        ) {
            return Err(self.provider_error(
                health
                    .message
                    .unwrap_or_else(|| "provider is unavailable".into()),
            ));
        }
        Ok(AgentInstallation {
            path: self.provider.executable.clone(),
            version: handshake.provider_version,
            capabilities: self.capabilities(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        let capabilities = &self.provider.manifest.capabilities;
        AgentCapabilities {
            streams_events: capabilities.streaming,
            native_output_schema: capabilities.structured_output,
            supports_resume: capabilities.resume,
            read_only_mode: !capabilities.development,
            supports_development: capabilities.development,
            supports_review: capabilities.review,
        }
    }

    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        let capabilities = self.capabilities();
        if req.role == RunRole::Planner
            || (req.role == RunRole::Developer && !capabilities.supports_development)
            || (req.role == RunRole::Reviewer && !capabilities.supports_review)
        {
            return Err(AdapterError::UnsupportedRole(req.role));
        }
        tokio::fs::create_dir_all(&req.run_dir).await?;
        let timeout_ms = duration_millis(req.timeout);
        let idle_timeout_ms = duration_millis(req.idle_timeout);
        let protocol_request = ProtocolRunRequest {
            request_id: format!("{}-{}", self.kind(), chrono::Utc::now().timestamp_millis()),
            task_id: req.task_id,
            revision: req.revision,
            commit_sha: req.commit_sha,
            worktree: req.worktree.to_string_lossy().into_owned(),
            run_dir: req.run_dir.to_string_lossy().into_owned(),
            role: req.role,
            input_file: req.input_file,
            timeout_ms,
            idle_timeout_ms,
            permission: match req.permission {
                PermissionTier::Normal | PermissionTier::ReadOnly => ProtocolPermission::Normal,
                PermissionTier::Yolo => ProtocolPermission::FullAccess,
            },
            resume_session_id: req.resume_session_id,
            extra_allowed_commands: req.extra_allowed_commands,
            env_denylist: req.env_denylist,
        };
        let outcome = ProtocolClient::new(self.provider.clone())
            .run(protocol_request, cancel, tx)
            .await
            .map_err(|error| self.provider_error(error))?;

        tokio::fs::write(req.run_dir.join("stderr.log"), &outcome.stderr).await?;
        if let Some(protocol_result) = &outcome.result {
            tokio::fs::write(
                req.run_dir.join("provider-telemetry.json"),
                serde_json::to_vec(&json!({
                    "session_id": protocol_result.session_id,
                    "cost_usd": protocol_result.cost_usd,
                    "tokens_in": protocol_result.tokens_in,
                    "tokens_out": protocol_result.tokens_out,
                }))
                .map_err(|error| AdapterError::InvalidResult(error.to_string()))?,
            )
            .await?;
            let json = serde_json::to_vec_pretty(&protocol_result.result)
                .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
            tokio::fs::write(req.run_dir.join("stdout.log"), &json).await?;
            match &protocol_result.result {
                ProtocolResult::Development(value) => {
                    tokio::fs::write(
                        req.run_dir.join("result.json"),
                        serde_json::to_vec_pretty(value)
                            .map_err(|error| AdapterError::InvalidResult(error.to_string()))?,
                    )
                    .await?;
                }
                ProtocolResult::Review(value) => {
                    tokio::fs::write(
                        req.run_dir.join("last-message.json"),
                        serde_json::to_vec_pretty(value)
                            .map_err(|error| AdapterError::InvalidResult(error.to_string()))?,
                    )
                    .await?;
                }
            }
        } else {
            tokio::fs::write(req.run_dir.join("stdout.log"), "").await?;
        }

        Ok(RunningAgent {
            outcome: ProcessOutcome {
                pid: outcome.pid,
                started_at: outcome.started_at,
                exit_code: outcome.exit_code,
                timed_out: outcome.timed_out,
                cancelled: outcome.cancelled,
                log_truncated: outcome.stderr_truncated,
            },
            run_dir: req.run_dir,
            role: req.role,
        })
    }

    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        match role {
            RunRole::Planner => Err(AdapterError::UnsupportedRole(role)),
            RunRole::Developer => read_development(&run_dir.join("result.json"))
                .await
                .map(CollectedResult::Development),
            RunRole::Reviewer => read_review(&run_dir.join("last-message.json"))
                .await
                .map(CollectedResult::Review),
            RunRole::Validator => Err(AdapterError::UnsupportedRole(role)),
        }
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[async_trait]
impl AgentProvider for UnavailableProviderAdapter {
    fn kind(&self) -> AgentKind {
        self.kind.clone()
    }

    async fn detect(&self, _env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        Err(self.error())
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streams_events: false,
            native_output_schema: false,
            supports_resume: false,
            read_only_mode: true,
            supports_development: false,
            supports_review: false,
        }
    }

    async fn start(
        &self,
        _req: AgentRunRequest,
        _cancel: CancellationToken,
        _tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        Err(self.error())
    }

    async fn collect_result(
        &self,
        _run_dir: &Path,
        _role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        Err(self.error())
    }
}
