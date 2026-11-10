// Town / studio contracts: project appearance, project members and agent profiles.
// Included into lib.rs so the types share the crate's derives and imports.

/// `None` = field absent (leave unchanged); `Some(None)` = explicit null (clear).
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// How a project looks in the town. Every field is optional; unknown `kind` strings are kept
/// as-is so newer clients can add building types (older ones draw a cottage).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAppearance {
    pub project_id: String,
    /// Building type chosen by the user; `None` means the client guesses from the name.
    pub kind: Option<String>,
    /// Roof / main colour as `#rrggbb`.
    pub roof: Option<String>,
    /// "wood" | "neon" | "stone"
    pub sign: Option<String>,
    /// Yard decorations, distinct, at most 6.
    pub yard: Vec<String>,
    pub updated_at: Option<String>,
}

/// Partial update: an absent field is left unchanged, an explicit `null` clears it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAppearancePatch {
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub kind: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub roof: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub sign: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub yard: Option<Vec<String>>,
}

/// What an assigned agent does in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Developer,
    Reviewer,
    Both,
}

impl MemberRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::Reviewer => "reviewer",
            Self::Both => "both",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "developer" => Some(Self::Developer),
            "reviewer" => Some(Self::Reviewer),
            "both" => Some(Self::Both),
            _ => None,
        }
    }
}

/// An agent the user assigned to a project. The town draws exactly these robots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMember {
    pub agent: AgentKind,
    pub role: MemberRole,
    /// Display order, smallest first.
    pub position: i32,
    pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemberInput {
    pub agent: AgentKind,
    pub role: MemberRole,
}

/// Where the town takes a project's robots from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "snake_case")]
pub enum AgentsSource {
    /// The user assigned members to the project.
    Assigned,
    /// No members yet: agents are derived from recent tasks.
    #[default]
    Derived,
}

/// A user's customisation of one agent, global across projects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    pub agent: AgentKind,
    /// 1–16 characters; `None` = default name.
    pub nickname: Option<String>,
    /// Body colour as `#rrggbb`.
    pub color: Option<String>,
    /// "none" | "cap" | "headphones" | "glasses" | "bowtie" | "antenna" | "scarf"
    pub accessory: Option<String>,
    /// At most 40 characters.
    pub catchphrase: Option<String>,
    pub updated_at: Option<String>,
}

/// Partial update: an absent field is left unchanged, an explicit `null` clears it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfilePatch {
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub nickname: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub color: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub accessory: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[specta(optional, type = Option<String>)]
    pub catchphrase: Option<Option<String>>,
}
