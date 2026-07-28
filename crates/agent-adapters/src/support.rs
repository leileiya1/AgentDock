const PROVIDER_ENV_KEYS: [&str; 9] = [
    "HOME", "USER", "LOGNAME", "PATH", "TMPDIR", "LANG", "LC_ALL", "SHELL", "TERM",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompatibilityMatrix {
    schema_version: u32,
    providers: HashMap<String, CliCompatibility>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliCompatibility {
    verified_versions: Vec<String>,
    required_flags: Vec<String>,
    runtime_probe: Option<String>,
    request_policy: Option<String>,
}

fn compatibility_matrix() -> Result<CompatibilityMatrix, AdapterError> {
    let matrix: CompatibilityMatrix = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config/provider-compatibility.json"
    )))
    .map_err(|error| AdapterError::Incompatible(format!("invalid support matrix: {error}")))?;
    if matrix.schema_version != 1 {
        return Err(AdapterError::Incompatible(format!(
            "unsupported support matrix schema {}",
            matrix.schema_version
        )));
    }
    Ok(matrix)
}

fn normalized_cli_version(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|token| {
        let candidate = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-');
        let starts_with_digit = candidate
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_digit);
        (starts_with_digit && candidate.contains('.')).then(|| candidate.to_string())
    })
}

fn cli_request_policy(name: &str) -> Option<String> {
    compatibility_matrix()
        .ok()?
        .providers
        .remove(name)?
        .request_policy
}

fn support_level(
    name: &str,
    version_output: Option<&str>,
    flags_compatible: bool,
    matrix: &CompatibilityMatrix,
) -> (CliSupportLevel, Vec<String>) {
    let Some(entry) = matrix.providers.get(name) else {
        return (CliSupportLevel::Untracked, Vec::new());
    };
    if !flags_compatible {
        return (
            CliSupportLevel::Unsupported,
            entry.verified_versions.clone(),
        );
    }
    let installed = version_output.and_then(normalized_cli_version);
    let level = if installed.as_ref().is_some_and(|version| {
        entry
            .verified_versions
            .iter()
            .any(|verified| verified == version)
    }) {
        CliSupportLevel::Verified
    } else {
        CliSupportLevel::CompatibleUntested
    };
    (level, entry.verified_versions.clone())
}

fn provider_environment(provider_name: &str) -> HashMap<String, String> {
    let mut provider_env = cli_credential_env(provider_name);
    provider_env.extend(provider_identity_environment());
    provider_env
}

fn provider_identity_environment() -> HashMap<String, String> {
    let mut provider_env = HashMap::new();
    for key in PROVIDER_ENV_KEYS {
        if let Ok(value) = std::env::var(key) {
            provider_env.insert(key.into(), value);
        }
    }
    provider_env
}

struct ProcessEnvironment {
    additional: HashMap<String, String>,
    suppress: Vec<String>,
    load_provider_credential: bool,
}

impl Default for ProcessEnvironment {
    fn default() -> Self {
        Self {
            additional: HashMap::new(),
            suppress: Vec::new(),
            load_provider_credential: true,
        }
    }
}

async fn start_process(
    provider_name: &str,
    program: PathBuf,
    args: Vec<String>,
    req: AgentRunRequest,
    cancel: CancellationToken,
    tx: mpsc::Sender<AgentEvent>,
) -> Result<RunningAgent, AdapterError> {
    start_process_with_env(
        provider_name,
        program,
        args,
        req,
        cancel,
        tx,
        ProcessEnvironment::default(),
    )
    .await
}

async fn start_process_with_env(
    provider_name: &str,
    program: PathBuf,
    args: Vec<String>,
    req: AgentRunRequest,
    cancel: CancellationToken,
    tx: mpsc::Sender<AgentEvent>,
    environment: ProcessEnvironment,
) -> Result<RunningAgent, AdapterError> {
    // Single choke point for every CLI Provider: an extra allowed command carrying list
    // separators would widen a CLI's own permission parsing beyond what was approved.
    // Fail before spawning rather than after the process already holds the wider grant.
    validate_extra_allowed_commands(&req.extra_allowed_commands)?;
    tokio::fs::create_dir_all(&req.run_dir).await?;
    let mut provider_env = if environment.load_provider_credential {
        provider_environment(provider_name)
    } else {
        provider_identity_environment()
    };
    provider_env.extend(environment.additional);
    for key in environment.suppress {
        provider_env.remove(&key);
    }
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
            clear_environment: true,
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

async fn resolve_codex_cli(path: &Path) -> Result<PathBuf, AdapterError> {
    if path != Path::new("codex") {
        return resolve_cli("codex", path).await;
    }

    let mut candidates = Vec::new();
    if let Ok(found) = resolve_cli("codex", path).await {
        candidates.push(found);
    }
    #[cfg(target_os = "macos")]
    candidates.push(PathBuf::from(
        "/Applications/ChatGPT.app/Contents/Resources/codex",
    ));
    candidates.dedup();

    let mut installed = Vec::new();
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        if let Ok(version) = output_text(&candidate, &["--version"]).await {
            installed.push((candidate, version));
        }
    }
    select_codex_candidate(&installed, codex_cache_client_version().as_deref())
        .ok_or_else(|| AdapterError::NotFound("codex".into()))
}

