#[async_trait]
impl AgentProvider for ClaudeCodeAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }
    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streams_events: true,
            native_output_schema: false,
            supports_resume: true,
            read_only_mode: true,
            supports_development: true,
            supports_review: true,
        }
    }
    fn budget_capabilities(&self) -> BudgetCapabilities {
        BudgetCapabilities {
            tokens: BudgetMode::Soft,
            // Claude Code enforces this inside the CLI with --max-budget-usd.
            cost: BudgetMode::Hard,
        }
    }
    async fn detect(&self, env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        detect_cli(
            "claude",
            env.explicit_path.as_ref().unwrap_or(&self.executable),
            &["--output-format", "--permission-mode"],
            self.capabilities(),
        )
        .await
    }
    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        let args = claude_args(&req);
        start_process("claude", self.executable.clone(), args, req, cancel, tx).await
    }
    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        match role {
            RunRole::Planner => read_plan_output(run_dir, "claude")
                .await
                .map(CollectedResult::Plan),
            RunRole::Developer => read_development_output(run_dir, "claude")
                .await
                .map(CollectedResult::Development),
            RunRole::Reviewer => read_review_from_claude(&run_dir.join("stdout.log"))
                .await
                .map(CollectedResult::Review),
            _ => Err(AdapterError::UnsupportedRole(role)),
        }
    }
}

fn claude_args(req: &AgentRunRequest) -> Vec<String> {
    let prompt = format!(
        "请完整阅读 {}，按其要求完成任务；结束前必须按说明写出结构化结果",
        req.input_file
    );
    let mut allowed = vec![
        "Edit",
        "Write",
        "Read",
        "Grep",
        "Glob",
        "Bash(git status:*)",
        "Bash(git diff:*)",
        "Bash(git log:*)",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    allowed.extend(
        req.extra_allowed_commands
            .iter()
            .map(|value| format!("Bash({value}:*)")),
    );
    let mut args = vec![
        "-p".into(),
        prompt,
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--permission-mode".into(),
        "acceptEdits".into(),
        "--allowedTools".into(),
        allowed.join(","),
        "--max-turns".into(),
        "100".into(),
    ];
    if req.role == RunRole::Reviewer || matches!(req.permission, PermissionTier::ReadOnly) {
        args.extend(["--disallowedTools".into(), "Write,Edit".into()]);
    }
    if matches!(req.permission, PermissionTier::Yolo) {
        args.push("--dangerously-skip-permissions".into());
    }
    if let Some(session_id) = &req.resume_session_id {
        args.extend(["--resume".into(), session_id.clone()]);
    }
    if let Some(remaining) = req.budget.remaining_cost_usd {
        args.extend(["--max-budget-usd".into(), format!("{remaining:.6}")]);
    }
    args
}

#[async_trait]
impl AgentProvider for CodexAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }
    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streams_events: true,
            native_output_schema: true,
            supports_resume: false,
            read_only_mode: true,
            supports_development: true,
            supports_review: true,
        }
    }
    async fn detect(&self, env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        let path = resolve_cli(
            "codex",
            env.explicit_path.as_ref().unwrap_or(&self.executable),
        )
        .await?;
        let version = output_text(&path, &["--version"]).await?;
        let help = output_text(&path, &["exec", "--help"]).await?;
        if !["--json", "--sandbox", "--ignore-user-config", "--ephemeral"]
            .iter()
            .all(|flag| help.contains(flag))
        {
            return Err(AdapterError::Incompatible(
                "codex exec misses required JSON, sandbox, or isolation flags".into(),
            ));
        }
        Ok(AgentInstallation {
            path,
            version,
            capabilities: self.capabilities(),
        })
    }
    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        let args = codex_args(&req, &self.schema_path);
        start_process("codex", self.executable.clone(), args, req, cancel, tx).await
    }
    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        match role {
            RunRole::Planner => read_plan_output(run_dir, "codex")
                .await
                .map(CollectedResult::Plan),
            RunRole::Developer => read_development_output(run_dir, "codex")
                .await
                .map(CollectedResult::Development),
            RunRole::Reviewer => read_review(&run_dir.join("last-message.json"))
                .await
                .map(CollectedResult::Review),
            _ => Err(AdapterError::UnsupportedRole(role)),
        }
    }
}

fn codex_args(req: &AgentRunRequest, schema_path: &Path) -> Vec<String> {
    let review = req.role == RunRole::Reviewer;
    let sandbox = if matches!(req.permission, PermissionTier::Yolo) {
        "danger-full-access"
    } else if review || matches!(req.permission, PermissionTier::ReadOnly) {
        "read-only"
    } else {
        "workspace-write"
    };
    let last = req.run_dir.join("last-message.json");
    let mut args = vec![
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
        req.worktree.to_string_lossy().into_owned(),
        "--sandbox".into(),
        sandbox.into(),
        "--json".into(),
        "-o".into(),
        last.to_string_lossy().into_owned(),
    ];
    if review || matches!(req.permission, PermissionTier::ReadOnly) {
        let output_schema = match req.role {
            RunRole::Reviewer => schema_path.to_path_buf(),
            RunRole::Planner => schema_path.with_file_name("plan.schema.json"),
            _ => schema_path.with_file_name("result.schema.json"),
        };
        args.extend([
            "--output-schema".into(),
            output_schema.to_string_lossy().into_owned(),
        ]);
    }
    args.push(format!(
        "请完整阅读 {}，独立完成指定的{}任务并严格按 schema 输出 JSON",
        req.input_file,
        match req.role {
            RunRole::Reviewer => "审查",
            RunRole::Planner => "只读规划",
            _ => "开发",
        }
    ));
    args
}

