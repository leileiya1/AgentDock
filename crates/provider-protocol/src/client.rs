use crate::{
    HandshakeParams, HandshakeResult, HealthResult, PROTOCOL_VERSION, ProtocolRunRequest,
    ProtocolRunResult, ProviderPermissionRequest, ResolvedProviderManifest, RpcNotification,
    RpcRequest, RpcResponse, SUPPORTED_PROTOCOLS,
};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use thiserror::Error;
use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter,
    },
    process::{Child, ChildStdin, ChildStdout, Command},
    task::JoinHandle,
    time::Instant,
};
use tokio_util::sync::CancellationToken;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_STDERR_BYTES: usize = 1024 * 1024;
/// An external sidecar is untrusted code. A line longer than this is a protocol violation,
/// not a large result: without a ceiling, a provider that never writes a newline grows the
/// daemon's memory without bound.
const MAX_STDOUT_LINE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("provider process I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid provider protocol message: {0}")]
    Json(#[from] serde_json::Error),
    #[error("provider returned RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("provider protocol is incompatible: {0}")]
    Incompatible(String),
    #[error("provider closed stdout before returning a response")]
    Closed,
    #[error("provider timed out while {0}")]
    Timeout(&'static str),
    #[error("provider stderr task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("provider wrote a stdout line above the {0} byte protocol limit")]
    LineTooLong(usize),
}

#[derive(Debug)]
pub struct ProtocolRunOutcome {
    pub pid: u32,
    pub started_at: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub stderr: String,
    pub stderr_truncated: bool,
    pub result: Option<ProtocolRunResult>,
    pub permission_request: Option<ProviderPermissionRequest>,
}

#[derive(Debug, Clone)]
pub struct ProtocolClient {
    provider: ResolvedProviderManifest,
}

impl ProtocolClient {
    pub fn new(provider: ResolvedProviderManifest) -> Self {
        Self { provider }
    }

    pub fn provider(&self) -> &ResolvedProviderManifest {
        &self.provider
    }

    /// Checks both protocol negotiation and provider readiness without starting a task.
    pub async fn probe(&self) -> Result<(HandshakeResult, HealthResult), ProtocolError> {
        let mut session = Session::spawn(&self.provider, &[]).await?;
        let handshake = session.handshake(&self.provider).await?;
        let health = session
            .request(2, "health", &Value::Null, STARTUP_TIMEOUT)
            .await?;
        session.shutdown().await?;
        Ok((handshake, health))
    }

    /// Executes one request. Provider events are forwarded as typed notifications instead of
    /// requiring the core to understand vendor-specific stdout formats.
    pub async fn run(
        &self,
        request: ProtocolRunRequest,
        cancel: CancellationToken,
        event_tx: tokio::sync::mpsc::Sender<agentflow_contracts::AgentEvent>,
    ) -> Result<ProtocolRunOutcome, ProtocolError> {
        let mut session = Session::spawn(&self.provider, &request.env_denylist).await?;
        session.handshake(&self.provider).await?;
        session.send(&RpcRequest::new(2, "run", &request)?).await?;

        let deadline = Instant::now() + Duration::from_millis(request.timeout_ms);
        let idle_timeout = Duration::from_millis(request.idle_timeout_ms);
        let mut result = None;
        let mut permission_request = None;
        let mut timed_out = false;
        let mut cancelled = false;

        loop {
            let now = Instant::now();
            if now >= deadline {
                timed_out = true;
                break;
            }
            let remaining = deadline.saturating_duration_since(now);
            let wait_for = idle_timeout.min(remaining);
            let mut line = String::new();
            let read = read_bounded_line(&mut session.stdout, &mut line);
            let outcome = tokio::select! {
                _ = cancel.cancelled() => {
                    cancelled = true;
                    break;
                }
                response = tokio::time::timeout(wait_for, read) => response,
            };
            // Every failure below still falls through to the kill/shutdown block: returning
            // early here would drop the Session and leave an orphaned sidecar running.
            match outcome {
                Err(_) => {
                    timed_out = true;
                    break;
                }
                Ok(Ok(0)) => {
                    session.kill().await?;
                    return Err(ProtocolError::Closed);
                }
                Ok(Ok(_)) => match handle_run_message(&line, &event_tx).await {
                    // A sidecar that prints diagnostics or a crash trace on stdout is common
                    // in CLI ecosystems; skip the stray line rather than abandoning the run.
                    Ok(RunMessage::Continue) | Err(ProtocolError::Json(_)) => {}
                    Ok(RunMessage::Result(run_result)) => {
                        result = Some(run_result);
                        break;
                    }
                    Ok(RunMessage::Permission(request)) => {
                        permission_request = Some(request);
                        break;
                    }
                    Err(error) => {
                        session.kill().await?;
                        return Err(error);
                    }
                },
                Ok(Err(error)) => {
                    session.kill().await?;
                    return Err(error);
                }
            }
        }

        if timed_out || cancelled {
            session.kill().await?;
        } else {
            session.shutdown().await?;
        }
        let stderr = session.stderr().await?;
        if let Some(permission) = permission_request.as_ref() {
            validate_provider_permission(&self.provider, permission)?;
        }
        let exit_code = result.as_ref().map(|value| value.exit_code);
        Ok(ProtocolRunOutcome {
            pid: session.pid,
            started_at: session.started_at,
            exit_code,
            timed_out,
            cancelled,
            stderr: stderr.text,
            stderr_truncated: stderr.truncated,
            result,
            permission_request,
        })
    }
}

fn validate_provider_permission(
    provider: &ResolvedProviderManifest,
    request: &ProviderPermissionRequest,
) -> Result<(), ProtocolError> {
    use agentflow_contracts::PermissionActionType as Action;
    let maximum = &provider.manifest.permissions;
    let allowed = match request.action_type {
        Action::WorktreeRead | Action::GitRead => maximum.worktree_read,
        Action::WorktreeWrite | Action::WorktreeDelete | Action::ControlPlaneWrite => {
            maximum.worktree_write
        }
        Action::CommandExecute => request
            .operation
            .argv
            .first()
            .is_some_and(|command| maximum.commands.contains(command)),
        Action::DependencyInstall => {
            request
                .operation
                .argv
                .first()
                .is_some_and(|command| maximum.commands.contains(command))
                && request
                    .operation
                    .network_domains
                    .iter()
                    .all(|domain| maximum.network_domains.contains(domain))
        }
        Action::NetworkAccess => request
            .operation
            .network_domains
            .iter()
            .all(|domain| maximum.network_domains.contains(domain)),
        Action::ProcessControl => true,
        Action::EnvironmentRead
        | Action::SecretAccess
        | Action::GitMutation
        | Action::SystemChange
        | Action::ExternalPath => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(ProtocolError::Incompatible(
            "PERMISSION_MANIFEST_EXCEEDED".into(),
        ))
    }
}

enum RunMessage {
    Continue,
    Result(ProtocolRunResult),
    Permission(ProviderPermissionRequest),
}

async fn handle_run_message(
    line: &str,
    event_tx: &tokio::sync::mpsc::Sender<agentflow_contracts::AgentEvent>,
) -> Result<RunMessage, ProtocolError> {
    let value: Value = serde_json::from_str(line)?;
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        if method == "permission/requested" {
            let params = value.get("params").cloned().ok_or_else(|| {
                ProtocolError::Incompatible("permission notification has no params".into())
            })?;
            return Ok(RunMessage::Permission(serde_json::from_value(params)?));
        }
        let notification: RpcNotification = serde_json::from_value(value)?;
        if notification.method == "event" {
            let _ = event_tx.send(notification.params).await;
        }
        return Ok(RunMessage::Continue);
    }
    let response: RpcResponse = serde_json::from_value(value)?;
    if response.id != 2 {
        return Ok(RunMessage::Continue);
    }
    decode_response(response).map(RunMessage::Result)
}

