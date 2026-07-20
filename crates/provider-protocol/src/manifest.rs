use crate::types::PROTOCOL_VERSION;
use agentflow_contracts::{
    AgentKind, DataEgress, ExecutionLocation, ProviderCapabilities, ProviderPermissions,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

fn manifest_version() -> u8 {
    1
}

fn default_protocol() -> String {
    PROTOCOL_VERSION.into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    StdioJsonRpc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManifest {
    #[serde(default = "manifest_version")]
    pub manifest_version: u8,
    pub id: AgentKind,
    pub display_name: String,
    #[serde(default = "default_protocol")]
    pub protocol_version: String,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub transport: TransportKind,
    pub capabilities: ProviderCapabilities,
    #[serde(default = "default_execution_location")]
    pub execution_location: ExecutionLocation,
    #[serde(default = "default_data_egress")]
    pub data_egress: DataEgress,
    #[serde(default)]
    pub permissions: ProviderPermissions,
    #[serde(default)]
    pub security: Option<ProviderSecurity>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSecurity {
    pub publisher: String,
    pub artifact_sha256: String,
    pub signature: String,
}

fn default_execution_location() -> ExecutionLocation {
    ExecutionLocation::Local
}

fn default_data_egress() -> DataEgress {
    DataEgress::None
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct ResolvedProviderManifest {
    pub manifest: ProviderManifest,
    pub package_dir: PathBuf,
    pub executable: PathBuf,
}

impl ProviderManifest {
    pub fn resolve(self, package_dir: &Path) -> Result<ResolvedProviderManifest, String> {
        if self.manifest_version != 1 {
            return Err(format!(
                "unsupported provider manifest version {}",
                self.manifest_version
            ));
        }
        if self.protocol_version.split('.').next() != PROTOCOL_VERSION.split('.').next() {
            return Err(format!(
                "provider protocol {} is incompatible with core {}",
                self.protocol_version, PROTOCOL_VERSION
            ));
        }
        let relative = Path::new(&self.executable);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("provider executable must stay inside its package directory".into());
        }
        let executable = package_dir.join(relative);
        if !executable.is_file() {
            return Err(format!(
                "provider executable does not exist: {}",
                executable.display()
            ));
        }
        Ok(ResolvedProviderManifest {
            manifest: self,
            package_dir: package_dir.to_path_buf(),
            executable,
        })
    }

    /// Stable bytes signed by a Provider publisher. Permissions are hashed into the payload so a
    /// package cannot gain network or filesystem access without invalidating its signature.
    pub fn signing_payload(&self) -> Result<Vec<u8>, String> {
        let security = self
            .security
            .as_ref()
            .ok_or_else(|| "provider v1.1 security metadata is required".to_string())?;
        let policy = serde_json::to_vec(&(
            self.execution_location,
            self.data_egress,
            &self.permissions,
            &self.capabilities,
        ))
        .map_err(|error| error.to_string())?;
        let policy_hash = format!("{:x}", Sha256::digest(policy));
        Ok(format!(
            "agentflow-provider-v1.1\n{}\n{}\n{}\n{}\n{}",
            self.id,
            self.protocol_version,
            security.publisher,
            security.artifact_sha256.to_ascii_lowercase(),
            policy_hash
        )
        .into_bytes())
    }
}
