use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::RequestId;
use crate::protocol::common::AuthMode;
use codex_experimental_api_macros::ExperimentalApi;
use codex_protocol::account::PlanType;
use codex_protocol::approvals::ElicitationRequest as CoreElicitationRequest;
use codex_protocol::approvals::ExecPolicyAmendment as CoreExecPolicyAmendment;
use codex_protocol::approvals::GuardianAssessmentAction as CoreGuardianAssessmentAction;
use codex_protocol::approvals::GuardianAssessmentDecisionSource as CoreGuardianAssessmentDecisionSource;
use codex_protocol::approvals::GuardianCommandSource as CoreGuardianCommandSource;
use codex_protocol::approvals::NetworkApprovalContext as CoreNetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol as CoreNetworkApprovalProtocol;
use codex_protocol::approvals::NetworkPolicyAmendment as CoreNetworkPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyRuleAction as CoreNetworkPolicyRuleAction;
use codex_protocol::config_types::ApprovalsReviewer as CoreApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::CollaborationModeMask as CoreCollaborationModeMask;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode as CoreSandboxMode;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::Verbosity;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WebSearchToolConfig;
use codex_protocol::items::AgentMessageContent as CoreAgentMessageContent;
use codex_protocol::items::TurnItem as CoreTurnItem;
use codex_protocol::mcp::CallToolResult as CoreMcpCallToolResult;
use codex_protocol::mcp::Resource as McpResource;
pub use codex_protocol::mcp::ResourceContent as McpResourceContent;
use codex_protocol::mcp::ResourceTemplate as McpResourceTemplate;
use codex_protocol::mcp::Tool as McpTool;
use codex_protocol::memory_citation::MemoryCitation as CoreMemoryCitation;
use codex_protocol::memory_citation::MemoryCitationEntry as CoreMemoryCitationEntry;
use codex_protocol::models::FileSystemPermissions as CoreFileSystemPermissions;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::NetworkPermissions as CoreNetworkPermissions;
use codex_protocol::models::PermissionProfile as CorePermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelAvailabilityNux as CoreModelAvailabilityNux;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::parse_command::ParsedCommand as CoreParsedCommand;
use codex_protocol::permissions::FileSystemAccessMode as CoreFileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath as CoreFileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry as CoreFileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath as CoreFileSystemSpecialPath;
use codex_protocol::plan_tool::PlanItemArg as CorePlanItemArg;
use codex_protocol::plan_tool::StepStatus as CorePlanStepStatus;
use codex_protocol::protocol::AgentStatus as CoreAgentStatus;
use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
use codex_protocol::protocol::CodexErrorInfo as CoreCodexErrorInfo;
use codex_protocol::protocol::CreditsSnapshot as CoreCreditsSnapshot;
use codex_protocol::protocol::ExecCommandSource as CoreExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
use codex_protocol::protocol::GranularApprovalConfig as CoreGranularApprovalConfig;
use codex_protocol::protocol::GuardianRiskLevel as CoreGuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization as CoreGuardianUserAuthorization;
use codex_protocol::protocol::HookEventName as CoreHookEventName;
use codex_protocol::protocol::HookExecutionMode as CoreHookExecutionMode;
use codex_protocol::protocol::HookHandlerType as CoreHookHandlerType;
use codex_protocol::protocol::HookOutputEntry as CoreHookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind as CoreHookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus as CoreHookRunStatus;
use codex_protocol::protocol::HookRunSummary as CoreHookRunSummary;
use codex_protocol::protocol::HookScope as CoreHookScope;
use codex_protocol::protocol::HookSource as CoreHookSource;
use codex_protocol::protocol::ModelRerouteReason as CoreModelRerouteReason;
use codex_protocol::protocol::NetworkAccess as CoreNetworkAccess;
use codex_protocol::protocol::NonSteerableTurnKind as CoreNonSteerableTurnKind;
use codex_protocol::protocol::PatchApplyStatus as CorePatchApplyStatus;
use codex_protocol::protocol::RateLimitReachedType as CoreRateLimitReachedType;
use codex_protocol::protocol::RateLimitSnapshot as CoreRateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow as CoreRateLimitWindow;
use codex_protocol::protocol::ReadOnlyAccess as CoreReadOnlyAccess;
use codex_protocol::protocol::RealtimeAudioFrame as CoreRealtimeAudioFrame;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use codex_protocol::protocol::ReviewDecision as CoreReviewDecision;
use codex_protocol::protocol::SessionSource as CoreSessionSource;
use codex_protocol::protocol::SkillDependencies as CoreSkillDependencies;
use codex_protocol::protocol::SkillInterface as CoreSkillInterface;
use codex_protocol::protocol::SkillMetadata as CoreSkillMetadata;
use codex_protocol::protocol::SkillScope as CoreSkillScope;
use codex_protocol::protocol::SkillToolDependency as CoreSkillToolDependency;
use codex_protocol::protocol::SubAgentSource as CoreSubAgentSource;
use codex_protocol::protocol::TokenUsage as CoreTokenUsage;
use codex_protocol::protocol::TokenUsageInfo as CoreTokenUsageInfo;
use codex_protocol::request_permissions::PermissionGrantScope as CorePermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use codex_protocol::user_input::ByteRange as CoreByteRange;
use codex_protocol::user_input::TextElement as CoreTextElement;
use codex_protocol::user_input::UserInput as CoreUserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::InstanceType;
use schemars::schema::Metadata;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_with::serde_as;
use thiserror::Error;
use ts_rs::TS;

