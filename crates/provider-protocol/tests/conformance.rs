use agentflow_contracts::{
    AgentKind, DataEgress, ExecutionLocation, ProviderCapabilities, ProviderPermissions, RunRole,
};
use agentflow_provider_protocol::{
    ProtocolClient, ProtocolPermission, ProtocolResult, ProtocolRunRequest, ProviderManifest,
    ProviderRegistry, ProviderSecurity, TransportKind,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn discovers_and_runs_a_conforming_sidecar() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let package = temp.path().join("fixture");
    tokio::fs::create_dir_all(&package).await?;
    install_fixture_binary(&package.join("provider-bin")).await?;
    let manifest = fixture_manifest(&package.join("provider-bin")).await?;
    install_trust_store(temp.path(), &manifest).await?;
    tokio::fs::write(
        package.join("provider.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .await?;

    let registry = ProviderRegistry::discover(temp.path()).await?;
    assert!(registry.problems().is_empty());
    let id = AgentKind::External("fixture_provider".into());
    let resolved = registry
        .get(&id)
        .ok_or("fixture provider was not discovered")?;
    let client = ProtocolClient::new(resolved.clone());
    let (handshake, health) = client.probe().await?;
    assert_eq!(handshake.provider_id, id);
    assert_eq!(
        health.status,
        agentflow_provider_protocol::HealthStatus::Ready
    );

    let (tx, mut rx) = mpsc::channel(4);
    let outcome = client
        .run(
            ProtocolRunRequest {
                request_id: "rpc-fixture-1".into(),
                task_id: "TASK-fixture".into(),
                revision: 2,
                commit_sha: None,
                worktree: temp.path().to_string_lossy().into_owned(),
                run_dir: package.to_string_lossy().into_owned(),
                role: RunRole::Developer,
                input_file: ".agentflow-in/input.md".into(),
                timeout_ms: Duration::from_secs(5).as_millis() as u64,
                idle_timeout_ms: Duration::from_secs(2).as_millis() as u64,
                permission: ProtocolPermission::Normal,
                resume_session_id: None,
                extra_allowed_commands: Vec::new(),
                env_denylist: Vec::new(),
            },
            CancellationToken::new(),
            tx,
        )
        .await?;
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.timed_out);
    assert_eq!(
        rx.recv().await.ok_or("fixture event missing")?.summary,
        "fixture handled request"
    );
    let result = outcome.result.ok_or("fixture result missing")?;
    assert_eq!(result.session_id.as_deref(), Some("fixture-session"));
    assert_eq!(result.tokens_in, Some(10));
    match result.result {
        ProtocolResult::Development(result) => {
            assert_eq!(result.task_id, "TASK-fixture");
            assert_eq!(result.revision, 2);
            assert_eq!(result.summary, "fixture completed");
        }
        ProtocolResult::Review(_) => return Err("unexpected review result".into()),
    }
    Ok(())
}

#[tokio::test]
async fn isolates_invalid_packages() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let package = temp.path().join("escape");
    tokio::fs::create_dir_all(&package).await?;
    let mut manifest = unsigned_fixture_manifest();
    manifest.executable = "../outside".into();
    tokio::fs::write(
        package.join("provider.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .await?;

    let registry = ProviderRegistry::discover(temp.path()).await?;
    assert!(registry.all().next().is_none());
    assert_eq!(registry.problems().len(), 1);
    Ok(())
}

fn unsigned_fixture_manifest() -> ProviderManifest {
    ProviderManifest {
        manifest_version: 1,
        id: AgentKind::External("fixture_provider".into()),
        display_name: "Protocol Fixture".into(),
        protocol_version: "1.1".into(),
        executable: "provider-bin".into(),
        args: Vec::new(),
        transport: TransportKind::StdioJsonRpc,
        capabilities: ProviderCapabilities {
            development: true,
            review: false,
            streaming: true,
            structured_output: true,
            sandbox: true,
            resume: false,
        },
        execution_location: ExecutionLocation::Local,
        data_egress: DataEgress::None,
        permissions: ProviderPermissions {
            worktree_read: true,
            worktree_write: true,
            network_domains: Vec::new(),
            commands: Vec::new(),
        },
        security: None,
        enabled: true,
    }
}

async fn fixture_manifest(binary: &Path) -> Result<ProviderManifest, Box<dyn std::error::Error>> {
    let mut manifest = unsigned_fixture_manifest();
    let digest = format!("{:x}", Sha256::digest(tokio::fs::read(binary).await?));
    manifest.security = Some(ProviderSecurity {
        publisher: "fixture-publisher".into(),
        artifact_sha256: digest,
        signature: String::new(),
    });
    let key = SigningKey::from_bytes(&[7_u8; 32]);
    let signature = key.sign(&manifest.signing_payload()?);
    manifest
        .security
        .as_mut()
        .ok_or("missing security")?
        .signature = STANDARD.encode(signature.to_bytes());
    Ok(manifest)
}

async fn install_trust_store(
    root: &Path,
    _manifest: &ProviderManifest,
) -> Result<(), Box<dyn std::error::Error>> {
    let key = SigningKey::from_bytes(&[7_u8; 32]);
    tokio::fs::write(
        root.join("provider-trust.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "publishers": {"fixture-publisher": STANDARD.encode(key.verifying_key().to_bytes())}
        }))?,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn quarantines_a_provider_after_permission_tampering()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let package = temp.path().join("tampered");
    tokio::fs::create_dir_all(&package).await?;
    let binary = package.join("provider-bin");
    install_fixture_binary(&binary).await?;
    let mut manifest = fixture_manifest(&binary).await?;
    install_trust_store(temp.path(), &manifest).await?;
    manifest
        .permissions
        .network_domains
        .push("example.com".into());
    tokio::fs::write(
        package.join("provider.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .await?;

    let registry = ProviderRegistry::discover(temp.path()).await?;
    assert!(registry.get(&manifest.id).is_none());
    assert_eq!(registry.quarantined().len(), 1);
    assert!(registry.problems()[0].contains("signature verification failed"));
    Ok(())
}

async fn install_fixture_binary(target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    tokio::fs::copy(env!("CARGO_BIN_EXE_conformance_provider"), target).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = tokio::fs::metadata(target).await?.permissions();
        permissions.set_mode(0o755);
        tokio::fs::set_permissions(target, permissions).await?;
    }
    Ok(())
}