fn select_codex_candidate(
    candidates: &[(PathBuf, String)],
    cache_client_version: Option<&str>,
) -> Option<PathBuf> {
    cache_client_version
        .and_then(|cache_version| {
            candidates.iter().find_map(|(path, version)| {
                (codex_base_version(version) == Some(cache_version)).then(|| path.clone())
            })
        })
        .or_else(|| candidates.first().map(|(path, _)| path.clone()))
}

fn codex_base_version(version_output: &str) -> Option<&str> {
    version_output
        .split_whitespace()
        .last()
        .and_then(|version| version.split('-').next())
        .filter(|version| !version.is_empty())
}

fn codex_cache_client_version() -> Option<String> {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))?;
    let cache = std::fs::read_to_string(codex_home.join("models_cache.json")).ok()?;
    serde_json::from_str::<Value>(&cache)
        .ok()?
        .get("client_version")?
        .as_str()
        .map(str::to_owned)
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

/// A run whose logs hit the supervisor's size ceiling lost the tail of stdout — exactly where a
/// Provider writes its final result. Recovering from such a log would accept an earlier draft
/// (echoed tool results, sample JSON in assistant text) as the authoritative deliverable, so the
/// stdout fallback is refused. Atomically written artifacts (result.json) stay trusted.
async fn log_recovery_is_unsafe(run_dir: &Path) -> bool {
    agentflow_process_supervisor::read_process_log_truncated(&run_dir.join("process-outcome.json"))
        .await
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
    if log_recovery_is_unsafe(run_dir).await {
        return Err(AdapterError::InvalidResult(format!(
            "provider logs were truncated at the size limit, so no result can be recovered from them: {}",
            errors.join("; ")
        )));
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
    if log_recovery_is_unsafe(run_dir).await {
        return Err(AdapterError::InvalidResult(
            "provider logs were truncated at the size limit, so no plan can be recovered from them"
                .into(),
        ));
    }
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
            let mut value: Value = serde_json::from_str(&candidate)
                .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
            if let Some(steps) = value.get_mut("steps").and_then(Value::as_array_mut) {
                for step in steps {
                    insert_null_for_missing(step, &["validation"]);
                }
            }
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
    let mut value: Value = serde_json::from_str(candidate)
        .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
    insert_null_for_missing(
        &mut value,
        &["question", "changed_files", "notes", "plan_sha256"],
    );
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
/// Same guard as [`log_recovery_is_unsafe`], for readers that are handed a file path. Only
/// mirrored logs can be truncated mid-result; atomically written artifacts stay trusted.
async fn log_file_recovery_is_unsafe(path: &Path) -> bool {
    if path.file_name().and_then(|name| name.to_str()) != Some("stdout.log") {
        return false;
    }
    match path.parent() {
        Some(run_dir) => log_recovery_is_unsafe(run_dir).await,
        None => false,
    }
}

pub(crate) async fn read_review(path: &Path) -> Result<ReviewResult, AdapterError> {
    if log_file_recovery_is_unsafe(path).await {
        return Err(AdapterError::InvalidResult(
            "provider logs were truncated at the size limit, so no review can be recovered from them".into(),
        ));
    }
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AdapterError::InvalidResult(format!("{}: {e}", path.display())))?;
    parse_review(&text)
}
async fn read_review_from_claude(path: &Path) -> Result<ReviewResult, AdapterError> {
    if log_file_recovery_is_unsafe(path).await {
        return Err(AdapterError::InvalidResult(
            "provider logs were truncated at the size limit, so no review can be recovered from them".into(),
        ));
    }
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
    if log_file_recovery_is_unsafe(path).await {
        return Err(AdapterError::InvalidResult(
            "provider logs were truncated at the size limit, so no review can be recovered from them".into(),
        ));
    }
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
    let mut value: Value = serde_json::from_str(candidate)
        .map_err(|error| AdapterError::InvalidResult(error.to_string()))?;
    if let Some(issues) = value.get_mut("issues").and_then(Value::as_array_mut) {
        for issue in issues {
            insert_null_for_missing(
                issue,
                &[
                    "file",
                    "line_start",
                    "line_end",
                    "description",
                    "suggested_action",
                ],
            );
        }
    }
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

fn insert_null_for_missing(value: &mut Value, fields: &[&str]) {
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            object.entry(*field).or_insert(Value::Null);
        }
    }
}

