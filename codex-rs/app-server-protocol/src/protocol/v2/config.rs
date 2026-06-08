use super::ApprovalsReviewer;
use super::AskForApproval;
use super::SandboxMode;
use super::WindowsSandboxSetupMode;
#[cfg(any(test, feature = "serde-compat"))]
use super::shared::default_enabled;
use codex_experimental_api_macros::ExperimentalApi;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WebSearchToolConfig;
use codex_protocol::openai_models::ReasoningEffort;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Deserialize;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(
    any(test, feature = "serde-compat"),
    serde(tag = "type", rename_all = "camelCase")
)]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum ConfigLayerSource {
    /// Managed preferences layer delivered by MDM (macOS only).
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    Mdm {
        domain: String,
        key: String,
    },

    /// Managed config layer from a file (usually `managed_config.toml`).
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    System {
        /// This is the path to the system config.toml file, though it is not
        /// guaranteed to exist.
        file: AbsolutePathBuf,
    },

    /// Enterprise-managed config layer delivered by the cloud config bundle.
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    EnterpriseManaged {
        /// Stable identifier for the delivered layer.
        id: String,

        /// Admin-facing name for the delivered layer. This is surfaced in
        /// diagnostics so users know which cloud layer needs administrator
        /// attention.
        name: String,
    },

    /// User config layer from $CODEX_HOME/config.toml. This layer is special
    /// in that it is expected to be:
    /// - writable by the user
    /// - generally outside the workspace directory
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    User {
        /// This is the path to the user's config.toml file, though it is not
        /// guaranteed to exist.
        file: AbsolutePathBuf,

        /// Name of the selected profile-v2 config layered on top of the base
        /// user config, when this layer represents one.
        profile: Option<String>,
    },

    /// Path to a .codex/ folder within a project. There could be multiple of
    /// these between `cwd` and the project/repo root.
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    Project {
        dot_codex_folder: AbsolutePathBuf,
    },

    /// Session-layer overrides supplied via `-c`/`--config`.
    SessionFlags,

    /// `managed_config.toml` was designed to be a config that was loaded
    /// as the last layer on top of everything else. This scheme did not quite
    /// work out as intended, but we keep this variant as a "best effort" while
    /// we phase out `managed_config.toml` in favor of `requirements.toml`.
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    LegacyManagedConfigTomlFromFile {
        file: AbsolutePathBuf,
    },

    LegacyManagedConfigTomlFromMdm,
}

impl ConfigLayerSource {
    /// A settings from a layer with a higher precedence will override a setting
    /// from a layer with a lower precedence.
    pub fn precedence(&self) -> i16 {
        match self {
            ConfigLayerSource::Mdm { .. } => 0,
            ConfigLayerSource::System { .. } => 10,
            ConfigLayerSource::EnterpriseManaged { .. } => 15,
            ConfigLayerSource::User { profile, .. } => {
                if profile.is_some() {
                    21
                } else {
                    20
                }
            }
            ConfigLayerSource::Project { .. } => 25,
            ConfigLayerSource::SessionFlags => 30,
            ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. } => 40,
            ConfigLayerSource::LegacyManagedConfigTomlFromMdm => 50,
        }
    }
}

/// Compares [ConfigLayerSource] by precedence, so `A < B` means settings from
/// layer `A` will be overridden by settings from layer `B`.
impl PartialOrd for ConfigLayerSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.precedence().cmp(&other.precedence()))
    }
}

