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
            resume_session_id: None,
            extra_allowed_commands: Vec::new(),
            env_denylist: Vec::new(),
        }
    }

    #[test]
    fn review_semantic_rule() {
        let raw = r#"{"schema_version":1,"task_id":"t","revision":1,"commit_sha":"1234567","decision":"request_changes","summary":"x","issues":[]}"#;
        assert!(parse_review(raw).is_err());
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
            .call_api("review this diff", &CancellationToken::new(), &tx)
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
            .call_api("review this diff", &CancellationToken::new(), &tx)
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
                    resume_session_id: None,
                    extra_allowed_commands: Vec::new(),
                    env_denylist: Vec::new(),
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
