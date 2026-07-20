//! Versioned process boundary for AgentFlow Provider sidecars.
//!
//! The core only depends on this crate. Vendor-specific CLI and HTTP details belong in an
//! independently updatable sidecar that speaks newline-delimited JSON-RPC over stdio.

mod client;
mod manifest;
mod registry;
mod types;

pub use client::{ProtocolClient, ProtocolError, ProtocolRunOutcome};
pub use manifest::{ProviderManifest, ProviderSecurity, ResolvedProviderManifest, TransportKind};
pub use registry::{ProviderRegistry, QuarantinedProvider, RegistryError};
pub use types::{
    HandshakeParams, HandshakeResult, HealthResult, HealthStatus, PROTOCOL_VERSION,
    ProtocolPermission, ProtocolResult, ProtocolRunRequest, ProtocolRunResult, RpcError,
    RpcNotification, RpcRequest, RpcResponse,
};