fn decode_response<T: DeserializeOwned>(response: RpcResponse) -> Result<T, ProtocolError> {
    if let Some(error) = response.error {
        return Err(ProtocolError::Rpc {
            code: error.code,
            message: error.message,
        });
    }
    let value = response.result.ok_or_else(|| {
        ProtocolError::Incompatible("RPC response contains neither result nor error".into())
    })?;
    Ok(serde_json::from_value(value)?)
}

struct CapturedStderr {
    text: String,
    truncated: bool,
}

struct Session {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr_task: Option<JoinHandle<Result<CapturedStderr, std::io::Error>>>,
    pid: u32,
    started_at: String,
}

impl Session {
    async fn spawn(
        provider: &ResolvedProviderManifest,
        env_denylist: &[String],
    ) -> Result<Self, ProtocolError> {
        let mut command = Command::new(&provider.executable);
        command
            .args(&provider.manifest.args)
            .current_dir(&provider.package_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.env_clear();
        for key in ["HOME", "PATH", "TMPDIR", "LANG", "LC_ALL", "SHELL", "TERM"] {
            if let Ok(value) = std::env::var(key) {
                command.env(key, value);
            }
        }
        for key in env_denylist {
            command.env_remove(key);
        }
        // Any early return between spawn and the explicit kill/shutdown must still reap the
        // sidecar: without this the dropped Session leaves an orphan holding CPU and API quota.
        command.kill_on_drop(true);
        let mut child = command.spawn()?;
        let pid = child.id().unwrap_or(0);
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("provider stdin pipe is missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("provider stdout pipe is missing"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::other("provider stderr pipe is missing"))?;
        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            stderr_task: Some(tokio::spawn(capture_stderr(stderr))),
            pid,
            started_at: Utc::now().to_rfc3339(),
        })
    }

