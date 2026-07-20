//! Signed, target-bound release staging with a two-slot rollback marker.
//!
//! This crate deliberately owns no network client and no private signing key. Callers download a
//! manifest and artifact, then this boundary verifies both before anything becomes activatable.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("release I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("release manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("release signature verification failed")]
    InvalidSignature,
    #[error("release artifact digest or size does not match the signed manifest")]
    ArtifactMismatch,
    #[error("release health check failed; previous slot restored")]
    HealthCheckFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedReleaseManifest {
    pub schema_version: u8,
    pub version: String,
    pub target: String,
    pub arch: String,
    pub artifact_sha256: String,
    pub artifact_size: u64,
    pub signature: String,
}

impl SignedReleaseManifest {
    pub fn signing_payload(&self) -> Vec<u8> {
        format!(
            "agentflow-release-v1\n{}\n{}\n{}\n{}\n{}",
            self.version,
            self.target,
            self.arch,
            self.artifact_sha256.to_ascii_lowercase(),
            self.artifact_size
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseVerifier {
    public_key: VerifyingKey,
    target: String,
    arch: String,
}

impl ReleaseVerifier {
    pub fn new(public_key_base64: &str, target: &str, arch: &str) -> Result<Self, ReleaseError> {
        let bytes = STANDARD
            .decode(public_key_base64)
            .map_err(|_| ReleaseError::InvalidManifest("public key is not valid base64".into()))?;
        let key: [u8; 32] = bytes.try_into().map_err(|_| {
            ReleaseError::InvalidManifest("public key must contain 32 bytes".into())
        })?;
        Ok(Self {
            public_key: VerifyingKey::from_bytes(&key)
                .map_err(|error| ReleaseError::InvalidManifest(error.to_string()))?,
            target: target.into(),
            arch: arch.into(),
        })
    }

    pub fn verify_manifest(&self, manifest: &SignedReleaseManifest) -> Result<(), ReleaseError> {
        if manifest.schema_version != 1
            || manifest.target != self.target
            || manifest.arch != self.arch
            || !safe_version(&manifest.version)
            || manifest.artifact_sha256.len() != 64
            || !manifest
                .artifact_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ReleaseError::InvalidManifest(
                "schema, target, arch, version, or digest is incompatible".into(),
            ));
        }
        let bytes = STANDARD
            .decode(&manifest.signature)
            .map_err(|_| ReleaseError::InvalidSignature)?;
        let signature =
            Signature::from_slice(&bytes).map_err(|_| ReleaseError::InvalidSignature)?;
        self.public_key
            .verify(&manifest.signing_payload(), &signature)
            .map_err(|_| ReleaseError::InvalidSignature)
    }

    pub fn verify_artifact(
        &self,
        manifest: &SignedReleaseManifest,
        artifact: &[u8],
    ) -> Result<(), ReleaseError> {
        self.verify_manifest(manifest)?;
        let digest = format!("{:x}", Sha256::digest(artifact));
        if artifact.len() as u64 != manifest.artifact_size
            || !digest.eq_ignore_ascii_case(&manifest.artifact_sha256)
        {
            return Err(ReleaseError::ArtifactMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SlotManager {
    root: PathBuf,
}

impl SlotManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub async fn stage(
        &self,
        manifest: &SignedReleaseManifest,
        artifact: &[u8],
        verifier: &ReleaseVerifier,
    ) -> Result<PathBuf, ReleaseError> {
        verifier.verify_artifact(manifest, artifact)?;
        tokio::fs::create_dir_all(self.root.join("slots")).await?;
        let slot = self.root.join("slots").join(&manifest.version);
        let staged = self
            .root
            .join("slots")
            .join(format!("{}.staging", manifest.version));
        if staged.exists() {
            tokio::fs::remove_file(&staged).await?;
        }
        tokio::fs::write(&staged, artifact).await?;
        tokio::fs::rename(&staged, &slot).await?;
        Ok(slot)
    }

    pub async fn activate_with_health<F, Fut>(
        &self,
        version: &str,
        health: F,
    ) -> Result<PathBuf, ReleaseError>
    where
        F: FnOnce(PathBuf) -> Fut,
        Fut: Future<Output = bool>,
    {
        if !safe_version(version) {
            return Err(ReleaseError::InvalidManifest("unsafe version".into()));
        }
        let slot = self.root.join("slots").join(version);
        if !slot.is_file() {
            return Err(ReleaseError::InvalidManifest("slot does not exist".into()));
        }
        tokio::fs::create_dir_all(&self.root).await?;
        let marker = self.root.join("current");
        let previous = tokio::fs::read_to_string(&marker).await.ok();
        let pending = self.root.join("current.pending");
        tokio::fs::write(&pending, version.as_bytes()).await?;
        tokio::fs::rename(&pending, &marker).await?;
        if health(slot.clone()).await {
            if let Some(previous) = previous {
                tokio::fs::write(self.root.join("previous"), previous).await?;
            }
            return Ok(slot);
        }
        match previous {
            Some(value) => tokio::fs::write(&marker, value).await?,
            None => tokio::fs::remove_file(&marker).await?,
        }
        Err(ReleaseError::HealthCheckFailed)
    }

    pub async fn current_version(&self) -> Result<Option<String>, ReleaseError> {
        match tokio::fs::read_to_string(self.root.join("current")).await {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests;
