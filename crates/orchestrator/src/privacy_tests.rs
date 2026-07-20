use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer as _, SigningKey};

#[tokio::test]
async fn api_review_requires_explicit_task_scoped_egress_approval()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("p", "/tmp/privacy", "main", "/tmp/wt")
        .await?;

    let denied = orchestrator
        .task_create(
            &project.id,
            "private review",
            "must remain local",
            AgentKind::ClaudeCode,
            AgentKind::DeepSeekApi,
            None,
            None,
        )
        .await;
    assert!(matches!(
        denied,
        Err(OrchestratorError::InvalidState(message))
            if message == "API_EGRESS_APPROVAL_REQUIRED"
    ));

    let approved = orchestrator
        .task_create_with_api_egress(
            &project.id,
            "approved review",
            "may use configured API",
            AgentKind::ClaudeCode,
            AgentKind::DeepSeekApi,
            None,
            None,
            true,
        )
        .await?;
    assert!(orchestrator.task(&approved.id).await?.api_egress_approved);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM events WHERE task_id=? AND event_type='privacy:api_egress_approved'",
    )
    .bind(&approved.id)
    .fetch_one(orchestrator.store.pool())
    .await?;
    assert_eq!(audit_count, 1);
    let (actor, payload): (String, String) = sqlx::query_as(
        "SELECT actor,payload_json FROM events WHERE task_id=? AND event_type='privacy:api_egress_approved'",
    )
    .bind(&approved.id)
    .fetch_one(orchestrator.store.pool())
    .await?;
    assert_eq!(actor, "human");
    assert!(payload.contains("deepseek_api"));
    Ok(())
}

#[tokio::test]
async fn signed_external_remote_provider_uses_the_same_egress_gate()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let providers = dir.path().join("providers");
    let package = providers.join("remote-fixture");
    tokio::fs::create_dir_all(&package).await?;
    let executable = package.join("provider-bin");
    tokio::fs::write(&executable, b"fixture executable").await?;
    let mut manifest = agentflow_provider_protocol::ProviderManifest {
        manifest_version: 1,
        id: AgentKind::External("remote_fixture".into()),
        display_name: "Remote Fixture".into(),
        protocol_version: agentflow_provider_protocol::PROTOCOL_VERSION.into(),
        executable: "provider-bin".into(),
        args: Vec::new(),
        transport: agentflow_provider_protocol::TransportKind::StdioJsonRpc,
        capabilities: ProviderCapabilities {
            development: false,
            review: true,
            streaming: true,
            structured_output: true,
            sandbox: true,
            resume: false,
        },
        execution_location: ExecutionLocation::Remote,
        data_egress: DataEgress::Diff,
        permissions: ProviderPermissions {
            worktree_read: false,
            worktree_write: false,
            network_domains: vec!["review.example".into()],
            commands: Vec::new(),
        },
        security: Some(agentflow_provider_protocol::ProviderSecurity {
            publisher: "test-publisher".into(),
            artifact_sha256: format!("{:x}", Sha256::digest(b"fixture executable")),
            signature: String::new(),
        }),
        enabled: true,
    };
    let key = SigningKey::from_bytes(&[13_u8; 32]);
    manifest
        .security
        .as_mut()
        .ok_or("security missing")?
        .signature = STANDARD.encode(key.sign(&manifest.signing_payload()?).to_bytes());
    tokio::fs::write(
        package.join("provider.json"),
        serde_json::to_vec(&manifest)?,
    )
    .await?;
    tokio::fs::write(
        providers.join("provider-trust.json"),
        serde_json::to_vec(&json!({"publishers":{"test-publisher":STANDARD.encode(key.verifying_key().to_bytes())}}))?,
    )
    .await?;

    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("p", "/tmp/external-privacy", "main", "/tmp/external-wt")
        .await?;
    let denied = orchestrator
        .task_create(
            &project.id,
            "remote review",
            "must ask first",
            AgentKind::ClaudeCode,
            manifest.id.clone(),
            None,
            None,
        )
        .await;
    assert!(
        matches!(denied, Err(OrchestratorError::InvalidState(message)) if message == "API_EGRESS_APPROVAL_REQUIRED")
    );
    let approved = orchestrator
        .task_create_with_api_egress(
            &project.id,
            "remote review approved",
            "explicitly allowed",
            AgentKind::ClaudeCode,
            manifest.id,
            None,
            None,
            true,
        )
        .await?;
    assert!(orchestrator.task(&approved.id).await?.api_egress_approved);
    Ok(())
}

#[tokio::test]
async fn council_egress_consent_audit_names_the_remote_member()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("p", "/tmp/council-privacy", "main", "/tmp/council-wt")
        .await?;
    let mut settings = ProjectSettings::default();
    settings.review_council.enabled = true;
    settings.review_council.reviewers = vec![AgentKind::ClaudeCode, AgentKind::DeepSeekApi];
    orchestrator
        .project_settings_update(&project.id, &settings)
        .await?;

    let approved = orchestrator
        .task_create_with_api_egress(
            &project.id,
            "council review",
            "remote member approved",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            None,
            true,
        )
        .await?;
    let payload: String = sqlx::query_scalar(
        "SELECT payload_json FROM events WHERE task_id=? AND event_type='privacy:api_egress_approved'",
    )
    .bind(&approved.id)
    .fetch_one(orchestrator.store.pool())
    .await?;
    assert!(payload.contains("deepseek_api"));
    Ok(())
}