#[async_trait]
impl AgentProvider for GeminiCliAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::GeminiCli
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streams_events: true,
            native_output_schema: false,
            // No stable, non-interactive resume contract is wired yet.
            supports_resume: false,
            read_only_mode: true,
            supports_development: true,
            supports_review: true,
        }
    }

    async fn detect(&self, env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        detect_cli(
            "gemini",
            env.explicit_path.as_ref().unwrap_or(&self.executable),
            &["--output-format", "--approval-mode", "--sandbox"],
            self.capabilities(),
        )
        .await
    }

    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        let args = gemini_args(&req);
        start_process("gemini", self.executable.clone(), args, req, cancel, tx).await
    }

    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        match role {
            RunRole::Planner => read_plan_output(run_dir, "gemini")
                .await
                .map(CollectedResult::Plan),
            RunRole::Developer => read_development_output(run_dir, "gemini")
                .await
                .map(CollectedResult::Development),
            RunRole::Reviewer => read_review_from_gemini(&run_dir.join("stdout.log"))
                .await
                .map(CollectedResult::Review),
            _ => Err(AdapterError::UnsupportedRole(role)),
        }
    }
}

#[async_trait]
impl AgentProvider for QwenCodeAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::QwenCode
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streams_events: true,
            native_output_schema: true,
            // No stable, non-interactive resume contract is wired yet.
            supports_resume: false,
            read_only_mode: true,
            supports_development: true,
            supports_review: true,
        }
    }

    async fn detect(&self, env: &CliEnv) -> Result<AgentInstallation, AdapterError> {
        detect_cli(
            "qwen",
            env.explicit_path.as_ref().unwrap_or(&self.executable),
            &[
                "--output-format",
                "--approval-mode",
                "--sandbox",
                "--max-wall-time",
            ],
            self.capabilities(),
        )
        .await
    }

    async fn start(
        &self,
        req: AgentRunRequest,
        cancel: CancellationToken,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RunningAgent, AdapterError> {
        let args = qwen_args(&req, &self.schema_path);
        start_process("qwen", self.executable.clone(), args, req, cancel, tx).await
    }

    async fn collect_result(
        &self,
        run_dir: &Path,
        role: RunRole,
    ) -> Result<CollectedResult, AdapterError> {
        match role {
            RunRole::Planner => read_plan_output(run_dir, "qwen")
                .await
                .map(CollectedResult::Plan),
            RunRole::Developer => read_development_output(run_dir, "qwen")
                .await
                .map(CollectedResult::Development),
            RunRole::Reviewer => read_review(&run_dir.join("stdout.log"))
                .await
                .map(CollectedResult::Review),
            _ => Err(AdapterError::UnsupportedRole(role)),
        }
    }
}

fn agentflow_prompt(req: &AgentRunRequest) -> String {
    format!(
        "请完整阅读 {}，独立完成指定的{}任务；必须遵守文件中的结构化输出约定",
        req.input_file,
        match req.role {
            RunRole::Reviewer => "只读审查",
            RunRole::Planner => "只读规划",
            _ => "开发",
        }
    )
}

fn gemini_args(req: &AgentRunRequest) -> Vec<String> {
    let review = req.role == RunRole::Reviewer;
    let mut args = vec![
        "-p".into(),
        agentflow_prompt(req),
        "--output-format".into(),
        if review || matches!(req.permission, PermissionTier::ReadOnly) {
            "json"
        } else {
            "stream-json"
        }
        .into(),
        "--approval-mode".into(),
        if review || matches!(req.permission, PermissionTier::ReadOnly) {
            "plan"
        } else {
            "yolo"
        }
        .into(),
    ];
    if review || !matches!(req.permission, PermissionTier::Yolo) {
        args.push("--sandbox".into());
    }
    args
}

fn qwen_args(req: &AgentRunRequest, schema_path: &Path) -> Vec<String> {
    let review = req.role == RunRole::Reviewer;
    let mut args = vec![
        "-p".into(),
        agentflow_prompt(req),
        "--output-format".into(),
        if review || matches!(req.permission, PermissionTier::ReadOnly) {
            "text"
        } else {
            "stream-json"
        }
        .into(),
        "--approval-mode".into(),
        if review || matches!(req.permission, PermissionTier::ReadOnly) {
            "plan"
        } else {
            "yolo"
        }
        .into(),
        "--max-wall-time".into(),
        format!("{}s", req.timeout.as_secs()),
        "--max-tool-calls".into(),
        "200".into(),
    ];
    if review || matches!(req.permission, PermissionTier::ReadOnly) {
        let output_schema = match req.role {
            RunRole::Reviewer => schema_path.to_path_buf(),
            RunRole::Planner => schema_path.with_file_name("plan.schema.json"),
            _ => schema_path.with_file_name("result.schema.json"),
        };
        args.extend([
            "--json-schema".into(),
            format!("@{}", output_schema.to_string_lossy()),
        ]);
    }
    if review || !matches!(req.permission, PermissionTier::Yolo) {
        args.push("--sandbox".into());
    }
    args
}
