async fn start_process(
    provider_name: &str,
    program: PathBuf,
    args: Vec<String>,
    req: AgentRunRequest,
    cancel: CancellationToken,
    tx: mpsc::Sender<AgentEvent>,
) -> Result<RunningAgent, AdapterError> {
    tokio::fs::create_dir_all(&req.run_dir).await?;
    let mut provider_env = cli_credential_env(provider_name);
    // A project-level deny rule remains authoritative even for AgentFlow-managed credentials.
    for key in &req.env_denylist {
        provider_env.remove(key);
    }
    let outcome = agentflow_process_supervisor::run(
        ProcessSpec {
            program,
            args,
            cwd: req.worktree,
            env: provider_env,
            env_denylist: req.env_denylist,
            timeout: req.timeout,
            idle_timeout: req.idle_timeout,
            stdout_path: req.run_dir.join("stdout.log"),
            stderr_path: req.run_dir.join("stderr.log"),
            lease_path: req.run_dir.join("process-lease.json"),
        },
        cancel,
        tx,
    )
    .await?;
    Ok(RunningAgent {
        outcome,
        run_dir: req.run_dir,
        role: req.role,
    })
}

async fn detect_cli(
    name: &str,
    path: &Path,
    flags: &[&str],
    capabilities: AgentCapabilities,
) -> Result<AgentInstallation, AdapterError> {
    let resolved = resolve_cli(name, path).await?;
    let version = output_text(&resolved, &["--version"]).await?;
    let help = output_text(&resolved, &["--help"]).await?;
    let missing = flags
        .iter()
        .filter(|f| !help.contains(**f))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(AdapterError::Incompatible(format!(
            "{name} misses {}",
            missing.join(", ")
        )));
    }
    Ok(AgentInstallation {
        path: resolved,
        version,
        capabilities,
    })
}
async fn resolve_cli(name: &str, path: &Path) -> Result<PathBuf, AdapterError> {
    if path.is_absolute() && path.exists() {
        return Ok(path.into());
    }
    if let Ok(found) = which::which(path) {
        return Ok(found);
    }
    #[cfg(unix)]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            let command = format!("command -v {name}");
            let out = Command::new(shell)
                .args(["-lic", &command])
                .stdin(Stdio::null())
                .output()
                .await?;
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !p.is_empty() {
                    return Ok(p.into());
                }
            }
        }
    }
    Err(AdapterError::NotFound(name.into()))
}
async fn output_text(program: &Path, args: &[&str]) -> Result<String, AdapterError> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await?;
    if !out.status.success() {
        return Err(AdapterError::Incompatible(
            String::from_utf8_lossy(&out.stderr).into(),
        ));
    }
    let mut text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&out.stderr).trim().to_string();
    }
    Ok(text)
}

pub(crate) async fn read_development(path: &Path) -> Result<DevelopmentResult, AdapterError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AdapterError::InvalidResult(format!("{}: {e}", path.display())))?;
    parse_development(&text)
}

