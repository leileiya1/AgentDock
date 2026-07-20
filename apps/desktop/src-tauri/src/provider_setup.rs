use super::Backend;
use agentflow_contracts::{
    AppError, CLAUDE_CLI_KEYCHAIN_SERVICE, CODEX_CLI_KEYCHAIN_SERVICE, EnvReport, ErrorCode,
};
use serde::Deserialize;
use specta::Type;
use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};
use tauri::State;
use tokio::{process::Command, time::timeout};

#[derive(Deserialize, Type)]
pub(crate) struct CliInstallArgs {
    tool: String,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiCredentialArgs {
    provider: String,
    api_key: Option<String>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliCredentialArgs {
    tool: String,
    api_key: Option<String>,
}

fn setup_error(code: ErrorCode, message: impl Into<String>, detail: Option<String>) -> AppError {
    AppError {
        code,
        message: message.into(),
        detail,
    }
}

fn cli_package(tool: &str) -> Option<&'static str> {
    // These are fixed official package names. Never pass user input to npm.
    match tool {
        "claude_code" => Some("@anthropic-ai/claude-code"),
        "codex" => Some("@openai/codex"),
        "gemini_cli" => Some("@google/gemini-cli"),
        "qwen_code" => Some("@qwen-code/qwen-code@latest"),
        "grok_cli" => Some("@xai-official/grok"),
        "kimi_cli" => Some("@moonshot-ai/kimi-code"),
        "minimax_cli" => Some("mmx-cli"),
        _ => None,
    }
}

fn api_service(provider: &str) -> Option<&'static str> {
    match provider {
        "openai_api" => Some("com.agentflow.openai-api"),
        "anthropic_api" => Some("com.agentflow.anthropic-api"),
        "deepseek_api" => Some("com.agentflow.deepseek-api"),
        "grok_api" => Some("com.agentflow.grok-api"),
        "minimax_api" => Some("com.agentflow.minimax-api"),
        "kimi_api" => Some("com.agentflow.kimi-api"),
        _ => None,
    }
}

fn cli_service(tool: &str) -> Option<&'static str> {
    match tool {
        "claude_code" => Some(CLAUDE_CLI_KEYCHAIN_SERVICE),
        "codex" => Some(CODEX_CLI_KEYCHAIN_SERVICE),
        _ => None,
    }
}

