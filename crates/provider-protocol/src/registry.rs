use crate::{ProviderManifest, ResolvedProviderManifest};
use agentflow_contracts::{AgentKind, ProviderDescriptor, ProviderSource, ProviderTrust};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::Verifier as _;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("provider registry I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid provider manifest {path}: {message}")]
    Invalid { path: String, message: String },
}

#[derive(Debug, Clone)]
pub struct QuarantinedProvider {
    pub manifest: ProviderManifest,
    pub problem: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<AgentKind, ResolvedProviderManifest>,
    quarantined: Vec<QuarantinedProvider>,
    problems: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderTrustStore {
    #[serde(default)]
    publishers: HashMap<String, String>,
}

impl ProviderRegistry {
    /// Only packages whose digest and publisher signature verify enter the executable map. Broken
    /// packages remain visible as quarantined descriptors without blocking built-ins or startup.
    pub async fn discover(root: &Path) -> Result<Self, RegistryError> {
        if !root.exists() {
            tokio::fs::create_dir_all(root).await?;
            return Ok(Self::default());
        }
        let trust = load_trust_store(root).await?;
        let mut entries = tokio::fs::read_dir(root).await?;
        let mut registry = Self::default();
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let package_dir = entry.path();
            let path = package_dir.join("provider.json");
            if !path.is_file() {
                continue;
            }
            let bytes = tokio::fs::read(&path).await?;
            let manifest: ProviderManifest = match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(error) => {
                    registry.problems.push(
                        RegistryError::Invalid {
                            path: path.display().to_string(),
                            message: error.to_string(),
                        }
                        .to_string(),
                    );
                    continue;
                }
            };
            let resolved = match manifest.clone().resolve(&package_dir) {
                Ok(value) => value,
                Err(message) => {
                    registry.problems.push(
                        RegistryError::Invalid {
                            path: path.display().to_string(),
                            message,
                        }
                        .to_string(),
                    );
                    continue;
                }
            };
            if !manifest.enabled {
                continue;
            }
            match verify_package(&resolved, &trust).await {
                Ok(()) => {
                    registry.providers.insert(manifest.id.clone(), resolved);
                }
                Err(problem) => {
                    registry
                        .problems
                        .push(format!("{}: {problem}", path.display()));
                    registry
                        .quarantined
                        .push(QuarantinedProvider { manifest, problem });
                }
            }
        }
        Ok(registry)
    }

    pub fn get(&self, id: &AgentKind) -> Option<&ResolvedProviderManifest> {
        self.providers.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &ResolvedProviderManifest> {
        self.providers.values()
    }

    pub fn quarantined(&self) -> &[QuarantinedProvider] {
        &self.quarantined
    }

    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    pub fn quarantined_descriptors(&self) -> Vec<ProviderDescriptor> {
        self.quarantined
            .iter()
            .map(|entry| ProviderDescriptor {
                id: entry.manifest.id.clone(),
                display_name: entry.manifest.display_name.clone(),
                source: ProviderSource::External,
                protocol_version: entry.manifest.protocol_version.clone(),
                capabilities: entry.manifest.capabilities.clone(),
                execution_location: entry.manifest.execution_location,
                data_egress: entry.manifest.data_egress,
                permissions: entry.manifest.permissions.clone(),
                trust: ProviderTrust::Quarantined,
                available: false,
                problem: Some(entry.problem.clone()),
            })
            .collect()
    }
}

async fn load_trust_store(root: &Path) -> Result<ProviderTrustStore, RegistryError> {
    let path = root.join("provider-trust.json");
    match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| RegistryError::Invalid {
            path: "provider-trust.json".into(),
            message: error.to_string(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(error) => Err(error.into()),
    }
}

async fn verify_package(
    resolved: &ResolvedProviderManifest,
    trust: &ProviderTrustStore,
) -> Result<(), String> {
    let security = resolved
        .manifest
        .security
        .as_ref()
        .ok_or_else(|| "unsigned legacy Provider is quarantined".to_string())?;
    if security.artifact_sha256.len() != 64
        || !security
            .artifact_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("artifactSha256 must be 64 hexadecimal characters".into());
    }
    let bytes = tokio::fs::read(&resolved.executable)
        .await
        .map_err(|error| error.to_string())?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(&security.artifact_sha256) {
        return Err("Provider executable digest does not match manifest".into());
    }
    let pinned = trust
        .publishers
        .get(&security.publisher)
        .ok_or_else(|| format!("publisher {} is not trusted", security.publisher))?;
    let key_bytes = STANDARD
        .decode(pinned)
        .map_err(|_| "trusted publisher key is not valid base64".to_string())?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| "trusted publisher key must be 32 bytes".to_string())?;
    let key = VerifyingKey::from_bytes(&key_array).map_err(|error| error.to_string())?;
    let signature_bytes = STANDARD
        .decode(&security.signature)
        .map_err(|_| "Provider signature is not valid base64".to_string())?;
    let signature = Signature::from_slice(&signature_bytes).map_err(|error| error.to_string())?;
    key.verify(&resolved.manifest.signing_payload()?, &signature)
        .map_err(|_| "Provider signature verification failed".to_string())
}
