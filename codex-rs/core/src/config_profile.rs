use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config_types::McpServerConfig;
use crate::config_types::Verbosity;
use crate::protocol::AskForApproval;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;

/// Collection of common configuration options that a user can define as a unit
/// in `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ConfigProfile {
    pub model: Option<String>,
    /// The key in the `model_providers` map identifying the
    /// [`ModelProviderInfo`] to use.
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub disable_response_storage: Option<bool>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub chatgpt_base_url: Option<String>,
    pub experimental_instructions_file: Option<PathBuf>,

    /// Per-profile MCP server definitions. When present, these entries are
    /// merged with the top-level `mcp_servers` map. On key conflicts, the
    /// profile entries take precedence. To opt out of inheriting global
    /// servers, set `inherit_global_mcp_servers` to `false`.
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,

    /// Whether this profile should inherit the top-level `mcp_servers`.
    /// Defaults to `true` when not specified.
    pub inherit_global_mcp_servers: Option<bool>,
}
