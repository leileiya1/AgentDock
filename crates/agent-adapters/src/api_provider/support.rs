fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(500_u64.saturating_mul(1_u64 << attempt.min(6)))
}

async fn emit_provider_event(tx: &mpsc::Sender<AgentEvent>, summary: String) {
    let _ = tx
        .send(AgentEvent {
            ts: Utc::now().to_rfc3339(),
            stream: EventStream::Stdout,
            kind: AgentEventKind::System,
            summary,
            text: None,
        })
        .await;
}

fn safe_error_body(body: &str) -> String {
    agentflow_process_supervisor::redact(body.chars().take(2_000).collect())
}

pub fn api_provider_status(settings: &ApiProviderSettings) -> ProviderStatus {
    let configured = !settings.model.trim().is_empty()
        && !settings.base_url.trim().is_empty()
        && !settings.api_key_env.trim().is_empty();
    let key_available = resolve_api_key(settings).is_ok();
    ProviderStatus {
        configured,
        available: configured && key_available,
        model: settings.model.clone(),
        base_url: settings.base_url.clone(),
        key_env: settings.api_key_env.clone(),
        problem: if !configured {
            Some("base URL, model, and key environment variable are required".into())
        } else if !key_available {
            Some(format!(
                "{} is not set and Keychain service {} has no key",
                settings.api_key_env, settings.keychain_service
            ))
        } else {
            None
        },
    }
}

fn resolve_api_key(settings: &ApiProviderSettings) -> Result<String, String> {
    if let Ok(key) = std::env::var(&settings.api_key_env)
        && !key.trim().is_empty()
    {
        return Ok(key);
    }
    #[cfg(target_os = "macos")]
    {
        // New desktop credentials use Security.framework directly so the secret never appears in
        // a process argument. Keep the `security` fallback for credentials created by older builds.
        if let Ok(bytes) = security_framework::passwords::get_generic_password(
            &settings.keychain_service,
            "AgentFlow",
        ) && let Ok(key) = String::from_utf8(bytes)
            && !key.trim().is_empty()
        {
            return Ok(key.trim().to_string());
        }
        let output = std::process::Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                &settings.keychain_service,
                "-w",
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }
    Err(format!(
        "{} is not set and no Keychain credential is available",
        settings.api_key_env
    ))
}