#[derive(Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct SandboxWorkspaceWrite {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub writable_roots: Vec<PathBuf>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub network_access: bool,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub exclude_tmpdir_env_var: bool,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub exclude_slash_tmp: bool,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct ToolsV2 {
    pub web_search: Option<WebSearchToolConfig>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct AnalyticsConfig {
    pub enabled: Option<bool>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default, flatten))]
    pub additional: HashMap<String, JsonValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub enum AppToolApproval {
    Auto,
    Prompt,
    Approve,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct AppsDefaultConfig {
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default = "default_enabled")
    )]
    pub enabled: bool,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default = "default_enabled")
    )]
    pub destructive_enabled: bool,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default = "default_enabled")
    )]
    pub open_world_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct AppToolConfig {
    pub enabled: Option<bool>,
    pub approval_mode: Option<AppToolApproval>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct AppToolsConfig {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default, flatten))]
    pub tools: HashMap<String, AppToolConfig>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct AppConfig {
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default = "default_enabled")
    )]
    pub enabled: bool,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub destructive_enabled: Option<bool>,
    pub open_world_enabled: Option<bool>,
    pub default_tools_approval_mode: Option<AppToolApproval>,
    pub default_tools_enabled: Option<bool>,
    pub tools: Option<AppToolsConfig>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct AppsConfig {
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, rename = "_default")
    )]
    pub default: Option<AppsDefaultConfig>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default, flatten))]
    pub apps: HashMap<String, AppConfig>,
}

/// Backward-compatible API shape for ChatGPT workspace login restrictions.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(untagged))]
#[ts(export_to = "v2/")]
pub enum ForcedChatgptWorkspaceIds {
    Single(String),
    Multiple(Vec<String>),
}

impl ForcedChatgptWorkspaceIds {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            Self::Single(value) => vec![value],
            Self::Multiple(values) => values,
        }
    }
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "snake_case"))]
#[ts(export_to = "v2/")]
pub struct Config {
    pub model: Option<String>,
    pub review_model: Option<String>,
    pub model_context_window: Option<i64>,
    pub model_auto_compact_token_limit: Option<i64>,
    pub model_auto_compact_token_limit_scope: Option<AutoCompactTokenLimitScope>,
    pub model_provider: Option<String>,
    #[experimental(nested)]
    pub approval_policy: Option<AskForApproval>,
    /// [UNSTABLE] Optional default for where approval requests are routed for
    /// review.
    #[experimental("config/read.approvalsReviewer")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_mode: Option<SandboxMode>,
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,
    pub forced_chatgpt_workspace_id: Option<ForcedChatgptWorkspaceIds>,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub web_search: Option<WebSearchMode>,
    pub tools: Option<ToolsV2>,
    pub instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub compact_prompt: Option<String>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub service_tier: Option<String>,
    pub analytics: Option<AnalyticsConfig>,
    #[experimental("config/read.apps")]
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub apps: Option<AppsConfig>,
    pub desktop: Option<HashMap<String, JsonValue>>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default, flatten))]
    pub additional: HashMap<String, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigLayerMetadata {
    pub name: ConfigLayerSource,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigLayer {
    pub name: ConfigLayerSource,
    pub version: String,
    pub config: JsonValue,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum MergeStrategy {
    Replace,
    Upsert,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum WriteStatus {
    Ok,
    OkOverridden,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct OverriddenMetadata {
    pub message: String,
    pub overriding_layer: ConfigLayerMetadata,
    pub effective_value: JsonValue,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigWriteResponse {
    pub status: WriteStatus,
    pub version: String,
    /// Canonical path to the config file that was written.
    pub file_path: AbsolutePathBuf,
    pub overridden_metadata: Option<OverriddenMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum ConfigWriteErrorCode {
    ConfigLayerReadonly,
    ConfigVersionConflict,
    ConfigValidationError,
    ConfigPathNotFound,
    ConfigSchemaUnknownKey,
    UserLayerNotFound,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigReadParams {
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "std::ops::Not::not")
    )]
    pub include_layers: bool,
    /// Optional working directory to resolve project config layers. If specified,
    /// return the effective config as seen from that directory (i.e., including any
    /// project layers between `cwd` and the project/repo root).
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigReadResponse {
    #[experimental(nested)]
    pub config: Config,
    pub origins: HashMap<String, ConfigLayerMetadata>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub layers: Option<Vec<ConfigLayer>>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigRequirements {
    #[experimental(nested)]
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
    #[experimental("configRequirements/read.allowedApprovalsReviewers")]
    pub allowed_approvals_reviewers: Option<Vec<ApprovalsReviewer>>,
    pub allowed_sandbox_modes: Option<Vec<SandboxMode>>,
    pub allowed_windows_sandbox_implementations: Option<Vec<WindowsSandboxSetupMode>>,
    pub allowed_permissions: Option<Vec<String>>,
    pub allowed_web_search_modes: Option<Vec<WebSearchMode>>,
    pub allow_managed_hooks_only: Option<bool>,
    pub allow_appshots: Option<bool>,
    pub computer_use: Option<ComputerUseRequirements>,
    pub feature_requirements: Option<BTreeMap<String, bool>>,
    #[experimental("configRequirements/read.hooks")]
    pub hooks: Option<ManagedHooksRequirements>,
    pub enforce_residency: Option<ResidencyRequirement>,
    #[experimental("configRequirements/read.network")]
    pub network: Option<NetworkRequirements>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ComputerUseRequirements {
    pub allow_locked_computer_use: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ManagedHooksRequirements {
    pub managed_dir: Option<PathBuf>,
    pub windows_managed_dir: Option<PathBuf>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "PreToolUse"))]
    #[ts(rename = "PreToolUse")]
    pub pre_tool_use: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(rename = "PermissionRequest")
    )]
    #[ts(rename = "PermissionRequest")]
    pub permission_request: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "PostToolUse"))]
    #[ts(rename = "PostToolUse")]
    pub post_tool_use: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "PreCompact"))]
    #[ts(rename = "PreCompact")]
    pub pre_compact: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "PostCompact"))]
    #[ts(rename = "PostCompact")]
    pub post_compact: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "SessionStart"))]
    #[ts(rename = "SessionStart")]
    pub session_start: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(rename = "UserPromptSubmit")
    )]
    #[ts(rename = "UserPromptSubmit")]
    pub user_prompt_submit: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "SubagentStart"))]
    #[ts(rename = "SubagentStart")]
    pub subagent_start: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "SubagentStop"))]
    #[ts(rename = "SubagentStop")]
    pub subagent_stop: Vec<ConfiguredHookMatcherGroup>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "Stop"))]
    #[ts(rename = "Stop")]
    pub stop: Vec<ConfiguredHookMatcherGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfiguredHookMatcherGroup {
    pub matcher: Option<String>,
    pub hooks: Vec<ConfiguredHookHandler>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(tag = "type"))]
