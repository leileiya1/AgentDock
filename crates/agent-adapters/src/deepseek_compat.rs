use crate::{AdapterError, cli_credential_env};
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::any,
};
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use toml::{Table, Value};

const DEEPSEEK_API_KEY: &str = "DEEPSEEK_API_KEY";
const LOCAL_PROXY_KEY: &str = "AGENTFLOW_GROK_PROXY_TOKEN";
const MAX_PROXY_REQUEST_BYTES: usize = 16 * 1024 * 1024;

struct ProxyState {
    upstream_base: String,
    upstream_api_key: String,
    local_token: String,
    client: reqwest::Client,
}

/// A per-run compatibility boundary for opaque CLIs. It binds only to loopback, accepts a random
/// run-scoped token, owns the real upstream credential, and exits when the child CLI exits.
pub(crate) struct GrokDeepSeekCompat {
    environment: HashMap<String, String>,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl GrokDeepSeekCompat {
    pub(crate) fn environment(&self) -> HashMap<String, String> {
        self.environment.clone()
    }

    pub(crate) async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let mut task = self.task;
        if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
}

/// Prepare the compatibility boundary only when Grok's selected model is an official DeepSeek
/// endpoint. Native xAI configurations and unrelated custom endpoints remain untouched.
pub(crate) async fn prepare_grok_deepseek_compat(
    run_dir: &Path,
    worktree: &Path,
    env_denylist: &[String],
) -> Result<Option<GrokDeepSeekCompat>, AdapterError> {
    if env_denylist.iter().any(|key| key == DEEPSEEK_API_KEY) {
        return Err(AdapterError::Incompatible(
            "项目禁止读取 DEEPSEEK_API_KEY，Grok 兼容网关已拒绝运行".into(),
        ));
    }
    if worktree.join(".grok/config.toml").is_file() {
        return Err(AdapterError::Incompatible(
            "项目级 .grok/config.toml 可能覆盖受控端点，Grok 兼容网关已拒绝运行".into(),
        ));
    }
    let source_home = std::env::var_os("GROK_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".grok"))
        });
    let Some(source_home) = source_home else {
        return Ok(None);
    };
    let source_config = match tokio::fs::read_to_string(source_home.join("config.toml")).await {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let Some(upstream_base) = selected_deepseek_upstream(&source_config)? else {
        return Ok(None);
    };
    let upstream_api_key = std::env::var(DEEPSEEK_API_KEY)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| cli_credential_env("grok").remove(DEEPSEEK_API_KEY))
        .ok_or_else(|| {
            AdapterError::Incompatible(
                "Grok 的 DeepSeek 配置缺少 Keychain/API 凭据，兼容网关拒绝启动".into(),
            )
        })?;
    let mut token_bytes = [0_u8; 32];
    getrandom::fill(&mut token_bytes).map_err(|error| {
        AdapterError::Incompatible(format!("无法生成 Grok 回环网关令牌：{error}"))
    })?;
    let local_token = hex::encode(token_bytes);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let proxy_base = format!("http://{address}");
    let isolated_home = run_dir.join("grok-home");
    tokio::fs::create_dir_all(&isolated_home).await?;
    let isolated_config = rewrite_grok_config(&source_config, &proxy_base)?;
    let config_path = isolated_home.join("config.toml");
    tokio::fs::write(&config_path, isolated_config).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600))?;
    }

    let state = Arc::new(ProxyState {
        upstream_base: upstream_base.trim_end_matches('/').to_string(),
        upstream_api_key,
        local_token: local_token.clone(),
        client: reqwest::Client::new(),
    });
    let router = Router::new()
        .route("/", any(forward_request))
        .route("/{*path}", any(forward_request))
        .layer(DefaultBodyLimit::max(MAX_PROXY_REQUEST_BYTES))
        .with_state(state);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
    });
    let environment = HashMap::from([
        (
            "GROK_HOME".into(),
            isolated_home.to_string_lossy().into_owned(),
        ),
        (LOCAL_PROXY_KEY.into(), local_token.clone()),
        ("XAI_API_KEY".into(), local_token),
        ("GROK_MODELS_BASE_URL".into(), proxy_base),
    ]);
    Ok(Some(GrokDeepSeekCompat {
        environment,
        shutdown: Some(shutdown_tx),
        task,
    }))
}