async fn read_development_output(
    run_dir: &Path,
    provider: &str,
) -> Result<DevelopmentResult, AdapterError> {
    let mut errors = Vec::new();
    match read_development(&run_dir.join("result.json")).await {
        Ok(result) => return Ok(result),
        Err(error) => errors.push(error.to_string()),
    }
    let paths = match provider {
        "codex" => vec!["last-message.json", "stdout.log"],
        _ => vec!["stdout.log"],
    };
    for name in paths {
        let path = run_dir.join(name);
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => {
                let extracted = provider_output_text(provider, &text);
                match parse_development(extracted.as_deref().unwrap_or(&text)) {
                    Ok(result) => return Ok(result),
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    Err(AdapterError::InvalidResult(errors.join("; ")))
}

async fn read_plan_output(run_dir: &Path, provider: &str) -> Result<PlanResult, AdapterError> {
    let paths = match provider {
        "codex" => vec!["last-message.json", "stdout.log"],
        _ => vec!["stdout.log"],
    };
    let mut errors = Vec::new();
    for name in paths {
        let path = run_dir.join(name);
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => {
                let extracted = provider_output_text(provider, &text);
                match parse_plan(extracted.as_deref().unwrap_or(&text)) {
                    Ok(result) => return Ok(result),
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    Err(AdapterError::InvalidResult(errors.join("; ")))
}

fn parse_plan(text: &str) -> Result<PlanResult, AdapterError> {
    let mut candidates = json_object_candidates(text)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            collect_json_strings(&value, &mut candidates);
        }
    }
    let mut last_error = None;
    for candidate in candidates.into_iter().rev() {
        let parsed = (|| {
            let value: Value = serde_json::from_str(&candidate)
                .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
            validate_schema(&value, &plan_result_schema())?;
            let result: PlanResult = serde_json::from_value(value)
                .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
            if result.schema_version != 1 || result.plan_version < 1 || result.steps.is_empty() {
                return Err(AdapterError::InvalidResult(
                    "plan version and at least one step are required".into(),
                ));
            }
            Ok(result)
        })();
        match parsed {
            Ok(result) => return Ok(result),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AdapterError::InvalidResult("planner output does not contain a valid JSON object".into())
    }))
}

fn provider_output_text(provider: &str, text: &str) -> Option<String> {
    if provider == "claude" {
        return text.lines().rev().find_map(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .filter(|value| value.get("type").and_then(Value::as_str) == Some("result"))
                .and_then(|value| value.get("result").and_then(Value::as_str).map(str::to_owned))
        });
    }
    if provider == "gemini" {
        return serde_json::from_str::<Value>(text.trim())
            .ok()
            .and_then(|value| value.get("response").and_then(Value::as_str).map(str::to_owned));
    }
    None
}

fn parse_development(text: &str) -> Result<DevelopmentResult, AdapterError> {
    let mut candidates = json_object_candidates(text)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            collect_json_strings(&value, &mut candidates);
        }
    }
    let mut last_error = None;
    for candidate in candidates.into_iter().rev() {
        match parse_development_object(&candidate) {
            Ok(result) => return Ok(result),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AdapterError::InvalidResult("development output does not contain a JSON object".into())
    }))
}

fn collect_json_strings(value: &Value, candidates: &mut Vec<String>) {
    match value {
        Value::String(text) => candidates.extend(
            json_object_candidates(text)
                .into_iter()
                .map(str::to_owned),
        ),
        Value::Array(values) => {
            for value in values {
                collect_json_strings(value, candidates);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_json_strings(value, candidates);
            }
        }
        _ => {}
    }
}

fn parse_development_object(candidate: &str) -> Result<DevelopmentResult, AdapterError> {
    let value: Value = serde_json::from_str(candidate)
        .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
    validate_schema(&value, &development_result_schema())?;
    let result: DevelopmentResult =
        serde_json::from_value(value).map_err(|e| AdapterError::InvalidResult(e.to_string()))?;
    if result.schema_version != 1 {
        return Err(AdapterError::InvalidResult(
            "schema_version must be 1".into(),
        ));
    }
    if result.summary.is_empty() || result.summary.len() > 4000 {
        return Err(AdapterError::InvalidResult("summary length invalid".into()));
    }
    if result.status == DevelopmentStatus::NeedsClarification
        && result.question.as_deref().unwrap_or("").is_empty()
    {
        return Err(AdapterError::InvalidResult("question required".into()));
    }
    Ok(result)
}
pub(crate) async fn read_review(path: &Path) -> Result<ReviewResult, AdapterError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AdapterError::InvalidResult(format!("{}: {e}", path.display())))?;
    parse_review(&text)
}
async fn read_review_from_claude(path: &Path) -> Result<ReviewResult, AdapterError> {
    let text = tokio::fs::read_to_string(path).await?;
    for line in text.lines().rev() {
        if let Ok(v) = serde_json::from_str::<Value>(line)
            && v.get("type").and_then(Value::as_str) == Some("result")
            && let Some(s) = v.get("result").and_then(Value::as_str)
        {
            return parse_review(s);
        }
    }
    Err(AdapterError::InvalidResult(
        "Claude result event missing".into(),
    ))
}
async fn read_review_from_gemini(path: &Path) -> Result<ReviewResult, AdapterError> {
    let text = tokio::fs::read_to_string(path).await?;
    let envelope: Value = serde_json::from_str(text.trim())
        .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
    let response = envelope
        .get("response")
        .and_then(Value::as_str)
        .ok_or_else(|| AdapterError::InvalidResult("Gemini response field missing".into()))?;
    parse_review(response)
}
fn parse_review(text: &str) -> Result<ReviewResult, AdapterError> {
    let mut last_error = None;
    for candidate in json_object_candidates(text).into_iter().rev() {
        match parse_review_object(candidate) {
            Ok(review) => return Ok(review),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AdapterError::InvalidResult("review output does not contain a JSON object".into())
    }))
}

