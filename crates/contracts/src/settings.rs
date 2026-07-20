pub const CLAUDE_CLI_KEYCHAIN_SERVICE: &str = "com.agentflow.claude-cli-api-key";
pub const CODEX_CLI_KEYCHAIN_SERVICE: &str = "com.agentflow.codex-cli-api-key";

/// Local trust decision for repository-owned AgentFlow configuration. The approved hash lives
/// outside the repository, so an Agent cannot grant itself command execution permissions by
/// editing `.agentflow/project.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfigTrust {
    pub exists: bool,
    pub path: String,
    pub sha256: Option<String>,
    pub trusted: bool,
    pub validation_steps: Vec<String>,
    pub extra_allowed_commands: Vec<String>,
    pub approved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ProjectSettings {
    pub claude_path: Option<String>,
    pub codex_path: Option<String>,
    pub gemini_path: Option<String>,
    pub qwen_path: Option<String>,
    pub grok_path: Option<String>,
    pub kimi_path: Option<String>,
    pub minimax_path: Option<String>,
    pub git_path: Option<String>,
    pub full_access: bool,
    pub resume_sessions: bool,
    pub env_denylist: Vec<String>,
    pub openai: ApiProviderSettings,
    pub anthropic: ApiProviderSettings,
    pub deepseek: ApiProviderSettings,
    pub grok: ApiProviderSettings,
    pub minimax: ApiProviderSettings,
    pub kimi: ApiProviderSettings,
    pub api_fallback_provider: Option<AgentKind>,
    pub developer_fallbacks: Vec<AgentKind>,
    pub reviewer_fallbacks: Vec<AgentKind>,
    pub review_council: ReviewCouncilSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ReviewCouncilSettings {
    pub enabled: bool,
    pub reviewers: Vec<AgentKind>,
    pub minimum_successful_reviews: u8,
    pub require_unanimous_pass: bool,
}

impl Default for ReviewCouncilSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            reviewers: Vec::new(),
            minimum_successful_reviews: 2,
            require_unanimous_pass: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ApiProviderSettings {
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub keychain_service: String,
    pub max_retries: u32,
    pub max_output_tokens: u32,
    /// Optional project-owned pricing snapshot used for deterministic cost budgets.
    /// Prices are never guessed because vendor/model rates change independently.
    pub input_cost_per_million: Option<f64>,
    pub output_cost_per_million: Option<f64>,
}

impl ApiProviderSettings {
    fn new(base_url: &str, model: &str, api_key_env: &str, keychain_service: &str) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key_env: api_key_env.into(),
            keychain_service: keychain_service.into(),
            max_retries: 3,
            max_output_tokens: 8_000,
            input_cost_per_million: None,
            output_cost_per_million: None,
        }
    }

    pub fn openai_default() -> Self {
        Self::new(
            "https://api.openai.com/v1",
            "gpt-5.1",
            "OPENAI_API_KEY",
            "com.agentflow.openai-api",
        )
    }

    pub fn anthropic_default() -> Self {
        Self::new(
            "https://api.anthropic.com/v1",
            "claude-sonnet-4-20250514",
            "ANTHROPIC_API_KEY",
            "com.agentflow.anthropic-api",
        )
    }

    pub fn deepseek_default() -> Self {
        Self::new(
            "https://api.deepseek.com",
            "deepseek-v4-flash",
            "DEEPSEEK_API_KEY",
            "com.agentflow.deepseek-api",
        )
    }

    pub fn grok_default() -> Self {
        Self::new(
            "https://api.x.ai/v1",
            "grok-4.5",
            "XAI_API_KEY",
            "com.agentflow.grok-api",
        )
    }

    pub fn minimax_default() -> Self {
        Self::new(
            "https://api.minimax.io/v1",
            "MiniMax-M2.7",
            "MINIMAX_API_KEY",
            "com.agentflow.minimax-api",
        )
    }

    pub fn kimi_default() -> Self {
        Self::new(
            "https://api.kimi.com/coding/v1",
            "kimi-for-coding",
            "KIMI_API_KEY",
            "com.agentflow.kimi-api",
        )
    }
}

impl Default for ApiProviderSettings {
    fn default() -> Self {
        Self::openai_default()
    }
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            claude_path: None,
            codex_path: None,
            gemini_path: None,
            qwen_path: None,
            grok_path: None,
            kimi_path: None,
            minimax_path: None,
            git_path: None,
            full_access: false,
            resume_sessions: false,
            env_denylist: Vec::new(),
            openai: ApiProviderSettings::openai_default(),
            anthropic: ApiProviderSettings::anthropic_default(),
            deepseek: ApiProviderSettings::deepseek_default(),
            grok: ApiProviderSettings::grok_default(),
            minimax: ApiProviderSettings::minimax_default(),
            kimi: ApiProviderSettings::kimi_default(),
            api_fallback_provider: Some(AgentKind::Codex),
            developer_fallbacks: vec![
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                AgentKind::GeminiCli,
                AgentKind::QwenCode,
            ],
            reviewer_fallbacks: vec![
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                AgentKind::GeminiCli,
                AgentKind::QwenCode,
                AgentKind::OpenAiApi,
                AgentKind::AnthropicApi,
                AgentKind::DeepSeekApi,
                AgentKind::GrokApi,
                AgentKind::MiniMaxApi,
                AgentKind::KimiApi,
            ],
            review_council: ReviewCouncilSettings::default(),
        }
    }
}
