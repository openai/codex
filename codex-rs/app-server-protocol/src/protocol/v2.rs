use std::collections::HashMap;
use std::path::PathBuf;

use crate::protocol::common::AuthMode;
use codex_protocol::account::PlanType;
use codex_protocol::approvals::ExecPolicyAmendment as CoreExecPolicyAmendment;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode as CoreSandboxMode;
use codex_protocol::config_types::Verbosity;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::items::AgentMessageContent as CoreAgentMessageContent;
use codex_protocol::items::TurnItem as CoreTurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::parse_command::ParsedCommand as CoreParsedCommand;
use codex_protocol::plan_tool::PlanItemArg as CorePlanItemArg;
use codex_protocol::plan_tool::StepStatus as CorePlanStepStatus;
use codex_protocol::protocol::AgentStatus as CoreAgentStatus;
use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
use codex_protocol::protocol::CodexErrorInfo as CoreCodexErrorInfo;
use codex_protocol::protocol::CreditsSnapshot as CoreCreditsSnapshot;
use codex_protocol::protocol::NetworkAccess as CoreNetworkAccess;
use codex_protocol::protocol::RateLimitSnapshot as CoreRateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow as CoreRateLimitWindow;
use codex_protocol::protocol::SessionSource as CoreSessionSource;
use codex_protocol::protocol::SkillDependencies as CoreSkillDependencies;
use codex_protocol::protocol::SkillErrorInfo as CoreSkillErrorInfo;
use codex_protocol::protocol::SkillInterface as CoreSkillInterface;
use codex_protocol::protocol::SkillMetadata as CoreSkillMetadata;
use codex_protocol::protocol::SkillScope as CoreSkillScope;
use codex_protocol::protocol::SkillToolDependency as CoreSkillToolDependency;
use codex_protocol::protocol::SubAgentSource as CoreSubAgentSource;
use codex_protocol::protocol::TokenUsage as CoreTokenUsage;
use codex_protocol::protocol::TokenUsageInfo as CoreTokenUsageInfo;
use codex_protocol::user_input::ByteRange as CoreByteRange;
use codex_protocol::user_input::TextElement as CoreTextElement;
use codex_protocol::user_input::UserInput as CoreUserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use mcp_types::ContentBlock as McpContentBlock;
use mcp_types::Resource as McpResource;
use mcp_types::ResourceTemplate as McpResourceTemplate;
use mcp_types::Tool as McpTool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use thiserror::Error;
use ts_rs::TS;

// Macro to declare a camelCased API v2 enum mirroring a core enum which
// tends to use either snake_case or kebab-case.
macro_rules! v2_enum_from_core {
    (
        pub enum $Name:ident from $Src:path { $( $Variant:ident ),+ $(,)? }
    ) => {
        #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
        #[serde(rename_all = "camelCase")]
        #[ts(export_to = "v2/")]
        pub enum $Name { $( $Variant ),+ }

        impl $Name {
            pub fn to_core(self) -> $Src {
                match self { $( $Name::$Variant => <$Src>::$Variant ),+ }
            }
        }

        impl From<$Src> for $Name {
            fn from(value: $Src) -> Self {
                match value { $( <$Src>::$Variant => $Name::$Variant ),+ }
            }
        }
    };
}

/// This translation layer make sure that we expose codex error code in camel case.
///
/// When an upstream HTTP status is available (for example, from the Responses API or a provider),
/// it is forwarded in `httpStatusCode` on the relevant `codexErrorInfo` variant.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CodexErrorInfo {
    ContextWindowExceeded,
    UsageLimitExceeded,
    ModelCap {
        model: String,
        reset_after_seconds: Option<u64>,
    },
    HttpConnectionFailed {
        #[serde(rename = "httpStatusCode")]
        #[ts(rename = "httpStatusCode")]
        http_status_code: Option<u16>,
    },
    /// Failed to connect to the response SSE stream.
    ResponseStreamConnectionFailed {
        #[serde(rename = "httpStatusCode")]
        #[ts(rename = "httpStatusCode")]
        http_status_code: Option<u16>,
    },
    InternalServerError,
    Unauthorized,
    BadRequest,
    ThreadRollbackFailed,
    SandboxError,
    /// The response SSE stream disconnected in the middle of a turn before completion.
    ResponseStreamDisconnected {
        #[serde(rename = "httpStatusCode")]
        #[ts(rename = "httpStatusCode")]
        http_status_code: Option<u16>,
    },
    /// Reached the retry limit for responses.
    ResponseTooManyFailedAttempts {
        #[serde(rename = "httpStatusCode")]
        #[ts(rename = "httpStatusCode")]
        http_status_code: Option<u16>,
    },
    Other,
}