/// Providers sometimes add a short explanation before their required JSON. Extract complete
/// top-level objects without being confused by braces or escaped quotes inside JSON strings.
///
/// Prose is not JSON, so an unmatched `{` in surrounding text (a `"{placeholder"` in a log line,
/// a code sample) must not swallow every later object. The scan therefore tracks string state at
/// every depth, and when it ends while still open it restarts just after the offending brace so
/// the real result is still found.
fn json_object_candidates(text: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let (found, resume) = scan_json_objects(text, offset);
        objects.extend(found);
        match resume {
            Some(next) if next > offset => offset = next,
            _ => break,
        }
    }
    objects
}

/// Scans from `offset` and returns the complete top-level objects found, plus the byte offset to
/// restart from when the scan ended inside an unterminated object.
fn scan_json_objects(text: &str, offset: usize) -> (Vec<&str>, Option<usize>) {
    let bytes = text.as_bytes();
    let mut objects = Vec::new();
    let mut start = None;
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;

    for index in offset..bytes.len() {
        let byte = bytes[index];
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
            // Tracked at every depth: a quote outside an object still opens a string, so a
            // stray brace inside prose quotes cannot unbalance the scan.
            b'"' => in_string = true,
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
    // An unterminated object means the text was truncated or contained a bare `{`. Resume just
    // past that brace so genuine objects appearing after it are still extracted.
    let resume = (depth > 0).then(|| start.map_or(bytes.len(), |value| value + 1));
    (objects, resume)
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
    let resolved = if name == "codex" {
        resolve_codex_cli(&candidate).await
    } else {
        resolve_cli(name, &candidate).await
    };
    match resolved {
        Ok(p) => {
            let version = output_text(&p, &["--version"]).await.ok();
            let matrix = compatibility_matrix();
            let required_flags = matrix
                .as_ref()
                .ok()
                .and_then(|matrix| matrix.providers.get(name))
                .map(|entry| entry.required_flags.iter().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_else(|| flags.to_vec());
            let help = if required_flags.is_empty() {
                None
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
                .ok()
            };
            let missing_flags = if required_flags.is_empty() {
                Vec::new()
            } else {
                required_flags
                    .iter()
                    .filter(|flag| help.as_deref().is_none_or(|value| !value.contains(**flag)))
                    .copied()
                    .collect::<Vec<_>>()
            };
            let compatible = if required_flags.is_empty() {
                true
            } else {
                missing_flags.is_empty()
            };
            let (authenticated, auth_method, auth_problem) = if compatible {
                cli_auth_status(name, &p).await
            } else {
                (None, None, None)
            };
            let (support_level, verified_versions) = matrix
                .as_ref()
                .map(|matrix| support_level(name, version.as_deref(), compatible, matrix))
                .unwrap_or((CliSupportLevel::Unsupported, Vec::new()));
            let problem = matrix
                .as_ref()
                .err()
                .map(ToString::to_string)
                .or_else(|| {
                    (!compatible).then(|| {
                        format!("required CLI flags are missing: {}", missing_flags.join(", "))
                    })
                });
            ToolStatus {
                found: true,
                path: Some(p.to_string_lossy().into_owned()),
                version,
                compatible,
                problem,
                authenticated,
                auth_method,
                auth_problem,
                support_level,
                verified_versions,
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
            support_level: CliSupportLevel::Untracked,
            verified_versions: compatibility_matrix()
                .ok()
                .and_then(|matrix| matrix.providers.get(name).cloned())
                .map(|entry| entry.verified_versions)
                .unwrap_or_default(),
        },
    }
}

const RUNTIME_PROBE_MARKER: &str = "AGENTFLOW_PROBE_OK";

/// Whether this CLI has a side-effect-free, non-interactive runtime probe implementation. The
/// matrix declaration and the implementation whitelist must both agree, so a typo or a future
/// unimplemented strategy fails closed instead of launching an unknown command shape.
pub fn runtime_probe_supported(name: &str) -> bool {
    compatibility_matrix()
        .ok()
        .and_then(|matrix| matrix.providers.get(name).cloned())
        .and_then(|entry| entry.runtime_probe)
        .is_some_and(|strategy| {
            matches!(
                strategy.as_str(),
                "claude_stream_json"
                    | "codex_strict_schema"
                    | "qoder_plan_json"
                    | "grok_plan_json"
            )
        })
}

fn classify_runtime_probe_failure(text: &str, exit_code: Option<i32>) -> String {
    let lower = text.to_ascii_lowercase();
    if [
        "not logged in",
        "please log in",
        "authentication_required",
        "authentication required",
        "unauthorized",
        "invalid api key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "真实探针认证失败，请在终端重新登录或更新 CLI 凭据".into();
    }
    if [
        "rate_limit",
        "rate limit",
        "usage limit",
        "hit your limit",
        "five-hour limit",
        "quota",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "真实探针触发额度或速率限制，已安全降级到后备 Provider".into();
    }
    if [
        "unknown option",
        "unexpected argument",
        "output schema",
        "invalid schema",
        "models_cache",
        "client_version",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "真实探针发现 CLI 协议或结构化输出不兼容，请切换到已验证版本".into();
    }
    format!(
        "真实探针未成功（退出码 {}），已安全降级到后备 Provider",
        exit_code.map_or_else(|| "未知".into(), |code| code.to_string())
    )
}

/// Execute a tiny, read-only request against the real Claude/Codex runtime. The probe runs inside
/// an isolated temporary git repository, carries the same minimal environment used by real jobs,
/// disallows writes/tools, and returns only a categorized verdict so provider output is never
/// surfaced to the desktop or persisted in the task database.
pub async fn probe_cli_runtime(name: &str, program: &Path) -> CliRuntimeProbe {
    probe_cli_runtime_in(name, program, None).await
}

/// Same safe probe with an explicitly authorized working directory. The desktop uses the
/// isolated default above; this entry point exists for production acceptance where the operator
/// deliberately confines every external Provider call to a known fixture repository.
pub async fn probe_cli_runtime_at(
    name: &str,
    program: &Path,
    authorized_cwd: &Path,
) -> CliRuntimeProbe {
    probe_cli_runtime_in(name, program, Some(authorized_cwd)).await
}

async fn probe_cli_runtime_in(
    name: &str,
    program: &Path,
    authorized_cwd: Option<&Path>,
) -> CliRuntimeProbe {
    let strategy = compatibility_matrix()
        .ok()
        .and_then(|matrix| matrix.providers.get(name).cloned())
        .and_then(|entry| entry.runtime_probe);
    if strategy.as_deref().is_none_or(|strategy| {
        !matches!(
            strategy,
            "claude_stream_json"
                | "codex_strict_schema"
                | "qoder_plan_json"
                | "grok_plan_json"
        )
    }) {
        return CliRuntimeProbe {
            passed: false,
            problem: Some(format!("{name} 尚未实现真实运行探针")),
        };
    }
    let temp = match tempfile::Builder::new()
        .prefix("agentflow-provider-probe-")
        .tempdir()
    {
        Ok(value) => value,
        Err(error) => {
            return CliRuntimeProbe {
                passed: false,
                problem: Some(format!("无法创建真实探针目录：{error}")),
            };
        }
    };
    let git_init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(temp.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    if !git_init.is_ok_and(|status| status.success()) {
        return CliRuntimeProbe {
            passed: false,
            problem: Some("无法初始化真实探针的临时 Git 仓库".into()),
        };
    }
    let probe_cwd = authorized_cwd.unwrap_or_else(|| temp.path());
    if !probe_cwd.is_dir() {
        return CliRuntimeProbe {
            passed: false,
            problem: Some(format!("真实探针目录不存在：{}", probe_cwd.display())),
        };
    }

    let last_message = temp.path().join("last-message.json");
    let schema_path = temp.path().join("probe.schema.json");
    let schema = json!({
        "type": "object",
        "properties": {"probe": {"type": "string", "const": RUNTIME_PROBE_MARKER}},
        "required": ["probe"],
        "additionalProperties": false
    });
    if let Err(error) = tokio::fs::write(
        &schema_path,
        serde_json::to_vec_pretty(&schema).unwrap_or_default(),
    )
    .await
    {
        return CliRuntimeProbe {
            passed: false,
            problem: Some(format!("无法写入真实探针 schema：{error}")),
        };
    }

    let args = match strategy.as_deref() {
        Some("claude_stream_json") => vec![
            "-p".into(),
            format!("只回复 {RUNTIME_PROBE_MARKER}，不要读取文件、不要调用工具"),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
            "--permission-mode".into(),
            "plan".into(),
            "--disallowedTools".into(),
            "Read,Write,Edit,Bash,Glob,Grep,WebFetch,WebSearch".into(),
            "--max-turns".into(),
            "1".into(),
        ],
        Some("codex_strict_schema") => vec![
            "exec".into(),
            "--ignore-user-config".into(),
            "--ephemeral".into(),
            "--disable".into(),
            "plugins".into(),
            "--disable".into(),
            "remote_plugin".into(),
            "--disable".into(),
            "apps".into(),
            "--disable".into(),
            "memories".into(),
            "--cd".into(),
            probe_cwd.to_string_lossy().into_owned(),
            "--sandbox".into(),
            "read-only".into(),
            "--json".into(),
            "-o".into(),
            last_message.to_string_lossy().into_owned(),
            "--output-schema".into(),
            schema_path.to_string_lossy().into_owned(),
            format!("只输出 JSON：{{\"probe\":\"{RUNTIME_PROBE_MARKER}\"}}，不要调用工具"),
        ],
        Some("qoder_plan_json") => vec![
            "-p".into(),
            format!("只回复 {RUNTIME_PROBE_MARKER}，不要读取文件、不要调用工具"),
            "--cwd".into(),
            probe_cwd.to_string_lossy().into_owned(),
            "--output-format".into(),
            "json".into(),
            "--permission-mode".into(),
            "plan".into(),
            "--tools".into(),
            "".into(),
            "--no-session-persistence".into(),
            "--max-output-tokens".into(),
            "128".into(),
        ],
        Some("grok_plan_json") => vec![
            "-p".into(),
            format!("只回复 {RUNTIME_PROBE_MARKER}，不要读取文件、不要调用工具"),
            "--cwd".into(),
            probe_cwd.to_string_lossy().into_owned(),
            "--output-format".into(),
            "json".into(),
            "--permission-mode".into(),
            "plan".into(),
            "--sandbox".into(),
            "read-only".into(),
            "--tools".into(),
            "".into(),
            "--max-turns".into(),
            "1".into(),
            "--no-memory".into(),
            "--no-subagents".into(),
            "--disable-web-search".into(),
        ],
        _ => Vec::new(),
    };
    let request_policy = cli_request_policy(name);
    if name == "grok"
        && request_policy.as_deref().is_some_and(|policy| {
            policy != "deepseek_forced_tool_choice_non_thinking"
        })
    {
        return CliRuntimeProbe {
            passed: false,
            problem: Some(format!(
                "未知 Grok 请求兼容策略：{}，已拒绝运行",
                request_policy.as_deref().unwrap_or_default()
            )),
        };
    }
    let compat = if name == "grok"
        && request_policy.as_deref() == Some("deepseek_forced_tool_choice_non_thinking")
    {
        match deepseek_compat::prepare_grok_deepseek_compat(temp.path(), probe_cwd, &[]).await {
            Ok(value) => value,
            Err(error) => {
                return CliRuntimeProbe {
                    passed: false,
                    problem: Some(format!("Grok 兼容网关探针失败：{error}")),
                };
            }
        }
    } else {
        None
    };
    let mut environment = if compat.is_some() {
        provider_identity_environment()
    } else {
        provider_environment(name)
    };
    if let Some(compat) = &compat {
        environment.extend(compat.environment());
        environment.remove("DEEPSEEK_API_KEY");
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(probe_cwd)
        .env_clear()
        .envs(environment)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(45), command.output()).await;
    if let Some(compat) = compat {
        compat.shutdown().await;
    }
    let output = match output {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return CliRuntimeProbe {
                passed: false,
                problem: Some(format!("无法启动真实探针：{error}")),
            };
        }
        Err(_) => {
            return CliRuntimeProbe {
                passed: false,
                problem: Some("真实探针 45 秒未响应，已安全降级到后备 Provider".into()),
            };
        }
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    if let Ok(last) = tokio::fs::read_to_string(&last_message).await {
        combined.push_str(&last);
    }
    let deepseek_aux_protocol_error = combined.contains("400")
        && combined.to_ascii_lowercase().contains("thinking")
        && combined.contains("tool_choice");
    if output.status.success()
        && combined.contains(RUNTIME_PROBE_MARKER)
        && !deepseek_aux_protocol_error
    {
        CliRuntimeProbe {
            passed: true,
            problem: None,
        }
    } else {
        CliRuntimeProbe {
            passed: false,
            problem: Some(classify_runtime_probe_failure(
                &combined,
                output.status.code(),
            )),
        }
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
