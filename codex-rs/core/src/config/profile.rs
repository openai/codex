use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

use crate::protocol::AskForApproval;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::Verbosity;
use codex_protocol::openai_models::ReasoningEffort;

/// Collection of common configuration options that a user can define as a unit
/// in `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfigProfile {
    pub model: Option<String>,
    /// The key in the `model_providers` map identifying the
    /// [`ModelProviderInfo`] to use.
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub chatgpt_base_url: Option<String>,
    pub experimental_instructions_file: Option<AbsolutePathBuf>,
    pub experimental_compact_prompt_file: Option<AbsolutePathBuf>,
    pub include_apply_patch_tool: Option<bool>,
    pub experimental_use_unified_exec_tool: Option<bool>,
    pub experimental_use_freeform_apply_patch: Option<bool>,
    pub tools_web_search: Option<bool>,
    pub tools_view_image: Option<bool>,
    /// Optional feature toggles scoped to this profile.
    #[serde(default)]
    pub features: Option<crate::features::FeaturesToml>,
    pub oss_provider: Option<String>,
}

impl From<ConfigProfile> for codex_app_server_protocol::Profile {
    fn from(config_profile: ConfigProfile) -> Self {
        Self {
            model: config_profile.model,
            model_provider: config_profile.model_provider,
            approval_policy: config_profile.approval_policy,
            model_reasoning_effort: config_profile.model_reasoning_effort,
            model_reasoning_summary: config_profile.model_reasoning_summary,
            model_verbosity: config_profile.model_verbosity,
            chatgpt_base_url: config_profile.chatgpt_base_url,
        }
    }
}