// Macro to declare a camelCased API v2 enum mirroring a core enum which
// tends to use either snake_case or kebab-case.
macro_rules! v2_enum_from_core {
    (
        $(#[$enum_meta:meta])*
        pub enum $Name:ident from $Src:path {
            $( $(#[$variant_meta:meta])* $Variant:ident ),+ $(,)?
        }
    ) => {
        #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
        $(#[$enum_meta])*
        #[serde(rename_all = "camelCase")]
        #[ts(export_to = "v2/")]
        pub enum $Name {
            $( $(#[$variant_meta])* $Variant ),+
        }

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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum NonSteerableTurnKind {
    Review,
    Compact,
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
    ServerOverloaded,
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
    /// Returned when `turn/start` or `turn/steer` is submitted while the current active turn
    /// cannot accept same-turn steering, for example `/review` or manual `/compact`.
    ActiveTurnNotSteerable {
        #[serde(rename = "turnKind")]
        #[ts(rename = "turnKind")]
        turn_kind: NonSteerableTurnKind,
    },
    Other,
}

impl From<CoreCodexErrorInfo> for CodexErrorInfo {
    fn from(value: CoreCodexErrorInfo) -> Self {
        match value {
            CoreCodexErrorInfo::ContextWindowExceeded => CodexErrorInfo::ContextWindowExceeded,
            CoreCodexErrorInfo::UsageLimitExceeded => CodexErrorInfo::UsageLimitExceeded,
            CoreCodexErrorInfo::ServerOverloaded => CodexErrorInfo::ServerOverloaded,
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
            CoreCodexErrorInfo::ActiveTurnNotSteerable { turn_kind } => {
                CodexErrorInfo::ActiveTurnNotSteerable {
                    turn_kind: turn_kind.into(),
                }
            }
            CoreCodexErrorInfo::Other => CodexErrorInfo::Other,
        }
    }
}

impl From<CoreNonSteerableTurnKind> for NonSteerableTurnKind {
    fn from(value: CoreNonSteerableTurnKind) -> Self {
        match value {
            CoreNonSteerableTurnKind::Review => Self::Review,
            CoreNonSteerableTurnKind::Compact => Self::Compact,
        }
    }
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS, ExperimentalApi,
)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case", export_to = "v2/")]
pub enum AskForApproval {
    #[serde(rename = "untrusted")]
    #[ts(rename = "untrusted")]
    UnlessTrusted,
    OnFailure,
    OnRequest,
    #[experimental("askForApproval.granular")]
    Granular {
        sandbox_approval: bool,
        rules: bool,
        #[serde(default)]
        skill_approval: bool,
        #[serde(default)]
        request_permissions: bool,
        mcp_elicitations: bool,
    },
    Never,
}

impl AskForApproval {
    pub fn to_core(self) -> CoreAskForApproval {
        match self {
            AskForApproval::UnlessTrusted => CoreAskForApproval::UnlessTrusted,
            AskForApproval::OnFailure => CoreAskForApproval::OnFailure,
            AskForApproval::OnRequest => CoreAskForApproval::OnRequest,
            AskForApproval::Granular {
                sandbox_approval,
                rules,
                skill_approval,
                request_permissions,
                mcp_elicitations,
            } => CoreAskForApproval::Granular(CoreGranularApprovalConfig {
                sandbox_approval,
                rules,
                skill_approval,
                request_permissions,
                mcp_elicitations,
            }),
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
            CoreAskForApproval::Granular(granular_config) => AskForApproval::Granular {
                sandbox_approval: granular_config.sandbox_approval,
                rules: granular_config.rules,
                skill_approval: granular_config.skill_approval,
                request_permissions: granular_config.request_permissions,
                mcp_elicitations: granular_config.mcp_elicitations,
            },
            CoreAskForApproval::Never => AskForApproval::Never,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(
    type = r#""user" | "auto_review" | "guardian_subagent""#,
    export_to = "v2/"
)]
/// Configures who approval requests are routed to for review. Examples
/// include sandbox escapes, blocked network access, MCP approval prompts, and
/// ARC escalations. Defaults to `user`. `auto_review` uses a carefully
/// prompted subagent to gather relevant context and apply a risk-based
/// decision framework before approving or denying the request.
pub enum ApprovalsReviewer {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "auto_review", alias = "guardian_subagent")]
    GuardianSubagent,
}

impl JsonSchema for ApprovalsReviewer {
    fn schema_name() -> String {
        "ApprovalsReviewer".to_string()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        string_enum_schema_with_description(
            &["user", "auto_review", "guardian_subagent"],
            "Configures who approval requests are routed to for review. Examples include sandbox escapes, blocked network access, MCP approval prompts, and ARC escalations. Defaults to `user`. `auto_review` uses a carefully prompted subagent to gather relevant context and apply a risk-based decision framework before approving or denying the request. The legacy value `guardian_subagent` is accepted for compatibility.",
        )
    }
}

fn string_enum_schema_with_description(values: &[&str], description: &str) -> Schema {
    let mut schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        metadata: Some(Box::new(Metadata {
            description: Some(description.to_string()),
            ..Default::default()
        })),
        ..Default::default()
    };
    schema.enum_values = Some(
        values
            .iter()
            .map(|value| JsonValue::String((*value).to_string()))
            .collect(),
    );
    Schema::Object(schema)
}

impl ApprovalsReviewer {
    pub fn to_core(self) -> CoreApprovalsReviewer {
        match self {
            ApprovalsReviewer::User => CoreApprovalsReviewer::User,
            ApprovalsReviewer::GuardianSubagent => CoreApprovalsReviewer::GuardianSubagent,
        }
    }
}

impl From<CoreApprovalsReviewer> for ApprovalsReviewer {
    fn from(value: CoreApprovalsReviewer) -> Self {
        match value {
            CoreApprovalsReviewer::User => ApprovalsReviewer::User,
            CoreApprovalsReviewer::GuardianSubagent => ApprovalsReviewer::GuardianSubagent,
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

v2_enum_from_core!(
    pub enum ModelRerouteReason from CoreModelRerouteReason {
        HighRiskCyberActivity
    }
);

v2_enum_from_core!(
    pub enum HookEventName from CoreHookEventName {
        PreToolUse, PermissionRequest, PostToolUse, SessionStart, UserPromptSubmit, Stop
    }
);

v2_enum_from_core!(
    pub enum HookHandlerType from CoreHookHandlerType {
        Command, Prompt, Agent
    }
);

v2_enum_from_core!(
    pub enum HookExecutionMode from CoreHookExecutionMode {
        Sync, Async
    }
);

v2_enum_from_core!(
    pub enum HookScope from CoreHookScope {
        Thread, Turn
    }
);

v2_enum_from_core!(
    pub enum HookSource from CoreHookSource {
        System,
        User,
        Project,
        Mdm,
        SessionFlags,
        LegacyManagedConfigFile,
        LegacyManagedConfigMdm,
        Unknown,
    }
);

fn default_hook_source() -> HookSource {
    HookSource::Unknown
}

v2_enum_from_core!(
    pub enum HookRunStatus from CoreHookRunStatus {
        Running, Completed, Failed, Blocked, Stopped
    }
);

v2_enum_from_core!(
    pub enum HookOutputEntryKind from CoreHookOutputEntryKind {
        Warning, Stop, Feedback, Context, Error
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadStartSource {
    Startup,
    Clear,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct HookOutputEntry {
    pub kind: HookOutputEntryKind,
    pub text: String,
}

impl From<CoreHookOutputEntry> for HookOutputEntry {
    fn from(value: CoreHookOutputEntry) -> Self {
        Self {
            kind: value.kind.into(),
            text: value.text,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct HookRunSummary {
    pub id: String,
    pub event_name: HookEventName,
    pub handler_type: HookHandlerType,
    pub execution_mode: HookExecutionMode,
    pub scope: HookScope,
    pub source_path: AbsolutePathBuf,
    #[serde(default = "default_hook_source")]
    pub source: HookSource,
    pub display_order: i64,
    pub status: HookRunStatus,
    pub status_message: Option<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub entries: Vec<HookOutputEntry>,
}

impl From<CoreHookRunSummary> for HookRunSummary {
    fn from(value: CoreHookRunSummary) -> Self {
        Self {
            id: value.id,
            event_name: value.event_name.into(),
            handler_type: value.handler_type.into(),
            execution_mode: value.execution_mode.into(),
            scope: value.scope.into(),
            source_path: value.source_path,
            source: value.source.into(),
            display_order: value.display_order,
            status: value.status.into(),
            status_message: value.status_message,
            started_at: value.started_at,
            completed_at: value.completed_at,
            duration_ms: value.duration_ms,
            entries: value.entries.into_iter().map(Into::into).collect(),
        }
    }
}

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
    pub web_search: Option<WebSearchToolConfig>,
    pub view_image: Option<bool>,
}

#[derive(Serialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: JsonValue,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub defer_loading: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DynamicToolSpecDe {
    name: String,
    description: String,
    input_schema: JsonValue,
    defer_loading: Option<bool>,
    expose_to_context: Option<bool>,
}

impl<'de> Deserialize<'de> for DynamicToolSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let DynamicToolSpecDe {
            name,
            description,
            input_schema,
            defer_loading,
            expose_to_context,
        } = DynamicToolSpecDe::deserialize(deserializer)?;

        Ok(Self {
            name,
            description,
            input_schema,
            defer_loading: defer_loading
                .unwrap_or_else(|| expose_to_context.map(|visible| !visible).unwrap_or(false)),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ProfileV2 {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    #[experimental(nested)]
    pub approval_policy: Option<AskForApproval>,
    /// [UNSTABLE] Optional profile-level override for where approval requests
    /// are routed for review. If omitted, the enclosing config default is
    /// used.
    #[experimental("config/read.approvalsReviewer")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub service_tier: Option<ServiceTier>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub web_search: Option<WebSearchMode>,
    pub tools: Option<ToolsV2>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub enum AppToolApproval {
    Auto,
    Prompt,
    Approve,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct AppsDefaultConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_enabled")]
    pub destructive_enabled: bool,
    #[serde(default = "default_enabled")]
    pub open_world_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct AppToolConfig {
    pub enabled: Option<bool>,
    pub approval_mode: Option<AppToolApproval>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct AppToolsConfig {
    #[serde(default, flatten)]
    pub tools: HashMap<String, AppToolConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct AppConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub destructive_enabled: Option<bool>,
    pub open_world_enabled: Option<bool>,
    pub default_tools_approval_mode: Option<AppToolApproval>,
    pub default_tools_enabled: Option<bool>,
    pub tools: Option<AppToolsConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct AppsConfig {
    #[serde(default, rename = "_default")]
    pub default: Option<AppsDefaultConfig>,
    #[serde(default, flatten)]
    pub apps: HashMap<String, AppConfig>,
}

const fn default_enabled() -> bool {
    true
}

const fn default_include_platform_defaults() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct Config {
    pub model: Option<String>,
    pub review_model: Option<String>,
    pub model_context_window: Option<i64>,
    pub model_auto_compact_token_limit: Option<i64>,
    pub model_provider: Option<String>,
    #[experimental(nested)]
    pub approval_policy: Option<AskForApproval>,
    /// [UNSTABLE] Optional default for where approval requests are routed for
    /// review.
    #[experimental("config/read.approvalsReviewer")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_mode: Option<SandboxMode>,
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub web_search: Option<WebSearchMode>,
    pub tools: Option<ToolsV2>,
    pub profile: Option<String>,
    #[experimental(nested)]
    #[serde(default)]
    pub profiles: HashMap<String, ProfileV2>,
    pub instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub compact_prompt: Option<String>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub service_tier: Option<ServiceTier>,
    pub analytics: Option<AnalyticsConfig>,
    #[experimental("config/read.apps")]
    #[serde(default)]
    pub apps: Option<AppsConfig>,
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
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigReadResponse {
    #[experimental(nested)]
    pub config: Config,
    pub origins: HashMap<String, ConfigLayerMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<ConfigLayer>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigRequirements {
    #[experimental(nested)]
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
    #[experimental("configRequirements/read.allowedApprovalsReviewers")]
    pub allowed_approvals_reviewers: Option<Vec<ApprovalsReviewer>>,
    pub allowed_sandbox_modes: Option<Vec<SandboxMode>>,
    pub allowed_web_search_modes: Option<Vec<WebSearchMode>>,
    pub feature_requirements: Option<BTreeMap<String, bool>>,
    pub enforce_residency: Option<ResidencyRequirement>,
    #[experimental("configRequirements/read.network")]
    pub network: Option<NetworkRequirements>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum NetworkDomainPermission {
    Allow,
    Deny,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum NetworkUnixSocketPermission {
    Allow,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ResidencyRequirement {
    Us,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigRequirementsReadResponse {
    /// Null if no requirements are configured (e.g. no requirements.toml/MDM entries).
    #[experimental(nested)]
    pub requirements: Option<ConfigRequirements>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, TS)]
#[ts(export_to = "v2/")]
pub enum ExternalAgentConfigMigrationItemType {
    #[serde(rename = "AGENTS_MD")]
    #[ts(rename = "AGENTS_MD")]
    AgentsMd,
    #[serde(rename = "CONFIG")]
    #[ts(rename = "CONFIG")]
    Config,
    #[serde(rename = "SKILLS")]
    #[ts(rename = "SKILLS")]
    Skills,
    #[serde(rename = "PLUGINS")]
    #[ts(rename = "PLUGINS")]
    Plugins,
    #[serde(rename = "MCP_SERVER_CONFIG")]
    #[ts(rename = "MCP_SERVER_CONFIG")]
    McpServerConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginsMigration {
    #[serde(rename = "marketplaceName")]
    #[ts(rename = "marketplaceName")]
    pub marketplace_name: String,
    #[serde(rename = "pluginNames")]
    #[ts(rename = "pluginNames")]
    pub plugin_names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MigrationDetails {
    pub plugins: Vec<PluginsMigration>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigMigrationItem {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    /// Null or empty means home-scoped migration; non-empty means repo-scoped migration.
    pub cwd: Option<PathBuf>,
    pub details: Option<MigrationDetails>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigDetectResponse {
    pub items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigDetectParams {
    /// If true, include detection under the user's home (~/.claude, ~/.codex, etc.).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_home: bool,
    /// Zero or more working directories to include for repo-scoped detection.
    #[ts(optional = nullable)]
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigImportParams {
    pub migration_items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigImportResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExternalAgentConfigImportCompletedNotification {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ConfigBatchWriteParams {
    pub edits: Vec<ConfigEdit>,
    /// Path to the config file to write; defaults to the user's `config.toml` when omitted.
    #[ts(optional = nullable)]
    pub file_path: Option<String>,
    #[ts(optional = nullable)]
    pub expected_version: Option<String>,
    /// When true, hot-reload the updated user config into all loaded threads after writing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reload_user_config: bool,
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
    /// User approved the command and future prompts in the same session-scoped
    /// approval cache should run without prompting.
    AcceptForSession,
    /// User approved the command, and wants to apply the proposed execpolicy amendment so future
    /// matching commands can run without prompting.
    AcceptWithExecpolicyAmendment {
        execpolicy_amendment: ExecPolicyAmendment,
    },
    /// User chose a persistent network policy rule (allow/deny) for this host.
    ApplyNetworkPolicyAmendment {
        network_policy_amendment: NetworkPolicyAmendment,
    },
    /// User denied the command. The agent will continue the turn.
    Decline,
    /// User denied the command. The turn will also be immediately interrupted.
    Cancel,
}

impl From<CoreReviewDecision> for CommandExecutionApprovalDecision {
    fn from(value: CoreReviewDecision) -> Self {
        match value {
            CoreReviewDecision::Approved => Self::Accept,
            CoreReviewDecision::ApprovedExecpolicyAmendment {
                proposed_execpolicy_amendment,
            } => Self::AcceptWithExecpolicyAmendment {
                execpolicy_amendment: proposed_execpolicy_amendment.into(),
            },
            CoreReviewDecision::ApprovedForSession => Self::AcceptForSession,
            CoreReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } => Self::ApplyNetworkPolicyAmendment {
                network_policy_amendment: network_policy_amendment.into(),
            },
            CoreReviewDecision::Abort => Self::Cancel,
            CoreReviewDecision::Denied => Self::Decline,
            CoreReviewDecision::TimedOut => Self::Decline,
        }
    }
}

v2_enum_from_core! {
    pub enum NetworkApprovalProtocol from CoreNetworkApprovalProtocol {
        Http,
        Https,
        Socks5Tcp,
        Socks5Udp,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct NetworkApprovalContext {
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
}

impl From<CoreNetworkApprovalContext> for NetworkApprovalContext {
    fn from(value: CoreNetworkApprovalContext) -> Self {
        Self {
            host: value.host,
            protocol: value.protocol.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AdditionalFileSystemPermissions {
    pub read: Option<Vec<AbsolutePathBuf>>,
    pub write: Option<Vec<AbsolutePathBuf>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub entries: Option<Vec<FileSystemSandboxEntry>>,
}

impl From<CoreFileSystemPermissions> for AdditionalFileSystemPermissions {
    fn from(value: CoreFileSystemPermissions) -> Self {
        if let Some((read, write)) = value.legacy_read_write_roots() {
            Self {
                read,
                write,
                entries: None,
            }
        } else {
            Self {
                read: None,
                write: None,
                entries: Some(
                    value
                        .entries
                        .into_iter()
                        .map(FileSystemSandboxEntry::from)
                        .collect(),
                ),
            }
        }
    }
}

impl From<AdditionalFileSystemPermissions> for CoreFileSystemPermissions {
    fn from(value: AdditionalFileSystemPermissions) -> Self {
        if let Some(entries) = value.entries {
            Self {
                entries: entries
                    .into_iter()
                    .map(CoreFileSystemSandboxEntry::from)
                    .collect(),
            }
        } else {
            CoreFileSystemPermissions::from_read_write_roots(value.read, value.write)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AdditionalNetworkPermissions {
    pub enabled: Option<bool>,
}

impl From<CoreNetworkPermissions> for AdditionalNetworkPermissions {
    fn from(value: CoreNetworkPermissions) -> Self {
        Self {
            enabled: value.enabled,
        }
    }
}

impl From<AdditionalNetworkPermissions> for CoreNetworkPermissions {
    fn from(value: AdditionalNetworkPermissions) -> Self {
        Self {
            enabled: value.enabled,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct RequestPermissionProfile {
    pub network: Option<AdditionalNetworkPermissions>,
    pub file_system: Option<AdditionalFileSystemPermissions>,
}

impl From<CoreRequestPermissionProfile> for RequestPermissionProfile {
    fn from(value: CoreRequestPermissionProfile) -> Self {
        Self {
            network: value.network.map(AdditionalNetworkPermissions::from),
            file_system: value.file_system.map(AdditionalFileSystemPermissions::from),
        }
    }
}

impl From<RequestPermissionProfile> for CoreRequestPermissionProfile {
    fn from(value: RequestPermissionProfile) -> Self {
        Self {
            network: value.network.map(CoreNetworkPermissions::from),
            file_system: value.file_system.map(CoreFileSystemPermissions::from),
        }
    }
}

v2_enum_from_core!(
    pub enum FileSystemAccessMode from CoreFileSystemAccessMode {
        Read,
        Write,
        None
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind")]
#[ts(export_to = "v2/")]
pub enum FileSystemSpecialPath {
    Root,
    Minimal,
    CurrentWorkingDirectory,
    ProjectRoots {
        subpath: Option<PathBuf>,
    },
    Tmpdir,
    SlashTmp,
    Unknown {
        path: String,
        subpath: Option<PathBuf>,
    },
}

impl From<CoreFileSystemSpecialPath> for FileSystemSpecialPath {
    fn from(value: CoreFileSystemSpecialPath) -> Self {
        match value {
            CoreFileSystemSpecialPath::Root => Self::Root,
            CoreFileSystemSpecialPath::Minimal => Self::Minimal,
            CoreFileSystemSpecialPath::CurrentWorkingDirectory => Self::CurrentWorkingDirectory,
            CoreFileSystemSpecialPath::ProjectRoots { subpath } => Self::ProjectRoots { subpath },
            CoreFileSystemSpecialPath::Tmpdir => Self::Tmpdir,
            CoreFileSystemSpecialPath::SlashTmp => Self::SlashTmp,
            CoreFileSystemSpecialPath::Unknown { path, subpath } => Self::Unknown { path, subpath },
        }
    }
}

impl From<FileSystemSpecialPath> for CoreFileSystemSpecialPath {
    fn from(value: FileSystemSpecialPath) -> Self {
        match value {
            FileSystemSpecialPath::Root => Self::Root,
            FileSystemSpecialPath::Minimal => Self::Minimal,
            FileSystemSpecialPath::CurrentWorkingDirectory => Self::CurrentWorkingDirectory,
            FileSystemSpecialPath::ProjectRoots { subpath } => Self::ProjectRoots { subpath },
            FileSystemSpecialPath::Tmpdir => Self::Tmpdir,
            FileSystemSpecialPath::SlashTmp => Self::SlashTmp,
            FileSystemSpecialPath::Unknown { path, subpath } => Self::Unknown { path, subpath },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum FileSystemPath {
    Path { path: AbsolutePathBuf },
    GlobPattern { pattern: String },
    Special { value: FileSystemSpecialPath },
}

impl From<CoreFileSystemPath> for FileSystemPath {
    fn from(value: CoreFileSystemPath) -> Self {
        match value {
            CoreFileSystemPath::Path { path } => Self::Path { path },
            CoreFileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            CoreFileSystemPath::Special { value } => Self::Special {
                value: value.into(),
            },
        }
    }
}

impl From<FileSystemPath> for CoreFileSystemPath {
    fn from(value: FileSystemPath) -> Self {
        match value {
            FileSystemPath::Path { path } => Self::Path { path },
            FileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            FileSystemPath::Special { value } => Self::Special {
                value: value.into(),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FileSystemSandboxEntry {
    pub path: FileSystemPath,
    pub access: FileSystemAccessMode,
}

impl From<CoreFileSystemSandboxEntry> for FileSystemSandboxEntry {
    fn from(value: CoreFileSystemSandboxEntry) -> Self {
        Self {
            path: value.path.into(),
            access: value.access.into(),
        }
    }
}

impl From<FileSystemSandboxEntry> for CoreFileSystemSandboxEntry {
    fn from(value: FileSystemSandboxEntry) -> Self {
        Self {
            path: value.path.into(),
            access: value.access.to_core(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AdditionalPermissionProfile {
    pub network: Option<AdditionalNetworkPermissions>,
    pub file_system: Option<AdditionalFileSystemPermissions>,
}

impl From<CorePermissionProfile> for AdditionalPermissionProfile {
    fn from(value: CorePermissionProfile) -> Self {
        Self {
            network: value.network.map(AdditionalNetworkPermissions::from),
            file_system: value.file_system.map(AdditionalFileSystemPermissions::from),
        }
    }
}

impl From<AdditionalPermissionProfile> for CorePermissionProfile {
    fn from(value: AdditionalPermissionProfile) -> Self {
        Self {
            network: value.network.map(CoreNetworkPermissions::from),
            file_system: value.file_system.map(CoreFileSystemPermissions::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GrantedPermissionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network: Option<AdditionalNetworkPermissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file_system: Option<AdditionalFileSystemPermissions>,
}

impl From<GrantedPermissionProfile> for CorePermissionProfile {
    fn from(value: GrantedPermissionProfile) -> Self {
        Self {
            network: value.network.map(CoreNetworkPermissions::from),
            file_system: value.file_system.map(CoreFileSystemPermissions::from),
        }
    }
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

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum ReadOnlyAccess {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Restricted {
        #[serde(default = "default_include_platform_defaults")]
        include_platform_defaults: bool,
        #[serde(default)]
        readable_roots: Vec<AbsolutePathBuf>,
    },
    #[default]
    FullAccess,
}

impl ReadOnlyAccess {
    pub fn to_core(&self) -> CoreReadOnlyAccess {
        match self {
            ReadOnlyAccess::Restricted {
                include_platform_defaults,
                readable_roots,
            } => CoreReadOnlyAccess::Restricted {
                include_platform_defaults: *include_platform_defaults,
                readable_roots: readable_roots.clone(),
            },
            ReadOnlyAccess::FullAccess => CoreReadOnlyAccess::FullAccess,
        }
    }
}

impl From<CoreReadOnlyAccess> for ReadOnlyAccess {
    fn from(value: CoreReadOnlyAccess) -> Self {
        match value {
            CoreReadOnlyAccess::Restricted {
                include_platform_defaults,
                readable_roots,
            } => ReadOnlyAccess::Restricted {
                include_platform_defaults,
                readable_roots,
            },
            CoreReadOnlyAccess::FullAccess => ReadOnlyAccess::FullAccess,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum SandboxPolicy {
    DangerFullAccess,
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ReadOnly {
        #[serde(default)]
        access: ReadOnlyAccess,
        #[serde(default)]
        network_access: bool,
    },
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
        read_only_access: ReadOnlyAccess,
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
            SandboxPolicy::ReadOnly {
                access,
                network_access,
            } => codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access: access.to_core(),
                network_access: *network_access,
            },
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
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: writable_roots.clone(),
                read_only_access: read_only_access.to_core(),
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
            codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access,
                network_access,
            } => SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::from(access),
                network_access,
            },
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
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access: ReadOnlyAccess::from(read_only_access),
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

v2_enum_from_core!(
    pub enum NetworkPolicyRuleAction from CoreNetworkPolicyRuleAction {
        Allow, Deny
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct NetworkPolicyAmendment {
    pub host: String,
    pub action: NetworkPolicyRuleAction,
}

impl NetworkPolicyAmendment {
    pub fn into_core(self) -> CoreNetworkPolicyAmendment {
        CoreNetworkPolicyAmendment {
            host: self.host,
            action: self.action.to_core(),
        }
    }
}

impl From<CoreNetworkPolicyAmendment> for NetworkPolicyAmendment {
    fn from(value: CoreNetworkPolicyAmendment) -> Self {
        Self {
            host: value.host,
            action: NetworkPolicyRuleAction::from(value.action),
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
        path: AbsolutePathBuf,
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
    Custom(String),
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
            CoreSessionSource::Custom(source) => SessionSource::Custom(source),
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
            SessionSource::Custom(source) => CoreSessionSource::Custom(source),
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
            } => CoreParsedCommand::Read {
                cmd,
                name,
                path: path.into_path_buf(),
            },
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

impl CommandAction {
    pub fn from_core_with_cwd(value: CoreParsedCommand, cwd: &AbsolutePathBuf) -> Self {
        match value {
            CoreParsedCommand::Read { cmd, name, path } => CommandAction::Read {
                command: cmd,
                name,
                path: cwd.join(path),
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
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
    #[serde(rename = "chatgptDeviceCode")]
    #[ts(rename = "chatgptDeviceCode")]
    ChatgptDeviceCode,
    /// [UNSTABLE] FOR OPENAI INTERNAL USE ONLY - DO NOT USE.
    /// The access token must contain the same scopes that Codex-managed ChatGPT auth tokens have.
    #[experimental("account/login/start.chatgptAuthTokens")]
    #[serde(rename = "chatgptAuthTokens", rename_all = "camelCase")]
    #[ts(rename = "chatgptAuthTokens", rename_all = "camelCase")]
    ChatgptAuthTokens {
        /// Access token (JWT) supplied by the client.
        /// This token is used for backend API requests and email extraction.
        access_token: String,
        /// Workspace/account identifier supplied by the client.
        chatgpt_account_id: String,
        /// Optional plan type supplied by the client.
        ///
        /// When `null`, Codex attempts to derive the plan type from access-token
        /// claims. If unavailable, the plan defaults to `unknown`.
        #[ts(optional = nullable)]
        chatgpt_plan_type: Option<String>,
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
    #[serde(rename = "chatgptDeviceCode", rename_all = "camelCase")]
    #[ts(rename = "chatgptDeviceCode", rename_all = "camelCase")]
    ChatgptDeviceCode {
        // Use plain String for identifiers to avoid TS/JSON Schema quirks around uuid-specific types.
        // Convert to/from UUIDs at the application layer as needed.
        login_id: String,
        /// URL the client should open in a browser to complete device code authorization.
        verification_url: String,
        /// One-time code the user must enter after signing in.
        user_code: String,
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
    /// This may be `null` when the prior auth state did not include a workspace
    /// identifier (`chatgpt_account_id`).
    #[ts(optional = nullable)]
    pub previous_account_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ChatgptAuthTokensRefreshResponse {
    pub access_token: String,
    pub chatgpt_account_id: String,
    pub chatgpt_plan_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GetAccountRateLimitsResponse {
    /// Backward-compatible single-bucket view; mirrors the historical payload.
    pub rate_limits: RateLimitSnapshot,
    /// Multi-bucket view keyed by metered `limit_id` (for example, `codex`).
    pub rate_limits_by_limit_id: Option<HashMap<String, RateLimitSnapshot>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SendAddCreditsNudgeEmailParams {
    pub credit_type: AddCreditsNudgeCreditType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/", rename_all = "snake_case")]
pub enum AddCreditsNudgeCreditType {
    Credits,
    UsageLimit,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SendAddCreditsNudgeEmailResponse {
    pub status: AddCreditsNudgeEmailStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/", rename_all = "snake_case")]
pub enum AddCreditsNudgeEmailStatus {
    Sent,
    CooldownActive,
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
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// When true, include models that are hidden from the default picker list.
    #[ts(optional = nullable)]
    pub include_hidden: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelAvailabilityNux {
    pub message: String,
}

impl From<CoreModelAvailabilityNux> for ModelAvailabilityNux {
    fn from(value: CoreModelAvailabilityNux) -> Self {
        Self {
            message: value.message,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Model {
    pub id: String,
    pub model: String,
    pub upgrade: Option<String>,
    pub upgrade_info: Option<ModelUpgradeInfo>,
    pub availability_nux: Option<ModelAvailabilityNux>,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    pub default_reasoning_effort: ReasoningEffort,
    #[serde(default = "default_input_modalities")]
    pub input_modalities: Vec<InputModality>,
    #[serde(default)]
    pub supports_personality: bool,
    #[serde(default)]
    pub additional_speed_tiers: Vec<String>,
    // Only one model should be marked as default.
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelUpgradeInfo {
    pub model: String,
    pub upgrade_copy: Option<String>,
    pub model_link: Option<String>,
    pub migration_markdown: Option<String>,
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

/// EXPERIMENTAL - collaboration mode preset metadata for clients.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CollaborationModeMask {
    pub name: String,
    pub mode: Option<ModeKind>,
    pub model: Option<String>,
    #[serde(rename = "reasoning_effort")]
    #[ts(rename = "reasoning_effort")]
    pub reasoning_effort: Option<Option<ReasoningEffort>>,
}

impl From<CoreCollaborationModeMask> for CollaborationModeMask {
    fn from(value: CoreCollaborationModeMask) -> Self {
        Self {
            name: value.name,
            mode: value.mode,
            model: value.model,
            reasoning_effort: value.reasoning_effort,
        }
    }
}

/// EXPERIMENTAL - collaboration mode presets response.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CollaborationModeListResponse {
    pub data: Vec<CollaborationModeMask>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ExperimentalFeatureStage {
    /// Feature is available for user testing and feedback.
    Beta,
    /// Feature is still being built and not ready for broad use.
    UnderDevelopment,
    /// Feature is production-ready.
    Stable,
    /// Feature is deprecated and should be avoided.
    Deprecated,
    /// Feature flag is retained only for backwards compatibility.
    Removed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeature {
    /// Stable key used in config.toml and CLI flag toggles.
    pub name: String,
    /// Lifecycle stage of this feature flag.
    pub stage: ExperimentalFeatureStage,
    /// User-facing display name shown in the experimental features UI.
    /// Null when this feature is not in beta.
    pub display_name: Option<String>,
    /// Short summary describing what the feature does.
    /// Null when this feature is not in beta.
    pub description: Option<String>,
    /// Announcement copy shown to users when the feature is introduced.
    /// Null when this feature is not in beta.
    pub announcement: Option<String>,
    /// Whether this feature is currently enabled in the loaded config.
    pub enabled: bool,
    /// Whether this feature is enabled by default.
    pub default_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureListResponse {
    pub data: Vec<ExperimentalFeature>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureEnablementSetParams {
    /// Process-wide runtime feature enablement keyed by canonical feature name.
    ///
    /// Only named features are updated. Omitted features are left unchanged.
    /// Send an empty map for a no-op.
    pub enablement: std::collections::BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureEnablementSetResponse {
    /// Feature enablement entries updated by this request.
    pub enablement: std::collections::BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ListMcpServerStatusParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a server-defined value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Controls how much MCP inventory data to fetch for each server.
    /// Defaults to `Full` when omitted.
    #[ts(optional = nullable)]
    pub detail: Option<McpServerStatusDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum McpServerStatusDetail {
    Full,
    ToolsAndAuthOnly,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpResourceReadParams {
    pub thread_id: String,
    pub server: String,
    pub uri: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpResourceReadResponse {
    pub contents: Vec<McpResourceContent>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerToolCallParams {
    pub thread_id: String,
    pub server: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub arguments: Option<JsonValue>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<JsonValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerToolCallResponse {
    pub content: Vec<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub structured_content: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub is_error: Option<bool>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<JsonValue>,
}

impl From<CoreMcpCallToolResult> for McpServerToolCallResponse {
    fn from(result: CoreMcpCallToolResult) -> Self {
        Self {
            content: result.content,
            structured_content: result.structured_content,
            is_error: result.is_error,
            meta: result.meta,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - list available apps/connectors.
pub struct AppsListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Optional thread id used to evaluate app feature gating from that thread's config.
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
    /// When true, bypass app caches and fetch the latest data from sources.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_refetch: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app metadata returned by app-list APIs.
pub struct AppBranding {
    pub category: Option<String>,
    pub developer: Option<String>,
    pub website: Option<String>,
    pub privacy_policy: Option<String>,
    pub terms_of_service: Option<String>,
    pub is_discoverable_app: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppReview {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppScreenshot {
    pub url: Option<String>,
    #[serde(alias = "file_id")]
    pub file_id: Option<String>,
    #[serde(alias = "user_prompt")]
    pub user_prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AppMetadata {
    pub review: Option<AppReview>,
    pub categories: Option<Vec<String>>,
    pub sub_categories: Option<Vec<String>>,
    pub seo_description: Option<String>,
    pub screenshots: Option<Vec<AppScreenshot>>,
    pub developer: Option<String>,
    pub version: Option<String>,
    pub version_id: Option<String>,
    pub version_notes: Option<String>,
    pub first_party_type: Option<String>,
    pub first_party_requires_install: Option<bool>,
    pub show_in_composer_when_unlinked: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app metadata returned by app-list APIs.
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub distribution_channel: Option<String>,
    pub branding: Option<AppBranding>,
    pub app_metadata: Option<AppMetadata>,
    pub labels: Option<HashMap<String, String>>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
    /// Whether this app is enabled in config.toml.
    /// Example:
    /// ```toml
    /// [apps.bad_app]
    /// enabled = false
    /// ```
    #[serde(default = "default_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app metadata summary for plugin responses.
pub struct AppSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub install_url: Option<String>,
    pub needs_auth: bool,
}

impl From<AppInfo> for AppSummary {
    fn from(value: AppInfo) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            install_url: value.install_url,
            needs_auth: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - app list response.
pub struct AppsListResponse {
    pub data: Vec<AppInfo>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - notification emitted when the app list changes.
pub struct AppListUpdatedNotification {
    pub data: Vec<AppInfo>,
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
    #[ts(optional = nullable)]
    pub scopes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
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
    #[ts(optional = nullable)]
    pub reason: Option<String>,
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
    pub include_logs: bool,
    #[ts(optional = nullable)]
    pub extra_log_files: Option<Vec<PathBuf>>,
    #[ts(optional = nullable)]
    pub tags: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FeedbackUploadResponse {
    pub thread_id: String,
}

/// Read a file from the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileParams {
    /// Absolute path to read.
    pub path: AbsolutePathBuf,
}

/// Base64-encoded file contents returned by `fs/readFile`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileResponse {
    /// File contents encoded as base64.
    pub data_base64: String,
}

/// Write a file on the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileParams {
    /// Absolute path to write.
    pub path: AbsolutePathBuf,
    /// File contents encoded as base64.
    pub data_base64: String,
}

/// Successful response for `fs/writeFile`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileResponse {}

/// Create a directory on the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCreateDirectoryParams {
    /// Absolute directory path to create.
    pub path: AbsolutePathBuf,
    /// Whether parent directories should also be created. Defaults to `true`.
    #[ts(optional = nullable)]
    pub recursive: Option<bool>,
}

/// Successful response for `fs/createDirectory`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCreateDirectoryResponse {}

/// Request metadata for an absolute path.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsGetMetadataParams {
    /// Absolute path to inspect.
    pub path: AbsolutePathBuf,
}

/// Metadata returned by `fs/getMetadata`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsGetMetadataResponse {
    /// Whether the path resolves to a directory.
    pub is_directory: bool,
    /// Whether the path resolves to a regular file.
    pub is_file: bool,
    /// Whether the path itself is a symbolic link.
    pub is_symlink: bool,
    /// File creation time in Unix milliseconds when available, otherwise `0`.
    #[ts(type = "number")]
    pub created_at_ms: i64,
    /// File modification time in Unix milliseconds when available, otherwise `0`.
    #[ts(type = "number")]
    pub modified_at_ms: i64,
}

/// List direct child names for a directory.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadDirectoryParams {
    /// Absolute directory path to read.
    pub path: AbsolutePathBuf,
}

/// A directory entry returned by `fs/readDirectory`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadDirectoryEntry {
    /// Direct child entry name only, not an absolute or relative path.
    pub file_name: String,
    /// Whether this entry resolves to a directory.
    pub is_directory: bool,
    /// Whether this entry resolves to a regular file.
    pub is_file: bool,
}

/// Directory entries returned by `fs/readDirectory`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadDirectoryResponse {
    /// Direct child entries in the requested directory.
    pub entries: Vec<FsReadDirectoryEntry>,
}

/// Remove a file or directory tree from the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsRemoveParams {
    /// Absolute path to remove.
    pub path: AbsolutePathBuf,
    /// Whether directory removal should recurse. Defaults to `true`.
    #[ts(optional = nullable)]
    pub recursive: Option<bool>,
    /// Whether missing paths should be ignored. Defaults to `true`.
    #[ts(optional = nullable)]
    pub force: Option<bool>,
}

/// Successful response for `fs/remove`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsRemoveResponse {}

/// Copy a file or directory tree on the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCopyParams {
    /// Absolute source path.
    pub source_path: AbsolutePathBuf,
    /// Absolute destination path.
    pub destination_path: AbsolutePathBuf,
    /// Required for directory copies; ignored for file copies.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recursive: bool,
}

/// Successful response for `fs/copy`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCopyResponse {}

/// Start filesystem watch notifications for an absolute path.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWatchParams {
    /// Connection-scoped watch identifier used for `fs/unwatch` and `fs/changed`.
    pub watch_id: String,
    /// Absolute file or directory path to watch.
    pub path: AbsolutePathBuf,
}

/// Successful response for `fs/watch`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWatchResponse {
    /// Canonicalized path associated with the watch.
    pub path: AbsolutePathBuf,
}

/// Stop filesystem watch notifications for a prior `fs/watch`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsUnwatchParams {
    /// Watch identifier previously provided to `fs/watch`.
    pub watch_id: String,
}

/// Successful response for `fs/unwatch`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsUnwatchResponse {}

/// Filesystem watch notification emitted for `fs/watch` subscribers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsChangedNotification {
    /// Watch identifier previously provided to `fs/watch`.
    pub watch_id: String,
    /// File or directory paths associated with this event.
    pub changed_paths: Vec<AbsolutePathBuf>,
}

/// PTY size in character cells for `command/exec` PTY sessions.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecTerminalSize {
    /// Terminal height in character cells.
    pub rows: u16,
    /// Terminal width in character cells.
    pub cols: u16,
}

/// Run a standalone command (argv vector) in the server sandbox without
/// creating a thread or turn.
///
/// The final `command/exec` response is deferred until the process exits and is
/// sent only after all `command/exec/outputDelta` notifications for that
/// connection have been emitted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecParams {
    /// Command argv vector. Empty arrays are rejected.
    pub command: Vec<String>,
    /// Optional client-supplied, connection-scoped process id.
    ///
    /// Required for `tty`, `streamStdin`, `streamStdoutStderr`, and follow-up
    /// `command/exec/write`, `command/exec/resize`, and
    /// `command/exec/terminate` calls. When omitted, buffered execution gets an
    /// internal id that is not exposed to the client.
    #[ts(optional = nullable)]
    pub process_id: Option<String>,
    /// Enable PTY mode.
    ///
    /// This implies `streamStdin` and `streamStdoutStderr`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tty: bool,
    /// Allow follow-up `command/exec/write` requests to write stdin bytes.
    ///
    /// Requires a client-supplied `processId`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream_stdin: bool,
    /// Stream stdout/stderr via `command/exec/outputDelta` notifications.
    ///
    /// Streamed bytes are not duplicated into the final response and require a
    /// client-supplied `processId`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream_stdout_stderr: bool,
    /// Optional per-stream stdout/stderr capture cap in bytes.
    ///
    /// When omitted, the server default applies. Cannot be combined with
    /// `disableOutputCap`.
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub output_bytes_cap: Option<usize>,
    /// Disable stdout/stderr capture truncation for this request.
    ///
    /// Cannot be combined with `outputBytesCap`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_output_cap: bool,
    /// Disable the timeout entirely for this request.
    ///
    /// Cannot be combined with `timeoutMs`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_timeout: bool,
    /// Optional timeout in milliseconds.
    ///
    /// When omitted, the server default applies. Cannot be combined with
    /// `disableTimeout`.
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub timeout_ms: Option<i64>,
    /// Optional working directory. Defaults to the server cwd.
    #[ts(optional = nullable)]
    pub cwd: Option<PathBuf>,
    /// Optional environment overrides merged into the server-computed
    /// environment.
    ///
    /// Matching names override inherited values. Set a key to `null` to unset
    /// an inherited variable.
    #[ts(optional = nullable)]
    pub env: Option<HashMap<String, Option<String>>>,
    /// Optional initial PTY size in character cells. Only valid when `tty` is
    /// true.
    #[ts(optional = nullable)]
    pub size: Option<CommandExecTerminalSize>,
    /// Optional sandbox policy for this command.
    ///
    /// Uses the same shape as thread/turn execution sandbox configuration and
    /// defaults to the user's configured policy when omitted.
    #[ts(optional = nullable)]
    pub sandbox_policy: Option<SandboxPolicy>,
}

/// Final buffered result for `command/exec`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecResponse {
    /// Process exit code.
    pub exit_code: i32,
    /// Buffered stdout capture.
    ///
    /// Empty when stdout was streamed via `command/exec/outputDelta`.
    pub stdout: String,
    /// Buffered stderr capture.
    ///
    /// Empty when stderr was streamed via `command/exec/outputDelta`.
    pub stderr: String,
}

/// Write stdin bytes to a running `command/exec` session, close stdin, or
/// both.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecWriteParams {
    /// Client-supplied, connection-scoped `processId` from the original
    /// `command/exec` request.
    pub process_id: String,
    /// Optional base64-encoded stdin bytes to write.
    #[ts(optional = nullable)]
    pub delta_base64: Option<String>,
    /// Close stdin after writing `deltaBase64`, if present.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub close_stdin: bool,
}

/// Empty success response for `command/exec/write`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecWriteResponse {}

/// Terminate a running `command/exec` session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecTerminateParams {
    /// Client-supplied, connection-scoped `processId` from the original
    /// `command/exec` request.
    pub process_id: String,
}

/// Empty success response for `command/exec/terminate`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecTerminateResponse {}

/// Resize a running PTY-backed `command/exec` session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecResizeParams {
    /// Client-supplied, connection-scoped `processId` from the original
    /// `command/exec` request.
    pub process_id: String,
    /// New PTY size in character cells.
    pub size: CommandExecTerminalSize,
}

/// Empty success response for `command/exec/resize`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecResizeResponse {}

/// Stream label for `command/exec/outputDelta` notifications.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CommandExecOutputStream {
    /// stdout stream. PTY mode multiplexes terminal output here.
    Stdout,
    /// stderr stream.
    Stderr,
}

// === Threads, Turns, and Items ===
// Thread APIs
#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS, ExperimentalApi,
)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadStartParams {
    #[ts(optional = nullable)]
    pub model: Option<String>,
    #[ts(optional = nullable)]
    pub model_provider: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::serde_helpers::deserialize_double_option",
        serialize_with = "super::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub service_tier: Option<Option<ServiceTier>>,
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    #[experimental(nested)]
    #[ts(optional = nullable)]
    pub approval_policy: Option<AskForApproval>,
    /// Override where approval requests are routed for review on this thread
    /// and subsequent turns.
    #[ts(optional = nullable)]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[ts(optional = nullable)]
    pub sandbox: Option<SandboxMode>,
    #[ts(optional = nullable)]
    pub config: Option<HashMap<String, JsonValue>>,
    #[ts(optional = nullable)]
    pub service_name: Option<String>,
    #[ts(optional = nullable)]
    pub base_instructions: Option<String>,
    #[ts(optional = nullable)]
    pub developer_instructions: Option<String>,
    #[ts(optional = nullable)]
    pub personality: Option<Personality>,
    #[ts(optional = nullable)]
    pub ephemeral: Option<bool>,
    #[ts(optional = nullable)]
    pub session_start_source: Option<ThreadStartSource>,
    #[experimental("thread/start.dynamicTools")]
    #[ts(optional = nullable)]
    pub dynamic_tools: Option<Vec<DynamicToolSpec>>,
    /// Test-only experimental field used to validate experimental gating and
    /// schema filtering behavior in a stable way.
    #[experimental("thread/start.mockExperimentalField")]
    #[ts(optional = nullable)]
    pub mock_experimental_field: Option<String>,
    /// If true, opt into emitting raw Responses API items on the event stream.
    /// This is for internal use only (e.g. Codex Cloud).
    #[experimental("thread/start.experimentalRawEvents")]
    #[serde(default)]
    pub experimental_raw_events: bool,
    /// If true, persist additional rollout EventMsg variants required to
    /// reconstruct a richer thread history on resume/fork/read.
    #[experimental("thread/start.persistFullHistory")]
    #[serde(default)]
    pub persist_extended_history: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MockExperimentalMethodParams {
    /// Test-only payload field.
    #[ts(optional = nullable)]
    pub value: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MockExperimentalMethodResponse {
    /// Echoes the input `value`.
    pub echoed: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadStartResponse {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub service_tier: Option<ServiceTier>,
    pub cwd: AbsolutePathBuf,
    /// Instruction source files currently loaded for this thread.
    #[serde(default)]
    pub instruction_sources: Vec<AbsolutePathBuf>,
    #[experimental(nested)]
    pub approval_policy: AskForApproval,
    /// Reviewer currently used for approval requests on this thread.
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(
    Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS, ExperimentalApi,
)]
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
    #[experimental("thread/resume.history")]
    #[ts(optional = nullable)]
    pub history: Option<Vec<ResponseItem>>,

    /// [UNSTABLE] Specify the rollout path to resume from.
    /// If specified, the thread_id param will be ignored.
    #[experimental("thread/resume.path")]
    #[ts(optional = nullable)]
    pub path: Option<PathBuf>,

    /// Configuration overrides for the resumed thread, if any.
    #[ts(optional = nullable)]
    pub model: Option<String>,
    #[ts(optional = nullable)]
    pub model_provider: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::serde_helpers::deserialize_double_option",
        serialize_with = "super::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub service_tier: Option<Option<ServiceTier>>,
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    #[experimental(nested)]
    #[ts(optional = nullable)]
    pub approval_policy: Option<AskForApproval>,
    /// Override where approval requests are routed for review on this thread
    /// and subsequent turns.
    #[ts(optional = nullable)]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[ts(optional = nullable)]
    pub sandbox: Option<SandboxMode>,
    #[ts(optional = nullable)]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[ts(optional = nullable)]
    pub base_instructions: Option<String>,
    #[ts(optional = nullable)]
    pub developer_instructions: Option<String>,
    #[ts(optional = nullable)]
    pub personality: Option<Personality>,
    /// If true, persist additional rollout EventMsg variants required to
    /// reconstruct a richer thread history on subsequent resume/fork/read.
    #[experimental("thread/resume.persistFullHistory")]
    #[serde(default)]
    pub persist_extended_history: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadResumeResponse {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub service_tier: Option<ServiceTier>,
    pub cwd: AbsolutePathBuf,
    /// Instruction source files currently loaded for this thread.
    #[serde(default)]
    pub instruction_sources: Vec<AbsolutePathBuf>,
    #[experimental(nested)]
    pub approval_policy: AskForApproval,
    /// Reviewer currently used for approval requests on this thread.
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(
    Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS, ExperimentalApi,
)]
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
    #[experimental("thread/fork.path")]
    #[ts(optional = nullable)]
    pub path: Option<PathBuf>,

    /// Configuration overrides for the forked thread, if any.
    #[ts(optional = nullable)]
    pub model: Option<String>,
    #[ts(optional = nullable)]
    pub model_provider: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::serde_helpers::deserialize_double_option",
        serialize_with = "super::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub service_tier: Option<Option<ServiceTier>>,
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    #[experimental(nested)]
    #[ts(optional = nullable)]
    pub approval_policy: Option<AskForApproval>,
    /// Override where approval requests are routed for review on this thread
    /// and subsequent turns.
    #[ts(optional = nullable)]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[ts(optional = nullable)]
    pub sandbox: Option<SandboxMode>,
    #[ts(optional = nullable)]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[ts(optional = nullable)]
    pub base_instructions: Option<String>,
    #[ts(optional = nullable)]
    pub developer_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ephemeral: bool,
    /// If true, persist additional rollout EventMsg variants required to
    /// reconstruct a richer thread history on subsequent resume/fork/read.
    #[experimental("thread/fork.persistFullHistory")]
    #[serde(default)]
    pub persist_extended_history: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadForkResponse {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub service_tier: Option<ServiceTier>,
    pub cwd: AbsolutePathBuf,
    /// Instruction source files currently loaded for this thread.
    #[serde(default)]
    pub instruction_sources: Vec<AbsolutePathBuf>,
    #[experimental(nested)]
    pub approval_policy: AskForApproval,
    /// Reviewer currently used for approval requests on this thread.
    pub approvals_reviewer: ApprovalsReviewer,
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
pub struct ThreadUnsubscribeParams {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUnsubscribeResponse {
    pub status: ThreadUnsubscribeStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ThreadUnsubscribeStatus {
    NotLoaded,
    NotSubscribed,
    Unsubscribed,
}

/// Parameters for `thread/increment_elicitation`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadIncrementElicitationParams {
    /// Thread whose out-of-band elicitation counter should be incremented.
    pub thread_id: String,
}

/// Response for `thread/increment_elicitation`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadIncrementElicitationResponse {
    /// Current out-of-band elicitation count after the increment.
    pub count: u64,
    /// Whether timeout accounting is paused after applying the increment.
    pub paused: bool,
}

/// Parameters for `thread/decrement_elicitation`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadDecrementElicitationParams {
    /// Thread whose out-of-band elicitation counter should be decremented.
    pub thread_id: String,
}

/// Response for `thread/decrement_elicitation`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadDecrementElicitationResponse {
    /// Current out-of-band elicitation count after the decrement.
    pub count: u64,
    /// Whether timeout accounting remains paused after applying the decrement.
    pub paused: bool,
}

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
pub struct ThreadMetadataUpdateParams {
    pub thread_id: String,
    /// Patch the stored Git metadata for this thread.
    /// Omit a field to leave it unchanged, set it to `null` to clear it, or
    /// provide a string to replace the stored value.
    #[ts(optional = nullable)]
    pub git_info: Option<ThreadMetadataGitInfoUpdateParams>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadMetadataGitInfoUpdateParams {
    /// Omit to leave the stored commit unchanged, set to `null` to clear it,
    /// or provide a non-empty string to replace it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::serde_helpers::serialize_double_option",
        deserialize_with = "super::serde_helpers::deserialize_double_option"
    )]
    #[ts(optional = nullable, type = "string | null")]
    pub sha: Option<Option<String>>,
    /// Omit to leave the stored branch unchanged, set to `null` to clear it,
    /// or provide a non-empty string to replace it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::serde_helpers::serialize_double_option",
        deserialize_with = "super::serde_helpers::deserialize_double_option"
    )]
    #[ts(optional = nullable, type = "string | null")]
    pub branch: Option<Option<String>>,
    /// Omit to leave the stored origin URL unchanged, set to `null` to clear it,
    /// or provide a non-empty string to replace it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::serde_helpers::serialize_double_option",
        deserialize_with = "super::serde_helpers::deserialize_double_option"
    )]
    #[ts(optional = nullable, type = "string | null")]
    pub origin_url: Option<Option<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadMetadataUpdateResponse {
    pub thread: Thread,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ThreadMemoryMode {
    Enabled,
    Disabled,
}

impl ThreadMemoryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    pub fn to_core(self) -> codex_protocol::protocol::ThreadMemoryMode {
        match self {
            Self::Enabled => codex_protocol::protocol::ThreadMemoryMode::Enabled,
            Self::Disabled => codex_protocol::protocol::ThreadMemoryMode::Disabled,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadMemoryModeSetParams {
    pub thread_id: String,
    pub mode: ThreadMemoryMode,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadMemoryModeSetResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MemoryResetResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUnarchiveResponse {
    pub thread: Thread,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadCompactStartParams {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadCompactStartResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadShellCommandParams {
    pub thread_id: String,
    /// Shell command string evaluated by the thread's configured shell.
    /// Unlike `command/exec`, this intentionally preserves shell syntax
    /// such as pipes, redirects, and quoting. This runs unsandboxed with full
    /// access rather than inheriting the thread sandbox policy.
    pub command: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadShellCommandResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadBackgroundTerminalsCleanParams {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadBackgroundTerminalsCleanResponse {}

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
pub struct ThreadListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Optional sort key; defaults to created_at.
    #[ts(optional = nullable)]
    pub sort_key: Option<ThreadSortKey>,
    /// Optional sort direction; defaults to descending (newest first).
    #[ts(optional = nullable)]
    pub sort_direction: Option<SortDirection>,
    /// Optional provider filter; when set, only sessions recorded under these
    /// providers are returned. When present but empty, includes all providers.
    #[ts(optional = nullable)]
    pub model_providers: Option<Vec<String>>,
    /// Optional source filter; when set, only sessions from these source kinds
    /// are returned. When omitted or empty, defaults to interactive sources.
    #[ts(optional = nullable)]
    pub source_kinds: Option<Vec<ThreadSourceKind>>,
    /// Optional archived filter; when set to true, only archived threads are returned.
    /// If false or null, only non-archived threads are returned.
    #[ts(optional = nullable)]
    pub archived: Option<bool>,
    /// Optional cwd filter; when set, only threads whose session cwd exactly
    /// matches this path are returned.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    /// Optional substring filter for the extracted thread title.
    #[ts(optional = nullable)]
    pub search_term: Option<String>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadListResponse {
    pub data: Vec<Thread>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// if None, there are no more items to return.
    pub next_cursor: Option<String>,
    /// Opaque cursor to pass as `cursor` when reversing `sortDirection`.
    /// This is only populated when the page contains at least one thread.
    /// Use it with the opposite `sortDirection`; for timestamp sorts it anchors
    /// at the start of the page timestamp so same-second updates are not skipped.
    pub backwards_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadLoadedListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to no limit.
    #[ts(optional = nullable)]
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
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum ThreadStatus {
    NotLoaded,
    Idle,
    SystemError,
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Active {
        active_flags: Vec<ThreadActiveFlag>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ThreadActiveFlag {
    WaitingOnApproval,
    WaitingOnUserInput,
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
pub struct ThreadTurnsListParams {
    pub thread_id: String,
    /// Opaque cursor to pass to the next call to continue after the last turn.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional turn page size.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Optional turn pagination direction; defaults to descending.
    #[ts(optional = nullable)]
    pub sort_direction: Option<SortDirection>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadTurnsListResponse {
    pub data: Vec<Turn>,
    /// Opaque cursor to pass to the next call to continue after the last turn.
    /// if None, there are no more turns to return.
    pub next_cursor: Option<String>,
    /// Opaque cursor to pass as `cursor` when reversing `sortDirection`.
    /// This is only populated when the page contains at least one turn.
    /// Use it with the opposite `sortDirection` to include the anchor turn again
    /// and catch updates to that turn.
    pub backwards_cursor: Option<String>,
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

    /// Optional per-cwd extra roots to scan as user-scoped skills.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub per_cwd_extra_user_roots: Option<Vec<SkillsListExtraRootsForCwd>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsListExtraRootsForCwd {
    pub cwd: PathBuf,
    pub extra_user_roots: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsListResponse {
    pub data: Vec<SkillsListEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MarketplaceAddParams {
    pub source: String,
    #[ts(optional = nullable)]
    pub ref_name: Option<String>,
    #[ts(optional = nullable)]
    pub sparse_paths: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MarketplaceAddResponse {
    pub marketplace_name: String,
    pub installed_root: AbsolutePathBuf,
    pub already_added: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MarketplaceRemoveParams {
    pub marketplace_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MarketplaceRemoveResponse {
    pub marketplace_name: String,
    pub installed_root: Option<AbsolutePathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginListParams {
    /// Optional working directories used to discover repo marketplaces. When omitted,
    /// only home-scoped marketplaces and the official curated marketplace are considered.
    #[ts(optional = nullable)]
    pub cwds: Option<Vec<AbsolutePathBuf>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginListResponse {
    pub marketplaces: Vec<PluginMarketplaceEntry>,
    #[serde(default)]
    pub marketplace_load_errors: Vec<MarketplaceLoadErrorInfo>,
    #[serde(default)]
    pub featured_plugin_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MarketplaceLoadErrorInfo {
    pub marketplace_path: AbsolutePathBuf,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginReadParams {
    #[ts(optional = nullable)]
    pub marketplace_path: Option<AbsolutePathBuf>,
    #[ts(optional = nullable)]
    pub remote_marketplace_name: Option<String>,
    pub plugin_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginReadResponse {
    pub plugin: PluginDetail,
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
    pub path: AbsolutePathBuf,
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
    pub icon_small: Option<AbsolutePathBuf>,
    #[ts(optional)]
    pub icon_large: Option<AbsolutePathBuf>,
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
pub struct PluginMarketplaceEntry {
    pub name: String,
    /// Local marketplace file path when the marketplace is backed by a local file.
    /// Remote-only catalog marketplaces do not have a local path.
    pub path: Option<AbsolutePathBuf>,
    pub interface: Option<MarketplaceInterface>,
    pub plugins: Vec<PluginSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MarketplaceInterface {
    pub display_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[ts(export_to = "v2/")]
pub enum PluginInstallPolicy {
    #[serde(rename = "NOT_AVAILABLE")]
    #[ts(rename = "NOT_AVAILABLE")]
    NotAvailable,
    #[serde(rename = "AVAILABLE")]
    #[ts(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "INSTALLED_BY_DEFAULT")]
    #[ts(rename = "INSTALLED_BY_DEFAULT")]
    InstalledByDefault,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[ts(export_to = "v2/")]
pub enum PluginAuthPolicy {
    #[serde(rename = "ON_INSTALL")]
    #[ts(rename = "ON_INSTALL")]
    OnInstall,
    #[serde(rename = "ON_USE")]
    #[ts(rename = "ON_USE")]
    OnUse,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub source: PluginSource,
    pub installed: bool,
    pub enabled: bool,
    pub install_policy: PluginInstallPolicy,
    pub auth_policy: PluginAuthPolicy,
    pub interface: Option<PluginInterface>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginDetail {
    pub marketplace_name: String,
    pub marketplace_path: AbsolutePathBuf,
    pub summary: PluginSummary,
    pub description: Option<String>,
    pub skills: Vec<SkillSummary>,
    pub apps: Vec<AppSummary>,
    pub mcp_servers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub interface: Option<SkillInterface>,
    pub path: AbsolutePathBuf,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub developer_name: Option<String>,
    pub category: Option<String>,
    pub capabilities: Vec<String>,
    pub website_url: Option<String>,
    pub privacy_policy_url: Option<String>,
    pub terms_of_service_url: Option<String>,
    /// Starter prompts for the plugin. Capped at 3 entries with a maximum of
    /// 128 characters per entry.
    pub default_prompt: Option<Vec<String>>,
    pub brand_color: Option<String>,
    /// Local composer icon path, resolved from the installed plugin package.
    pub composer_icon: Option<AbsolutePathBuf>,
    /// Remote composer icon URL from the plugin catalog.
    pub composer_icon_url: Option<String>,
    /// Local logo path, resolved from the installed plugin package.
    pub logo: Option<AbsolutePathBuf>,
    /// Remote logo URL from the plugin catalog.
    pub logo_url: Option<String>,
    /// Local screenshot paths, resolved from the installed plugin package.
    pub screenshots: Vec<AbsolutePathBuf>,
    /// Remote screenshot URLs from the plugin catalog.
    pub screenshot_urls: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum PluginSource {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Local { path: AbsolutePathBuf },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Git {
        url: String,
        path: Option<String>,
        ref_name: Option<String>,
        sha: Option<String>,
    },
    /// The plugin is available in the remote catalog. Download metadata is
    /// kept server-side and is not exposed through the app-server API.
    Remote,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsConfigWriteParams {
    /// Path-based selector.
    #[ts(optional = nullable)]
    pub path: Option<AbsolutePathBuf>,
    /// Name-based selector.
    #[ts(optional = nullable)]
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SkillsConfigWriteResponse {
    pub effective_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginInstallParams {
    #[ts(optional = nullable)]
    pub marketplace_path: Option<AbsolutePathBuf>,
    #[ts(optional = nullable)]
    pub remote_marketplace_name: Option<String>,
    pub plugin_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginInstallResponse {
    pub auth_policy: PluginAuthPolicy,
    pub apps_needing_auth: Vec<AppSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginUninstallParams {
    pub plugin_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PluginUninstallResponse {}

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Thread {
    pub id: String,
    /// Source thread id when this thread was created by forking another thread.
    pub forked_from_id: Option<String>,
    /// Usually the first user message in the thread, if available.
    pub preview: String,
    /// Whether the thread is ephemeral and should not be materialized on disk.
    pub ephemeral: bool,
    /// Model provider used for this thread (for example, 'openai').
    pub model_provider: String,
    /// Unix timestamp (in seconds) when the thread was created.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Unix timestamp (in seconds) when the thread was last updated.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// Current runtime status for the thread.
    pub status: ThreadStatus,
    /// [UNSTABLE] Path to the thread on disk.
    pub path: Option<PathBuf>,
    /// Working directory captured for the thread.
    pub cwd: AbsolutePathBuf,
    /// Version of the CLI that created the thread.
    pub cli_version: String,
    /// Origin of the thread (CLI, VSCode, codex exec, codex app-server, etc.).
    pub source: SessionSource,
    /// Optional random unique nickname assigned to an AgentControl-spawned sub-agent.
    pub agent_nickname: Option<String>,
    /// Optional role (agent_role) assigned to an AgentControl-spawned sub-agent.
    pub agent_role: Option<String>,
    /// Optional Git metadata captured when the thread was created.
    pub git_info: Option<GitInfo>,
    /// Optional user-facing thread title.
    pub name: Option<String>,
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
    pub plan_type: Option<PlanType>,
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
    /// Unix timestamp (in seconds) when the turn started.
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    /// Unix timestamp (in seconds) when the turn completed.
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    /// Duration between turn start and completion in milliseconds, if known.
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MemoryCitation {
    pub entries: Vec<MemoryCitationEntry>,
    pub thread_ids: Vec<String>,
}

impl From<CoreMemoryCitation> for MemoryCitation {
    fn from(value: CoreMemoryCitation) -> Self {
        Self {
            entries: value.entries.into_iter().map(Into::into).collect(),
            thread_ids: value.rollout_ids,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct MemoryCitationEntry {
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub note: String,
}

impl From<CoreMemoryCitationEntry> for MemoryCitationEntry {
    fn from(value: CoreMemoryCitationEntry) -> Self {
        Self {
            path: value.path,
            line_start: value.line_start,
            line_end: value.line_end,
            note: value.note,
        }
    }
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

/// EXPERIMENTAL - thread realtime audio chunk.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAudioChunk {
    pub data: String,
    pub sample_rate: u32,
    pub num_channels: u16,
    pub samples_per_channel: Option<u32>,
    pub item_id: Option<String>,
}

impl From<CoreRealtimeAudioFrame> for ThreadRealtimeAudioChunk {
    fn from(value: CoreRealtimeAudioFrame) -> Self {
        let CoreRealtimeAudioFrame {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        } = value;
        Self {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        }
    }
}

impl From<ThreadRealtimeAudioChunk> for CoreRealtimeAudioFrame {
    fn from(value: ThreadRealtimeAudioChunk) -> Self {
        let ThreadRealtimeAudioChunk {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        } = value;
        Self {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        }
    }
}

/// EXPERIMENTAL - start a thread-scoped realtime session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStartParams {
    pub thread_id: String,
    /// Selects text or audio output for the realtime session. Transport and voice stay
    /// independent so clients can choose how they connect separately from what the model emits.
    pub output_modality: RealtimeOutputModality,
    #[serde(
        default,
        deserialize_with = "super::serde_helpers::deserialize_double_option",
        serialize_with = "super::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub prompt: Option<Option<String>>,
    #[ts(optional = nullable)]
    pub session_id: Option<String>,
    #[ts(optional = nullable)]
    pub transport: Option<ThreadRealtimeStartTransport>,
    #[ts(optional = nullable)]
    pub voice: Option<RealtimeVoice>,
}

/// EXPERIMENTAL - transport used by thread realtime.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "v2/", tag = "type")]
pub enum ThreadRealtimeStartTransport {
    Websocket,
    Webrtc {
        /// SDP offer generated by a WebRTC RTCPeerConnection after configuring audio and the
        /// realtime events data channel.
        sdp: String,
    },
}

/// EXPERIMENTAL - response for starting thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStartResponse {}

/// EXPERIMENTAL - append audio input to thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendAudioParams {
    pub thread_id: String,
    pub audio: ThreadRealtimeAudioChunk,
}

/// EXPERIMENTAL - response for appending realtime audio input.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendAudioResponse {}

/// EXPERIMENTAL - append text input to thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendTextParams {
    pub thread_id: String,
    pub text: String,
}

/// EXPERIMENTAL - response for appending realtime text input.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendTextResponse {}

/// EXPERIMENTAL - stop thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStopParams {
    pub thread_id: String,
}

/// EXPERIMENTAL - response for stopping thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStopResponse {}

/// EXPERIMENTAL - list voices supported by thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeListVoicesParams {}

/// EXPERIMENTAL - response for listing supported realtime voices.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeListVoicesResponse {
    pub voices: RealtimeVoicesList,
}

/// EXPERIMENTAL - emitted when thread realtime startup is accepted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStartedNotification {
    pub thread_id: String,
    pub session_id: Option<String>,
    pub version: RealtimeConversationVersion,
}

/// EXPERIMENTAL - raw non-audio thread realtime item emitted by the backend.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeItemAddedNotification {
    pub thread_id: String,
    pub item: JsonValue,
}

/// EXPERIMENTAL - flat transcript delta emitted whenever realtime
/// transcript text changes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeTranscriptDeltaNotification {
    pub thread_id: String,
    pub role: String,
    /// Live transcript delta from the realtime event.
    pub delta: String,
}

/// EXPERIMENTAL - final transcript text emitted when realtime completes
/// a transcript part.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeTranscriptDoneNotification {
    pub thread_id: String,
    pub role: String,
    /// Final complete text for the transcript part.
    pub text: String,
}

/// EXPERIMENTAL - streamed output audio emitted by thread realtime.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeOutputAudioDeltaNotification {
    pub thread_id: String,
    pub audio: ThreadRealtimeAudioChunk,
}

/// EXPERIMENTAL - emitted with the remote SDP for a WebRTC realtime session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeSdpNotification {
    pub thread_id: String,
    pub sdp: String,
}

/// EXPERIMENTAL - emitted when thread realtime encounters an error.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeErrorNotification {
    pub thread_id: String,
    pub message: String,
}

/// EXPERIMENTAL - emitted when thread realtime transport closes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeClosedNotification {
    pub thread_id: String,
    pub reason: Option<String>,
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
#[derive(
    Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS, ExperimentalApi,
)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    /// Optional turn-scoped Responses API client metadata.
    #[experimental("turn/start.responsesapiClientMetadata")]
    #[ts(optional = nullable)]
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
    /// Override the working directory for this turn and subsequent turns.
    #[ts(optional = nullable)]
    pub cwd: Option<PathBuf>,
    /// Override the approval policy for this turn and subsequent turns.
    #[experimental(nested)]
    #[ts(optional = nullable)]
    pub approval_policy: Option<AskForApproval>,
    /// Override where approval requests are routed for review on this turn and
    /// subsequent turns.
    #[ts(optional = nullable)]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    /// Override the sandbox policy for this turn and subsequent turns.
    #[ts(optional = nullable)]
    pub sandbox_policy: Option<SandboxPolicy>,
    /// Override the model for this turn and subsequent turns.
    #[ts(optional = nullable)]
    pub model: Option<String>,
    /// Override the service tier for this turn and subsequent turns.
    #[serde(
        default,
        deserialize_with = "super::serde_helpers::deserialize_double_option",
        serialize_with = "super::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub service_tier: Option<Option<ServiceTier>>,
    /// Override the reasoning effort for this turn and subsequent turns.
    #[ts(optional = nullable)]
    pub effort: Option<ReasoningEffort>,
    /// Override the reasoning summary for this turn and subsequent turns.
    #[ts(optional = nullable)]
    pub summary: Option<ReasoningSummary>,
    /// Override the personality for this turn and subsequent turns.
    #[ts(optional = nullable)]
    pub personality: Option<Personality>,
    /// Optional JSON Schema used to constrain the final assistant message for
    /// this turn.
    #[ts(optional = nullable)]
    pub output_schema: Option<JsonValue>,

    /// EXPERIMENTAL - Set a pre-set collaboration mode.
    /// Takes precedence over model, reasoning_effort, and developer instructions if set.
    ///
    /// For `collaboration_mode.settings.developer_instructions`, `null` means
    /// "use the built-in instructions for the selected mode".
    #[experimental("turn/start.collaborationMode")]
    #[ts(optional = nullable)]
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
    #[ts(optional = nullable)]
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
pub struct ThreadInjectItemsParams {
    pub thread_id: String,
    /// Raw Responses API items to append to the thread's model-visible history.
    pub items: Vec<JsonValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadInjectItemsResponse {}

#[derive(
    Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS, ExperimentalApi,
)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnSteerParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    /// Optional turn-scoped Responses API client metadata.
    #[experimental("turn/steer.responsesapiClientMetadata")]
    #[ts(optional = nullable)]
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
    /// Required active turn id precondition. The request fails when it does not
    /// match the currently active turn.
    pub expected_turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct TurnSteerResponse {
    pub turn_id: String,
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

impl UserInput {
    pub fn text_char_count(&self) -> usize {
        match self {
            UserInput::Text { text, .. } => text.chars().count(),
            UserInput::Image { .. }
            | UserInput::LocalImage { .. }
            | UserInput::Skill { .. }
            | UserInput::Mention { .. } => 0,
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
    HookPrompt {
        id: String,
        fragments: Vec<HookPromptFragment>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    AgentMessage {
        id: String,
        text: String,
        #[serde(default)]
        phase: Option<MessagePhase>,
        #[serde(default)]
        memory_citation: Option<MemoryCitation>,
    },
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
        cwd: AbsolutePathBuf,
        /// Identifier for the underlying PTY process (when available).
        process_id: Option<String>,
        #[serde(default)]
        source: CommandExecutionSource,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        mcp_app_resource_uri: Option<String>,
        result: Option<Box<McpToolCallResult>>,
        error: Option<McpToolCallError>,
        /// The duration of the MCP tool call in milliseconds.
        #[ts(type = "number | null")]
        duration_ms: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    DynamicToolCall {
        id: String,
        tool: String,
        arguments: JsonValue,
        status: DynamicToolCallStatus,
        content_items: Option<Vec<DynamicToolCallOutputContentItem>>,
        success: Option<bool>,
        /// The duration of the dynamic tool call in milliseconds.
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
        /// Model requested for the spawned agent, when applicable.
        model: Option<String>,
        /// Reasoning effort requested for the spawned agent, when applicable.
        reasoning_effort: Option<ReasoningEffort>,
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
    ImageView { id: String, path: AbsolutePathBuf },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ImageGeneration {
        id: String,
        status: String,
        revised_prompt: Option<String>,
        result: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        saved_path: Option<AbsolutePathBuf>,
    },
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct HookPromptFragment {
    pub text: String,
    pub hook_run_id: String,
}

impl ThreadItem {
    pub fn id(&self) -> &str {
        match self {
            ThreadItem::UserMessage { id, .. }
            | ThreadItem::HookPrompt { id, .. }
            | ThreadItem::AgentMessage { id, .. }
            | ThreadItem::Plan { id, .. }
            | ThreadItem::Reasoning { id, .. }
            | ThreadItem::CommandExecution { id, .. }
            | ThreadItem::FileChange { id, .. }
            | ThreadItem::McpToolCall { id, .. }
            | ThreadItem::DynamicToolCall { id, .. }
            | ThreadItem::CollabAgentToolCall { id, .. }
            | ThreadItem::WebSearch { id, .. }
            | ThreadItem::ImageView { id, .. }
            | ThreadItem::ImageGeneration { id, .. }
            | ThreadItem::EnteredReviewMode { id, .. }
            | ThreadItem::ExitedReviewMode { id, .. }
            | ThreadItem::ContextCompaction { id, .. } => id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// [UNSTABLE] Lifecycle state for an approval auto-review.
pub enum GuardianApprovalReviewStatus {
    InProgress,
    Approved,
    Denied,
    TimedOut,
    Aborted,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// [UNSTABLE] Source that produced a terminal approval auto-review decision.
pub enum AutoReviewDecisionSource {
    Agent,
}

impl From<CoreGuardianAssessmentDecisionSource> for AutoReviewDecisionSource {
    fn from(value: CoreGuardianAssessmentDecisionSource) -> Self {
        match value {
            CoreGuardianAssessmentDecisionSource::Agent => Self::Agent,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
/// [UNSTABLE] Risk level assigned by approval auto-review.
pub enum GuardianRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl From<CoreGuardianRiskLevel> for GuardianRiskLevel {
    fn from(value: CoreGuardianRiskLevel) -> Self {
        match value {
            CoreGuardianRiskLevel::Low => Self::Low,
            CoreGuardianRiskLevel::Medium => Self::Medium,
            CoreGuardianRiskLevel::High => Self::High,
            CoreGuardianRiskLevel::Critical => Self::Critical,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
/// [UNSTABLE] Authorization level assigned by approval auto-review.
pub enum GuardianUserAuthorization {
    Unknown,
    Low,
    Medium,
    High,
}

impl From<CoreGuardianUserAuthorization> for GuardianUserAuthorization {
    fn from(value: CoreGuardianUserAuthorization) -> Self {
        match value {
            CoreGuardianUserAuthorization::Unknown => Self::Unknown,
            CoreGuardianUserAuthorization::Low => Self::Low,
            CoreGuardianUserAuthorization::Medium => Self::Medium,
            CoreGuardianUserAuthorization::High => Self::High,
        }
    }
}

/// [UNSTABLE] Temporary approval auto-review payload used by
/// `item/autoApprovalReview/*` notifications. This shape is expected to change
/// soon.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianApprovalReview {
    pub status: GuardianApprovalReviewStatus,
    pub risk_level: Option<GuardianRiskLevel>,
    pub user_authorization: Option<GuardianUserAuthorization>,
    pub rationale: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum GuardianCommandSource {
    Shell,
    UnifiedExec,
}

impl From<CoreGuardianCommandSource> for GuardianCommandSource {
    fn from(value: CoreGuardianCommandSource) -> Self {
        match value {
            CoreGuardianCommandSource::Shell => Self::Shell,
            CoreGuardianCommandSource::UnifiedExec => Self::UnifiedExec,
        }
    }
}

impl From<GuardianCommandSource> for CoreGuardianCommandSource {
    fn from(value: GuardianCommandSource) -> Self {
        match value {
            GuardianCommandSource::Shell => Self::Shell,
            GuardianCommandSource::UnifiedExec => Self::UnifiedExec,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianCommandReviewAction {
    pub source: GuardianCommandSource,
    pub command: String,
    pub cwd: AbsolutePathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianExecveReviewAction {
    pub source: GuardianCommandSource,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd: AbsolutePathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianApplyPatchReviewAction {
    pub cwd: AbsolutePathBuf,
    pub files: Vec<AbsolutePathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianNetworkAccessReviewAction {
    pub target: String,
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianMcpToolCallReviewAction {
    pub server: String,
    pub tool_name: String,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    pub tool_title: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum GuardianApprovalReviewAction {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Command {
        source: GuardianCommandSource,
        command: String,
        cwd: AbsolutePathBuf,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Execve {
        source: GuardianCommandSource,
        program: String,
        argv: Vec<String>,
        cwd: AbsolutePathBuf,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    ApplyPatch {
        cwd: AbsolutePathBuf,
        files: Vec<AbsolutePathBuf>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    NetworkAccess {
        target: String,
        host: String,
        protocol: NetworkApprovalProtocol,
        port: u16,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    McpToolCall {
        server: String,
        tool_name: String,
        connector_id: Option<String>,
        connector_name: Option<String>,
        tool_title: Option<String>,
    },
}

impl From<CoreGuardianAssessmentAction> for GuardianApprovalReviewAction {
    fn from(value: CoreGuardianAssessmentAction) -> Self {
        match value {
            CoreGuardianAssessmentAction::Command {
                source,
                command,
                cwd,
            } => Self::Command {
                source: source.into(),
                command,
                cwd,
            },
            CoreGuardianAssessmentAction::Execve {
                source,
                program,
                argv,
                cwd,
            } => Self::Execve {
                source: source.into(),
                program,
                argv,
                cwd,
            },
            CoreGuardianAssessmentAction::ApplyPatch { cwd, files } => {
                Self::ApplyPatch { cwd, files }
            }
            CoreGuardianAssessmentAction::NetworkAccess {
                target,
                host,
                protocol,
                port,
            } => Self::NetworkAccess {
                target,
                host,
                protocol: protocol.into(),
                port,
            },
            CoreGuardianAssessmentAction::McpToolCall {
                server,
                tool_name,
                connector_id,
                connector_name,
                tool_title,
            } => Self::McpToolCall {
                server,
                tool_name,
                connector_id,
                connector_name,
                tool_title,
            },
        }
    }
}

impl From<GuardianApprovalReviewAction> for CoreGuardianAssessmentAction {
    fn from(value: GuardianApprovalReviewAction) -> Self {
        match value {
            GuardianApprovalReviewAction::Command {
                source,
                command,
                cwd,
            } => Self::Command {
                source: source.into(),
                command,
                cwd,
            },
            GuardianApprovalReviewAction::Execve {
                source,
                program,
                argv,
                cwd,
            } => Self::Execve {
                source: source.into(),
                program,
                argv,
                cwd,
            },
            GuardianApprovalReviewAction::ApplyPatch { cwd, files } => {
                Self::ApplyPatch { cwd, files }
            }
            GuardianApprovalReviewAction::NetworkAccess {
                target,
                host,
                protocol,
                port,
            } => Self::NetworkAccess {
                target,
                host,
                protocol: protocol.to_core(),
                port,
            },
            GuardianApprovalReviewAction::McpToolCall {
                server,
                tool_name,
                connector_id,
                connector_name,
                tool_title,
            } => Self::McpToolCall {
                server,
                tool_name,
                connector_id,
                connector_name,
                tool_title,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "v2/")]
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
            CoreTurnItem::HookPrompt(hook_prompt) => ThreadItem::HookPrompt {
                id: hook_prompt.id,
                fragments: hook_prompt
                    .fragments
                    .into_iter()
                    .map(HookPromptFragment::from)
                    .collect(),
            },
            CoreTurnItem::AgentMessage(agent) => {
                let text = agent
                    .content
                    .into_iter()
                    .map(|entry| match entry {
                        CoreAgentMessageContent::Text { text } => text,
                    })
                    .collect::<String>();
                ThreadItem::AgentMessage {
                    id: agent.id,
                    text,
                    phase: agent.phase,
                    memory_citation: agent.memory_citation.map(Into::into),
                }
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
            CoreTurnItem::ImageGeneration(image) => ThreadItem::ImageGeneration {
                id: image.id,
                status: image.status,
                revised_prompt: image.revised_prompt,
                result: image.result,
                saved_path: image.saved_path,
            },
            CoreTurnItem::ContextCompaction(compaction) => {
                ThreadItem::ContextCompaction { id: compaction.id }
            }
        }
    }
}

impl From<codex_protocol::items::HookPromptFragment> for HookPromptFragment {
    fn from(value: codex_protocol::items::HookPromptFragment) -> Self {
        Self {
            text: value.text,
            hook_run_id: value.hook_run_id,
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

impl From<CoreExecCommandStatus> for CommandExecutionStatus {
    fn from(value: CoreExecCommandStatus) -> Self {
        Self::from(&value)
    }
}

impl From<&CoreExecCommandStatus> for CommandExecutionStatus {
    fn from(value: &CoreExecCommandStatus) -> Self {
        match value {
            CoreExecCommandStatus::Completed => CommandExecutionStatus::Completed,
            CoreExecCommandStatus::Failed => CommandExecutionStatus::Failed,
            CoreExecCommandStatus::Declined => CommandExecutionStatus::Declined,
        }
    }
}

v2_enum_from_core! {
    #[derive(Default)]
    pub enum CommandExecutionSource from CoreExecCommandSource {
        #[default]
        Agent,
        UserShell,
        UnifiedExecStartup,
        UnifiedExecInteraction,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum CollabAgentTool {
    SpawnAgent,
    SendInput,
    ResumeAgent,
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

impl From<CorePatchApplyStatus> for PatchApplyStatus {
    fn from(value: CorePatchApplyStatus) -> Self {
        Self::from(&value)
    }
}

impl From<&CorePatchApplyStatus> for PatchApplyStatus {
    fn from(value: &CorePatchApplyStatus) -> Self {
        match value {
            CorePatchApplyStatus::Completed => PatchApplyStatus::Completed,
            CorePatchApplyStatus::Failed => PatchApplyStatus::Failed,
            CorePatchApplyStatus::Declined => PatchApplyStatus::Declined,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum McpToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum DynamicToolCallStatus {
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
    Interrupted,
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
            CoreAgentStatus::Interrupted => Self {
                status: CollabAgentStatus::Interrupted,
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
    // NOTE: `rmcp::model::Content` (and its `RawContent` variants) would be a more precise Rust
    // representation of MCP content blocks. We intentionally use `serde_json::Value` here because
    // this crate exports JSON schema + TS types (`schemars`/`ts-rs`), and the rmcp model types
    // aren't set up to be schema/TS friendly (and would introduce heavier coupling to rmcp's Rust
    // representations). Using `JsonValue` keeps the payload wire-shaped and easy to export.
    pub content: Vec<JsonValue>,
    pub structured_content: Option<JsonValue>,
    #[serde(rename = "_meta")]
    #[ts(rename = "_meta")]
    pub meta: Option<JsonValue>,
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
pub struct ThreadStatusChangedNotification {
    pub thread_id: String,
    pub status: ThreadStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadArchivedNotification {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadUnarchivedNotification {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadClosedNotification {
    pub thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// Notification emitted when watched local skill files change.
///
/// Treat this as an invalidation signal and re-run `skills/list` with the
/// client's current parameters when refreshed skill metadata is needed.
pub struct SkillsChangedNotification {}

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
pub struct HookStartedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
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
pub struct HookCompletedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
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
/// [UNSTABLE] Temporary notification payload for approval auto-review. This
/// shape is expected to change soon.
pub struct ItemGuardianApprovalReviewStartedNotification {
    pub thread_id: String,
    pub turn_id: String,
    /// Stable identifier for this review.
    pub review_id: String,
    /// Identifier for the reviewed item or tool call when one exists.
    ///
    /// In most cases, one review maps to one target item. The exceptions are
    /// - execve reviews, where a single command may contain multiple execve
    ///   calls to review (only possible when using the shell_zsh_fork feature)
    /// - network policy reviews, where there is no target item
    ///
    /// A network call is triggered by a CommandExecution item, so having a
    /// target_item_id set to the CommandExecution item would be misleading
    /// because the review is about the network call, not the command execution.
    /// Therefore, target_item_id is set to None for network policy reviews.
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: GuardianApprovalReviewAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// [UNSTABLE] Temporary notification payload for approval auto-review. This
/// shape is expected to change soon.
pub struct ItemGuardianApprovalReviewCompletedNotification {
    pub thread_id: String,
    pub turn_id: String,
    /// Stable identifier for this review.
    pub review_id: String,
    /// Identifier for the reviewed item or tool call when one exists.
    ///
    /// In most cases, one review maps to one target item. The exceptions are
    /// - execve reviews, where a single command may contain multiple execve
    ///   calls to review (only possible when using the shell_zsh_fork feature)
    /// - network policy reviews, where there is no target item
    ///
    /// A network call is triggered by a CommandExecution item, so having a
    /// target_item_id set to the CommandExecution item would be misleading
    /// because the review is about the network call, not the command execution.
    /// Therefore, target_item_id is set to None for network policy reviews.
    pub target_item_id: Option<String>,
    pub decision_source: AutoReviewDecisionSource,
    pub review: GuardianApprovalReview,
    pub action: GuardianApprovalReviewAction,
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

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecutionOutputDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

/// Base64-encoded output chunk emitted for a streaming `command/exec` request.
///
/// These notifications are connection-scoped. If the originating connection
/// closes, the server terminates the process.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecOutputDeltaNotification {
    /// Client-supplied, connection-scoped `processId` from the original
    /// `command/exec` request.
    pub process_id: String,
    /// Output stream for this chunk.
    pub stream: CommandExecOutputStream,
    /// Base64-encoded output bytes.
    pub delta_base64: String,
    /// `true` on the final streamed chunk for a stream when `outputBytesCap`
    /// truncated later output on that stream.
    pub cap_reached: bool,
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
pub struct FileChangePatchUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<FileUpdateChange>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ServerRequestResolvedNotification {
    pub thread_id: String,
    pub request_id: RequestId,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum McpServerStartupState {
    Starting,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerStatusUpdatedNotification {
    pub name: String,
    pub status: McpServerStartupState,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WindowsSandboxSetupMode {
    Elevated,
    Unelevated,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxSetupStartParams {
    pub mode: WindowsSandboxSetupMode,
    #[ts(optional = nullable)]
    pub cwd: Option<AbsolutePathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxSetupStartResponse {
    pub started: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxSetupCompletedNotification {
    pub mode: WindowsSandboxSetupMode,
    pub success: bool,
    pub error: Option<String>,
}

/// Deprecated: Use `ContextCompaction` item type instead.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ContextCompactedNotification {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, ExperimentalApi)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct CommandExecutionRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    /// Unique identifier for this specific approval callback.
    ///
    /// For regular shell/unified_exec approvals, this is null.
    ///
    /// For zsh-exec-bridge subcommand approvals, multiple callbacks can belong to
    /// one parent `itemId`, so `approvalId` is a distinct opaque callback id
    /// (a UUID) used to disambiguate routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub approval_id: Option<String>,
    /// Optional explanatory reason (e.g. request for network access).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub reason: Option<String>,
    /// Optional context for a managed-network approval prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub network_approval_context: Option<NetworkApprovalContext>,
    /// The command to be executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub command: Option<String>,
    /// The command's working directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub cwd: Option<AbsolutePathBuf>,
    /// Best-effort parsed command actions for friendly display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub command_actions: Option<Vec<CommandAction>>,
    /// Optional additional permissions requested for this command.
    #[experimental("item/commandExecution/requestApproval.additionalPermissions")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    /// Optional proposed execpolicy amendment to allow similar commands without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    /// Optional proposed network policy amendments (allow/deny host) for future requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>,
    /// Ordered list of decisions the client may present for this prompt.
    #[experimental("item/commandExecution/requestApproval.availableDecisions")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub available_decisions: Option<Vec<CommandExecutionApprovalDecision>>,
}

impl CommandExecutionRequestApprovalParams {
    pub fn strip_experimental_fields(&mut self) {
        // TODO: Avoid hardcoding individual experimental fields here.
        // We need a generic outbound compatibility design for stripping or
        // otherwise handling experimental server->client payloads.
        self.additional_permissions = None;
    }
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
    #[ts(optional = nullable)]
    pub reason: Option<String>,
    /// [UNSTABLE] When set, the agent is asking the user to allow writes under this root
    /// for the remainder of the session (unclear if this is honored today).
    #[ts(optional = nullable)]
    pub grant_root: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[ts(export_to = "v2/")]
pub struct FileChangeRequestApprovalResponse {
    pub decision: FileChangeApprovalDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum McpServerElicitationAction {
    Accept,
    Decline,
    Cancel,
}

impl McpServerElicitationAction {
    pub fn to_core(self) -> codex_protocol::approvals::ElicitationAction {
        match self {
            Self::Accept => codex_protocol::approvals::ElicitationAction::Accept,
            Self::Decline => codex_protocol::approvals::ElicitationAction::Decline,
            Self::Cancel => codex_protocol::approvals::ElicitationAction::Cancel,
        }
    }
}

impl From<McpServerElicitationAction> for rmcp::model::ElicitationAction {
    fn from(value: McpServerElicitationAction) -> Self {
        match value {
            McpServerElicitationAction::Accept => Self::Accept,
            McpServerElicitationAction::Decline => Self::Decline,
            McpServerElicitationAction::Cancel => Self::Cancel,
        }
    }
}

impl From<rmcp::model::ElicitationAction> for McpServerElicitationAction {
    fn from(value: rmcp::model::ElicitationAction) -> Self {
        match value {
            rmcp::model::ElicitationAction::Accept => Self::Accept,
            rmcp::model::ElicitationAction::Decline => Self::Decline,
            rmcp::model::ElicitationAction::Cancel => Self::Cancel,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerElicitationRequestParams {
    pub thread_id: String,
    /// Active Codex turn when this elicitation was observed, if app-server could correlate one.
    ///
    /// This is nullable because MCP models elicitation as a standalone server-to-client request
    /// identified by the MCP server request id. It may be triggered during a turn, but turn
    /// context is app-server correlation rather than part of the protocol identity of the
    /// elicitation itself.
    pub turn_id: Option<String>,
    pub server_name: String,
    #[serde(flatten)]
    pub request: McpServerElicitationRequest,
    // TODO: When core can correlate an elicitation with an MCP tool call, expose the associated
    // McpToolCall item id here as an optional field. The current core event does not carry that
    // association.
}

/// Typed form schema for MCP `elicitation/create` requests.
///
/// This matches the `requestedSchema` shape from the MCP 2025-11-25
/// `ElicitRequestFormParams` schema.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationSchema {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    #[ts(optional, rename = "$schema")]
    pub schema_uri: Option<String>,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationObjectType,
    pub properties: BTreeMap<String, McpElicitationPrimitiveSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub required: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum McpElicitationObjectType {
    Object,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export_to = "v2/")]
pub enum McpElicitationPrimitiveSchema {
    Enum(McpElicitationEnumSchema),
    String(McpElicitationStringSchema),
    Number(McpElicitationNumberSchema),
    Boolean(McpElicitationBooleanSchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationStringSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationStringType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub min_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub format: Option<McpElicitationStringFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum McpElicitationStringType {
    String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case", export_to = "v2/")]
pub enum McpElicitationStringFormat {
    Email,
    Uri,
    Date,
    DateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationNumberSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationNumberType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum McpElicitationNumberType {
    Number,
    Integer,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationBooleanSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationBooleanType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum McpElicitationBooleanType {
    Boolean,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export_to = "v2/")]
pub enum McpElicitationEnumSchema {
    SingleSelect(McpElicitationSingleSelectEnumSchema),
    MultiSelect(McpElicitationMultiSelectEnumSchema),
    Legacy(McpElicitationLegacyTitledEnumSchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationLegacyTitledEnumSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationStringType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(rename = "enum")]
    #[ts(rename = "enum")]
    pub enum_: Vec<String>,
    #[serde(rename = "enumNames", skip_serializing_if = "Option::is_none")]
    #[ts(optional, rename = "enumNames")]
    pub enum_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export_to = "v2/")]
pub enum McpElicitationSingleSelectEnumSchema {
    Untitled(McpElicitationUntitledSingleSelectEnumSchema),
    Titled(McpElicitationTitledSingleSelectEnumSchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationUntitledSingleSelectEnumSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationStringType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(rename = "enum")]
    #[ts(rename = "enum")]
    pub enum_: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationTitledSingleSelectEnumSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationStringType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(rename = "oneOf")]
    #[ts(rename = "oneOf")]
    pub one_of: Vec<McpElicitationConstOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export_to = "v2/")]
pub enum McpElicitationMultiSelectEnumSchema {
    Untitled(McpElicitationUntitledMultiSelectEnumSchema),
    Titled(McpElicitationTitledMultiSelectEnumSchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationUntitledMultiSelectEnumSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationArrayType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub min_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_items: Option<u64>,
    pub items: McpElicitationUntitledEnumItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationTitledMultiSelectEnumSchema {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationArrayType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub min_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_items: Option<u64>,
    pub items: McpElicitationTitledEnumItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "v2/")]
pub enum McpElicitationArrayType {
    Array,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationUntitledEnumItems {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub type_: McpElicitationStringType,
    #[serde(rename = "enum")]
    #[ts(rename = "enum")]
    pub enum_: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationTitledEnumItems {
    #[serde(rename = "anyOf", alias = "oneOf")]
    #[ts(rename = "anyOf")]
    pub any_of: Vec<McpElicitationConstOption>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct McpElicitationConstOption {
    #[serde(rename = "const")]
    #[ts(rename = "const")]
    pub const_: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "mode", rename_all = "camelCase")]
#[ts(tag = "mode")]
#[ts(export_to = "v2/")]
pub enum McpServerElicitationRequest {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Form {
        #[serde(rename = "_meta")]
        #[ts(rename = "_meta")]
        meta: Option<JsonValue>,
        message: String,
        requested_schema: McpElicitationSchema,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Url {
        #[serde(rename = "_meta")]
        #[ts(rename = "_meta")]
        meta: Option<JsonValue>,
        message: String,
        url: String,
        elicitation_id: String,
    },
}

impl TryFrom<CoreElicitationRequest> for McpServerElicitationRequest {
    type Error = serde_json::Error;

    fn try_from(value: CoreElicitationRequest) -> Result<Self, Self::Error> {
        match value {
            CoreElicitationRequest::Form {
                meta,
                message,
                requested_schema,
            } => Ok(Self::Form {
                meta,
                message,
                requested_schema: serde_json::from_value(requested_schema)?,
            }),
            CoreElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            } => Ok(Self::Url {
                meta,
                message,
                url,
                elicitation_id,
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct McpServerElicitationRequestResponse {
    pub action: McpServerElicitationAction,
    /// Structured user input for accepted elicitations, mirroring RMCP `CreateElicitationResult`.
    ///
    /// This is nullable because decline/cancel responses have no content.
    pub content: Option<JsonValue>,
    /// Optional client metadata for form-mode action handling.
    #[serde(rename = "_meta")]
    #[ts(rename = "_meta")]
    pub meta: Option<JsonValue>,
}

impl From<McpServerElicitationRequestResponse> for rmcp::model::CreateElicitationResult {
    fn from(value: McpServerElicitationRequestResponse) -> Self {
        Self {
            action: value.action.into(),
            content: value.content,
        }
    }
}

impl From<rmcp::model::CreateElicitationResult> for McpServerElicitationRequestResponse {
    fn from(value: rmcp::model::CreateElicitationResult) -> Self {
        Self {
            action: value.action.into(),
            content: value.content,
            meta: None,
        }
    }
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
pub struct PermissionsRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
}

v2_enum_from_core!(
    #[derive(Default)]
    pub enum PermissionGrantScope from CorePermissionGrantScope {
        #[default]
        Turn,
        Session
    }
);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct PermissionsRequestApprovalResponse {
    pub permissions: GrantedPermissionProfile,
    #[serde(default)]
    pub scope: PermissionGrantScope,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolCallResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum DynamicToolCallOutputContentItem {
    #[serde(rename_all = "camelCase")]
    InputText { text: String },
    #[serde(rename_all = "camelCase")]
    InputImage { image_url: String },
}

impl From<DynamicToolCallOutputContentItem>
    for codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem
{
    fn from(item: DynamicToolCallOutputContentItem) -> Self {
        match item {
            DynamicToolCallOutputContentItem::InputText { text } => Self::InputText { text },
            DynamicToolCallOutputContentItem::InputImage { image_url } => {
                Self::InputImage { image_url }
            }
        }
    }
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
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub credits: Option<CreditsSnapshot>,
    pub plan_type: Option<PlanType>,
    pub rate_limit_reached_type: Option<RateLimitReachedType>,
}

impl From<CoreRateLimitSnapshot> for RateLimitSnapshot {
    fn from(value: CoreRateLimitSnapshot) -> Self {
        Self {
            limit_id: value.limit_id,
            limit_name: value.limit_name,
            primary: value.primary.map(RateLimitWindow::from),
            secondary: value.secondary.map(RateLimitWindow::from),
            credits: value.credits.map(CreditsSnapshot::from),
            plan_type: value.plan_type,
            rate_limit_reached_type: value
                .rate_limit_reached_type
                .map(RateLimitReachedType::from),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/", rename_all = "snake_case")]
pub enum RateLimitReachedType {
    RateLimitReached,
    WorkspaceOwnerCreditsDepleted,
    WorkspaceMemberCreditsDepleted,
    WorkspaceOwnerUsageLimitReached,
    WorkspaceMemberUsageLimitReached,
}

impl From<CoreRateLimitReachedType> for RateLimitReachedType {
    fn from(value: CoreRateLimitReachedType) -> Self {
        match value {
            CoreRateLimitReachedType::RateLimitReached => Self::RateLimitReached,
            CoreRateLimitReachedType::WorkspaceOwnerCreditsDepleted => {
                Self::WorkspaceOwnerCreditsDepleted
            }
            CoreRateLimitReachedType::WorkspaceMemberCreditsDepleted => {
                Self::WorkspaceMemberCreditsDepleted
            }
            CoreRateLimitReachedType::WorkspaceOwnerUsageLimitReached => {
                Self::WorkspaceOwnerUsageLimitReached
            }
            CoreRateLimitReachedType::WorkspaceMemberUsageLimitReached => {
                Self::WorkspaceMemberUsageLimitReached
            }
        }
    }
}

impl From<RateLimitReachedType> for CoreRateLimitReachedType {
    fn from(value: RateLimitReachedType) -> Self {
        match value {
            RateLimitReachedType::RateLimitReached => Self::RateLimitReached,
            RateLimitReachedType::WorkspaceOwnerCreditsDepleted => {
                Self::WorkspaceOwnerCreditsDepleted
            }
            RateLimitReachedType::WorkspaceMemberCreditsDepleted => {
                Self::WorkspaceMemberCreditsDepleted
            }
            RateLimitReachedType::WorkspaceOwnerUsageLimitReached => {
                Self::WorkspaceOwnerUsageLimitReached
            }
            RateLimitReachedType::WorkspaceMemberUsageLimitReached => {
                Self::WorkspaceMemberUsageLimitReached
            }
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelReroutedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub from_model: String,
    pub to_model: String,
    pub reason: ModelRerouteReason,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WarningNotification {
    /// Optional thread target when the warning applies to a specific thread.
    pub thread_id: Option<String>,
    /// Concise warning message for the user.
    pub message: String,
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
    use codex_protocol::protocol::ReadOnlyAccess as CoreReadOnlyAccess;
    use codex_protocol::user_input::UserInput as CoreUserInput;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::PathBuf;

    fn absolute_path_string(path: &str) -> String {
        let path = format!("/{}", path.trim_start_matches('/'));
        test_path_buf(&path).display().to_string()
    }

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        let path = format!("/{}", path.trim_start_matches('/'));
        test_path_buf(&path).abs()
    }

    fn test_absolute_path() -> AbsolutePathBuf {
        absolute_path("readable")
    }

    #[test]
    fn approvals_reviewer_serializes_auto_review_and_accepts_legacy_guardian_subagent() {
        assert_eq!(
            serde_json::to_string(&ApprovalsReviewer::User).expect("serialize reviewer"),
            "\"user\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalsReviewer::GuardianSubagent)
                .expect("serialize reviewer"),
            "\"auto_review\""
        );

        for value in ["user", "auto_review", "guardian_subagent"] {
            let json = format!("\"{value}\"");
            let reviewer: ApprovalsReviewer =
                serde_json::from_str(&json).expect("deserialize reviewer");
            let expected = if value == "user" {
                ApprovalsReviewer::User
            } else {
                ApprovalsReviewer::GuardianSubagent
            };
            assert_eq!(expected, reviewer);
        }
    }

    #[test]
    fn collab_agent_state_maps_interrupted_status() {
        assert_eq!(
            CollabAgentState::from(CoreAgentStatus::Interrupted),
            CollabAgentState {
                status: CollabAgentStatus::Interrupted,
                message: None,
            }
        );
    }

    #[test]
    fn external_agent_config_plugins_details_round_trip() {
        let item: ExternalAgentConfigMigrationItem = serde_json::from_value(json!({
            "itemType": "PLUGINS",
            "description": "Install supported plugins from Claude settings",
            "cwd": absolute_path_string("repo"),
            "details": {
                "plugins": [
                    {
                        "marketplaceName": "team-marketplace",
                        "pluginNames": ["asana"]
                    }
                ]
            }
        }))
        .expect("plugins migration item should deserialize");

        assert_eq!(
            item,
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Plugins,
                description: "Install supported plugins from Claude settings".to_string(),
                cwd: Some(PathBuf::from(absolute_path_string("repo"))),
                details: Some(MigrationDetails {
                    plugins: vec![PluginsMigration {
                        marketplace_name: "team-marketplace".to_string(),
                        plugin_names: vec!["asana".to_string()],
                    }],
                }),
            }
        );
    }

    #[test]
    fn command_execution_request_approval_rejects_relative_additional_permission_paths() {
        let err = serde_json::from_value::<CommandExecutionRequestApprovalParams>(json!({
            "threadId": "thr_123",
            "turnId": "turn_123",
            "itemId": "call_123",
            "command": "cat file",
            "cwd": absolute_path_string("tmp"),
            "commandActions": null,
            "reason": null,
            "networkApprovalContext": null,
            "additionalPermissions": {
                "network": null,
                "fileSystem": {
                    "read": ["relative/path"],
                    "write": null
                }
            },
            "proposedExecpolicyAmendment": null,
            "proposedNetworkPolicyAmendments": null,
            "availableDecisions": null
        }))
        .expect_err("relative additional permission paths should fail");
        assert!(
            err.to_string()
                .contains("AbsolutePathBuf deserialized without a base path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn permissions_request_approval_uses_request_permission_profile() {
        let read_only_path = if cfg!(windows) {
            r"C:\tmp\read-only"
        } else {
            "/tmp/read-only"
        };
        let read_write_path = if cfg!(windows) {
            r"C:\tmp\read-write"
        } else {
            "/tmp/read-write"
        };
        let params = serde_json::from_value::<PermissionsRequestApprovalParams>(json!({
            "threadId": "thr_123",
            "turnId": "turn_123",
            "itemId": "call_123",
            "reason": "Select a workspace root",
            "permissions": {
                "network": {
                    "enabled": true,
                },
                "fileSystem": {
                    "read": [read_only_path],
                    "write": [read_write_path],
                },
            },
        }))
        .expect("permissions request should deserialize");

        assert_eq!(
            params.permissions,
            RequestPermissionProfile {
                network: Some(AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(AdditionalFileSystemPermissions {
                    read: Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                            .expect("path must be absolute"),
                    ]),
                    write: Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                            .expect("path must be absolute"),
                    ]),
                    entries: None,
                }),
            }
        );

        assert_eq!(
            CoreRequestPermissionProfile::from(params.permissions),
            CoreRequestPermissionProfile {
                network: Some(CoreNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(CoreFileSystemPermissions::from_read_write_roots(
                    Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                            .expect("path must be absolute"),
                    ]),
                    Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                            .expect("path must be absolute"),
                    ]),
                )),
            }
        );
    }

    #[test]
    fn permissions_request_approval_rejects_macos_permissions() {
        let err = serde_json::from_value::<PermissionsRequestApprovalParams>(json!({
            "threadId": "thr_123",
            "turnId": "turn_123",
            "itemId": "call_123",
            "reason": "Select a workspace root",
            "permissions": {
                "network": null,
                "fileSystem": null,
                "macos": {
                    "preferences": "read_only",
                    "automations": "none",
                    "launchServices": false,
                    "accessibility": false,
                    "calendar": false,
                    "reminders": false,
                    "contacts": "none",
                },
            },
        }))
        .expect_err("permissions request should reject macos permissions");

        assert!(
            err.to_string().contains("unknown field `macos`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn additional_file_system_permissions_preserves_canonical_entries() {
        let core_permissions = CoreFileSystemPermissions {
            entries: vec![
                CoreFileSystemSandboxEntry {
                    path: CoreFileSystemPath::Special {
                        value: CoreFileSystemSpecialPath::Root,
                    },
                    access: CoreFileSystemAccessMode::Write,
                },
                CoreFileSystemSandboxEntry {
                    path: CoreFileSystemPath::GlobPattern {
                        pattern: "**/*.env".to_string(),
                    },
                    access: CoreFileSystemAccessMode::None,
                },
            ],
        };

        let permissions = AdditionalFileSystemPermissions::from(core_permissions.clone());
        assert_eq!(
            permissions,
            AdditionalFileSystemPermissions {
                read: None,
                write: None,
                entries: Some(vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Write,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::GlobPattern {
                            pattern: "**/*.env".to_string(),
                        },
                        access: FileSystemAccessMode::None,
                    },
                ]),
            }
        );
        assert_eq!(
            CoreFileSystemPermissions::from(permissions),
            core_permissions
        );
    }

    #[test]
    fn permissions_request_approval_response_uses_granted_permission_profile_without_macos() {
        let read_only_path = if cfg!(windows) {
            r"C:\tmp\read-only"
        } else {
            "/tmp/read-only"
        };
        let read_write_path = if cfg!(windows) {
            r"C:\tmp\read-write"
        } else {
            "/tmp/read-write"
        };
        let response = serde_json::from_value::<PermissionsRequestApprovalResponse>(json!({
            "permissions": {
                "network": {
                    "enabled": true,
                },
                "fileSystem": {
                    "read": [read_only_path],
                    "write": [read_write_path],
                },
            },
        }))
        .expect("permissions response should deserialize");

        assert_eq!(
            response.permissions,
            GrantedPermissionProfile {
                network: Some(AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(AdditionalFileSystemPermissions {
                    read: Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                            .expect("path must be absolute"),
                    ]),
                    write: Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                            .expect("path must be absolute"),
                    ]),
                    entries: None,
                }),
            }
        );

        assert_eq!(
            CorePermissionProfile::from(response.permissions),
            CorePermissionProfile {
                network: Some(CoreNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(CoreFileSystemPermissions::from_read_write_roots(
                    Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_only_path))
                            .expect("path must be absolute"),
                    ]),
                    Some(vec![
                        AbsolutePathBuf::try_from(PathBuf::from(read_write_path))
                            .expect("path must be absolute"),
                    ]),
                )),
            }
        );
    }

    #[test]
    fn permissions_request_approval_response_defaults_scope_to_turn() {
        let response = serde_json::from_value::<PermissionsRequestApprovalResponse>(json!({
            "permissions": {},
        }))
        .expect("response should deserialize");

        assert_eq!(response.scope, PermissionGrantScope::Turn);
    }

    #[test]
    fn fs_get_metadata_response_round_trips_minimal_fields() {
        let response = FsGetMetadataResponse {
            is_directory: false,
            is_file: true,
            is_symlink: false,
            created_at_ms: 123,
            modified_at_ms: 456,
        };

        let value = serde_json::to_value(&response).expect("serialize fs/getMetadata response");
        assert_eq!(
            value,
            json!({
                "isDirectory": false,
                "isFile": true,
                "isSymlink": false,
                "createdAtMs": 123,
                "modifiedAtMs": 456,
            })
        );

        let decoded = serde_json::from_value::<FsGetMetadataResponse>(value)
            .expect("deserialize fs/getMetadata response");
        assert_eq!(decoded, response);
    }

    #[test]
    fn fs_read_file_response_round_trips_base64_data() {
        let response = FsReadFileResponse {
            data_base64: "aGVsbG8=".to_string(),
        };

        let value = serde_json::to_value(&response).expect("serialize fs/readFile response");
        assert_eq!(
            value,
            json!({
                "dataBase64": "aGVsbG8=",
            })
        );

        let decoded = serde_json::from_value::<FsReadFileResponse>(value)
            .expect("deserialize fs/readFile response");
        assert_eq!(decoded, response);
    }

    #[test]
    fn fs_read_file_params_round_trip() {
        let params = FsReadFileParams {
            path: absolute_path("tmp/example.txt"),
        };

        let value = serde_json::to_value(&params).expect("serialize fs/readFile params");
        assert_eq!(
            value,
            json!({
                "path": absolute_path_string("tmp/example.txt"),
            })
        );

        let decoded = serde_json::from_value::<FsReadFileParams>(value)
            .expect("deserialize fs/readFile params");
        assert_eq!(decoded, params);
    }

    #[test]
    fn fs_create_directory_params_round_trip_with_default_recursive() {
        let params = FsCreateDirectoryParams {
            path: absolute_path("tmp/example"),
            recursive: None,
        };

        let value = serde_json::to_value(&params).expect("serialize fs/createDirectory params");
        assert_eq!(
            value,
            json!({
                "path": absolute_path_string("tmp/example"),
                "recursive": null,
            })
        );

        let decoded = serde_json::from_value::<FsCreateDirectoryParams>(value)
            .expect("deserialize fs/createDirectory params");
        assert_eq!(decoded, params);
    }

    #[test]
    fn fs_write_file_params_round_trip_with_base64_data() {
        let params = FsWriteFileParams {
            path: absolute_path("tmp/example.bin"),
            data_base64: "AAE=".to_string(),
        };

        let value = serde_json::to_value(&params).expect("serialize fs/writeFile params");
        assert_eq!(
            value,
            json!({
                "path": absolute_path_string("tmp/example.bin"),
                "dataBase64": "AAE=",
            })
        );

        let decoded = serde_json::from_value::<FsWriteFileParams>(value)
            .expect("deserialize fs/writeFile params");
        assert_eq!(decoded, params);
    }

    #[test]
    fn fs_copy_params_round_trip_with_recursive_directory_copy() {
        let params = FsCopyParams {
            source_path: absolute_path("tmp/source"),
            destination_path: absolute_path("tmp/destination"),
            recursive: true,
        };

        let value = serde_json::to_value(&params).expect("serialize fs/copy params");
        assert_eq!(
            value,
            json!({
                "sourcePath": absolute_path_string("tmp/source"),
                "destinationPath": absolute_path_string("tmp/destination"),
                "recursive": true,
            })
        );

        let decoded =
            serde_json::from_value::<FsCopyParams>(value).expect("deserialize fs/copy params");
        assert_eq!(decoded, params);
    }

    #[test]
    fn thread_shell_command_params_round_trip() {
        let params = ThreadShellCommandParams {
            thread_id: "thr_123".to_string(),
            command: "printf 'hello world\\n'".to_string(),
        };

        let value = serde_json::to_value(&params).expect("serialize thread/shellCommand params");
        assert_eq!(
            value,
            json!({
                "threadId": "thr_123",
                "command": "printf 'hello world\\n'",
            })
        );

        let decoded = serde_json::from_value::<ThreadShellCommandParams>(value)
            .expect("deserialize thread/shellCommand params");
        assert_eq!(decoded, params);
    }

    #[test]
    fn thread_shell_command_response_round_trip() {
        let response = ThreadShellCommandResponse {};

        let value =
            serde_json::to_value(&response).expect("serialize thread/shellCommand response");
        assert_eq!(value, json!({}));

        let decoded = serde_json::from_value::<ThreadShellCommandResponse>(value)
            .expect("deserialize thread/shellCommand response");
        assert_eq!(decoded, response);
    }

    #[test]
    fn fs_changed_notification_round_trips() {
        let notification = FsChangedNotification {
            watch_id: "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1".to_string(),
            changed_paths: vec![
                absolute_path("tmp/repo/.git/HEAD"),
                absolute_path("tmp/repo/.git/FETCH_HEAD"),
            ],
        };

        let value = serde_json::to_value(&notification).expect("serialize fs/changed notification");
        assert_eq!(
            value,
            json!({
                "watchId": "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1",
                "changedPaths": [
                    absolute_path_string("tmp/repo/.git/HEAD"),
                    absolute_path_string("tmp/repo/.git/FETCH_HEAD"),
                ],
            })
        );

        let decoded = serde_json::from_value::<FsChangedNotification>(value)
            .expect("deserialize fs/changed notification");
        assert_eq!(decoded, notification);
    }

    #[test]
    fn command_exec_params_default_optional_streaming_flags() {
        let params = serde_json::from_value::<CommandExecParams>(json!({
            "command": ["ls", "-la"],
            "timeoutMs": 1000,
            "cwd": "/tmp"
        }))
        .expect("command/exec payload should deserialize");

        assert_eq!(
            params,
            CommandExecParams {
                command: vec!["ls".to_string(), "-la".to_string()],
                process_id: None,
                tty: false,
                stream_stdin: false,
                stream_stdout_stderr: false,
                output_bytes_cap: None,
                disable_output_cap: false,
                disable_timeout: false,
                timeout_ms: Some(1000),
                cwd: Some(PathBuf::from("/tmp")),
                env: None,
                size: None,
                sandbox_policy: None,
            }
        );
    }

    #[test]
    fn command_exec_params_round_trips_disable_timeout() {
        let params = CommandExecParams {
            command: vec!["sleep".to_string(), "30".to_string()],
            process_id: Some("sleep-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: true,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec params");
        assert_eq!(
            value,
            json!({
                "command": ["sleep", "30"],
                "processId": "sleep-1",
                "disableTimeout": true,
                "timeoutMs": null,
                "cwd": null,
                "env": null,
                "size": null,
                "sandboxPolicy": null,
                "outputBytesCap": null,
            })
        );

        let decoded =
            serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_params_round_trips_disable_output_cap() {
        let params = CommandExecParams {
            command: vec!["yes".to_string()],
            process_id: Some("yes-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: true,
            output_bytes_cap: None,
            disable_output_cap: true,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec params");
        assert_eq!(
            value,
            json!({
                "command": ["yes"],
                "processId": "yes-1",
                "streamStdoutStderr": true,
                "outputBytesCap": null,
                "disableOutputCap": true,
                "timeoutMs": null,
                "cwd": null,
                "env": null,
                "size": null,
                "sandboxPolicy": null,
            })
        );

        let decoded =
            serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_params_round_trips_env_overrides_and_unsets() {
        let params = CommandExecParams {
            command: vec!["printenv".to_string(), "FOO".to_string()],
            process_id: Some("env-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: Some(HashMap::from([
                ("FOO".to_string(), Some("override".to_string())),
                ("BAR".to_string(), Some("added".to_string())),
                ("BAZ".to_string(), None),
            ])),
            size: None,
            sandbox_policy: None,
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec params");
        assert_eq!(
            value,
            json!({
                "command": ["printenv", "FOO"],
                "processId": "env-1",
                "outputBytesCap": null,
                "timeoutMs": null,
                "cwd": null,
                "env": {
                    "FOO": "override",
                    "BAR": "added",
                    "BAZ": null,
                },
                "size": null,
                "sandboxPolicy": null,
            })
        );

        let decoded =
            serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_write_round_trips_close_only_payload() {
        let params = CommandExecWriteParams {
            process_id: "proc-7".to_string(),
            delta_base64: None,
            close_stdin: true,
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec/write params");
        assert_eq!(
            value,
            json!({
                "processId": "proc-7",
                "deltaBase64": null,
                "closeStdin": true,
            })
        );

        let decoded = serde_json::from_value::<CommandExecWriteParams>(value)
            .expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_terminate_round_trips() {
        let params = CommandExecTerminateParams {
            process_id: "proc-8".to_string(),
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec/terminate params");
        assert_eq!(
            value,
            json!({
                "processId": "proc-8",
            })
        );

        let decoded = serde_json::from_value::<CommandExecTerminateParams>(value)
            .expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_params_round_trip_with_size() {
        let params = CommandExecParams {
            command: vec!["top".to_string()],
            process_id: Some("pty-1".to_string()),
            tty: true,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: Some(CommandExecTerminalSize {
                rows: 40,
                cols: 120,
            }),
            sandbox_policy: None,
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec params");
        assert_eq!(
            value,
            json!({
                "command": ["top"],
                "processId": "pty-1",
                "tty": true,
                "outputBytesCap": null,
                "timeoutMs": null,
                "cwd": null,
                "env": null,
                "size": {
                    "rows": 40,
                    "cols": 120,
                },
                "sandboxPolicy": null,
            })
        );

        let decoded =
            serde_json::from_value::<CommandExecParams>(value).expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_resize_round_trips() {
        let params = CommandExecResizeParams {
            process_id: "proc-9".to_string(),
            size: CommandExecTerminalSize {
                rows: 50,
                cols: 160,
            },
        };

        let value = serde_json::to_value(&params).expect("serialize command/exec/resize params");
        assert_eq!(
            value,
            json!({
                "processId": "proc-9",
                "size": {
                    "rows": 50,
                    "cols": 160,
                },
            })
        );

        let decoded = serde_json::from_value::<CommandExecResizeParams>(value)
            .expect("deserialize round-trip");
        assert_eq!(decoded, params);
    }

    #[test]
    fn command_exec_output_delta_round_trips() {
        let notification = CommandExecOutputDeltaNotification {
            process_id: "proc-1".to_string(),
            stream: CommandExecOutputStream::Stdout,
            delta_base64: "AQI=".to_string(),
            cap_reached: false,
        };

        let value = serde_json::to_value(&notification)
            .expect("serialize command/exec/outputDelta notification");
        assert_eq!(
            value,
            json!({
                "processId": "proc-1",
                "stream": "stdout",
                "deltaBase64": "AQI=",
                "capReached": false,
            })
        );

        let decoded = serde_json::from_value::<CommandExecOutputDeltaNotification>(value)
            .expect("deserialize round-trip");
        assert_eq!(decoded, notification);
    }

    #[test]
    fn command_execution_output_delta_round_trips() {
        let notification = CommandExecutionOutputDeltaNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            delta: "\u{fffd}a\n".to_string(),
        };

        let value = serde_json::to_value(&notification)
            .expect("serialize item/commandExecution/outputDelta notification");
        assert_eq!(
            value,
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "delta": "\u{fffd}a\n",
            })
        );

        let decoded = serde_json::from_value::<CommandExecutionOutputDeltaNotification>(value)
            .expect("deserialize round-trip");
        assert_eq!(decoded, notification);
    }

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
    fn sandbox_policy_round_trips_read_only_access() {
        let readable_root = test_absolute_path();
        let v2_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![readable_root.clone()],
            },
            network_access: true,
        };

        let core_policy = v2_policy.to_core();
        assert_eq!(
            core_policy,
            codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access: CoreReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![readable_root],
                },
                network_access: true,
            }
        );

        let back_to_v2 = SandboxPolicy::from(core_policy);
        assert_eq!(back_to_v2, v2_policy);
    }

    #[test]
    fn ask_for_approval_granular_round_trips_request_permissions_flag() {
        let v2_policy = AskForApproval::Granular {
            sandbox_approval: true,
            rules: false,
            skill_approval: false,
            request_permissions: true,
            mcp_elicitations: false,
        };

        let core_policy = v2_policy.to_core();
        assert_eq!(
            core_policy,
            CoreAskForApproval::Granular(CoreGranularApprovalConfig {
                sandbox_approval: true,
                rules: false,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: false,
            })
        );

        let back_to_v2 = AskForApproval::from(core_policy);
        assert_eq!(back_to_v2, v2_policy);
    }

    #[test]
    fn ask_for_approval_granular_defaults_missing_optional_flags_to_false() {
        let decoded = serde_json::from_value::<AskForApproval>(serde_json::json!({
            "granular": {
                "sandbox_approval": true,
                "rules": false,
                "mcp_elicitations": true,
            }
        }))
        .expect("granular approval policy should deserialize");

        assert_eq!(
            decoded,
            AskForApproval::Granular {
                sandbox_approval: true,
                rules: false,
                skill_approval: false,
                request_permissions: false,
                mcp_elicitations: true,
            }
        );
    }

    #[test]
    fn ask_for_approval_granular_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
            &AskForApproval::Granular {
                sandbox_approval: true,
                rules: false,
                skill_approval: false,
                request_permissions: false,
                mcp_elicitations: true,
            },
        );

        assert_eq!(reason, Some("askForApproval.granular"));
        assert_eq!(
            crate::experimental_api::ExperimentalApi::experimental_reason(
                &AskForApproval::OnRequest,
            ),
            None
        );
    }

    #[test]
    fn profile_v2_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&ProfileV2 {
            model: None,
            model_provider: None,
            approval_policy: Some(AskForApproval::Granular {
                sandbox_approval: true,
                rules: false,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: false,
            }),
            approvals_reviewer: None,
            service_tier: None,
            model_reasoning_effort: None,
            model_reasoning_summary: None,
            model_verbosity: None,
            web_search: None,
            tools: None,
            chatgpt_base_url: None,
            additional: HashMap::new(),
        });

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn config_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
            model: None,
            review_model: None,
            model_context_window: None,
            model_auto_compact_token_limit: None,
            model_provider: None,
            approval_policy: Some(AskForApproval::Granular {
                sandbox_approval: false,
                rules: true,
                skill_approval: false,
                request_permissions: false,
                mcp_elicitations: true,
            }),
            approvals_reviewer: None,
            sandbox_mode: None,
            sandbox_workspace_write: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            web_search: None,
            tools: None,
            profile: None,
            profiles: HashMap::new(),
            instructions: None,
            developer_instructions: None,
            compact_prompt: None,
            model_reasoning_effort: None,
            model_reasoning_summary: None,
            model_verbosity: None,
            service_tier: None,
            analytics: None,
            apps: None,
            additional: HashMap::new(),
        });

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn config_approvals_reviewer_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
            model: None,
            review_model: None,
            model_context_window: None,
            model_auto_compact_token_limit: None,
            model_provider: None,
            approval_policy: None,
            approvals_reviewer: Some(ApprovalsReviewer::GuardianSubagent),
            sandbox_mode: None,
            sandbox_workspace_write: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            web_search: None,
            tools: None,
            profile: None,
            profiles: HashMap::new(),
            instructions: None,
            developer_instructions: None,
            compact_prompt: None,
            model_reasoning_effort: None,
            model_reasoning_summary: None,
            model_verbosity: None,
            service_tier: None,
            analytics: None,
            apps: None,
            additional: HashMap::new(),
        });

        assert_eq!(reason, Some("config/read.approvalsReviewer"));
    }

    #[test]
    fn config_nested_profile_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
            model: None,
            review_model: None,
            model_context_window: None,
            model_auto_compact_token_limit: None,
            model_provider: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox_mode: None,
            sandbox_workspace_write: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            web_search: None,
            tools: None,
            profile: None,
            profiles: HashMap::from([(
                "default".to_string(),
                ProfileV2 {
                    model: None,
                    model_provider: None,
                    approval_policy: Some(AskForApproval::Granular {
                        sandbox_approval: true,
                        rules: false,
                        skill_approval: false,
                        request_permissions: false,
                        mcp_elicitations: true,
                    }),
                    approvals_reviewer: None,
                    service_tier: None,
                    model_reasoning_effort: None,
                    model_reasoning_summary: None,
                    model_verbosity: None,
                    web_search: None,
                    tools: None,
                    chatgpt_base_url: None,
                    additional: HashMap::new(),
                },
            )]),
            instructions: None,
            developer_instructions: None,
            compact_prompt: None,
            model_reasoning_effort: None,
            model_reasoning_summary: None,
            model_verbosity: None,
            service_tier: None,
            analytics: None,
            apps: None,
            additional: HashMap::new(),
        });

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn config_nested_profile_approvals_reviewer_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(&Config {
            model: None,
            review_model: None,
            model_context_window: None,
            model_auto_compact_token_limit: None,
            model_provider: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox_mode: None,
            sandbox_workspace_write: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            web_search: None,
            tools: None,
            profile: None,
            profiles: HashMap::from([(
                "default".to_string(),
                ProfileV2 {
                    model: None,
                    model_provider: None,
                    approval_policy: None,
                    approvals_reviewer: Some(ApprovalsReviewer::GuardianSubagent),
                    service_tier: None,
                    model_reasoning_effort: None,
                    model_reasoning_summary: None,
                    model_verbosity: None,
                    web_search: None,
                    tools: None,
                    chatgpt_base_url: None,
                    additional: HashMap::new(),
                },
            )]),
            instructions: None,
            developer_instructions: None,
            compact_prompt: None,
            model_reasoning_effort: None,
            model_reasoning_summary: None,
            model_verbosity: None,
            service_tier: None,
            analytics: None,
            apps: None,
            additional: HashMap::new(),
        });

        assert_eq!(reason, Some("config/read.approvalsReviewer"));
    }

    #[test]
    fn config_requirements_granular_allowed_approval_policy_is_marked_experimental() {
        let reason =
            crate::experimental_api::ExperimentalApi::experimental_reason(&ConfigRequirements {
                allowed_approval_policies: Some(vec![AskForApproval::Granular {
                    sandbox_approval: true,
                    rules: true,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: false,
                }]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                feature_requirements: None,
                enforce_residency: None,
                network: None,
            });

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn client_request_thread_start_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
            &crate::ClientRequest::ThreadStart {
                request_id: crate::RequestId::Integer(1),
                params: ThreadStartParams {
                    approval_policy: Some(AskForApproval::Granular {
                        sandbox_approval: true,
                        rules: false,
                        skill_approval: false,
                        request_permissions: true,
                        mcp_elicitations: false,
                    }),
                    ..Default::default()
                },
            },
        );

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn client_request_thread_resume_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
            &crate::ClientRequest::ThreadResume {
                request_id: crate::RequestId::Integer(2),
                params: ThreadResumeParams {
                    thread_id: "thr_123".to_string(),
                    approval_policy: Some(AskForApproval::Granular {
                        sandbox_approval: false,
                        rules: true,
                        skill_approval: false,
                        request_permissions: false,
                        mcp_elicitations: true,
                    }),
                    ..Default::default()
                },
            },
        );

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn client_request_thread_fork_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
            &crate::ClientRequest::ThreadFork {
                request_id: crate::RequestId::Integer(3),
                params: ThreadForkParams {
                    thread_id: "thr_456".to_string(),
                    approval_policy: Some(AskForApproval::Granular {
                        sandbox_approval: true,
                        rules: false,
                        skill_approval: false,
                        request_permissions: false,
                        mcp_elicitations: true,
                    }),
                    ..Default::default()
                },
            },
        );

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn client_request_turn_start_granular_approval_policy_is_marked_experimental() {
        let reason = crate::experimental_api::ExperimentalApi::experimental_reason(
            &crate::ClientRequest::TurnStart {
                request_id: crate::RequestId::Integer(4),
                params: TurnStartParams {
                    thread_id: "thr_123".to_string(),
                    input: Vec::new(),
                    approval_policy: Some(AskForApproval::Granular {
                        sandbox_approval: false,
                        rules: true,
                        skill_approval: false,
                        request_permissions: false,
                        mcp_elicitations: true,
                    }),
                    ..Default::default()
                },
            },
        );

        assert_eq!(reason, Some("askForApproval.granular"));
    }

    #[test]
    fn mcp_server_elicitation_response_round_trips_rmcp_result() {
        let rmcp_result = rmcp::model::CreateElicitationResult {
            action: rmcp::model::ElicitationAction::Accept,
            content: Some(json!({
                "confirmed": true,
            })),
        };

        let v2_response = McpServerElicitationRequestResponse::from(rmcp_result.clone());
        assert_eq!(
            v2_response,
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Accept,
                content: Some(json!({
                    "confirmed": true,
                })),
                meta: None,
            }
        );
        assert_eq!(
            rmcp::model::CreateElicitationResult::from(v2_response),
            rmcp_result
        );
    }

    #[test]
    fn mcp_server_elicitation_request_from_core_url_request() {
        let request = McpServerElicitationRequest::try_from(CoreElicitationRequest::Url {
            meta: None,
            message: "Finish sign-in".to_string(),
            url: "https://example.com/complete".to_string(),
            elicitation_id: "elicitation-123".to_string(),
        })
        .expect("URL request should convert");

        assert_eq!(
            request,
            McpServerElicitationRequest::Url {
                meta: None,
                message: "Finish sign-in".to_string(),
                url: "https://example.com/complete".to_string(),
                elicitation_id: "elicitation-123".to_string(),
            }
        );
    }

    #[test]
    fn mcp_server_elicitation_request_from_core_form_request() {
        let request = McpServerElicitationRequest::try_from(CoreElicitationRequest::Form {
            meta: None,
            message: "Allow this request?".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {
                    "confirmed": {
                        "type": "boolean",
                    }
                },
                "required": ["confirmed"],
            }),
        })
        .expect("form request should convert");

        let expected_schema: McpElicitationSchema = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "confirmed": {
                    "type": "boolean",
                }
            },
            "required": ["confirmed"],
        }))
        .expect("expected schema should deserialize");

        assert_eq!(
            request,
            McpServerElicitationRequest::Form {
                meta: None,
                message: "Allow this request?".to_string(),
                requested_schema: expected_schema,
            }
        );
    }

    #[test]
    fn mcp_elicitation_schema_matches_mcp_2025_11_25_primitives() {
        let schema: McpElicitationSchema = serde_json::from_value(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "title": "Email",
                    "description": "Work email address",
                    "format": "email",
                    "default": "dev@example.com",
                },
                "count": {
                    "type": "integer",
                    "title": "Count",
                    "description": "How many items to create",
                    "minimum": 1,
                    "maximum": 5,
                    "default": 3,
                },
                "confirmed": {
                    "type": "boolean",
                    "title": "Confirm",
                    "description": "Approve the pending action",
                    "default": true,
                },
                "legacyChoice": {
                    "type": "string",
                    "title": "Action",
                    "description": "Legacy titled enum form",
                    "enum": ["allow", "deny"],
                    "enumNames": ["Allow", "Deny"],
                    "default": "allow",
                },
            },
            "required": ["email", "confirmed"],
        }))
        .expect("schema should deserialize");

        assert_eq!(
            schema,
            McpElicitationSchema {
                schema_uri: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
                type_: McpElicitationObjectType::Object,
                properties: BTreeMap::from([
                    (
                        "confirmed".to_string(),
                        McpElicitationPrimitiveSchema::Boolean(McpElicitationBooleanSchema {
                            type_: McpElicitationBooleanType::Boolean,
                            title: Some("Confirm".to_string()),
                            description: Some("Approve the pending action".to_string()),
                            default: Some(true),
                        }),
                    ),
                    (
                        "count".to_string(),
                        McpElicitationPrimitiveSchema::Number(McpElicitationNumberSchema {
                            type_: McpElicitationNumberType::Integer,
                            title: Some("Count".to_string()),
                            description: Some("How many items to create".to_string()),
                            minimum: Some(1.0),
                            maximum: Some(5.0),
                            default: Some(3.0),
                        }),
                    ),
                    (
                        "email".to_string(),
                        McpElicitationPrimitiveSchema::String(McpElicitationStringSchema {
                            type_: McpElicitationStringType::String,
                            title: Some("Email".to_string()),
                            description: Some("Work email address".to_string()),
                            min_length: None,
                            max_length: None,
                            format: Some(McpElicitationStringFormat::Email),
                            default: Some("dev@example.com".to_string()),
                        }),
                    ),
                    (
                        "legacyChoice".to_string(),
                        McpElicitationPrimitiveSchema::Enum(McpElicitationEnumSchema::Legacy(
                            McpElicitationLegacyTitledEnumSchema {
                                type_: McpElicitationStringType::String,
                                title: Some("Action".to_string()),
                                description: Some("Legacy titled enum form".to_string()),
                                enum_: vec!["allow".to_string(), "deny".to_string()],
                                enum_names: Some(vec!["Allow".to_string(), "Deny".to_string(),]),
                                default: Some("allow".to_string()),
                            },
                        )),
                    ),
                ]),
                required: Some(vec!["email".to_string(), "confirmed".to_string()]),
            }
        );
    }

    #[test]
    fn mcp_server_elicitation_request_rejects_null_core_form_schema() {
        let result = McpServerElicitationRequest::try_from(CoreElicitationRequest::Form {
            meta: Some(json!({
                "persist": "session",
            })),
            message: "Allow this request?".to_string(),
            requested_schema: JsonValue::Null,
        });

        assert!(result.is_err());
    }

    #[test]
    fn mcp_server_elicitation_request_rejects_invalid_core_form_schema() {
        let result = McpServerElicitationRequest::try_from(CoreElicitationRequest::Form {
            meta: None,
            message: "Allow this request?".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {
                    "confirmed": {
                        "type": "object",
                    }
                },
            }),
        });

        assert!(result.is_err());
    }

    #[test]
    fn mcp_server_elicitation_response_serializes_nullable_content() {
        let response = McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Decline,
            content: None,
            meta: None,
        };

        assert_eq!(
            serde_json::to_value(response).expect("response should serialize"),
            json!({
                "action": "decline",
                "content": null,
                "_meta": null,
            })
        );
    }

    #[test]
    fn sandbox_policy_round_trips_workspace_write_read_only_access() {
        let readable_root = test_absolute_path();
        let v2_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![readable_root.clone()],
            },
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let core_policy = v2_policy.to_core();
        assert_eq!(
            core_policy,
            codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: CoreReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![readable_root],
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        );

        let back_to_v2 = SandboxPolicy::from(core_policy);
        assert_eq!(back_to_v2, v2_policy);
    }

    #[test]
    fn sandbox_policy_deserializes_legacy_read_only_without_access_field() {
        let policy: SandboxPolicy = serde_json::from_value(json!({
            "type": "readOnly"
        }))
        .expect("read-only policy should deserialize");
        assert_eq!(
            policy,
            SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::FullAccess,
                network_access: false,
            }
        );
    }

    #[test]
    fn sandbox_policy_deserializes_legacy_workspace_write_without_read_only_access_field() {
        let policy: SandboxPolicy = serde_json::from_value(json!({
            "type": "workspaceWrite",
            "writableRoots": [],
            "networkAccess": false,
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false
        }))
        .expect("workspace-write policy should deserialize");
        assert_eq!(
            policy,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        );
    }

    #[test]
    fn automatic_approval_review_deserializes_aborted_status() {
        let review: GuardianApprovalReview = serde_json::from_value(json!({
            "status": "aborted",
            "riskLevel": null,
            "userAuthorization": null,
            "rationale": null
        }))
        .expect("aborted automatic review should deserialize");
        assert_eq!(
            review,
            GuardianApprovalReview {
                status: GuardianApprovalReviewStatus::Aborted,
                risk_level: None,
                user_authorization: None,
                rationale: None,
            }
        );
    }

    #[test]
    fn guardian_approval_review_action_round_trips_command_shape() {
        let value = json!({
            "type": "command",
            "source": "shell",
            "command": "rm -rf /tmp/example.sqlite",
            "cwd": absolute_path_string("tmp"),
        });
        let action: GuardianApprovalReviewAction =
            serde_json::from_value(value.clone()).expect("guardian review action");

        assert_eq!(
            action,
            GuardianApprovalReviewAction::Command {
                source: GuardianCommandSource::Shell,
                command: "rm -rf /tmp/example.sqlite".to_string(),
                cwd: absolute_path("tmp"),
            }
        );
        assert_eq!(
            serde_json::to_value(&action).expect("serialize guardian review action"),
            value
        );
    }

    #[test]
    fn network_requirements_deserializes_legacy_fields() {
        let requirements: NetworkRequirements = serde_json::from_value(json!({
            "allowedDomains": ["api.openai.com"],
            "deniedDomains": ["blocked.example.com"],
            "allowUnixSockets": ["/tmp/proxy.sock"]
        }))
        .expect("legacy network requirements should deserialize");

        assert_eq!(
            requirements,
            NetworkRequirements {
                enabled: None,
                http_port: None,
                socks_port: None,
                allow_upstream_proxy: None,
                dangerously_allow_non_loopback_proxy: None,
                dangerously_allow_all_unix_sockets: None,
                domains: None,
                managed_allowed_domains_only: None,
                allowed_domains: Some(vec!["api.openai.com".to_string()]),
                denied_domains: Some(vec!["blocked.example.com".to_string()]),
                unix_sockets: None,
                allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
                allow_local_binding: None,
            }
        );
    }

    #[test]
    fn network_requirements_serializes_canonical_and_legacy_fields() {
        let requirements = NetworkRequirements {
            enabled: Some(true),
            http_port: Some(8080),
            socks_port: Some(1080),
            allow_upstream_proxy: Some(false),
            dangerously_allow_non_loopback_proxy: Some(false),
            dangerously_allow_all_unix_sockets: Some(true),
            domains: Some(BTreeMap::from([
                ("api.openai.com".to_string(), NetworkDomainPermission::Allow),
                (
                    "blocked.example.com".to_string(),
                    NetworkDomainPermission::Deny,
                ),
            ])),
            managed_allowed_domains_only: Some(true),
            allowed_domains: Some(vec!["api.openai.com".to_string()]),
            denied_domains: Some(vec!["blocked.example.com".to_string()]),
            unix_sockets: Some(BTreeMap::from([
                (
                    "/tmp/proxy.sock".to_string(),
                    NetworkUnixSocketPermission::Allow,
                ),
                (
                    "/tmp/ignored.sock".to_string(),
                    NetworkUnixSocketPermission::None,
                ),
            ])),
            allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
            allow_local_binding: Some(true),
        };

        assert_eq!(
            serde_json::to_value(requirements).expect("network requirements should serialize"),
            json!({
                "enabled": true,
                "httpPort": 8080,
                "socksPort": 1080,
                "allowUpstreamProxy": false,
                "dangerouslyAllowNonLoopbackProxy": false,
                "dangerouslyAllowAllUnixSockets": true,
                "domains": {
                    "api.openai.com": "allow",
                    "blocked.example.com": "deny"
                },
                "managedAllowedDomainsOnly": true,
                "allowedDomains": ["api.openai.com"],
                "deniedDomains": ["blocked.example.com"],
                "unixSockets": {
                    "/tmp/ignored.sock": "none",
                    "/tmp/proxy.sock": "allow"
                },
                "allowUnixSockets": ["/tmp/proxy.sock"],
                "allowLocalBinding": true
            })
        );
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
            phase: None,
            memory_citation: None,
        });

        assert_eq!(
            ThreadItem::from(agent_item),
            ThreadItem::AgentMessage {
                id: "agent-1".to_string(),
                text: "Hello world".to_string(),
                phase: None,
                memory_citation: None,
            }
        );

        let agent_item_with_phase = TurnItem::AgentMessage(AgentMessageItem {
            id: "agent-2".to_string(),
            content: vec![AgentMessageContent::Text {
                text: "final".to_string(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: Some(CoreMemoryCitation {
                entries: vec![CoreMemoryCitationEntry {
                    path: "MEMORY.md".to_string(),
                    line_start: 1,
                    line_end: 2,
                    note: "summary".to_string(),
                }],
                rollout_ids: vec!["rollout-1".to_string()],
            }),
        });

        assert_eq!(
            ThreadItem::from(agent_item_with_phase),
            ThreadItem::AgentMessage {
                id: "agent-2".to_string(),
                text: "final".to_string(),
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: Some(MemoryCitation {
                    entries: vec![MemoryCitationEntry {
                        path: "MEMORY.md".to_string(),
                        line_start: 1,
                        line_end: 2,
                        note: "summary".to_string(),
                    }],
                    thread_ids: vec!["rollout-1".to_string()],
                }),
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
                per_cwd_extra_user_roots: None,
            })
            .unwrap(),
            json!({
                "perCwdExtraUserRoots": null,
            }),
        );

        assert_eq!(
            serde_json::to_value(SkillsListParams {
                cwds: vec![PathBuf::from("/repo")],
                force_reload: true,
                per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                    cwd: PathBuf::from("/repo"),
                    extra_user_roots: vec![
                        PathBuf::from("/shared/skills"),
                        PathBuf::from("/tmp/x")
                    ],
                }]),
            })
            .unwrap(),
            json!({
                "cwds": ["/repo"],
                "forceReload": true,
                "perCwdExtraUserRoots": [
                    {
                        "cwd": "/repo",
                        "extraUserRoots": ["/shared/skills", "/tmp/x"],
                    }
                ],
            }),
        );
    }

    #[test]
    fn plugin_source_serializes_local_git_and_remote_variants() {
        let local_path = if cfg!(windows) {
            r"C:\plugins\linear"
        } else {
            "/plugins/linear"
        };
        let local_path = AbsolutePathBuf::try_from(PathBuf::from(local_path)).unwrap();
        let local_path_json = local_path.as_path().display().to_string();

        assert_eq!(
            serde_json::to_value(PluginSource::Local { path: local_path }).unwrap(),
            json!({
                "type": "local",
                "path": local_path_json,
            }),
        );

        assert_eq!(
            serde_json::to_value(PluginSource::Git {
                url: "https://github.com/openai/example.git".to_string(),
                path: Some("plugins/example".to_string()),
                ref_name: Some("main".to_string()),
                sha: Some("abc123".to_string()),
            })
            .unwrap(),
            json!({
                "type": "git",
                "url": "https://github.com/openai/example.git",
                "path": "plugins/example",
                "refName": "main",
                "sha": "abc123",
            }),
        );

        assert_eq!(
            serde_json::to_value(PluginSource::Remote).unwrap(),
            json!({
                "type": "remote",
            }),
        );
    }

    #[test]
    fn marketplace_add_params_serialization_uses_optional_ref_name_and_sparse_paths() {
        assert_eq!(
            serde_json::to_value(MarketplaceAddParams {
                source: "owner/repo".to_string(),
                ref_name: None,
                sparse_paths: None,
            })
            .unwrap(),
            json!({
                "source": "owner/repo",
                "refName": null,
                "sparsePaths": null,
            }),
        );

        assert_eq!(
            serde_json::to_value(MarketplaceAddParams {
                source: "owner/repo".to_string(),
                ref_name: Some("main".to_string()),
                sparse_paths: Some(vec!["plugins/foo".to_string()]),
            })
            .unwrap(),
            json!({
                "source": "owner/repo",
                "refName": "main",
                "sparsePaths": ["plugins/foo"],
            }),
        );
    }

    #[test]
    fn plugin_marketplace_entry_serializes_remote_only_path_as_null() {
        assert_eq!(
            serde_json::to_value(PluginMarketplaceEntry {
                name: "openai-curated".to_string(),
                path: None,
                interface: None,
                plugins: Vec::new(),
            })
            .unwrap(),
            json!({
                "name": "openai-curated",
                "path": null,
                "interface": null,
                "plugins": [],
            }),
        );
    }

    #[test]
    fn plugin_interface_serializes_local_paths_and_remote_urls_separately() {
        let composer_icon = if cfg!(windows) {
            r"C:\plugins\linear\icon.png"
        } else {
            "/plugins/linear/icon.png"
        };
        let composer_icon = AbsolutePathBuf::try_from(PathBuf::from(composer_icon)).unwrap();
        let composer_icon_json = composer_icon.as_path().display().to_string();

        let interface = PluginInterface {
            display_name: Some("Linear".to_string()),
            short_description: None,
            long_description: None,
            developer_name: None,
            category: Some("Productivity".to_string()),
            capabilities: Vec::new(),
            website_url: None,
            privacy_policy_url: None,
            terms_of_service_url: None,
            default_prompt: None,
            brand_color: None,
            composer_icon: Some(composer_icon),
            composer_icon_url: Some("https://example.com/linear/icon.png".to_string()),
            logo: None,
            logo_url: Some("https://example.com/linear/logo.png".to_string()),
            screenshots: Vec::new(),
            screenshot_urls: vec!["https://example.com/linear/screenshot.png".to_string()],
        };

        assert_eq!(
            serde_json::to_value(interface).unwrap(),
            json!({
                "displayName": "Linear",
                "shortDescription": null,
                "longDescription": null,
                "developerName": null,
                "category": "Productivity",
                "capabilities": [],
                "websiteUrl": null,
                "privacyPolicyUrl": null,
                "termsOfServiceUrl": null,
                "defaultPrompt": null,
                "brandColor": null,
                "composerIcon": composer_icon_json,
                "composerIconUrl": "https://example.com/linear/icon.png",
                "logo": null,
                "logoUrl": "https://example.com/linear/logo.png",
                "screenshots": [],
                "screenshotUrls": ["https://example.com/linear/screenshot.png"],
            }),
        );
    }

    #[test]
    fn plugin_list_params_ignore_removed_force_remote_sync_field() {
        assert_eq!(
            serde_json::from_value::<PluginListParams>(json!({
                "cwds": null,
                "forceRemoteSync": true,
            }))
            .unwrap(),
            PluginListParams { cwds: None },
        );
    }

    #[test]
    fn plugin_read_params_serialization_uses_install_source_fields() {
        let marketplace_path = if cfg!(windows) {
            r"C:\plugins\marketplace.json"
        } else {
            "/plugins/marketplace.json"
        };
        let marketplace_path = AbsolutePathBuf::try_from(PathBuf::from(marketplace_path)).unwrap();
        let marketplace_path_json = marketplace_path.as_path().display().to_string();
        assert_eq!(
            serde_json::to_value(PluginReadParams {
                marketplace_path: Some(marketplace_path.clone()),
                remote_marketplace_name: None,
                plugin_name: "gmail".to_string(),
            })
            .unwrap(),
            json!({
                "marketplacePath": marketplace_path_json,
                "remoteMarketplaceName": null,
                "pluginName": "gmail",
            }),
        );

        assert_eq!(
            serde_json::from_value::<PluginReadParams>(json!({
                "marketplacePath": marketplace_path_json,
                "pluginName": "gmail",
                "forceRemoteSync": true,
            }))
            .unwrap(),
            PluginReadParams {
                marketplace_path: Some(marketplace_path),
                remote_marketplace_name: None,
                plugin_name: "gmail".to_string(),
            },
        );

        assert_eq!(
            serde_json::from_value::<PluginReadParams>(json!({
                "remoteMarketplaceName": "openai-curated",
                "pluginName": "gmail",
            }))
            .unwrap(),
            PluginReadParams {
                marketplace_path: None,
                remote_marketplace_name: Some("openai-curated".to_string()),
                plugin_name: "gmail".to_string(),
            },
        );
    }

    #[test]
    fn plugin_install_params_serialization_omits_force_remote_sync() {
        let marketplace_path = if cfg!(windows) {
            r"C:\plugins\marketplace.json"
        } else {
            "/plugins/marketplace.json"
        };
        let marketplace_path = AbsolutePathBuf::try_from(PathBuf::from(marketplace_path)).unwrap();
        let marketplace_path_json = marketplace_path.as_path().display().to_string();
        assert_eq!(
            serde_json::to_value(PluginInstallParams {
                marketplace_path: Some(marketplace_path.clone()),
                remote_marketplace_name: None,
                plugin_name: "gmail".to_string(),
            })
            .unwrap(),
            json!({
                "marketplacePath": marketplace_path_json,
                "remoteMarketplaceName": null,
                "pluginName": "gmail",
            }),
        );

        assert_eq!(
            serde_json::from_value::<PluginInstallParams>(json!({
                "marketplacePath": marketplace_path_json,
                "pluginName": "gmail",
                "forceRemoteSync": true,
            }))
            .unwrap(),
            PluginInstallParams {
                marketplace_path: Some(marketplace_path),
                remote_marketplace_name: None,
                plugin_name: "gmail".to_string(),
            },
        );

        assert_eq!(
            serde_json::from_value::<PluginInstallParams>(json!({
                "remoteMarketplaceName": "openai-curated",
                "pluginName": "gmail",
                "forceRemoteSync": true,
            }))
            .unwrap(),
            PluginInstallParams {
                marketplace_path: None,
                remote_marketplace_name: Some("openai-curated".to_string()),
                plugin_name: "gmail".to_string(),
            },
        );
    }

    #[test]
    fn plugin_uninstall_params_serialization_omits_force_remote_sync() {
        assert_eq!(
            serde_json::to_value(PluginUninstallParams {
                plugin_id: "gmail@openai-curated".to_string(),
            })
            .unwrap(),
            json!({
                "pluginId": "gmail@openai-curated",
            }),
        );

        assert_eq!(
            serde_json::from_value::<PluginUninstallParams>(json!({
                "pluginId": "gmail@openai-curated",
                "forceRemoteSync": true,
            }))
            .unwrap(),
            PluginUninstallParams {
                plugin_id: "gmail@openai-curated".to_string(),
            },
        );
    }

    #[test]
    fn marketplace_remove_response_serializes_nullable_installed_root() {
        let installed_root = if cfg!(windows) {
            r"C:\marketplaces\debug"
        } else {
            "/tmp/marketplaces/debug"
        };
        let installed_root = AbsolutePathBuf::try_from(PathBuf::from(installed_root)).unwrap();
        let installed_root_json = installed_root.as_path().display().to_string();
        assert_eq!(
            serde_json::to_value(MarketplaceRemoveResponse {
                marketplace_name: "debug".to_string(),
                installed_root: Some(installed_root),
            })
            .unwrap(),
            json!({
                "marketplaceName": "debug",
                "installedRoot": installed_root_json,
            }),
        );

        assert_eq!(
            serde_json::to_value(MarketplaceRemoveResponse {
                marketplace_name: "debug".to_string(),
                installed_root: None,
            })
            .unwrap(),
            json!({
                "marketplaceName": "debug",
                "installedRoot": null,
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

    #[test]
    fn codex_error_info_serializes_active_turn_not_steerable_turn_kind_in_camel_case() {
        let value = CodexErrorInfo::ActiveTurnNotSteerable {
            turn_kind: NonSteerableTurnKind::Review,
        };

        assert_eq!(
            serde_json::to_value(value).unwrap(),
            json!({
                "activeTurnNotSteerable": {
                    "turnKind": "review"
                }
            })
        );
    }

    #[test]
    fn dynamic_tool_response_serializes_content_items() {
        let value = serde_json::to_value(DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: "dynamic-ok".to_string(),
            }],
            success: true,
        })
        .unwrap();

        assert_eq!(
            value,
            json!({
                "contentItems": [
                    {
                        "type": "inputText",
                        "text": "dynamic-ok"
                    }
                ],
                "success": true,
            })
        );
    }

    #[test]
    fn dynamic_tool_response_serializes_text_and_image_content_items() {
        let value = serde_json::to_value(DynamicToolCallResponse {
            content_items: vec![
                DynamicToolCallOutputContentItem::InputText {
                    text: "dynamic-ok".to_string(),
                },
                DynamicToolCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                },
            ],
            success: true,
        })
        .unwrap();

        assert_eq!(
            value,
            json!({
                "contentItems": [
                    {
                        "type": "inputText",
                        "text": "dynamic-ok"
                    },
                    {
                        "type": "inputImage",
                        "imageUrl": "data:image/png;base64,AAA"
                    }
                ],
                "success": true,
            })
        );
    }

    #[test]
    fn dynamic_tool_spec_deserializes_defer_loading() {
        let value = json!({
            "name": "lookup_ticket",
            "description": "Fetch a ticket",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                }
            },
            "deferLoading": true,
        });

        let actual: DynamicToolSpec = serde_json::from_value(value).expect("deserialize");

        assert_eq!(
            actual,
            DynamicToolSpec {
                name: "lookup_ticket".to_string(),
                description: "Fetch a ticket".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    }
                }),
                defer_loading: true,
            }
        );
    }

    #[test]
    fn dynamic_tool_spec_legacy_expose_to_context_inverts_to_defer_loading() {
        let value = json!({
            "name": "lookup_ticket",
            "description": "Fetch a ticket",
            "inputSchema": {
                "type": "object",
                "properties": {}
            },
            "exposeToContext": false,
        });

        let actual: DynamicToolSpec = serde_json::from_value(value).expect("deserialize");

        assert!(actual.defer_loading);
    }

    #[test]
    fn thread_start_params_preserve_explicit_null_service_tier() {
        let params: ThreadStartParams = serde_json::from_value(json!({ "serviceTier": null }))
            .expect("params should deserialize");
        assert_eq!(params.service_tier, Some(None));

        let serialized = serde_json::to_value(&params).expect("params should serialize");
        assert_eq!(
            serialized.get("serviceTier"),
            Some(&serde_json::Value::Null)
        );

        let serialized_without_override =
            serde_json::to_value(ThreadStartParams::default()).expect("params should serialize");
        assert_eq!(serialized_without_override.get("serviceTier"), None);
    }

    #[test]
    fn thread_lifecycle_responses_default_missing_instruction_sources() {
        let response = json!({
            "thread": {
                "id": "thread-id",
                "forkedFromId": null,
                "preview": "",
                "ephemeral": false,
                "modelProvider": "openai",
                "createdAt": 1,
                "updatedAt": 1,
                "status": { "type": "idle" },
                "path": null,
                "cwd": absolute_path_string("tmp"),
                "cliVersion": "0.0.0",
                "source": "exec",
                "agentNickname": null,
                "agentRole": null,
                "gitInfo": null,
                "name": null,
                "turns": []
            },
            "model": "gpt-5",
            "modelProvider": "openai",
            "serviceTier": null,
            "cwd": absolute_path_string("tmp"),
            "approvalPolicy": "on-failure",
            "approvalsReviewer": "user",
            "sandbox": { "type": "dangerFullAccess" },
            "reasoningEffort": null
        });

        let start: ThreadStartResponse =
            serde_json::from_value(response.clone()).expect("thread/start response");
        let resume: ThreadResumeResponse =
            serde_json::from_value(response.clone()).expect("thread/resume response");
        let fork: ThreadForkResponse =
            serde_json::from_value(response).expect("thread/fork response");

        assert_eq!(start.instruction_sources, Vec::<AbsolutePathBuf>::new());
        assert_eq!(resume.instruction_sources, Vec::<AbsolutePathBuf>::new());
        assert_eq!(fork.instruction_sources, Vec::<AbsolutePathBuf>::new());
    }

    #[test]
    fn turn_start_params_preserve_explicit_null_service_tier() {
        let params: TurnStartParams = serde_json::from_value(json!({
            "threadId": "thread_123",
            "input": [],
            "serviceTier": null
        }))
        .expect("params should deserialize");
        assert_eq!(params.service_tier, Some(None));

        let serialized = serde_json::to_value(&params).expect("params should serialize");
        assert_eq!(
            serialized.get("serviceTier"),
            Some(&serde_json::Value::Null)
        );

        let without_override = TurnStartParams {
            thread_id: "thread_123".to_string(),
            input: vec![],
            responsesapi_client_metadata: None,
            cwd: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox_policy: None,
            model: None,
            service_tier: None,
            effort: None,
            summary: None,
            output_schema: None,
            collaboration_mode: None,
            personality: None,
        };
        let serialized_without_override =
            serde_json::to_value(&without_override).expect("params should serialize");
        assert_eq!(serialized_without_override.get("serviceTier"), None);
    }
}
