use super::*;
use ed25519_dalek::{Signer as _, SigningKey};

fn signed(artifact: &[u8], target: &str, arch: &str) -> (SigningKey, SignedReleaseManifest) {
    let key = SigningKey::from_bytes(&[11_u8; 32]);
    let mut manifest = SignedReleaseManifest {
        schema_version: 1,
        version: "1.2.3".into(),
        target: target.into(),
        arch: arch.into(),
        artifact_sha256: format!("{:x}", Sha256::digest(artifact)),
        artifact_size: artifact.len() as u64,
        signature: String::new(),
    };
    manifest.signature = STANDARD.encode(key.sign(&manifest.signing_payload()).to_bytes());
    (key, manifest)
}

#[tokio::test]
async fn stages_only_a_valid_target_bound_artifact() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let artifact = b"signed app bytes";
    let (key, manifest) = signed(artifact, "darwin", "aarch64");
    let verifier = ReleaseVerifier::new(
        &STANDARD.encode(key.verifying_key().to_bytes()),
        "darwin",
        "aarch64",
    )?;
    let slot = SlotManager::new(root.path())
        .stage(&manifest, artifact, &verifier)
        .await?;
    assert_eq!(tokio::fs::read(slot).await?, artifact);
    Ok(())
}

#[test]
fn rejects_wrong_target_and_tampered_artifact() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = b"signed app bytes";
    let (key, manifest) = signed(artifact, "darwin", "aarch64");
    let wrong_target = ReleaseVerifier::new(
        &STANDARD.encode(key.verifying_key().to_bytes()),
        "windows",
        "x86_64",
    )?;
    assert!(matches!(
        wrong_target.verify_manifest(&manifest),
        Err(ReleaseError::InvalidManifest(_))
    ));
    let verifier = ReleaseVerifier::new(
        &STANDARD.encode(key.verifying_key().to_bytes()),
        "darwin",
        "aarch64",
    )?;
    assert!(matches!(
        verifier.verify_artifact(&manifest, b"tampered"),
        Err(ReleaseError::ArtifactMismatch)
    ));
    Ok(())
}

#[tokio::test]
async fn failed_health_check_restores_previous_slot() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let manager = SlotManager::new(root.path());
    tokio::fs::create_dir_all(root.path().join("slots")).await?;
    tokio::fs::write(root.path().join("slots/1.0.0"), b"old").await?;
    tokio::fs::write(root.path().join("slots/1.1.0"), b"new").await?;
    tokio::fs::write(root.path().join("current"), "1.0.0").await?;
    let result = manager
        .activate_with_health("1.1.0", |_| async { false })
        .await;
    assert!(matches!(result, Err(ReleaseError::HealthCheckFailed)));
    assert_eq!(manager.current_version().await?.as_deref(), Some("1.0.0"));
    Ok(())
}