#[ts(tag = "type", export_to = "v2/")]
pub enum ConfiguredHookHandler {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "command"))]
    #[ts(rename = "command")]
    Command {
        command: String,
        #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "commandWindows"))]
        #[ts(rename = "commandWindows")]
        command_windows: Option<String>,
        #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "timeoutSec"))]
        #[ts(rename = "timeoutSec")]
        timeout_sec: Option<u64>,
        r#async: bool,
        #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "statusMessage"))]
        #[ts(rename = "statusMessage")]
        status_message: Option<String>,
    },
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "prompt"))]
    #[ts(rename = "prompt")]
    Prompt {},
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "agent"))]
    #[ts(rename = "agent")]
    Agent {},
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct NetworkRequirements {
    pub enabled: Option<bool>,
    pub http_port: Option<u16>,
    pub socks_port: Option<u16>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    /// Canonical network permission map for `experimental_network`.
    pub domains: Option<BTreeMap<String, NetworkDomainPermission>>,
    /// When true, only managed allowlist entries are respected while managed
    /// network enforcement is active.
    pub managed_allowed_domains_only: Option<bool>,
    /// Legacy compatibility view derived from `domains`.
    pub allowed_domains: Option<Vec<String>>,
    /// Legacy compatibility view derived from `domains`.
    pub denied_domains: Option<Vec<String>>,
    /// Canonical unix socket permission map for `experimental_network`.
    pub unix_sockets: Option<BTreeMap<String, NetworkUnixSocketPermission>>,
    /// Legacy compatibility view derived from `unix_sockets`.
    pub allow_unix_sockets: Option<Vec<String>>,
    pub allow_local_binding: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "lowercase"))]
