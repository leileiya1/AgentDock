use agentflow_contracts::{CLAUDE_CLI_KEYCHAIN_SERVICE, CODEX_CLI_KEYCHAIN_SERVICE};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
struct CliCredentialSpec {
    env_key: &'static str,
    keychain_service: &'static str,
}

fn credential_spec(name: &str) -> Option<CliCredentialSpec> {
    match name {
        // Claude reads this variable in both interactive and print mode.
        "claude" => Some(CliCredentialSpec {
            env_key: "ANTHROPIC_API_KEY",
            keychain_service: CLAUDE_CLI_KEYCHAIN_SERVICE,
        }),
        // CODEX_API_KEY is intentionally limited by Codex to non-interactive `codex exec`.
        "codex" => Some(CliCredentialSpec {
            env_key: "CODEX_API_KEY",
            keychain_service: CODEX_CLI_KEYCHAIN_SERVICE,
        }),
        _ => None,
    }
}

/// Resolves a CLI-only API key without logging it or placing it in argv. A key explicitly saved
/// by AgentFlow wins over a process environment variable, so Finder/launchd runs are deterministic.
pub(crate) fn cli_credential_env(name: &str) -> HashMap<String, String> {
    let Some(spec) = credential_spec(name) else {
        return HashMap::new();
    };
    let key = keychain_key(spec.keychain_service).or_else(|| {
        std::env::var(spec.env_key)
            .ok()
            .filter(|value| !value.trim().is_empty())
    });
    key.map(|value| HashMap::from([(spec.env_key.to_string(), value)]))
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn keychain_key(service: &str) -> Option<String> {
    security_framework::passwords::get_generic_password(service, "AgentFlow")
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_os = "macos"))]
fn keychain_key(_service: &str) -> Option<String> {
    None
}
