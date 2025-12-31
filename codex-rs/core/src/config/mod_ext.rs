use serde::Deserialize;
use serde::Serialize;

use crate::tools::spec_ext::ToolFilter;

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigExt {
    /// Maximum number of model output tokens.
    pub model_max_output_tokens: Option<i64>,

    /// Common LLM sampling parameters (temperature, top_p, etc.).
    pub model_parameters: Option<codex_protocol::config_types_ext::ModelParameters>,

    /// Web search configuration (provider, max_results).
    pub web_search_config: codex_protocol::config_types_ext::WebSearchConfig,

    /// Web fetch configuration (timeout, max_content_length, user_agent).
    pub web_fetch_config: codex_protocol::config_types_ext::WebFetchConfig,

    /// Logging configuration for tracing subscriber (location, timezone, levels).
    pub logging: crate::config::types_ext::LoggingConfig,

    /// Compact V2 configuration (thresholds, micro-compact, context restoration).
    pub compact: crate::compact_v2::CompactConfig,

    /// Tool filter. Main session: None. Subagent: from_agent_definition().
    pub tool_filter: Option<ToolFilter>,

    /// System reminder configuration for contextual injection.
    pub system_reminder: crate::config::SystemReminderConfig,
}

impl Default for ConfigExt {
    fn default() -> Self {
        Self {
            model_max_output_tokens: None,
            model_parameters: None,
            web_search_config: codex_protocol::config_types_ext::WebSearchConfig::default(),
            web_fetch_config: codex_protocol::config_types_ext::WebFetchConfig::default(),
            logging: crate::config::types_ext::LoggingConfig::default(),
            compact: crate::compact_v2::CompactConfig::default(),
            tool_filter: None,
            system_reminder: crate::config::SystemReminderConfig::default(),
        }
    }
}

/// Base config deserialized from ~/.codex/config.toml.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigTomlExt {
    /// Maximum number of model output tokens.
    #[serde(default)]
    pub model_max_output_tokens: Option<i64>,

    /// Common LLM sampling parameters (temperature, top_p, etc.).
    #[serde(default)]
    pub model_parameters: Option<codex_protocol::config_types_ext::ModelParameters>,

    /// Logging configuration for tracing subscriber.
    #[serde(default)]
    pub logging: Option<crate::config::types_ext::LoggingConfig>,

    /// Compact V2 configuration (thresholds, micro-compact, context restoration).
    #[serde(default)]
    pub compact: Option<crate::compact_v2::CompactConfig>,

    /// System reminder configuration for contextual injection.
    #[serde(default)]
    pub system_reminder: Option<crate::config::SystemReminderConfig>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ToolsTomlExt {
    #[serde(default, alias = "web_search_request")]
    pub web_search: Option<bool>,

    /// Enable the `view_image` tool that lets the agent attach local images.
    #[serde(default)]
    pub view_image: Option<bool>,

    /// Web search configuration (provider, max_results, etc.)
    #[serde(default)]
    pub web_search_config: Option<WebSearchConfigToml>,

    /// Web fetch configuration (timeout, max_content_length, user_agent)
    #[serde(default)]
    pub web_fetch_config: Option<WebFetchConfigToml>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct WebSearchConfigToml {
    #[serde(default)]
    pub provider: Option<codex_protocol::config_types_ext::WebSearchProvider>,
    #[serde(default)]
    pub max_results: Option<usize>,
    /// API key for Tavily provider (falls back to TAVILY_API_KEY env var)
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct WebFetchConfigToml {
    #[serde(default)]
    pub max_content_length: Option<usize>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub user_agent: Option<String>,
}