fn parse_review_object(candidate: &str) -> Result<ReviewResult, AdapterError> {
    let value: Value = serde_json::from_str(candidate)
        .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
    validate_schema(&value, &review_result_schema())?;
    let result: ReviewResult =
        serde_json::from_value(value).map_err(|e| AdapterError::InvalidResult(e.to_string()))?;
    if result.schema_version != 1 {
        return Err(AdapterError::InvalidResult(
            "schema_version must be 1".into(),
        ));
    }
    if result.decision != ReviewDecision::Pass && result.issues.is_empty() {
        return Err(AdapterError::InvalidResult(
            "non-pass review requires issues".into(),
        ));
    }
    Ok(result)
}

/// Providers sometimes add a short explanation before their required JSON. Extract complete
/// top-level objects without being confused by braces or escaped quotes inside JSON strings.
fn json_object_candidates(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut objects = Vec::new();
    let mut start = None;
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' if depth > 0 => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(object_start) = start.take()
                {
                    objects.push(&text[object_start..=index]);
                }
            }
            _ => {}
        }
    }
    objects
}
fn validate_schema<T: serde::Serialize>(value: &Value, schema: &T) -> Result<(), AdapterError> {
    let schema =
        serde_json::to_value(schema).map_err(|e| AdapterError::InvalidResult(e.to_string()))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|e| AdapterError::InvalidResult(e.to_string()))?;
    let errors = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AdapterError::InvalidResult(errors.join("; ")))
    }
}

pub async fn tool_status(name: &str, path: Option<PathBuf>, flags: &[&str]) -> ToolStatus {
    let candidate = path.unwrap_or_else(|| PathBuf::from(name));
    match resolve_cli(name, &candidate).await {
        Ok(p) => {
            let version = output_text(&p, &["--version"]).await.ok();
            let compatible = if flags.is_empty() {
                true
            } else {
                output_text(
                    &p,
                    if name == "codex" {
                        &["exec", "--help"]
                    } else {
                        &["--help"]
                    },
                )
                .await
                .map(|h| flags.iter().all(|f| h.contains(f)))
                .unwrap_or(false)
            };
            let (authenticated, auth_method, auth_problem) = if compatible {
                cli_auth_status(name, &p).await
            } else {
                (None, None, None)
            };
            ToolStatus {
                found: true,
                path: Some(p.to_string_lossy().into_owned()),
                version,
                compatible,
                problem: (!compatible).then(|| "required CLI flags are missing".into()),
                authenticated,
                auth_method,
                auth_problem,
            }
        }
        Err(e) => ToolStatus {
            found: false,
            path: None,
            version: None,
            compatible: false,
            problem: Some(e.to_string()),
            authenticated: None,
            auth_method: None,
            auth_problem: None,
        },
    }
}

