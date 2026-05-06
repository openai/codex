use super::AdditionalPermissionProfile;
use super::ExecPolicyAmendment;
use super::McpToolCallError;
use super::McpToolCallResult;
use super::NetworkApprovalContext;
use super::NetworkApprovalProtocol;
use super::NetworkPolicyAmendment;
use super::RequestPermissionProfile;
use super::UserInput;
use super::shared::v2_enum_from_core;
use crate::protocol::item_builders::convert_patch_changes;
use codex_experimental_api_macros::ExperimentalApi;
use codex_protocol::approvals::GuardianAssessmentAction as CoreGuardianAssessmentAction;
use codex_protocol::approvals::GuardianAssessmentDecisionSource as CoreGuardianAssessmentDecisionSource;
use codex_protocol::approvals::GuardianCommandSource as CoreGuardianCommandSource;
use codex_protocol::items::AgentMessageContent as CoreAgentMessageContent;
use codex_protocol::items::McpToolCallStatus as CoreMcpToolCallStatus;
use codex_protocol::items::TurnItem as CoreTurnItem;
use codex_protocol::memory_citation::MemoryCitation as CoreMemoryCitation;
use codex_protocol::memory_citation::MemoryCitationEntry as CoreMemoryCitationEntry;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::parse_command::ParsedCommand as CoreParsedCommand;
use codex_protocol::protocol::AgentStatus as CoreAgentStatus;
use codex_protocol::protocol::ExecCommandSource as CoreExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
use codex_protocol::protocol::GuardianRiskLevel as CoreGuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization as CoreGuardianUserAuthorization;
use codex_protocol::protocol::PatchApplyStatus as CorePatchApplyStatus;
use codex_protocol::protocol::ReviewDecision as CoreReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_with::serde_as;
use std::collections::HashMap;
use std::path::PathBuf;
use ts_rs::TS;

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
        namespace: Option<String>,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GuardianRequestPermissionsReviewAction {
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
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
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    RequestPermissions {
        reason: Option<String>,
        permissions: RequestPermissionProfile,
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
            CoreGuardianAssessmentAction::RequestPermissions {
                reason,
                permissions,
            } => Self::RequestPermissions {
                reason,
                permissions: permissions.into(),
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
            GuardianApprovalReviewAction::RequestPermissions {
                reason,
                permissions,
            } => Self::RequestPermissions {
                reason,
                permissions: permissions.into(),
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
            CoreTurnItem::ImageView(image) => ThreadItem::ImageView {
                id: image.id,
                path: image.path,
            },
            CoreTurnItem::ImageGeneration(image) => ThreadItem::ImageGeneration {
                id: image.id,
                status: image.status,
                revised_prompt: image.revised_prompt,
                result: image.result,
                saved_path: image.saved_path,
            },
            CoreTurnItem::FileChange(file_change) => ThreadItem::FileChange {
                id: file_change.id,
                changes: convert_patch_changes(&file_change.changes),
                status: file_change
                    .status
                    .as_ref()
                    .map(PatchApplyStatus::from)
                    .unwrap_or(PatchApplyStatus::InProgress),
            },
            CoreTurnItem::McpToolCall(mcp) => {
                let duration_ms = mcp
                    .duration
                    .and_then(|duration| i64::try_from(duration.as_millis()).ok());

                ThreadItem::McpToolCall {
                    id: mcp.id,
                    server: mcp.server,
                    tool: mcp.tool,
                    status: McpToolCallStatus::from(mcp.status),
                    arguments: mcp.arguments,
                    mcp_app_resource_uri: mcp.mcp_app_resource_uri,
                    result: mcp.result.map(McpToolCallResult::from).map(Box::new),
                    error: mcp.error.map(McpToolCallError::from),
                    duration_ms,
                }
            }
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

impl From<CoreMcpToolCallStatus> for McpToolCallStatus {
    fn from(value: CoreMcpToolCallStatus) -> Self {
        match value {
            CoreMcpToolCallStatus::InProgress => McpToolCallStatus::InProgress,
            CoreMcpToolCallStatus::Completed => McpToolCallStatus::Completed,
            CoreMcpToolCallStatus::Failed => McpToolCallStatus::Failed,
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
pub struct ItemStartedNotification {
    pub item: ThreadItem,
    pub thread_id: String,
    pub turn_id: String,
    /// Unix timestamp (in milliseconds) when this item lifecycle started.
    #[ts(type = "number")]
    pub started_at_ms: i64,
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
    /// Unix timestamp (in milliseconds) when this item lifecycle completed.
    #[ts(type = "number")]
    pub completed_at_ms: i64,
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
/// Deprecated legacy notification for `apply_patch` textual output.
///
/// The server no longer emits this notification.
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: JsonValue,
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
