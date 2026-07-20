use agentflow_contracts::{
    AgentEvent, AgentEventKind, AgentKind, DevelopmentResult, DevelopmentStatus, EventStream,
    ProviderCapabilities,
};
use agentflow_provider_protocol::{
    HandshakeResult, HealthResult, HealthStatus, ProtocolResult, ProtocolRunRequest,
    ProtocolRunResult, RpcNotification, RpcRequest, RpcResponse,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Minimal reference sidecar used by protocol conformance tests and third-party implementers.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let request: RpcRequest = serde_json::from_str(&line)?;
        match request.method.as_str() {
            "handshake" => {
                respond(
                    &mut stdout,
                    request.id,
                    &HandshakeResult {
                        protocol_version: "1.1".into(),
                        provider_id: AgentKind::External("fixture_provider".into()),
                        display_name: "Protocol Fixture".into(),
                        provider_version: "0.1.0".into(),
                        capabilities: capabilities(),
                    },
                )
                .await?;
            }
            "health" => {
                respond(
                    &mut stdout,
                    request.id,
                    &HealthResult {
                        status: HealthStatus::Ready,
                        message: None,
                    },
                )
                .await?;
            }
            "run" => {
                let run: ProtocolRunRequest = serde_json::from_value(request.params)?;
                notify(
                    &mut stdout,
                    &RpcNotification {
                        jsonrpc: "2.0".into(),
                        method: "event".into(),
                        params: AgentEvent {
                            ts: Utc::now().to_rfc3339(),
                            stream: EventStream::Stdout,
                            kind: AgentEventKind::AssistantText,
                            summary: "fixture handled request".into(),
                            text: None,
                        },
                    },
                )
                .await?;
                respond(
                    &mut stdout,
                    request.id,
                    &ProtocolRunResult {
                        exit_code: 0,
                        result: ProtocolResult::Development(DevelopmentResult {
                            schema_version: 1,
                            task_id: run.task_id,
                            revision: run.revision,
                            status: DevelopmentStatus::Completed,
                            summary: "fixture completed".into(),
                            question: None,
                            changed_files: Some(Vec::new()),
                            notes: None,
                        }),
                        session_id: Some("fixture-session".into()),
                        cost_usd: Some(0.01),
                        tokens_in: Some(10),
                        tokens_out: Some(2),
                    },
                )
                .await?;
            }
            "shutdown" => {
                respond(&mut stdout, request.id, &json!({ "ok": true })).await?;
                break;
            }
            _ => {
                let response = RpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: None,
                    error: Some(agentflow_provider_protocol::RpcError {
                        code: -32601,
                        message: "method not found".into(),
                        data: Some(Value::String(request.method)),
                    }),
                };
                notify(&mut stdout, &response).await?;
            }
        }
    }
    Ok(())
}

fn capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        development: true,
        review: false,
        streaming: true,
        structured_output: true,
        sandbox: true,
        resume: false,
    }
}

async fn respond<W: AsyncWriteExt + Unpin, T: Serialize>(
    writer: &mut W,
    id: u64,
    result: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = RpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(serde_json::to_value(result)?),
        error: None,
    };
    notify(writer, &response).await
}

async fn notify<W: AsyncWriteExt + Unpin, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