fn find_npm() -> Option<PathBuf> {
    which::which("npm").ok().or_else(|| {
        [
            "/opt/homebrew/bin/npm",
            "/usr/local/bin/npm",
            "/usr/bin/npm",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
    })
}

fn install_path(npm: &std::path::Path) -> OsString {
    let mut paths = Vec::new();
    if let Some(parent) = npm.parent() {
        paths.push(parent.to_path_buf());
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    std::env::join_paths(paths).unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
}

fn output_detail(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let tail = text.chars().rev().take(2_000).collect::<String>();
    tail.chars().rev().collect()
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cli_install(
    state: State<'_, Backend>,
    args: CliInstallArgs,
) -> Result<EnvReport, AppError> {
    let package = cli_package(&args.tool)
        .ok_or_else(|| setup_error(ErrorCode::CliInstallFailed, "不支持安装这个 CLI", None))?;
    let npm = find_npm().ok_or_else(|| {
        setup_error(
            ErrorCode::CliInstallFailed,
            "没有找到 npm",
            Some("请先安装 Node.js/npm，或手动安装该 CLI。".into()),
        )
    })?;
    let _guard = state.2.lock().await;
    let mut command = Command::new(&npm);
    command
        .args(["install", "--global", package, "--no-fund", "--no-audit"])
        .env("PATH", install_path(&npm))
        .env("NO_UPDATE_NOTIFIER", "1")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(600), command.output())
        .await
        .map_err(|_| setup_error(ErrorCode::CliInstallFailed, "安装超时", None))?
        .map_err(|error| {
            setup_error(
                ErrorCode::CliInstallFailed,
                "无法启动安装程序",
                Some(error.to_string()),
            )
        })?;
    if !output.status.success() {
        return Err(setup_error(
            ErrorCode::CliInstallFailed,
            "CLI 安装失败",
            Some(output_detail(&output.stderr)),
        ));
    }
    Ok(state.0.env_check().await)
}

#[cfg(target_os = "macos")]
async fn update_keychain(service: &str, api_key: Option<&str>) -> Result<(), AppError> {
    let service = service.to_string();
    let key = api_key.map(str::as_bytes).map(ToOwned::to_owned);
    let deleting = key.is_none();
    let result = tokio::task::spawn_blocking(move || match key {
        Some(value) => {
            security_framework::passwords::set_generic_password(&service, "AgentFlow", &value)
        }
        None => security_framework::passwords::delete_generic_password(&service, "AgentFlow"),
    })
    .await
    .map_err(|error| {
        setup_error(
            ErrorCode::ApiCredentialFailed,
            "钥匙串操作失败",
            Some(error.to_string()),
        )
    })?;

    match result {
        Ok(()) => Ok(()),
        // Removing a credential is intentionally idempotent.
        Err(error) if deleting && error.code() == -25_300 => Ok(()),
        Err(error) if error.code() == -25_293 => Err(setup_error(
            ErrorCode::ApiCredentialFailed,
            "登录钥匙串当前处于锁定状态",
            Some("请先在“钥匙串访问”中解锁登录钥匙串，然后重试。".into()),
        )),
        Err(error) => Err(setup_error(
            ErrorCode::ApiCredentialFailed,
            "钥匙串操作失败",
            Some(error.to_string()),
        )),
    }
}

#[cfg(not(target_os = "macos"))]
async fn update_keychain(_service: &str, _api_key: Option<&str>) -> Result<(), AppError> {
    Err(setup_error(
        ErrorCode::ApiCredentialFailed,
        "当前系统请使用对应的 API 环境变量",
        None,
    ))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn api_credential_set(
    state: State<'_, Backend>,
    args: ApiCredentialArgs,
) -> Result<EnvReport, AppError> {
    let service = api_service(&args.provider).ok_or_else(|| {
        setup_error(
            ErrorCode::ApiCredentialFailed,
            "不支持这个 API Provider",
            None,
        )
    })?;
    let key = args.api_key.as_deref().map(str::trim).unwrap_or_default();
    if key.is_empty() || key.len() > 4_096 {
        return Err(setup_error(
            ErrorCode::ApiCredentialFailed,
            "请输入有效的 API 密钥",
            None,
        ));
    }
    update_keychain(service, Some(key)).await?;
    Ok(state.0.env_check().await)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn api_credential_delete(
    state: State<'_, Backend>,
    args: ApiCredentialArgs,
) -> Result<EnvReport, AppError> {
    let service = api_service(&args.provider).ok_or_else(|| {
        setup_error(
            ErrorCode::ApiCredentialFailed,
            "不支持这个 API Provider",
            None,
        )
    })?;
    update_keychain(service, None).await?;
    Ok(state.0.env_check().await)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cli_credential_set(
    state: State<'_, Backend>,
    args: CliCredentialArgs,
) -> Result<EnvReport, AppError> {
    let service = cli_service(&args.tool).ok_or_else(|| {
        setup_error(
            ErrorCode::ApiCredentialFailed,
            "这个 CLI 暂不支持 API Key 认证",
            None,
        )
    })?;
    let key = args.api_key.as_deref().map(str::trim).unwrap_or_default();
    if key.is_empty() || key.len() > 4_096 {
        return Err(setup_error(
            ErrorCode::ApiCredentialFailed,
            "请输入有效的 API 密钥",
            None,
        ));
    }
    update_keychain(service, Some(key)).await?;
    Ok(state.0.env_check().await)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cli_credential_delete(
    state: State<'_, Backend>,
    args: CliCredentialArgs,
) -> Result<EnvReport, AppError> {
    let service = cli_service(&args.tool).ok_or_else(|| {
        setup_error(
            ErrorCode::ApiCredentialFailed,
            "这个 CLI 暂不支持 API Key 认证",
            None,
        )
    })?;
    update_keychain(service, None).await?;
    Ok(state.0.env_check().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installers_are_a_closed_allowlist() {
        assert_eq!(cli_package("gemini_cli"), Some("@google/gemini-cli"));
        assert_eq!(
            cli_package("qwen_code"),
            Some("@qwen-code/qwen-code@latest")
        );
        assert_eq!(cli_package("grok_cli"), Some("@xai-official/grok"));
        assert_eq!(cli_package("kimi_cli"), Some("@moonshot-ai/kimi-code"));
        assert_eq!(cli_package("minimax_cli"), Some("mmx-cli"));
        assert_eq!(cli_package("anything_else"), None);
    }

    #[test]
    fn keychain_services_cannot_be_supplied_by_the_frontend() {
        assert_eq!(api_service("openai_api"), Some("com.agentflow.openai-api"));
        assert_eq!(api_service("external_provider"), None);
        assert_eq!(api_service("grok_api"), Some("com.agentflow.grok-api"));
        assert_eq!(
            api_service("minimax_api"),
            Some("com.agentflow.minimax-api")
        );
        assert_eq!(api_service("kimi_api"), Some("com.agentflow.kimi-api"));
        assert_eq!(
            cli_service("claude_code"),
            Some(CLAUDE_CLI_KEYCHAIN_SERVICE)
        );
        assert_eq!(cli_service("codex"), Some(CODEX_CLI_KEYCHAIN_SERVICE));
        assert_eq!(cli_service("gemini_cli"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn keychain_roundtrip_uses_security_framework_without_a_shell() {
        use security_framework::os::macos::keychain::CreateOptions;

        let service = format!("com.agentflow.provider-setup-test.{}", std::process::id());
        let secret = b"not-a-real-key";
        let path = std::env::temp_dir().join(format!(
            "agentflow-provider-setup-test-{}.keychain-db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let keychain = CreateOptions::new()
            .password("agentflow-test-only")
            .create(&path)
            .expect("create temporary keychain");
        keychain
            .set_generic_password(&service, "AgentFlow", secret)
            .expect("write temporary keychain item");
        let (stored, item) = keychain
            .find_generic_password(&service, "AgentFlow")
            .expect("read temporary keychain item");
        assert_eq!(&*stored, secret);
        item.delete();
        drop(keychain);
        std::fs::remove_file(path).expect("remove temporary keychain");
    }
}