async fn cli_auth_status(
    name: &str,
    program: &Path,
) -> (Option<bool>, Option<String>, Option<String>) {
    let args: &[&str] = match name {
        "claude" => &["auth", "status", "--json"],
        "codex" => &["login", "status"],
        "mmx" => &["auth", "status"],
        // Gemini and Qwen currently expose interactive authentication flows, but no stable
        // side-effect-free status command. Their credentials are verified on the first run.
        _ => return (None, None, None),
    };
    let credential_env = cli_credential_env(name);
    // CODEX_API_KEY is an official one-run credential for `codex exec`; it deliberately does not
    // alter `codex login status`, so its presence is the side-effect-free availability check.
    if name == "codex" && credential_env.contains_key("CODEX_API_KEY") {
        return (Some(true), Some("api_key".into()), None);
    }
    let output = Command::new(program)
        .args(args)
        .envs(&credential_env)
        .stdin(Stdio::null())
        .output()
        .await;
    let Ok(output) = output else {
        return (
            Some(false),
            None,
            Some("authentication status command failed".into()),
        );
    };
    let parsed_json = (name == "claude")
        .then(|| serde_json::from_slice::<Value>(&output.stdout).ok())
        .flatten();
    let authenticated = if name == "claude" {
        parsed_json
            .as_ref()
            .and_then(|value| value.get("loggedIn").and_then(Value::as_bool))
            .unwrap_or(output.status.success())
    } else if name == "mmx" {
        output.status.success()
    } else {
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output.status.success() && text.to_ascii_lowercase().contains("logged in")
    };
    let auth_method = if authenticated && name == "claude" {
        parsed_json.as_ref().and_then(claude_auth_method)
    } else if authenticated && name == "codex" {
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        codex_auth_method(&text).map(str::to_string)
    } else {
        None
    };
    let auth_problem = if authenticated {
        None
    } else if name == "claude" {
        claude_auth_problem(program).await
    } else {
        Some(format!("{name} is installed but not logged in"))
    };
    (Some(authenticated), auth_method, auth_problem)
}

fn claude_auth_method(status: &Value) -> Option<String> {
    if status.get("apiKeySource").and_then(Value::as_str).is_some() {
        return Some("api_key".into());
    }
    match status.get("authMethod").and_then(Value::as_str) {
        Some("claude.ai") => Some("account".into()),
        Some("oauth_token") => Some("oauth_token".into()),
        Some("api_key") => Some("api_key".into()),
        Some(method) if !method.is_empty() => Some(method.to_string()),
        _ => None,
    }
}

fn codex_auth_method(status: &str) -> Option<&'static str> {
    let text = status.to_ascii_lowercase();
    if text.contains("api key") {
        Some("api_key")
    } else if text.contains("access token") {
        Some("access_token")
    } else if text.contains("chatgpt") {
        Some("account")
    } else {
        None
    }
}

/// Claude stores subscription OAuth credentials in the macOS login keychain.
/// Surface the actionable cause when that keychain cannot be read or written,
/// instead of reducing every failure to the ambiguous "not logged in" state.
async fn claude_auth_problem(program: &Path) -> Option<String> {
    let doctor = tokio::time::timeout(
        Duration::from_secs(6),
        Command::new(program)
            .arg("doctor")
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .ok()
    .and_then(Result::ok);

    if let Some(output) = doctor {
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(problem) = claude_doctor_auth_problem(&text) {
            return Some(problem.into());
        }
    }

    Some("Claude Code 已安装，但当前进程没有可用的登录凭据".into())
}

fn claude_doctor_auth_problem(output: &str) -> Option<&'static str> {
    (output.contains("macOS Keychain is not writable")
        || output.contains("SecKeychainItemCreateFromContent")
        || output.contains("returned -25293"))
    .then_some("macOS 登录钥匙串不可写或密码不同步，Claude 无法保存 OAuth 登录凭据")
}