fn selected_deepseek_upstream(source: &str) -> Result<Option<String>, AdapterError> {
    let root = source
        .parse::<Table>()
        .map_err(|error| AdapterError::Incompatible(format!("Grok config.toml 无效：{error}")))?;
    let default = root
        .get("models")
        .and_then(Value::as_table)
        .and_then(|models| models.get("default"))
        .and_then(Value::as_str);
    let Some(default) = default else {
        return Ok(None);
    };
    let base_url = root
        .get("model")
        .and_then(Value::as_table)
        .and_then(|models| models.get(default))
        .and_then(Value::as_table)
        .and_then(|model| model.get("base_url"))
        .and_then(Value::as_str);
    Ok(base_url
        .filter(|url| is_official_deepseek_endpoint(url))
        .map(str::to_owned))
}

fn is_official_deepseek_endpoint(value: &str) -> bool {
    matches!(
        value.trim_end_matches('/'),
        "https://api.deepseek.com" | "https://api.deepseek.com/v1"
    )
}

fn rewrite_grok_config(source: &str, proxy_base: &str) -> Result<String, AdapterError> {
    let root = source
        .parse::<Table>()
        .map_err(|error| AdapterError::Incompatible(format!("Grok config.toml 无效：{error}")))?;
    let mut models = root
        .get("models")
        .and_then(Value::as_table)
        .cloned()
        .ok_or_else(|| AdapterError::Incompatible("Grok 配置缺少 [models]".into()))?;
    let default = models
        .get("default")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AdapterError::Incompatible("Grok 配置缺少 models.default".into()))?;
    let source_entries = root
        .get("model")
        .and_then(Value::as_table)
        .ok_or_else(|| AdapterError::Incompatible("Grok 配置缺少 [model.*]".into()))?;
    let mut rewritten_entries = Table::new();
    for (name, value) in source_entries {
        let Some(entry) = value.as_table() else {
            continue;
        };
        let deepseek = entry
            .get("base_url")
            .and_then(Value::as_str)
            .is_some_and(is_official_deepseek_endpoint);
        if !deepseek {
            continue;
        }
        let mut entry = entry.clone();
        entry.insert("base_url".into(), Value::String(proxy_base.into()));
        entry.insert("env_key".into(), Value::String(LOCAL_PROXY_KEY.into()));
        entry.remove("api_key");
        rewritten_entries.insert(name.clone(), Value::Table(entry));
    }
    if !rewritten_entries.contains_key(&default) {
        return Err(AdapterError::Incompatible(
            "Grok 默认模型不是可代理的 DeepSeek 模型".into(),
        ));
    }
    let replacement_aux = rewritten_entries
        .iter()
        .find_map(|(name, value)| {
            value
                .as_table()
                .and_then(|entry| entry.get("model"))
                .and_then(Value::as_str)
                .filter(|model| model.contains("v4-flash"))
                .map(|_| name.clone())
        })
        .unwrap_or_else(|| default.clone());
    for purpose in ["session_summary", "prompt_suggestion"] {
        let deprecated = models
            .get(purpose)
            .and_then(Value::as_str)
            .and_then(|name| rewritten_entries.get(name))
            .and_then(Value::as_table)
            .and_then(|entry| entry.get("model"))
            .and_then(Value::as_str)
            == Some("deepseek-chat");
        if deprecated || models.get(purpose).is_none() {
            models.insert(purpose.into(), Value::String(replacement_aux.clone()));
        }
    }
    let mut sanitized = Table::new();
    sanitized.insert("models".into(), Value::Table(models));
    sanitized.insert("model".into(), Value::Table(rewritten_entries));
    toml::to_string_pretty(&sanitized)
        .map_err(|error| AdapterError::Incompatible(format!("无法生成 Grok 隔离配置：{error}")))
}

fn forced_tool_choice(value: &serde_json::Value) -> bool {
    match value.get("tool_choice") {
        Some(serde_json::Value::Object(_)) => true,
        Some(serde_json::Value::String(choice)) => !matches!(choice.as_str(), "auto" | "none"),
        _ => false,
    }
}

fn adapt_deepseek_request(path: &str, body: Bytes) -> Bytes {
    if !path.ends_with("/chat/completions") {
        return body;
    }
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return body;
    };
    if !forced_tool_choice(&value) {
        return body;
    }
    value["thinking"] = serde_json::json!({"type": "disabled"});
    serde_json::to_vec(&value).map(Bytes::from).unwrap_or(body)
}

