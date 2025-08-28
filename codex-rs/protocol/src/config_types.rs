use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use strum_macros::EnumIter;
use ts_rs::TS;

use crate::mcp_protocol::AuthMode;
use crate::protocol::AskForApproval;

/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning
#[derive(
    Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display, TS, EnumIter,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    #[default]
    Medium,
    High,
}

/// A summary of the reasoning performed by the model. This can be useful for
/// debugging and understanding the model's reasoning process.
/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#reasoning-summaries
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display, TS)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningSummary {
    #[default]
    Auto,
    Concise,
    Detailed,
    /// Option to disable reasoning summaries.
    None,
}

/// Controls output length/detail on GPT-5 models via the Responses API.
/// Serialized with lowercase values to match the OpenAI API.
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display, TS)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Verbosity {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default, Serialize, Display, TS)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    #[default]
    ReadOnly,

    #[serde(rename = "workspace-write")]
    WorkspaceWrite,

    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

/// Available tools for the model
#[derive(Deserialize, Debug, Clone, Default, PartialEq, TS, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    // Renamed from `web_search_request`; keep alias for backwards compatibility.
    #[serde(default, alias = "web_search_request")]
    pub web_search: Option<bool>,

    /// Enable the `view_image` tool that lets the agent attach local images.
    #[serde(default)]
    pub view_image: Option<bool>,
}

/// Collection of common configuration options that a user can define as a unit
/// in `config.toml`
#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
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
    // Excluding experimental_instructions_file
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SandboxWorkspaceWrite {
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub exclude_tmpdir_env_var: bool,
    #[serde(default)]
    pub exclude_slash_tmp: bool,
}

/// UserSavedConfig contains mcp client-configurable settings that can be
/// specified in the saved config.toml file. This is currently a subset of
/// [`codex_core::config::ConfigToml`] but ideally we should simplify and
/// standardize the format.
#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserSavedConfig {
    /// Approvals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,

    /// Relevant model-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_reasoning_summary: Option<ReasoningSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_verbosity: Option<Verbosity>,

    /// Tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Tools>,

    // Auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_auth_method: Option<AuthMode>,

    /// Profiles
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, ConfigProfile>,
}