#[ts(export_to = "v2/")]
pub enum NetworkDomainPermission {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "lowercase"))]
#[ts(export_to = "v2/")]
pub enum NetworkUnixSocketPermission {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum ResidencyRequirement {
    Us,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigRequirementsReadResponse {
    /// Null if no requirements are configured (e.g. no requirements.toml/MDM entries).
    #[experimental(nested)]
    pub requirements: Option<ConfigRequirements>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[ts(export_to = "v2/")]
pub enum ExternalAgentConfigMigrationItemType {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "AGENTS_MD"))]
    #[ts(rename = "AGENTS_MD")]
    AgentsMd,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "CONFIG"))]
    #[ts(rename = "CONFIG")]
    Config,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "SKILLS"))]
    #[ts(rename = "SKILLS")]
    Skills,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "PLUGINS"))]
    #[ts(rename = "PLUGINS")]
    Plugins,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(rename = "MCP_SERVER_CONFIG")
    )]
    #[ts(rename = "MCP_SERVER_CONFIG")]
    McpServerConfig,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "SUBAGENTS"))]
    #[ts(rename = "SUBAGENTS")]
    Subagents,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "HOOKS"))]
    #[ts(rename = "HOOKS")]
    Hooks,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "COMMANDS"))]
    #[ts(rename = "COMMANDS")]
    Commands,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "SESSIONS"))]
    #[ts(rename = "SESSIONS")]
    Sessions,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct PluginsMigration {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "marketplaceName"))]
    #[ts(rename = "marketplaceName")]
    pub marketplace_name: String,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "pluginNames"))]
    #[ts(rename = "pluginNames")]
    pub plugin_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct SessionMigration {
    pub path: PathBuf,
    pub cwd: PathBuf,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerMigration {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct HookMigration {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct SubagentMigration {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct CommandMigration {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct MigrationDetails {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub plugins: Vec<PluginsMigration>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub sessions: Vec<SessionMigration>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub mcp_servers: Vec<McpServerMigration>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub hooks: Vec<HookMigration>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub subagents: Vec<SubagentMigration>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(default))]
    pub commands: Vec<CommandMigration>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigMigrationItem {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    /// Null or empty means home-scoped migration; non-empty means repo-scoped migration.
    pub cwd: Option<PathBuf>,
    pub details: Option<MigrationDetails>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigDetectResponse {
    pub items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigDetectParams {
    /// If true, include detection under the user's home (~/.claude, ~/.codex, etc.).
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "std::ops::Not::not")
    )]
    pub include_home: bool,
    /// Zero or more working directories to include for repo-scoped detection.
    #[ts(optional = nullable)]
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigImportParams {
    pub migration_items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigImportResponse {}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigImportCompletedNotification {}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigValueWriteParams {
    pub key_path: String,
    pub value: JsonValue,
    pub merge_strategy: MergeStrategy,
    /// Path to the config file to write; defaults to the user's `config.toml` when omitted.
    #[ts(optional = nullable)]
    pub file_path: Option<String>,
    #[ts(optional = nullable)]
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigBatchWriteParams {
    pub edits: Vec<ConfigEdit>,
    /// Path to the config file to write; defaults to the user's `config.toml` when omitted.
    #[ts(optional = nullable)]
    pub file_path: Option<String>,
    #[ts(optional = nullable)]
    pub expected_version: Option<String>,
    /// When true, hot-reload the updated user config into all loaded threads after writing.
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "std::ops::Not::not")
    )]
    pub reload_user_config: bool,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigEdit {
    pub key_path: String,
    pub value: JsonValue,
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct TextPosition {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number (in Unicode scalar values).
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ConfigWarningNotification {
    /// Concise summary of the warning.
    pub summary: String,
    /// Optional extra guidance or error details.
    pub details: Option<String>,
    /// Optional path to the config file that triggered the warning.
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub path: Option<String>,
    /// Optional range for the error location inside the config file.
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub range: Option<TextRange>,
}