async fn forward_request(State(state): State<Arc<ProxyState>>, request: Request) -> Response {
    if !authorized(request.headers(), &state.local_token) {
        return plain_response(StatusCode::UNAUTHORIZED, "invalid loopback token");
    }
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, MAX_PROXY_REQUEST_BYTES).await {
        Ok(body) => adapt_deepseek_request(parts.uri.path(), body),
        Err(_) => return plain_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large"),
    };
    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let url = format!("{}{}", state.upstream_base, path_and_query);
    let mut upstream = state
        .client
        .request(parts.method, url)
        .bearer_auth(&state.upstream_api_key)
        .body(body);
    for (name, value) in &parts.headers {
        if !is_hop_by_hop(name.as_str())
            && name != header::AUTHORIZATION
            && name != header::HOST
            && name != header::CONTENT_LENGTH
        {
            upstream = upstream.header(name, value);
        }
    }
    let upstream = match upstream.send().await {
        Ok(response) => response,
        Err(_) => return plain_response(StatusCode::BAD_GATEWAY, "upstream transport failed"),
    };
    let status = upstream.status();
    let response_headers = upstream.headers().clone();
    let stream = upstream.bytes_stream();
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;
    for (name, value) in response_headers {
        if let Some(name) = name
            && !is_hop_by_hop(name.as_str())
            && name != header::CONTENT_LENGTH
        {
            response.headers_mut().insert(name, value);
        }
    }
    response
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        == Some(token)
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn plain_response(status: StatusCode, message: &'static str) -> Response {
    let mut response = Response::new(Body::from(message));
    *response.status_mut() = status;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"
[models]
default = "deepseek-v4-pro"
session_summary = "legacy-aux"
prompt_suggestion = "legacy-aux"

[model."deepseek-v4-pro"]
model = "deepseek-v4-pro"
base_url = "https://api.deepseek.com"
env_key = "DEEPSEEK_API_KEY"

[model."deepseek-v4-flash"]
model = "deepseek-v4-flash"
base_url = "https://api.deepseek.com"
env_key = "DEEPSEEK_API_KEY"

[model."legacy-aux"]
model = "deepseek-chat"
base_url = "https://api.deepseek.com"
env_key = "DEEPSEEK_API_KEY"
"#;

    #[test]
    fn isolated_config_replaces_deprecated_auxiliary_and_real_credential_name()
    -> Result<(), Box<dyn std::error::Error>> {
        let rewritten = rewrite_grok_config(CONFIG, "http://127.0.0.1:54321")?;
        assert!(rewritten.contains("session_summary = \"deepseek-v4-flash\""));
        assert!(rewritten.contains("prompt_suggestion = \"deepseek-v4-flash\""));
        assert!(rewritten.contains("env_key = \"AGENTFLOW_GROK_PROXY_TOKEN\""));
        assert!(!rewritten.contains("https://api.deepseek.com"));
        Ok(())
    }

    #[test]
    fn only_forced_tool_choice_disables_thinking() {
        let forced = Bytes::from_static(
            br#"{"model":"deepseek-v4-pro","tool_choice":{"type":"function","function":{"name":"session_title"}}}"#,
        );
        let adapted = adapt_deepseek_request("/chat/completions", forced);
        let value: serde_json::Value = serde_json::from_slice(&adapted).unwrap_or_default();
        assert_eq!(value["thinking"]["type"], "disabled");

        let automatic = Bytes::from_static(
            br#"{"model":"deepseek-v4-pro","tool_choice":"auto","thinking":{"type":"enabled"}}"#,
        );
        let adapted = adapt_deepseek_request("/chat/completions", automatic.clone());
        assert_eq!(adapted, automatic);
    }

    #[tokio::test]
    async fn project_scoped_grok_config_fails_closed_before_credential_access()
    -> Result<(), Box<dyn std::error::Error>> {
        let run = tempfile::tempdir()?;
        let worktree = tempfile::tempdir()?;
        tokio::fs::create_dir_all(worktree.path().join(".grok")).await?;
        tokio::fs::write(worktree.path().join(".grok/config.toml"), CONFIG).await?;
        let result = prepare_grok_deepseek_compat(run.path(), worktree.path(), &[]).await;
        assert!(
            matches!(result, Err(AdapterError::Incompatible(message)) if message.contains("项目级"))
        );
        Ok(())
    }
}
