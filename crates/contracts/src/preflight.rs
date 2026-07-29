// Task preflight readiness. Unlike the onboarding catalog (which answers "does the command
// exist?"), this answers the question a task actually cares about: for the developer and reviewer
// roles the task will use, is there at least one provider in the ordered fallback chain that is
// installed, protocol-compatible AND authenticated — i.e. that can really run right now (P0-01)?
// The full chain is returned so the desktop can list every skipped provider and why, instead of
// letting the run start and surface only the last provider that happened to fail (P0-02).
string_enum!(PreflightRole { Developer => "developer", Reviewer => "reviewer" });

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReadiness {
    pub provider: AgentKind,
    pub display_name: String,
    /// True only when the provider is installed, protocol-compatible and authenticated.
    pub available: bool,
    /// Why the provider cannot run right now (missing CLI, not logged in, incompatible protocol…).
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct RoleReadiness {
    pub role: PreflightRole,
    /// The provider the task tries first for this role.
    pub primary: AgentKind,
    /// At least one provider in the ordered fallback chain can actually run.
    pub ready: bool,
    /// The whole ordered chain (primary first), each annotated with whether — and why not — it runs.
    pub chain: Vec<ProviderReadiness>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskPreflightReport {
    /// Every required role has at least one runnable provider, so a run is worth starting.
    pub ready: bool,
    pub roles: Vec<RoleReadiness>,
}