    async fn handshake(
        &mut self,
        provider: &ResolvedProviderManifest,
    ) -> Result<HandshakeResult, ProtocolError> {
        let params = HandshakeParams {
            core_version: env!("CARGO_PKG_VERSION").into(),
            supported_protocols: SUPPORTED_PROTOCOLS
                .iter()
                .map(|version| (*version).to_string())
                .collect(),
        };
        let result: HandshakeResult = self
            .request(1, "handshake", &params, STARTUP_TIMEOUT)
            .await?;
        if result.provider_id != provider.manifest.id {
            return Err(ProtocolError::Incompatible(format!(
                "manifest id {} does not match handshake id {}",
                provider.manifest.id, result.provider_id
            )));
        }
        if result.protocol_version.split('.').next() != PROTOCOL_VERSION.split('.').next() {
            return Err(ProtocolError::Incompatible(format!(
                "provider protocol {} is incompatible with core {}",
                result.protocol_version, PROTOCOL_VERSION
            )));
        }
        Ok(result)
    }

    async fn request<T: DeserializeOwned, P: Serialize>(
        &mut self,
        id: u64,
        method: &str,
        params: &P,
        timeout: Duration,
    ) -> Result<T, ProtocolError> {
        self.send(&RpcRequest::new(id, method, params)?).await?;
        let mut line = String::new();
        let read = tokio::time::timeout(timeout, read_bounded_line(&mut self.stdout, &mut line))
            .await
            .map_err(|_| ProtocolError::Timeout("waiting for provider response"))??;
        if read == 0 {
            return Err(ProtocolError::Closed);
        }
        let response: RpcResponse = serde_json::from_str(&line)?;
        if response.id != id {
            return Err(ProtocolError::Incompatible(format!(
                "expected RPC response {id}, received {}",
                response.id
            )));
        }
        decode_response(response)
    }

    async fn send<T: Serialize>(&mut self, message: &T) -> Result<(), ProtocolError> {
        let mut bytes = serde_json::to_vec(message)?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), ProtocolError> {
        let request = RpcRequest::new(3, "shutdown", &Value::Null)?;
        let _ = self.send(&request).await;
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, self.child.wait())
            .await
            .is_err()
        {
            self.kill().await?;
        }
        Ok(())
    }

    async fn kill(&mut self) -> Result<(), ProtocolError> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().await?;
            let _ = self.child.wait().await?;
        }
        Ok(())
    }

    async fn stderr(&mut self) -> Result<CapturedStderr, ProtocolError> {
        let Some(task) = self.stderr_task.take() else {
            return Ok(CapturedStderr {
                text: String::new(),
                truncated: false,
            });
        };
        Ok(task.await??)
    }
}

/// Reads one NDJSON line with a hard ceiling. `BufReader::read_line` grows its target string
/// without limit, so an untrusted provider that never emits a newline could exhaust memory.
/// Returns the byte count read, or `LineTooLong` once the limit is crossed.
async fn read_bounded_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    line: &mut String,
) -> Result<usize, ProtocolError> {
    let mut bytes = Vec::new();
    loop {
        let available = match reader.fill_buf().await {
            Ok(buffer) => buffer,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ProtocolError::Io(error)),
        };
        if available.is_empty() {
            break;
        }
        let (consumed, done) = match available.iter().position(|byte| *byte == b'\n') {
            Some(index) => (index + 1, true),
            None => (available.len(), false),
        };
        if bytes.len().saturating_add(consumed) > MAX_STDOUT_LINE_BYTES {
            reader.consume(consumed);
            return Err(ProtocolError::LineTooLong(MAX_STDOUT_LINE_BYTES));
        }
        bytes.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if done {
            break;
        }
    }
    let read = bytes.len();
    line.push_str(&String::from_utf8_lossy(&bytes));
    Ok(read)
}

async fn capture_stderr<R: AsyncRead + Unpin>(
    mut reader: R,
) -> Result<CapturedStderr, std::io::Error> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let remaining = MAX_STDERR_BYTES.saturating_sub(output.len());
        let keep = count.min(remaining);
        output.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    Ok(CapturedStderr {
        text: String::from_utf8_lossy(&output).into_owned(),
        truncated,
    })
}