impl From<CoreCodexErrorInfo> for CodexErrorInfo {
    fn from(value: CoreCodexErrorInfo) -> Self {
        match value {
            CoreCodexErrorInfo::ContextWindowExceeded => CodexErrorInfo::ContextWindowExceeded,
            CoreCodexErrorInfo::UsageLimitExceeded => CodexErrorInfo::UsageLimitExceeded,
            CoreCodexErrorInfo::ModelCap {
                model,
                reset_after_seconds,
            } => CodexErrorInfo::ModelCap {
                model,
                reset_after_seconds,
            },
            CoreCodexErrorInfo::HttpConnectionFailed { http_status_code } => {
                CodexErrorInfo::HttpConnectionFailed { http_status_code }
            }
            CoreCodexErrorInfo::ResponseStreamConnectionFailed { http_status_code } => {
                CodexErrorInfo::ResponseStreamConnectionFailed { http_status_code }
            }
            CoreCodexErrorInfo::InternalServerError => CodexErrorInfo::InternalServerError,
            CoreCodexErrorInfo::Unauthorized => CodexErrorInfo::Unauthorized,
            CoreCodexErrorInfo::BadRequest => CodexErrorInfo::BadRequest,
            CoreCodexErrorInfo::ThreadRollbackFailed => CodexErrorInfo::ThreadRollbackFailed,
            CoreCodexErrorInfo::SandboxError => CodexErrorInfo::SandboxError,
            CoreCodexErrorInfo::ResponseStreamDisconnected { http_status_code } => {
                CodexErrorInfo::ResponseStreamDisconnected { http_status_code }
            }
            CoreCodexErrorInfo::ResponseTooManyFailedAttempts { http_status_code } => {
                CodexErrorInfo::ResponseTooManyFailedAttempts { http_status_code }
            }
            CoreCodexErrorInfo::Other => CodexErrorInfo::Other,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case", export_to = "v2/")]
pub enum AskForApproval {
    #[serde(rename = "untrusted")]
    #[ts(rename = "untrusted")]
    UnlessTrusted,
    OnFailure,
    OnRequest,
    Never,
}

impl AskForApproval {
    pub fn to_core(self) -> CoreAskForApproval {
        match self {
            AskForApproval::UnlessTrusted => CoreAskForApproval::UnlessTrusted,
            AskForApproval::OnFailure => CoreAskForApproval::OnFailure,
            AskForApproval::OnRequest => CoreAskForApproval::OnRequest,
            AskForApproval::Never => CoreAskForApproval::Never,
        }
    }
}

impl From<CoreAskForApproval> for AskForApproval {
    fn from(value: CoreAskForApproval) -> Self {
        match value {
            CoreAskForApproval::UnlessTrusted => AskForApproval::UnlessTrusted,
            CoreAskForApproval::OnFailure => AskForApproval::OnFailure,
            CoreAskForApproval::OnRequest => AskForApproval::OnRequest,
            CoreAskForApproval::Never => AskForApproval::Never,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case", export_to = "v2/")]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    pub fn to_core(self) -> CoreSandboxMode {
        match self {
            SandboxMode::ReadOnly => CoreSandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite => CoreSandboxMode::WorkspaceWrite,
            SandboxMode::DangerFullAccess => CoreSandboxMode::DangerFullAccess,
        }
    }
}

impl From<CoreSandboxMode> for SandboxMode {
    fn from(value: CoreSandboxMode) -> Self {
        match value {
            CoreSandboxMode::ReadOnly => SandboxMode::ReadOnly,
            CoreSandboxMode::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            CoreSandboxMode::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}

v2_enum_from_core!(
    pub enum ReviewDelivery from codex_protocol::protocol::ReviewDelivery {
        Inline, Detached
    }
);

v2_enum_from_core!(
    pub enum McpAuthStatus from codex_protocol::protocol::McpAuthStatus {
        Unsupported,
        NotLoggedIn,
        BearerToken,
        OAuth
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum ConfigLayerSource {
    /// Managed preferences layer delivered by MDM (macOS only).
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Mdm {
        domain: String,
        key: String,
    },

    /// Managed config layer from a file (usually `managed_config.toml`).
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    System {
        /// This is the path to the system config.toml file, though it is not
        /// guaranteed to exist.
        file: AbsolutePathBuf,
    },

    /// User config layer from $CODEX_HOME/config.toml. This layer is special
    /// in that it is expected to be:
    /// - writable by the user
    /// - generally outside the workspace directory
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    User {
        /// This is the path to the user's config.toml file, though it is not
        /// guaranteed to exist.
        file: AbsolutePathBuf,
    },

    /// Path to a .codex/ folder within a project. There could be multiple of
    /// these between `cwd` and the project/repo root.
    #[serde(rename_all = "camelCase")]
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
    #[serde(rename_all = "camelCase")]
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
            ConfigLayerSource::User { .. } => 20,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ToolsV2 {
    #[serde(alias = "web_search_request")]
    pub web_search: Option<bool>,
    pub view_image: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ProfileV2 {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub web_search: Option<WebSearchMode>,
    pub chatgpt_base_url: Option<String>,
    #[serde(default, flatten)]
    pub additional: HashMap<String, JsonValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct AnalyticsConfig {
    pub enabled: Option<bool>,
    #[serde(default, flatten)]
    pub additional: HashMap<String, JsonValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct Config {
    pub model: Option<String>,
    pub review_model: Option<String>,
    pub model_context_window: Option<i64>,
    pub model_auto_compact_token_limit: Option<i64>,
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub web_search: Option<WebSearchMode>,
    pub tools: Option<ToolsV2>,
    pub profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileV2>,
    pub instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub compact_prompt: Option<String>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub analytics: Option<AnalyticsConfig>,
    #[serde(default, flatten)]
    pub additional: HashMap<String, JsonValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigLayerMetadata {
    pub name: ConfigLayerSource,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigLayer {
    pub name: ConfigLayerSource,
    pub version: String,
    pub config: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum MergeStrategy {
    Replace,
    Upsert,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WriteStatus {
    Ok,
    OkOverridden,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct OverriddenMetadata {
    pub message: String,
    pub overriding_layer: ConfigLayerMetadata,
    pub effective_value: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigWriteResponse {
    pub status: WriteStatus,
    pub version: String,
    /// Canonical path to the config file that was written.
    pub file_path: AbsolutePathBuf,
    pub overridden_metadata: Option<OverriddenMetadata>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ConfigWriteErrorCode {
    ConfigLayerReadonly,
    ConfigVersionConflict,
    ConfigValidationError,
    ConfigPathNotFound,
    ConfigSchemaUnknownKey,
    UserLayerNotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigReadParams {
    #[serde(default)]
    pub include_layers: bool,
    /// Optional working directory to resolve project config layers. If specified,
    /// return the effective config as seen from that directory (i.e., including any
    /// project layers between `cwd` and the project/repo root).
    pub cwd: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigReadResponse {
    pub config: Config,
    pub origins: HashMap<String, ConfigLayerMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<ConfigLayer>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigRequirements {
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
    pub allowed_sandbox_modes: Option<Vec<SandboxMode>>,
    pub enforce_residency: Option<ResidencyRequirement>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum ResidencyRequirement {
    Us,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigRequirementsReadResponse {
    /// Null if no requirements are configured (e.g. no requirements.toml/MDM entries).
    pub requirements: Option<ConfigRequirements>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigValueWriteParams {
    pub key_path: String,
    pub value: JsonValue,
    pub merge_strategy: MergeStrategy,
    /// Path to the config file to write; defaults to the user's `config.toml` when omitted.
    pub file_path: Option<String>,
    pub expected_version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigBatchWriteParams {
    pub edits: Vec<ConfigEdit>,
    /// Path to the config file to write; defaults to the user's `config.toml` when omitted.
    pub file_path: Option<String>,
    pub expected_version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigEdit {
    pub key_path: String,
    pub value: JsonValue,
    pub merge_strategy: MergeStrategy,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CommandExecutionApprovalDecision {
    /// User approved the command.
    Accept,
    /// User approved the command and future identical commands should run without prompting.
    AcceptForSession,
    /// User approved the command, and wants to apply the proposed execpolicy amendment so future
    /// matching commands can run without prompting.
    AcceptWithExecpolicyAmendment {
        execpolicy_amendment: ExecPolicyAmendment,
    },
    /// User denied the command. The agent will continue the turn.
    Decline,
    /// User denied the command. The turn will also be immediately interrupted.
    Cancel,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum FileChangeApprovalDecision {
    /// User approved the file changes.
    Accept,
    /// User approved the file changes and future changes to the same files should run without prompting.
    AcceptForSession,
    /// User denied the file changes. The agent will continue the turn.
    Decline,
    /// User denied the file changes. The turn will also be immediately interrupted.
    Cancel,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum NetworkAccess {
    #[default]
    Restricted,
    Enabled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum SandboxPolicy {
    DangerFullAccess,
    ReadOnly,
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ExternalSandbox {
        #[serde(default)]
        network_access: NetworkAccess,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    WorkspaceWrite {
        #[serde(default)]
        writable_roots: Vec<AbsolutePathBuf>,
        #[serde(default)]
        network_access: bool,
        #[serde(default)]
        exclude_tmpdir_env_var: bool,
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

impl SandboxPolicy {
    pub fn to_core(&self) -> codex_protocol::protocol::SandboxPolicy {
        match self {
            SandboxPolicy::DangerFullAccess => {
                codex_protocol::protocol::SandboxPolicy::DangerFullAccess
            }
            SandboxPolicy::ReadOnly => codex_protocol::protocol::SandboxPolicy::ReadOnly,
            SandboxPolicy::ExternalSandbox { network_access } => {
                codex_protocol::protocol::SandboxPolicy::ExternalSandbox {
                    network_access: match network_access {
                        NetworkAccess::Restricted => CoreNetworkAccess::Restricted,
                        NetworkAccess::Enabled => CoreNetworkAccess::Enabled,
                    },
                }
            }
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: writable_roots.clone(),
                network_access: *network_access,
                exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                exclude_slash_tmp: *exclude_slash_tmp,
            },
        }
    }
}

impl From<codex_protocol::protocol::SandboxPolicy> for SandboxPolicy {
    fn from(value: codex_protocol::protocol::SandboxPolicy) -> Self {
        match value {
            codex_protocol::protocol::SandboxPolicy::DangerFullAccess => {
                SandboxPolicy::DangerFullAccess
            }
            codex_protocol::protocol::SandboxPolicy::ReadOnly => SandboxPolicy::ReadOnly,
            codex_protocol::protocol::SandboxPolicy::ExternalSandbox { network_access } => {
                SandboxPolicy::ExternalSandbox {
                    network_access: match network_access {
                        CoreNetworkAccess::Restricted => NetworkAccess::Restricted,
                        CoreNetworkAccess::Enabled => NetworkAccess::Enabled,
                    },
                }
            }
            codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(transparent)]
#[ts(type = "Array<string>", export_to = "v2/")]
pub struct ExecPolicyAmendment {
    pub command: Vec<String>,
}

impl ExecPolicyAmendment {
    pub fn into_core(self) -> CoreExecPolicyAmendment {
        CoreExecPolicyAmendment::new(self.command)
    }
}

impl From<CoreExecPolicyAmendment> for ExecPolicyAmendment {
    fn from(value: CoreExecPolicyAmendment) -> Self {
        Self {
            command: value.command().to_vec(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum CommandAction {
    Read {
        command: String,
        name: String,
        path: PathBuf,
    },
    ListFiles {
        command: String,
        path: Option<String>,
    },
    Search {
        command: String,
        query: Option<String>,
        path: Option<String>,
    },
    Unknown {
        command: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
#[derive(Default)]
pub enum SessionSource {
    Cli,
    #[serde(rename = "vscode")]
    #[ts(rename = "vscode")]
    #[default]
    VsCode,
    Exec,
    AppServer,
    SubAgent(CoreSubAgentSource),
    #[serde(other)]
    Unknown,
}

impl From<CoreSessionSource> for SessionSource {
    fn from(value: CoreSessionSource) -> Self {
        match value {
            CoreSessionSource::Cli => SessionSource::Cli,
            CoreSessionSource::VSCode => SessionSource::VsCode,
            CoreSessionSource::Exec => SessionSource::Exec,
            CoreSessionSource::Mcp => SessionSource::AppServer,
            CoreSessionSource::SubAgent(sub) => SessionSource::SubAgent(sub),
            CoreSessionSource::Unknown => SessionSource::Unknown,
        }
    }
}

impl From<SessionSource> for CoreSessionSource {
    fn from(value: SessionSource) -> Self {
        match value {
            SessionSource::Cli => CoreSessionSource::Cli,
            SessionSource::VsCode => CoreSessionSource::VSCode,
            SessionSource::Exec => CoreSessionSource::Exec,
            SessionSource::AppServer => CoreSessionSource::Mcp,
            SessionSource::SubAgent(sub) => CoreSessionSource::SubAgent(sub),
            SessionSource::Unknown => CoreSessionSource::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}

impl CommandAction {
    pub fn into_core(self) -> CoreParsedCommand {
        match self {
            CommandAction::Read {
                command: cmd,
                name,
                path,
            } => CoreParsedCommand::Read { cmd, name, path },
            CommandAction::ListFiles { command: cmd, path } => {
                CoreParsedCommand::ListFiles { cmd, path }
            }
            CommandAction::Search {
                command: cmd,
                query,
                path,
            } => CoreParsedCommand::Search { cmd, query, path },
            CommandAction::Unknown { command: cmd } => CoreParsedCommand::Unknown { cmd },
        }
    }
}

impl From<CoreParsedCommand> for CommandAction {
    fn from(value: CoreParsedCommand) -> Self {
        match value {
            CoreParsedCommand::Read { cmd, name, path } => CommandAction::Read {
                command: cmd,
                name,
                path,
            },
            CoreParsedCommand::ListFiles { cmd, path } => {
                CommandAction::ListFiles { command: cmd, path }
            }
            CoreParsedCommand::Search { cmd, query, path } => CommandAction::Search {
                command: cmd,
                query,
                path,
            },
            CoreParsedCommand::Unknown { cmd } => CommandAction::Unknown { command: cmd },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum Account {
    #[serde(rename = "apiKey", rename_all = "camelCase")]
    #[ts(rename = "apiKey", rename_all = "camelCase")]
    ApiKey {},

    #[serde(rename = "chatgpt", rename_all = "camelCase")]
    #[ts(rename = "chatgpt", rename_all = "camelCase")]
    Chatgpt { email: String, plan_type: PlanType },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum LoginAccountParams {
    #[serde(rename = "apiKey", rename_all = "camelCase")]
    #[ts(rename = "apiKey", rename_all = "camelCase")]
    ApiKey {
        #[serde(rename = "apiKey")]
        #[ts(rename = "apiKey")]
        api_key: String,
    },
    #[serde(rename = "chatgpt")]
    #[ts(rename = "chatgpt")]
    Chatgpt,
    /// [UNSTABLE] FOR OPENAI INTERNAL USE ONLY - DO NOT USE.
    /// The access token must contain the same scopes that Codex-managed ChatGPT auth tokens have.
    #[serde(rename = "chatgptAuthTokens")]
    #[ts(rename = "chatgptAuthTokens")]
    ChatgptAuthTokens {
        /// ID token (JWT) supplied by the client.
        ///
        /// This token is used for identity and account metadata (email, plan type,
        /// workspace id).
        #[serde(rename = "idToken")]
        #[ts(rename = "idToken")]
        id_token: String,
        /// Access token (JWT) supplied by the client.
        /// This token is used for backend API requests.
        #[serde(rename = "accessToken")]
        #[ts(rename = "accessToken")]
        access_token: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum LoginAccountResponse {
    #[serde(rename = "apiKey", rename_all = "camelCase")]
    #[ts(rename = "apiKey", rename_all = "camelCase")]
    ApiKey {},
    #[serde(rename = "chatgpt", rename_all = "camelCase")]
    #[ts(rename = "chatgpt", rename_all = "camelCase")]
    Chatgpt {
        // Use plain String for identifiers to avoid TS/JSON Schema quirks around uuid-specific types.
        // Convert to/from UUIDs at the application layer as needed.
        login_id: String,
        /// URL the client should open in a browser to initiate the OAuth flow.
        auth_url: String,
    },
    #[serde(rename = "chatgptAuthTokens", rename_all = "camelCase")]
    #[ts(rename = "chatgptAuthTokens", rename_all = "camelCase")]
    ChatgptAuthTokens {},
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CancelLoginAccountParams {
    pub login_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CancelLoginAccountStatus {
    Canceled,
    NotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CancelLoginAccountResponse {
    pub status: CancelLoginAccountStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct LogoutAccountResponse {}

/// CODEZ - A short summary of a configured account.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum AccountKind {
    Chatgpt,
    ApiKey,
}

/// CODEZ - A short account summary for account switching UIs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AccountSummary {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<AccountKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub email: Option<String>,
}

/// CODEZ - List available accounts (multi-account support).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ListAccountsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub active_account: Option<String>,
    pub accounts: Vec<AccountSummary>,
}

/// CODEZ - Switch the active account (multi-account support).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SwitchAccountParams {
    pub name: String,
    pub create_if_missing: bool,
}

/// CODEZ - Switch account response (multi-account support).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SwitchAccountResponse {
    pub active_account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub migrated_legacy: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ChatgptAuthTokensRefreshReason {
    /// Codex attempted a backend request and received `401 Unauthorized`.
    Unauthorized,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ChatgptAuthTokensRefreshParams {
    pub reason: ChatgptAuthTokensRefreshReason,
    /// Workspace/account identifier that Codex was previously using.
    ///
    /// Clients that manage multiple accounts/workspaces can use this as a hint
    /// to refresh the token for the correct workspace.
    ///
    /// This may be `null` when the prior ID token did not include a workspace
    /// identifier (`chatgpt_account_id`) or when the token could not be parsed.
    pub previous_account_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ChatgptAuthTokensRefreshResponse {
    pub id_token: String,
    pub access_token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GetAccountRateLimitsResponse {
    pub rate_limits: RateLimitSnapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GetAccountParams {
    /// When `true`, requests a proactive token refresh before returning.
    ///
    /// In managed auth mode this triggers the normal refresh-token flow. In
    /// external auth mode this flag is ignored. Clients should refresh tokens
    /// themselves and call `account/login/start` with `chatgptAuthTokens`.
    #[serde(default)]
    pub refresh_token: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GetAccountResponse {
    pub account: Option<Account>,
    pub requires_openai_auth: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelListParams {
    /// Opaque pagination cursor returned by a previous call.
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Model {
    pub id: String,
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    pub default_reasoning_effort: ReasoningEffort,
    #[serde(default)]
    pub supports_personality: bool,
    // Only one model should be marked as default.
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReasoningEffortOption {
    pub reasoning_effort: ReasoningEffort,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelListResponse {
    pub data: Vec<Model>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

/// EXPERIMENTAL - list collaboration mode presets.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CollaborationModeListParams {}

/// EXPERIMENTAL - collaboration mode presets response.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CollaborationModeListResponse {
    pub data: Vec<CollaborationModeMask>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ListMcpServerStatusParams {
    /// Opaque pagination cursor returned by a previous call.
    pub cursor: Option<String>,
    /// Optional page size; defaults to a server-defined value.
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerStatus {
    pub name: String,
    pub tools: std::collections::HashMap<String, McpTool>,
    pub resources: Vec<McpResource>,
    pub resource_templates: Vec<McpResourceTemplate>,
    pub auth_status: McpAuthStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ListMcpServerStatusResponse {
    pub data: Vec<McpServerStatus>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppsListParams {
    /// Opaque pagination cursor returned by a previous call.
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub distribution_channel: Option<String>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppsListResponse {
    pub data: Vec<AppInfo>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerRefreshParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerRefreshResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerOauthLoginParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scopes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timeout_secs: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerOauthLoginResponse {
    pub authorization_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackUploadParams {
    pub classification: String,
    pub reason: Option<String>,
    pub thread_id: Option<String>,
    pub include_logs: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackUploadResponse {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecParams {
    pub command: Vec<String>,
    #[ts(type = "number | null")]
    pub timeout_ms: Option<i64>,
    pub cwd: Option<PathBuf>,
    pub sandbox_policy: Option<SandboxPolicy>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

// === Threads, Turns, and Items ===
// Thread APIs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadStartParams {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub cwd: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox: Option<SandboxMode>,
    pub config: Option<HashMap<String, JsonValue>>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub personality: Option<Personality>,
    pub ephemeral: Option<bool>,
    pub dynamic_tools: Option<Vec<DynamicToolSpec>>,
    /// If true, opt into emitting raw response items on the event stream.
    ///
    /// This is for internal use only (e.g. Codex Cloud).
    /// (TODO): Figure out a better way to categorize internal / experimental events & protocols.
    #[serde(default)]
    pub experimental_raw_events: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadStartResponse {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// There are three ways to resume a thread:
/// 1. By thread_id: load the thread from disk by thread_id and resume it.
/// 2. By history: instantiate the thread from memory and resume it.
/// 3. By path: load the thread from disk by path and resume it.
///
/// The precedence is: history > path > thread_id.
/// If using history or path, the thread_id param will be ignored.
///
/// Prefer using thread_id whenever possible.
pub struct ThreadResumeParams {
    pub thread_id: String,

    /// [UNSTABLE] FOR CODEX CLOUD - DO NOT USE.
    /// If specified, the thread will be resumed with the provided history
    /// instead of loaded from disk.
    pub history: Option<Vec<ResponseItem>>,

    /// [UNSTABLE] Specify the rollout path to resume from.
    /// If specified, the thread_id param will be ignored.
    pub path: Option<PathBuf>,

    /// Configuration overrides for the resumed thread, if any.
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub cwd: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox: Option<SandboxMode>,
    pub config: Option<HashMap<String, serde_json::Value>>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub personality: Option<Personality>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadResumeResponse {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// There are two ways to fork a thread:
/// 1. By thread_id: load the thread from disk by thread_id and fork it into a new thread.
/// 2. By path: load the thread from disk by path and fork it into a new thread.
///
/// If using path, the thread_id param will be ignored.
///
/// Prefer using thread_id whenever possible.
pub struct ThreadForkParams {
    pub thread_id: String,

    /// [UNSTABLE] Specify the rollout path to fork from.
    /// If specified, the thread_id param will be ignored.
    pub path: Option<PathBuf>,

    /// Configuration overrides for the forked thread, if any.
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub cwd: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox: Option<SandboxMode>,
    pub config: Option<HashMap<String, serde_json::Value>>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadForkResponse {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadArchiveResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSetNameParams {
    pub thread_id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUnarchiveParams {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadSetNameResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUnarchiveResponse {
    pub thread: Thread,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRollbackParams {
    pub thread_id: String,
    /// The number of turns to drop from the end of the thread. Must be >= 1.
    ///
    /// This only modifies the thread's history and does not revert local file changes
    /// that have been made by the agent. Clients are responsible for reverting these changes.
    pub num_turns: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRollbackResponse {
    /// The updated thread after applying the rollback, with `turns` populated.
    ///
    /// The ThreadItems stored in each Turn are lossy since we explicitly do not
    /// persist all agent interactions, such as command executions. This is the same
    /// behavior as `thread/resume`.
    pub thread: Thread,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadCompactParams {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadCompactResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadListParams {
    /// Opaque pagination cursor returned by a previous call.
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    pub limit: Option<u32>,
    /// Optional sort key; defaults to created_at.
    pub sort_key: Option<ThreadSortKey>,
    /// Optional provider filter; when set, only sessions recorded under these
    /// providers are returned. When present but empty, includes all providers.
    pub model_providers: Option<Vec<String>>,
    /// Optional source filter; when set, only sessions from these source kinds
    /// are returned. When omitted or empty, defaults to interactive sources.
    pub source_kinds: Option<Vec<ThreadSourceKind>>,
    /// Optional archived filter; when set to true, only archived threads are returned.
    /// If false or null, only non-archived threads are returned.
    pub archived: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadSourceKind {
    Cli,
    #[serde(rename = "vscode")]
    #[ts(rename = "vscode")]
    VsCode,
    Exec,
    AppServer,
    SubAgent,
    SubAgentReview,
    SubAgentCompact,
    SubAgentThreadSpawn,
    SubAgentOther,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub enum ThreadSortKey {
    CreatedAt,
    UpdatedAt,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadListResponse {
    pub data: Vec<Thread>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// if None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadLoadedListParams {
    /// Opaque pagination cursor returned by a previous call.
    pub cursor: Option<String>,
    /// Optional page size; defaults to no limit.
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadLoadedListResponse {
    /// Thread ids for sessions currently loaded in memory.
    pub data: Vec<String>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// if None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadReadParams {
    pub thread_id: String,
    /// When true, include turns and their items from rollout history.
    #[serde(default)]
    pub include_turns: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadReadResponse {
    pub thread: Thread,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsListParams {
    /// When empty, defaults to the current session working directory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cwds: Vec<PathBuf>,

    /// When true, bypass the skills cache and re-scan skills from disk.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_reload: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsListResponse {
    pub data: Vec<SkillsListEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub enum SkillScope {
    User,
    Repo,
    System,
    Admin,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    /// Legacy short_description from SKILL.md. Prefer SKILL.json interface.short_description.
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interface: Option<SkillInterface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dependencies: Option<SkillDependencies>,
    pub path: PathBuf,
    pub scope: SkillScope,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillInterface {
    #[ts(optional)]
    pub display_name: Option<String>,
    #[ts(optional)]
    pub short_description: Option<String>,
    #[ts(optional)]
    pub icon_small: Option<PathBuf>,
    #[ts(optional)]
    pub icon_large: Option<PathBuf>,
    #[ts(optional)]
    pub brand_color: Option<String>,
    #[ts(optional)]
    pub default_prompt: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillDependencies {
    pub tools: Vec<SkillToolDependency>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillToolDependency {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub r#type: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillErrorInfo {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsListEntry {
    pub cwd: PathBuf,
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillErrorInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsConfigWriteParams {
    pub path: PathBuf,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsConfigWriteResponse {
    pub effective_enabled: bool,
}

impl From<CoreSkillMetadata> for SkillMetadata {
    fn from(value: CoreSkillMetadata) -> Self {
        Self {
            name: value.name,
            description: value.description,
            short_description: value.short_description,
            interface: value.interface.map(SkillInterface::from),
            dependencies: value.dependencies.map(SkillDependencies::from),
            path: value.path,
            scope: value.scope.into(),
            enabled: true,
        }
    }
}

impl From<CoreSkillInterface> for SkillInterface {
    fn from(value: CoreSkillInterface) -> Self {
        Self {
            display_name: value.display_name,
            short_description: value.short_description,
            brand_color: value.brand_color,
            default_prompt: value.default_prompt,
            icon_small: value.icon_small,
            icon_large: value.icon_large,
        }
    }
}

impl From<CoreSkillDependencies> for SkillDependencies {
    fn from(value: CoreSkillDependencies) -> Self {
        Self {
            tools: value
                .tools
                .into_iter()
                .map(SkillToolDependency::from)
                .collect(),
        }
    }
}

impl From<CoreSkillToolDependency> for SkillToolDependency {
    fn from(value: CoreSkillToolDependency) -> Self {
        Self {
            r#type: value.r#type,
            value: value.value,
            description: value.description,
            transport: value.transport,
            command: value.command,
            url: value.url,
        }
    }
}

impl From<CoreSkillScope> for SkillScope {
    fn from(value: CoreSkillScope) -> Self {
        match value {
            CoreSkillScope::User => Self::User,
            CoreSkillScope::Repo => Self::Repo,
            CoreSkillScope::System => Self::System,
            CoreSkillScope::Admin => Self::Admin,
        }
    }
}

impl From<CoreSkillErrorInfo> for SkillErrorInfo {
    fn from(value: CoreSkillErrorInfo) -> Self {
        Self {
            path: value.path,
            message: value.message,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Thread {
    pub id: String,
    /// Usually the first user message in the thread, if available.
    pub preview: String,
    /// Model provider used for this thread (for example, 'openai').
    pub model_provider: String,
    /// Unix timestamp (in seconds) when the thread was created.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Unix timestamp (in seconds) when the thread was last updated.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// [UNSTABLE] Path to the thread on disk.
    pub path: Option<PathBuf>,
    /// Working directory captured for the thread.
    pub cwd: PathBuf,
    /// Version of the CLI that created the thread.
    pub cli_version: String,
    /// Origin of the thread (CLI, VSCode, codex exec, codex app-server, etc.).
    pub source: SessionSource,
    /// Optional Git metadata captured when the thread was created.
    pub git_info: Option<GitInfo>,
    /// Only populated on `thread/resume`, `thread/rollback`, `thread/fork`, and `thread/read`
    /// (when `includeTurns` is true) responses.
    /// For all other responses and notifications returning a Thread,
    /// the turns field will be an empty list.
    pub turns: Vec<Turn>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AccountUpdatedNotification {
    pub auth_mode: Option<AuthMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub active_account: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadTokenUsageUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub token_usage: ThreadTokenUsage,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadTokenUsage {
    pub total: TokenUsageBreakdown,
    pub last: TokenUsageBreakdown,
    // TODO(aibrahim): make this not optional
    #[ts(type = "number | null")]
    pub model_context_window: Option<i64>,
}

impl From<CoreTokenUsageInfo> for ThreadTokenUsage {
    fn from(value: CoreTokenUsageInfo) -> Self {
        Self {
            total: value.total_token_usage.into(),
            last: value.last_token_usage.into(),
            model_context_window: value.model_context_window,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TokenUsageBreakdown {
    #[ts(type = "number")]
    pub total_tokens: i64,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub cached_input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
    #[ts(type = "number")]
    pub reasoning_output_tokens: i64,
}

impl From<CoreTokenUsage> for TokenUsageBreakdown {
    fn from(value: CoreTokenUsage) -> Self {
        Self {
            total_tokens: value.total_tokens,
            input_tokens: value.input_tokens,
            cached_input_tokens: value.cached_input_tokens,
            output_tokens: value.output_tokens,
            reasoning_output_tokens: value.reasoning_output_tokens,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Turn {
    pub id: String,
    /// Only populated on a `thread/resume` or `thread/fork` response.
    /// For all other responses and notifications returning a Turn,
    /// the items field will be an empty list.
    pub items: Vec<ThreadItem>,
    pub status: TurnStatus,
    /// Only populated when the Turn's status is failed.
    pub error: Option<TurnError>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, Error)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
#[error("{message}")]
pub struct TurnError {
    pub message: String,
    pub codex_error_info: Option<CodexErrorInfo>,
    #[serde(default)]
    pub additional_details: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ErrorNotification {
    pub error: TurnError,
    // Set to true if the error is transient and the app-server process will automatically retry.
    // If true, this will not interrupt a turn.
    pub will_retry: bool,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
}

// Turn APIs
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    /// Override the working directory for this turn and subsequent turns.
    pub cwd: Option<PathBuf>,
    /// Override the approval policy for this turn and subsequent turns.
    pub approval_policy: Option<AskForApproval>,
    /// Override the sandbox policy for this turn and subsequent turns.
    pub sandbox_policy: Option<SandboxPolicy>,
    /// Override the model for this turn and subsequent turns.
    pub model: Option<String>,
    /// Override the reasoning effort for this turn and subsequent turns.
    pub effort: Option<ReasoningEffort>,
    /// Override the reasoning summary for this turn and subsequent turns.
    pub summary: Option<ReasoningSummary>,
    /// Override the personality for this turn and subsequent turns.
    pub personality: Option<Personality>,
    /// Optional JSON Schema used to constrain the final assistant message for this turn.
    pub output_schema: Option<JsonValue>,

    /// EXPERIMENTAL - set a pre-set collaboration mode.
    /// Takes precedence over model, reasoning_effort, and developer instructions if set.
    pub collaboration_mode: Option<CollaborationMode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStartParams {
    pub thread_id: String,
    pub target: ReviewTarget,

    /// Where to run the review: inline (default) on the current thread or
    /// detached on a new thread (returned in `reviewThreadId`).
    #[serde(default)]
    pub delivery: Option<ReviewDelivery>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStartResponse {
    pub turn: Turn,
    /// Identifies the thread where the review runs.
    ///
    /// For inline reviews, this is the original thread id.
    /// For detached reviews, this is the id of the new review thread.
    pub review_thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", export_to = "v2/")]
pub enum ReviewTarget {
    /// Review the working tree: staged, unstaged, and untracked files.
    UncommittedChanges,

    /// Review changes between the current branch and the given base branch.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    BaseBranch { branch: String },

    /// Review the changes introduced by a specific commit.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Commit {
        sha: String,
        /// Optional human-readable label (e.g., commit subject) for UIs.
        title: Option<String>,
    },

    /// Arbitrary instructions, equivalent to the old free-form prompt.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Custom { instructions: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnStartResponse {
    pub turn: Turn,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnInterruptResponse {}

// User input types
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl From<CoreByteRange> for ByteRange {
    fn from(value: CoreByteRange) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

impl From<ByteRange> for CoreByteRange {
    fn from(value: ByteRange) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TextElement {
    /// Byte range in the parent `text` buffer that this element occupies.
    pub byte_range: ByteRange,
    /// Optional human-readable placeholder for the element, displayed in the UI.
    placeholder: Option<String>,
}

impl TextElement {
    pub fn new(byte_range: ByteRange, placeholder: Option<String>) -> Self {
        Self {
            byte_range,
            placeholder,
        }
    }

    pub fn set_placeholder(&mut self, placeholder: Option<String>) {
        self.placeholder = placeholder;
    }

    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }
}

impl From<CoreTextElement> for TextElement {
    fn from(value: CoreTextElement) -> Self {
        Self::new(
            value.byte_range.into(),
            value._placeholder_for_conversion_only().map(str::to_string),
        )
    }
}

impl From<TextElement> for CoreTextElement {
    fn from(value: TextElement) -> Self {
        Self::new(value.byte_range.into(), value.placeholder)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum UserInput {
    Text {
        text: String,
        /// UI-defined spans within `text` used to render or persist special elements.
        #[serde(default)]
        text_elements: Vec<TextElement>,
    },
    Image {
        url: String,
    },
    LocalImage {
        path: PathBuf,
    },
    Skill {
        name: String,
        path: PathBuf,
    },
    Mention {
        name: String,
        path: String,
    },
}

impl UserInput {
    pub fn into_core(self) -> CoreUserInput {
        match self {
            UserInput::Text {
                text,
                text_elements,
            } => CoreUserInput::Text {
                text,
                text_elements: text_elements.into_iter().map(Into::into).collect(),
            },
            UserInput::Image { url } => CoreUserInput::Image { image_url: url },
            UserInput::LocalImage { path } => CoreUserInput::LocalImage { path },
            UserInput::Skill { name, path } => CoreUserInput::Skill { name, path },
            UserInput::Mention { name, path } => CoreUserInput::Mention { name, path },
        }
    }
}

impl From<CoreUserInput> for UserInput {
    fn from(value: CoreUserInput) -> Self {
        match value {
            CoreUserInput::Text {
                text,
                text_elements,
            } => UserInput::Text {
                text,
                text_elements: text_elements.into_iter().map(Into::into).collect(),
            },
            CoreUserInput::Image { image_url } => UserInput::Image { url: image_url },
            CoreUserInput::LocalImage { path } => UserInput::LocalImage { path },
            CoreUserInput::Skill { name, path } => UserInput::Skill { name, path },
            CoreUserInput::Mention { name, path } => UserInput::Mention { name, path },
            _ => unreachable!("unsupported user input variant"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum ThreadItem {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    UserMessage { id: String, content: Vec<UserInput> },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    AgentMessage { id: String, text: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    /// EXPERIMENTAL - proposed plan item content. The completed plan item is
    /// authoritative and may not match the concatenation of `PlanDelta` text.
    Plan { id: String, text: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<String>,
        #[serde(default)]
        content: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    CommandExecution {
        id: String,
        /// The command to be executed.
        command: String,
        /// The command's working directory.
        cwd: PathBuf,
        /// Identifier for the underlying PTY process (when available).
        process_id: Option<String>,
        status: CommandExecutionStatus,
        /// A best-effort parsing of the command to understand the action(s) it will perform.
        /// This returns a list of CommandAction objects because a single shell command may
        /// be composed of many commands piped together.
        command_actions: Vec<CommandAction>,
        /// The command's output, aggregated from stdout and stderr.
        aggregated_output: Option<String>,
        /// The command's exit code.
        exit_code: Option<i32>,
        /// The duration of the command execution in milliseconds.
        #[ts(type = "number | null")]
        duration_ms: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    FileChange {
        id: String,
        changes: Vec<FileUpdateChange>,
        status: PatchApplyStatus,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    McpToolCall {
        id: String,
        server: String,
        tool: String,
        status: McpToolCallStatus,
        arguments: JsonValue,
        result: Option<McpToolCallResult>,
        error: Option<McpToolCallError>,
        /// The duration of the MCP tool call in milliseconds.
        #[ts(type = "number | null")]
        duration_ms: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    CollabAgentToolCall {
        /// Unique identifier for this collab tool call.
        id: String,
        /// Name of the collab tool that was invoked.
        tool: CollabAgentTool,
        /// Current status of the collab tool call.
        status: CollabAgentToolCallStatus,
        /// Thread ID of the agent issuing the collab request.
        sender_thread_id: String,
        /// Thread ID of the receiving agent, when applicable. In case of spawn operation,
        /// this corresponds to the newly spawned agent.
        receiver_thread_ids: Vec<String>,
        /// Prompt text sent as part of the collab tool call, when available.
        prompt: Option<String>,
        /// Last known status of the target agents, when available.
        agents_states: HashMap<String, CollabAgentState>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    WebSearch {
        id: String,
        query: String,
        action: Option<WebSearchAction>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ImageView { id: String, path: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    EnteredReviewMode { id: String, review: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ExitedReviewMode { id: String, review: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ContextCompaction { id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
pub enum WebSearchAction {
    Search {
        query: Option<String>,
        queries: Option<Vec<String>>,
    },
    OpenPage {
        url: Option<String>,
    },
    FindInPage {
        url: Option<String>,
        pattern: Option<String>,
    },
    #[serde(other)]
    Other,
}

impl From<codex_protocol::models::WebSearchAction> for WebSearchAction {
    fn from(value: codex_protocol::models::WebSearchAction) -> Self {
        match value {
            codex_protocol::models::WebSearchAction::Search { query, queries } => {
                WebSearchAction::Search { query, queries }
            }
            codex_protocol::models::WebSearchAction::OpenPage { url } => {
                WebSearchAction::OpenPage { url }
            }
            codex_protocol::models::WebSearchAction::FindInPage { url, pattern } => {
                WebSearchAction::FindInPage { url, pattern }
            }
            codex_protocol::models::WebSearchAction::Other => WebSearchAction::Other,
        }
    }
}

impl From<CoreTurnItem> for ThreadItem {
    fn from(value: CoreTurnItem) -> Self {
        match value {
            CoreTurnItem::UserMessage(user) => ThreadItem::UserMessage {
                id: user.id,
                content: user.content.into_iter().map(UserInput::from).collect(),
            },
            CoreTurnItem::AgentMessage(agent) => {
                let text = agent
                    .content
                    .into_iter()
                    .map(|entry| match entry {
                        CoreAgentMessageContent::Text { text } => text,
                    })
                    .collect::<String>();
                ThreadItem::AgentMessage { id: agent.id, text }
            }
            CoreTurnItem::Plan(plan) => ThreadItem::Plan {
                id: plan.id,
                text: plan.text,
            },
            CoreTurnItem::Reasoning(reasoning) => ThreadItem::Reasoning {
                id: reasoning.id,
                summary: reasoning.summary_text,
                content: reasoning.raw_content,
            },
            CoreTurnItem::WebSearch(search) => ThreadItem::WebSearch {
                id: search.id,
                query: search.query,
                action: Some(WebSearchAction::from(search.action)),
            },
            CoreTurnItem::ContextCompaction(compaction) => {
                ThreadItem::ContextCompaction { id: compaction.id }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CommandExecutionStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CollabAgentTool {
    SpawnAgent,
    SendInput,
    Wait,
    CloseAgent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: PatchChangeKind,
    pub diff: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update { move_path: Option<PathBuf> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum PatchApplyStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum McpToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CollabAgentToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CollabAgentStatus {
    PendingInit,
    Running,
    Completed,
    Errored,
    Shutdown,
    NotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CollabAgentState {
    pub status: CollabAgentStatus,
    pub message: Option<String>,
}

impl From<CoreAgentStatus> for CollabAgentState {
    fn from(value: CoreAgentStatus) -> Self {
        match value {
            CoreAgentStatus::PendingInit => Self {
                status: CollabAgentStatus::PendingInit,
                message: None,
            },
            CoreAgentStatus::Running => Self {
                status: CollabAgentStatus::Running,
                message: None,
            },
            CoreAgentStatus::Completed(message) => Self {
                status: CollabAgentStatus::Completed,
                message,
            },
            CoreAgentStatus::Errored(message) => Self {
                status: CollabAgentStatus::Errored,
                message: Some(message),
            },
            CoreAgentStatus::Shutdown => Self {
                status: CollabAgentStatus::Shutdown,
                message: None,
            },
            CoreAgentStatus::NotFound => Self {
                status: CollabAgentStatus::NotFound,
                message: None,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpToolCallResult {
    pub content: Vec<McpContentBlock>,
    pub structured_content: Option<JsonValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpToolCallError {
    pub message: String,
}

// === Server Notifications ===
// Thread/Turn lifecycle notifications and item progress events
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadStartedNotification {
    pub thread: Thread,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadNameUpdatedNotification {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub thread_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnStartedNotification {
    pub thread_id: String,
    pub turn: Turn,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Usage {
    pub input_tokens: i32,
    pub cached_input_tokens: i32,
    pub output_tokens: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnCompletedNotification {
    pub thread_id: String,
    pub turn: Turn,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// Notification that the turn-level unified diff has changed.
/// Contains the latest aggregated diff across all file changes in the turn.
pub struct TurnDiffUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub diff: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnPlanUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub explanation: Option<String>,
    pub plan: Vec<TurnPlanStep>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnPlanStep {
    pub step: String,
    pub status: TurnPlanStepStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum TurnPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

impl From<CorePlanItemArg> for TurnPlanStep {
    fn from(value: CorePlanItemArg) -> Self {
        Self {
            step: value.step,
            status: value.status.into(),
        }
    }
}

impl From<CorePlanStepStatus> for TurnPlanStepStatus {
    fn from(value: CorePlanStepStatus) -> Self {
        match value {
            CorePlanStepStatus::Pending => Self::Pending,
            CorePlanStepStatus::InProgress => Self::InProgress,
            CorePlanStepStatus::Completed => Self::Completed,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ItemStartedNotification {
    pub item: ThreadItem,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ItemCompletedNotification {
    pub item: ThreadItem,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RawResponseItemCompletedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item: ResponseItem,
}

// Item-specific progress notifications
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AgentMessageDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - proposed plan streaming deltas for plan items. Clients should
/// not assume concatenated deltas match the completed plan item content.
pub struct PlanDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReasoningSummaryTextDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
    #[ts(type = "number")]
    pub summary_index: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReasoningSummaryPartAddedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[ts(type = "number")]
    pub summary_index: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReasoningTextDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
    #[ts(type = "number")]
    pub content_index: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TerminalInteractionNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub process_id: String,
    pub stdin: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecutionOutputDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FileChangeOutputDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpToolCallProgressNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerOauthLoginCompletedNotification {
    pub name: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WindowsWorldWritableWarningNotification {
    pub sample_paths: Vec<String>,
    pub extra_count: usize,
    pub failed_scan: bool,
}

/// Deprecated: Use `ContextCompaction` item type instead.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ContextCompactedNotification {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecutionRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    /// Optional explanatory reason (e.g. request for network access).
    pub reason: Option<String>,
    /// The command to be executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub command: Option<String>,
    /// The command's working directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<PathBuf>,
    /// Best-effort parsed command actions for friendly display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub command_actions: Option<Vec<CommandAction>>,
    /// Optional proposed execpolicy amendment to allow similar commands without prompting.
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecutionRequestApprovalResponse {
    pub decision: CommandExecutionApprovalDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    /// Optional explanatory reason (e.g. request for extra write access).
    pub reason: Option<String>,
    /// [UNSTABLE] When set, the agent is asking the user to allow writes under this root
    /// for the remainder of the session (unclear if this is honored today).
    pub grant_root: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[ts(export_to = "v2/")]
pub struct FileChangeRequestApprovalResponse {
    pub decision: FileChangeApprovalDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolCallResponse {
    pub output: String,
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL. Defines a single selectable option for request_user_input.
pub struct ToolRequestUserInputOption {
    pub label: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL. Represents one request_user_input question and its required options.
pub struct ToolRequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub is_other: bool,
    #[serde(default)]
    pub is_secret: bool,
    pub options: Option<Vec<ToolRequestUserInputOption>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL. Params sent with a request_user_input event.
pub struct ToolRequestUserInputParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL. Captures a user's answer to a request_user_input question.
pub struct ToolRequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL. Response payload mapping question ids to answers.
pub struct ToolRequestUserInputResponse {
    pub answers: HashMap<String, ToolRequestUserInputAnswer>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AccountRateLimitsUpdatedNotification {
    pub rate_limits: RateLimitSnapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RateLimitSnapshot {
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub credits: Option<CreditsSnapshot>,
    pub plan_type: Option<PlanType>,
}

impl From<CoreRateLimitSnapshot> for RateLimitSnapshot {
    fn from(value: CoreRateLimitSnapshot) -> Self {
        Self {
            primary: value.primary.map(RateLimitWindow::from),
            secondary: value.secondary.map(RateLimitWindow::from),
            credits: value.credits.map(CreditsSnapshot::from),
            plan_type: value.plan_type,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RateLimitWindow {
    pub used_percent: i32,
    #[ts(type = "number | null")]
    pub window_duration_mins: Option<i64>,
    #[ts(type = "number | null")]
    pub resets_at: Option<i64>,
}

impl From<CoreRateLimitWindow> for RateLimitWindow {
    fn from(value: CoreRateLimitWindow) -> Self {
        Self {
            used_percent: value.used_percent.round() as i32,
            window_duration_mins: value.window_minutes,
            resets_at: value.resets_at,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CreditsSnapshot {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

impl From<CoreCreditsSnapshot> for CreditsSnapshot {
    fn from(value: CoreCreditsSnapshot) -> Self {
        Self {
            has_credits: value.has_credits,
            unlimited: value.unlimited,
            balance: value.balance,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AccountLoginCompletedNotification {
    // Use plain String for identifiers to avoid TS/JSON Schema quirks around uuid-specific types.
    // Convert to/from UUIDs at the application layer as needed.
    pub login_id: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeprecationNoticeNotification {
    /// Concise summary of what is deprecated.
    pub summary: String,
    /// Optional extra guidance, such as migration steps or rationale.
    pub details: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TextPosition {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number (in Unicode scalar values).
    pub column: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigWarningNotification {
    /// Concise summary of the warning.
    pub summary: String,
    /// Optional extra guidance or error details.
    pub details: Option<String>,
    /// Optional path to the config file that triggered the warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    /// Optional range for the error location inside the config file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub range: Option<TextRange>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::items::AgentMessageContent;
    use codex_protocol::items::AgentMessageItem;
    use codex_protocol::items::ReasoningItem;
    use codex_protocol::items::TurnItem;
    use codex_protocol::items::UserMessageItem;
    use codex_protocol::items::WebSearchItem;
    use codex_protocol::models::WebSearchAction as CoreWebSearchAction;
    use codex_protocol::protocol::NetworkAccess as CoreNetworkAccess;
    use codex_protocol::user_input::UserInput as CoreUserInput;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn sandbox_policy_round_trips_external_sandbox_network_access() {
        let v2_policy = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        };

        let core_policy = v2_policy.to_core();
        assert_eq!(
            core_policy,
            codex_protocol::protocol::SandboxPolicy::ExternalSandbox {
                network_access: CoreNetworkAccess::Enabled,
            }
        );

        let back_to_v2 = SandboxPolicy::from(core_policy);
        assert_eq!(back_to_v2, v2_policy);
    }

    #[test]
    fn core_turn_item_into_thread_item_converts_supported_variants() {
        let user_item = TurnItem::UserMessage(UserMessageItem {
            id: "user-1".to_string(),
            content: vec![
                CoreUserInput::Text {
                    text: "hello".to_string(),
                    text_elements: Vec::new(),
                },
                CoreUserInput::Image {
                    image_url: "https://example.com/image.png".to_string(),
                },
                CoreUserInput::LocalImage {
                    path: PathBuf::from("local/image.png"),
                },
                CoreUserInput::Skill {
                    name: "skill-creator".to_string(),
                    path: PathBuf::from("/repo/.codex/skills/skill-creator/SKILL.md"),
                },
                CoreUserInput::Mention {
                    name: "Demo App".to_string(),
                    path: "app://demo-app".to_string(),
                },
            ],
        });

        assert_eq!(
            ThreadItem::from(user_item),
            ThreadItem::UserMessage {
                id: "user-1".to_string(),
                content: vec![
                    UserInput::Text {
                        text: "hello".to_string(),
                        text_elements: Vec::new(),
                    },
                    UserInput::Image {
                        url: "https://example.com/image.png".to_string(),
                    },
                    UserInput::LocalImage {
                        path: PathBuf::from("local/image.png"),
                    },
                    UserInput::Skill {
                        name: "skill-creator".to_string(),
                        path: PathBuf::from("/repo/.codex/skills/skill-creator/SKILL.md"),
                    },
                    UserInput::Mention {
                        name: "Demo App".to_string(),
                        path: "app://demo-app".to_string(),
                    },
                ],
            }
        );

        let agent_item = TurnItem::AgentMessage(AgentMessageItem {
            id: "agent-1".to_string(),
            content: vec![
                AgentMessageContent::Text {
                    text: "Hello ".to_string(),
                },
                AgentMessageContent::Text {
                    text: "world".to_string(),
                },
            ],
        });

        assert_eq!(
            ThreadItem::from(agent_item),
            ThreadItem::AgentMessage {
                id: "agent-1".to_string(),
                text: "Hello world".to_string(),
            }
        );

        let reasoning_item = TurnItem::Reasoning(ReasoningItem {
            id: "reasoning-1".to_string(),
            summary_text: vec!["line one".to_string(), "line two".to_string()],
            raw_content: vec![],
        });

        assert_eq!(
            ThreadItem::from(reasoning_item),
            ThreadItem::Reasoning {
                id: "reasoning-1".to_string(),
                summary: vec!["line one".to_string(), "line two".to_string()],
                content: vec![],
            }
        );

        let search_item = TurnItem::WebSearch(WebSearchItem {
            id: "search-1".to_string(),
            query: "docs".to_string(),
            action: CoreWebSearchAction::Search {
                query: Some("docs".to_string()),
                queries: None,
            },
        });

        assert_eq!(
            ThreadItem::from(search_item),
            ThreadItem::WebSearch {
                id: "search-1".to_string(),
                query: "docs".to_string(),
                action: Some(WebSearchAction::Search {
                    query: Some("docs".to_string()),
                    queries: None,
                }),
            }
        );
    }

    #[test]
    fn skills_list_params_serialization_uses_force_reload() {
        assert_eq!(
            serde_json::to_value(SkillsListParams {
                cwds: Vec::new(),
                force_reload: false,
            })
            .unwrap(),
            json!({}),
        );

        assert_eq!(
            serde_json::to_value(SkillsListParams {
                cwds: vec![PathBuf::from("/repo")],
                force_reload: true,
            })
            .unwrap(),
            json!({
                "cwds": ["/repo"],
                "forceReload": true,
            }),
        );
    }

    #[test]
    fn codex_error_info_serializes_http_status_code_in_camel_case() {
        let value = CodexErrorInfo::ResponseTooManyFailedAttempts {
            http_status_code: Some(401),
        };

        assert_eq!(
            serde_json::to_value(value).unwrap(),
            json!({
                "responseTooManyFailedAttempts": {
                    "httpStatusCode": 401
                }
            })
        );
    }
}
