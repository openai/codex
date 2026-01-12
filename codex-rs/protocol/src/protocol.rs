//! Defines the protocol for a Codex session between a client and an agent.
//!
//! Uses a SQ (Submission Queue) / EQ (Event Queue) pattern to asynchronously communicate
//! between user and agent.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use crate::ThreadId;
use crate::approvals::ElicitationRequestEvent;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::custom_prompts::CustomPrompt;
use crate::items::TurnItem;
use crate::message_history::HistoryEntry;
use crate::models::ContentItem;
use crate::models::ResponseItem;
use crate::num_format::format_with_separators;
use crate::openai_models::ReasoningEffort as ReasoningEffortConfig;
use crate::parse_command::ParsedCommand;
use crate::plan_tool::UpdatePlanArgs;
use crate::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use mcp_types::CallToolResult;
use mcp_types::RequestId;
use mcp_types::Resource as McpResource;
use mcp_types::ResourceTemplate as McpResourceTemplate;
use mcp_types::Tool as McpTool;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_with::serde_as;
use strum_macros::Display;
use tracing::error;
use ts_rs::TS;

pub use crate::approvals::ApplyPatchApprovalRequestEvent;
pub use crate::approvals::ElicitationAction;
pub use crate::approvals::ExecApprovalRequestEvent;
pub use crate::approvals::ExecPolicyAmendment;

/// Open/close tags for special user-input blocks. Used across crates to avoid
/// duplicated hardcoded strings.
pub const USER_INSTRUCTIONS_OPEN_TAG: &str = "<user_instructions>";
pub const USER_INSTRUCTIONS_CLOSE_TAG: &str = "</user_instructions>";
pub const ENVIRONMENT_CONTEXT_OPEN_TAG: &str = "<environment_context>";
pub const ENVIRONMENT_CONTEXT_CLOSE_TAG: &str = "</environment_context>";
pub const USER_MESSAGE_BEGIN: &str = "## My request for Codex:";

/// Submission Queue Entry - requests from user
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct Submission {
    /// Unique id for this Submission to correlate with Events
    pub id: String,
    /// Payload
    pub op: Op,
}

/// Config payload for refreshing MCP servers.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
pub struct McpServerRefreshConfig {
    pub mcp_servers: Value,
    pub mcp_oauth_credentials_store_mode: Value,
}

/// Submission operation
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Op {
    /// Abort current task.
    /// This server sends [`EventMsg::TurnAborted`] in response.
    Interrupt,

    /// Input from the user
    UserInput {
        /// User input items, see `InputItem`
        items: Vec<UserInput>,
        /// Optional JSON Schema used to constrain the final assistant message for this turn.
        #[serde(skip_serializing_if = "Option::is_none")]
        final_output_json_schema: Option<Value>,
    },

    /// Similar to [`Op::UserInput`], but contains additional context required
    /// for a turn of a [`crate::codex_thread::CodexThread`].
    UserTurn {
        /// User input items, see `InputItem`
        items: Vec<UserInput>,

        /// `cwd` to use with the [`SandboxPolicy`] and potentially tool calls
        /// such as `local_shell`.
        cwd: PathBuf,

        /// Policy to use for command approval.
        approval_policy: AskForApproval,

        /// Policy to use for tool calls such as `local_shell`.
        sandbox_policy: SandboxPolicy,

        /// Must be a valid model slug for the configured client session
        /// associated with this conversation.
        model: String,

        /// Will only be honored if the model is configured to use reasoning.
        #[serde(skip_serializing_if = "Option::is_none")]
        effort: Option<ReasoningEffortConfig>,

        /// Will only be honored if the model is configured to use reasoning.
        summary: ReasoningSummaryConfig,
        // The JSON schema to use for the final assistant message
        final_output_json_schema: Option<Value>,
    },

    /// Override parts of the persistent turn context for subsequent turns.
    ///
    /// All fields are optional; when omitted, the existing value is preserved.
    /// This does not enqueue any input – it only updates defaults used for
    /// future `UserInput` turns.
    OverrideTurnContext {
        /// Updated `cwd` for sandbox/tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,

        /// Updated command approval policy.
        #[serde(skip_serializing_if = "Option::is_none")]
        approval_policy: Option<AskForApproval>,

        /// Updated sandbox policy for tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        sandbox_policy: Option<SandboxPolicy>,

        /// Updated model slug. When set, the model info is derived
        /// automatically.
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,

        /// Updated reasoning effort (honored only for reasoning-capable models).
        ///
        /// Use `Some(Some(_))` to set a specific effort, `Some(None)` to clear
        /// the effort, or `None` to leave the existing value unchanged.
        #[serde(skip_serializing_if = "Option::is_none")]
        effort: Option<Option<ReasoningEffortConfig>>,

        /// Updated reasoning summary preference (honored only for reasoning-capable models).
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<ReasoningSummaryConfig>,
    },

    /// Approve a command execution
    ExecApproval {
        /// The id of the submission we are approving
        id: String,
        /// The user's decision in response to the request.
        decision: ReviewDecision,
    },

    /// Approve a code patch
    PatchApproval {
        /// The id of the submission we are approving
        id: String,
        /// The user's decision in response to the request.
        decision: ReviewDecision,
    },

    /// Resolve an MCP elicitation request.
    ResolveElicitation {
        /// Name of the MCP server that issued the request.
        server_name: String,
        /// Request identifier from the MCP server.
        request_id: RequestId,
        /// User's decision for the request.
        decision: ElicitationAction,
    },

    /// Append an entry to the persistent cross-session message history.
    ///
    /// Note the entry is not guaranteed to be logged if the user has
    /// history disabled, it matches the list of "sensitive" patterns, etc.
    AddToHistory {
        /// The message text to be stored.
        text: String,
    },

    /// Request a single history entry identified by `log_id` + `offset`.
    GetHistoryEntryRequest { offset: usize, log_id: u64 },

    /// Request the list of MCP tools available across all configured servers.
    /// Reply is delivered via `EventMsg::McpListToolsResponse`.
    ListMcpTools,

    /// Request MCP servers to reinitialize and refresh cached tool lists.
    RefreshMcpServers { config: McpServerRefreshConfig },

    /// Request the list of available custom prompts.
    ListCustomPrompts,

    /// Request the list of skills for the provided `cwd` values or the session default.
    ListSkills {
        /// Working directories to scope repo skills discovery.
        ///
        /// When empty, the session default working directory is used.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cwds: Vec<PathBuf>,

        /// When true, recompute skills even if a cached result exists.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        force_reload: bool,
    },

    /// Request the agent to summarize the current conversation context.
    /// The agent will use its existing context (either conversation history or previous response id)
    /// to generate a summary which will be returned as an AgentMessage event.
    Compact,

    /// Request Codex to undo a turn (turn are stacked so it is the same effect as CMD + Z).
    Undo,

    /// Request Codex to drop the last N user turns from in-memory context.
    ///
    /// This does not attempt to revert local filesystem changes. Clients are
    /// responsible for undoing any edits on disk.
    ThreadRollback { num_turns: u32 },

    /// Request a code review from the agent.
    Review { review_request: ReviewRequest },

    /// Request to shut down codex instance.
    Shutdown,

    /// Execute a user-initiated one-off shell command (triggered by "!cmd").
    ///
    /// The command string is executed using the user's default shell and may
    /// include shell syntax (pipes, redirects, etc.). Output is streamed via
    /// `ExecCommand*` events and the UI regains control upon `TurnComplete`.
    RunUserShellCommand {
        /// The raw command string after '!'
        command: String,
    },

    /// Request the list of available models.
    ListModels,
}

/// Determines the conditions under which the user is consulted to approve
/// running the command proposed by Codex.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    JsonSchema,
    TS,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum AskForApproval {
    /// Under this policy, only "known safe" commands—as determined by
    /// `is_safe_command()`—that **only read files** are auto‑approved.
    /// Everything else will ask the user to approve.
    #[serde(rename = "untrusted")]
    #[strum(serialize = "untrusted")]
    UnlessTrusted,

    /// *All* commands are auto‑approved, but they are expected to run inside a
    /// sandbox where network access is disabled and writes are confined to a
    /// specific set of paths. If the command fails, it will be escalated to
    /// the user to approve execution without a sandbox.
    OnFailure,

    /// The model decides when to ask the user for approval.
    #[default]
    OnRequest,

    /// Never ask the user to approve commands. Failures are immediately returned
    /// to the model, and never escalated to the user for approval.
    Never,
}

/// Represents whether outbound network access is available to the agent.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, Default, JsonSchema, TS,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NetworkAccess {
    #[default]
    Restricted,
    Enabled,
}

impl NetworkAccess {
    pub fn is_enabled(self) -> bool {
        matches!(self, NetworkAccess::Enabled)
    }
}

/// Determines execution restrictions for model shell commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, JsonSchema, TS)]
#[strum(serialize_all = "kebab-case")]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SandboxPolicy {
    /// No restrictions whatsoever. Use with caution.
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,

    /// Read-only access to the entire file-system.
    #[serde(rename = "read-only")]
    ReadOnly,

    /// Indicates the process is already in an external sandbox. Allows full
    /// disk access while honoring the provided network setting.
    #[serde(rename = "external-sandbox")]
    ExternalSandbox {
        /// Whether the external sandbox permits outbound network traffic.
        #[serde(default)]
        network_access: NetworkAccess,
    },

    /// Same as `ReadOnly` but additionally grants write access to the current
    /// working directory ("workspace").
    #[serde(rename = "workspace-write")]
    WorkspaceWrite {
        /// Additional folders (beyond cwd and possibly TMPDIR) that should be
        /// writable from within the sandbox.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        writable_roots: Vec<AbsolutePathBuf>,

        /// When set to `true`, outbound network access is allowed. `false` by
        /// default.
        #[serde(default)]
        network_access: bool,

        /// When set to `true`, will NOT include the per-user `TMPDIR`
        /// environment variable among the default writable roots. Defaults to
        /// `false`.
        #[serde(default)]
        exclude_tmpdir_env_var: bool,

        /// When set to `true`, will NOT include the `/tmp` among the default
        /// writable roots on UNIX. Defaults to `false`.
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

/// A writable root path accompanied by a list of subpaths that should remain
/// read‑only even when the root is writable. This is primarily used to ensure
/// that folders containing files that could be modified to escalate the
/// privileges of the agent (e.g. `.codex`, `.git`, notably `.git/hooks`) under
/// a writable root are not modified by the agent.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct WritableRoot {
    pub root: AbsolutePathBuf,

    /// By construction, these subpaths are all under `root`.
    pub read_only_subpaths: Vec<AbsolutePathBuf>,
}

impl WritableRoot {
    pub fn is_path_writable(&self, path: &Path) -> bool {
        // Check if the path is under the root.
        if !path.starts_with(&self.root) {
            return false;
        }

        // Check if the path is under any of the read-only subpaths.
        for subpath in &self.read_only_subpaths {
            if path.starts_with(subpath) {
                return false;
            }
        }

        true
    }
}

impl FromStr for SandboxPolicy {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl SandboxPolicy {
    /// Returns a policy with read-only disk access and no network.
    pub fn new_read_only_policy() -> Self {
        SandboxPolicy::ReadOnly
    }

    /// Returns a policy that can read the entire disk, but can only write to
    /// the current working directory and the per-user tmp dir on macOS. It does
    /// not allow network access.
    pub fn new_workspace_write_policy() -> Self {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

    /// Always returns `true`; restricting read access is not supported.
    pub fn has_full_disk_read_access(&self) -> bool {
        true
    }

    pub fn has_full_disk_write_access(&self) -> bool {
        match self {
            SandboxPolicy::DangerFullAccess => true,
            SandboxPolicy::ExternalSandbox { .. } => true,
            SandboxPolicy::ReadOnly => false,
            SandboxPolicy::WorkspaceWrite { .. } => false,
        }
    }

    pub fn has_full_network_access(&self) -> bool {
        match self {
            SandboxPolicy::DangerFullAccess => true,
            SandboxPolicy::ExternalSandbox { network_access } => network_access.is_enabled(),
            SandboxPolicy::ReadOnly => false,
            SandboxPolicy::WorkspaceWrite { network_access, .. } => *network_access,
        }
    }

    /// Returns the list of writable roots (tailored to the current working
    /// directory) together with subpaths that should remain read‑only under
    /// each writable root.
    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        match self {
            SandboxPolicy::DangerFullAccess => Vec::new(),
            SandboxPolicy::ExternalSandbox { .. } => Vec::new(),
            SandboxPolicy::ReadOnly => Vec::new(),
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
                network_access: _,
            } => {
                // Start from explicitly configured writable roots.
                let mut roots: Vec<AbsolutePathBuf> = writable_roots.clone();

                // Always include defaults: cwd, /tmp (if present on Unix), and
                // on macOS, the per-user TMPDIR unless explicitly excluded.
                // TODO(mbolin): cwd param should be AbsolutePathBuf.
                let cwd_absolute = AbsolutePathBuf::from_absolute_path(cwd);
                match cwd_absolute {
                    Ok(cwd) => {
                        roots.push(cwd);
                    }
                    Err(e) => {
                        error!(
                            "Ignoring invalid cwd {:?} for sandbox writable root: {}",
                            cwd, e
                        );
                    }
                }

                // Include /tmp on Unix unless explicitly excluded.
                if cfg!(unix) && !exclude_slash_tmp {
                    #[allow(clippy::expect_used)]
                    let slash_tmp =
                        AbsolutePathBuf::from_absolute_path("/tmp").expect("/tmp is absolute");
                    if slash_tmp.as_path().is_dir() {
                        roots.push(slash_tmp);
                    }
                }

                // Include $TMPDIR unless explicitly excluded. On macOS, TMPDIR
                // is per-user, so writes to TMPDIR should not be readable by
                // other users on the system.
                //
                // By comparison, TMPDIR is not guaranteed to be defined on
                // Linux or Windows, but supporting it here gives users a way to
                // provide the model with their own temporary directory without
                // having to hardcode it in the config.
                if !exclude_tmpdir_env_var
                    && let Some(tmpdir) = std::env::var_os("TMPDIR")
                    && !tmpdir.is_empty()
                {
                    match AbsolutePathBuf::from_absolute_path(PathBuf::from(&tmpdir)) {
                        Ok(tmpdir_path) => {
                            roots.push(tmpdir_path);
                        }
                        Err(e) => {
                            error!(
                                "Ignoring invalid TMPDIR value {tmpdir:?} for sandbox writable root: {e}",
                            );
                        }
                    }
                }

                // For each root, compute subpaths that should remain read-only.
                roots
                    .into_iter()
                    .map(|writable_root| {
                        let mut subpaths = Vec::new();
                        #[allow(clippy::expect_used)]
                        let top_level_git = writable_root
                            .join(".git")
                            .expect(".git is a valid relative path");
                        if top_level_git.as_path().is_dir() {
                            subpaths.push(top_level_git);
                        }
                        #[allow(clippy::expect_used)]
                        let top_level_codex = writable_root
                            .join(".codex")
                            .expect(".codex is a valid relative path");
                        if top_level_codex.as_path().is_dir() {
                            subpaths.push(top_level_codex);
                        }
                        WritableRoot {
                            root: writable_root,
                            read_only_subpaths: subpaths,
                        }
                    })
                    .collect()
            }
        }
    }
}

/// Event Queue Entry - events from agent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Event {
    /// Submission `id` that this event is correlated with.
    pub id: String,
    /// Payload
    pub msg: EventMsg,
}

/// Response event from the agent
/// NOTE: Make sure none of these values have optional types, as it will mess up the extension code-gen.
#[derive(Debug, Clone, Deserialize, Serialize, Display, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type")]
#[strum(serialize_all = "snake_case")]
pub enum EventMsg {
    /// Error while executing a submission
    Error(ErrorEvent),

    /// Warning issued while processing a submission. Unlike `Error`, this
    /// indicates the turn continued but the user should still be notified.
    Warning(WarningEvent),

    /// Conversation history was compacted (either automatically or manually).
    ContextCompacted(ContextCompactedEvent),

    /// Conversation history was rolled back by dropping the last N user turns.
    ThreadRolledBack(ThreadRolledBackEvent),

    /// Agent has started a turn.
    /// v1 wire format uses `task_started`; accept `turn_started` for v2 interop.
    #[serde(rename = "task_started", alias = "turn_started")]
    TurnStarted(TurnStartedEvent),

    /// Agent has completed all actions.
    /// v1 wire format uses `task_complete`; accept `turn_complete` for v2 interop.
    #[serde(rename = "task_complete", alias = "turn_complete")]
    TurnComplete(TurnCompleteEvent),

    /// Usage update for the current session, including totals and last turn.
    /// Optional means unknown — UIs should not display when `None`.
    TokenCount(TokenCountEvent),

    /// Agent text output message
    AgentMessage(AgentMessageEvent),

    /// User/system input message (what was sent to the model)
    UserMessage(UserMessageEvent),

    /// Agent text output delta message
    AgentMessageDelta(AgentMessageDeltaEvent),

    /// Reasoning event from agent.
    AgentReasoning(AgentReasoningEvent),

    /// Agent reasoning delta event from agent.
    AgentReasoningDelta(AgentReasoningDeltaEvent),

    /// Raw chain-of-thought from agent.
    AgentReasoningRawContent(AgentReasoningRawContentEvent),

    /// Agent reasoning content delta event from agent.
    AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent),
    /// Signaled when the model begins a new reasoning summary section (e.g., a new titled block).
    AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent),

    /// Ack the client's configure message.
    SessionConfigured(SessionConfiguredEvent),

    /// Incremental MCP startup progress updates.
    McpStartupUpdate(McpStartupUpdateEvent),

    /// Aggregate MCP startup completion summary.
    McpStartupComplete(McpStartupCompleteEvent),

    McpToolCallBegin(McpToolCallBeginEvent),

    McpToolCallEnd(McpToolCallEndEvent),

    WebSearchBegin(WebSearchBeginEvent),

    WebSearchEnd(WebSearchEndEvent),

    /// Notification that the server is about to execute a command.
    ExecCommandBegin(ExecCommandBeginEvent),

    /// Incremental chunk of output from a running command.
    ExecCommandOutputDelta(ExecCommandOutputDeltaEvent),

    /// Terminal interaction for an in-progress command (stdin sent and stdout observed).
    TerminalInteraction(TerminalInteractionEvent),

    ExecCommandEnd(ExecCommandEndEvent),

    /// Notification that the agent attached a local image via the view_image tool.
    ViewImageToolCall(ViewImageToolCallEvent),

    ExecApprovalRequest(ExecApprovalRequestEvent),

    ElicitationRequest(ElicitationRequestEvent),

    ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent),

    /// Notification advising the user that something they are using has been
    /// deprecated and should be phased out.
    DeprecationNotice(DeprecationNoticeEvent),

    BackgroundEvent(BackgroundEventEvent),

    UndoStarted(UndoStartedEvent),

    UndoCompleted(UndoCompletedEvent),

    /// Notification that a model stream experienced an error or disconnect
    /// and the system is handling it (e.g., retrying with backoff).
    StreamError(StreamErrorEvent),

    /// Notification that the agent is about to apply a code patch. Mirrors
    /// `ExecCommandBegin` so front‑ends can show progress indicators.
    PatchApplyBegin(PatchApplyBeginEvent),

    /// Notification that a patch application has finished.
    PatchApplyEnd(PatchApplyEndEvent),

    TurnDiff(TurnDiffEvent),

    /// Response to GetHistoryEntryRequest.
    GetHistoryEntryResponse(GetHistoryEntryResponseEvent),

    /// List of MCP tools available to the agent.
    McpListToolsResponse(McpListToolsResponseEvent),

    /// List of custom prompts available to the agent.
    ListCustomPromptsResponse(ListCustomPromptsResponseEvent),

    /// List of skills available to the agent.
    ListSkillsResponse(ListSkillsResponseEvent),

    /// Notification that skill data may have been updated and clients may want to reload.
    SkillsUpdateAvailable,

    PlanUpdate(UpdatePlanArgs),

    TurnAborted(TurnAbortedEvent),

    /// Notification that the agent is shutting down.
    ShutdownComplete,

    /// Entered review mode.
    EnteredReviewMode(ReviewRequest),

    /// Exited review mode with an optional final result to apply.
    ExitedReviewMode(ExitedReviewModeEvent),

    RawResponseItem(RawResponseItemEvent),

    ItemStarted(ItemStartedEvent),
    ItemCompleted(ItemCompletedEvent),

    AgentMessageContentDelta(AgentMessageContentDeltaEvent),
    ReasoningContentDelta(ReasoningContentDeltaEvent),
    ReasoningRawContentDelta(ReasoningRawContentDeltaEvent),
}

/// Agent lifecycle status, derived from emitted events.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is waiting for initialization.
    #[default]
    PendingInit,
    /// Agent is currently running.
    Running,
    /// Agent is done. Contains the final assistant message.
    Completed(Option<String>),
    /// Agent encountered an error.
    Errored(String),
    /// Agent has been shutdown.
    Shutdown,
    /// Agent is not found.
    NotFound,
}

/// Codex errors that we expose to clients.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum CodexErrorInfo {
    ContextWindowExceeded,
    UsageLimitExceeded,
    HttpConnectionFailed {
        http_status_code: Option<u16>,
    },
    /// Failed to connect to the response SSE stream.
    ResponseStreamConnectionFailed {
        http_status_code: Option<u16>,
    },
    InternalServerError,
    Unauthorized,
    BadRequest,
    SandboxError,
    /// The response SSE stream disconnected in the middle of a turnbefore completion.
    ResponseStreamDisconnected {
        http_status_code: Option<u16>,
    },
    /// Reached the retry limit for responses.
    ResponseTooManyFailedAttempts {
        http_status_code: Option<u16>,
    },
    ThreadRollbackFailed,
    Other,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct RawResponseItemEvent {
    pub item: ResponseItem,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct ItemStartedEvent {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub item: TurnItem,
}

impl HasLegacyEvent for ItemStartedEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        match &self.item {
            TurnItem::WebSearch(item) => vec![EventMsg::WebSearchBegin(WebSearchBeginEvent {
                call_id: item.id.clone(),
            })],
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct ItemCompletedEvent {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub item: TurnItem,
}

pub trait HasLegacyEvent {
    fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg>;
}

impl HasLegacyEvent for ItemCompletedEvent {
    fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        self.item.as_legacy_events(show_raw_agent_reasoning)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct AgentMessageContentDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

impl HasLegacyEvent for AgentMessageContentDeltaEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        vec![EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: self.delta.clone(),
        })]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct ReasoningContentDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
    // load with default value so it's backward compatible with the old format.
    #[serde(default)]
    pub summary_index: i64,
}

impl HasLegacyEvent for ReasoningContentDeltaEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        vec![EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: self.delta.clone(),
        })]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct ReasoningRawContentDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
    // load with default value so it's backward compatible with the old format.
    #[serde(default)]
    pub content_index: i64,
}

impl HasLegacyEvent for ReasoningRawContentDeltaEvent {
    fn as_legacy_events(&self, _: bool) -> Vec<EventMsg> {
        vec![EventMsg::AgentReasoningRawContentDelta(
            AgentReasoningRawContentDeltaEvent {
                delta: self.delta.clone(),
            },
        )]
    }
}

impl HasLegacyEvent for EventMsg {
    fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        match self {
            EventMsg::ItemCompleted(event) => event.as_legacy_events(show_raw_agent_reasoning),
            EventMsg::AgentMessageContentDelta(event) => {
                event.as_legacy_events(show_raw_agent_reasoning)
            }
            EventMsg::ReasoningContentDelta(event) => {
                event.as_legacy_events(show_raw_agent_reasoning)
            }
            EventMsg::ReasoningRawContentDelta(event) => {
                event.as_legacy_events(show_raw_agent_reasoning)
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ExitedReviewModeEvent {
    pub review_output: Option<ReviewOutputEvent>,
}

// Individual event payload types matching each `EventMsg` variant.

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ErrorEvent {
    pub message: String,
    #[serde(default)]
    pub codex_error_info: Option<CodexErrorInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct WarningEvent {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ContextCompactedEvent;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct TurnCompleteEvent {
    pub last_agent_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct TurnStartedEvent {
    // TODO(aibrahim): make this not optional
    pub model_context_window: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq, JsonSchema, TS)]
pub struct TokenUsage {
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub cached_input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
    #[ts(type = "number")]
    pub reasoning_output_tokens: i64,
    #[ts(type = "number")]
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct TokenUsageInfo {
    pub total_token_usage: TokenUsage,
    pub last_token_usage: TokenUsage,
    // TODO(aibrahim): make this not optional
    #[ts(type = "number | null")]
    pub model_context_window: Option<i64>,
}

impl TokenUsageInfo {
    pub fn new_or_append(
        info: &Option<TokenUsageInfo>,
        last: &Option<TokenUsage>,
        model_context_window: Option<i64>,
    ) -> Option<Self> {
        if info.is_none() && last.is_none() {
            return None;
        }

        let mut info = match info {
            Some(info) => info.clone(),
            None => Self {
                total_token_usage: TokenUsage::default(),
                last_token_usage: TokenUsage::default(),
                model_context_window,
            },
        };
        if let Some(last) = last {
            info.append_last_usage(last);
        }
        Some(info)
    }

    pub fn append_last_usage(&mut self, last: &TokenUsage) {
        self.total_token_usage.add_assign(last);
        self.last_token_usage = last.clone();
    }

    pub fn fill_to_context_window(&mut self, context_window: i64) {
        let previous_total = self.total_token_usage.total_tokens;
        let delta = (context_window - previous_total).max(0);

        self.model_context_window = Some(context_window);
        self.total_token_usage = TokenUsage {
            total_tokens: context_window,
            ..TokenUsage::default()
        };
        self.last_token_usage = TokenUsage {
            total_tokens: delta,
            ..TokenUsage::default()
        };
    }

    pub fn full_context_window(context_window: i64) -> Self {
        let mut info = Self {
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage::default(),
            model_context_window: Some(context_window),
        };
        info.fill_to_context_window(context_window);
        info
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct TokenCountEvent {
    pub info: Option<TokenUsageInfo>,
    pub rate_limits: Option<RateLimitSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct RateLimitSnapshot {
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub credits: Option<CreditsSnapshot>,
    pub plan_type: Option<crate::account::PlanType>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct RateLimitWindow {
    /// Percentage (0-100) of the window that has been consumed.
    pub used_percent: f64,
    /// Rolling window duration, in minutes.
    #[ts(type = "number | null")]
    pub window_minutes: Option<i64>,
    /// Unix timestamp (seconds since epoch) when the window resets.
    #[ts(type = "number | null")]
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct CreditsSnapshot {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

// Includes prompts, tools and space to call compact.
const BASELINE_TOKENS: i64 = 12000;

impl TokenUsage {
    pub fn is_zero(&self) -> bool {
        self.total_tokens == 0
    }

    pub fn cached_input(&self) -> i64 {
        self.cached_input_tokens.max(0)
    }

    pub fn non_cached_input(&self) -> i64 {
        (self.input_tokens - self.cached_input()).max(0)
    }

    /// Primary count for display as a single absolute value: non-cached input + output.
    pub fn blended_total(&self) -> i64 {
        (self.non_cached_input() + self.output_tokens.max(0)).max(0)
    }

    pub fn tokens_in_context_window(&self) -> i64 {
        self.total_tokens
    }

    /// Estimate the remaining user-controllable percentage of the model's context window.
    ///
    /// `context_window` is the total size of the model's context window.
    /// `BASELINE_TOKENS` should capture tokens that are always present in
    /// the context (e.g., system prompt and fixed tool instructions) so that
    /// the percentage reflects the portion the user can influence.
    ///
    /// This normalizes both the numerator and denominator by subtracting the
    /// baseline, so immediately after the first prompt the UI shows 100% left
    /// and trends toward 0% as the user fills the effective window.
    pub fn percent_of_context_window_remaining(&self, context_window: i64) -> i64 {
        if context_window <= BASELINE_TOKENS {
            return 0;
        }

        let effective_window = context_window - BASELINE_TOKENS;
        let used = (self.tokens_in_context_window() - BASELINE_TOKENS).max(0);
        let remaining = (effective_window - used).max(0);
        ((remaining as f64 / effective_window as f64) * 100.0)
            .clamp(0.0, 100.0)
            .round() as i64
    }

    /// In-place element-wise sum of token counts.
    pub fn add_assign(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.output_tokens += other.output_tokens;
        self.reasoning_output_tokens += other.reasoning_output_tokens;
        self.total_tokens += other.total_tokens;
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct FinalOutput {
    pub token_usage: TokenUsage,
}

impl From<TokenUsage> for FinalOutput {
    fn from(token_usage: TokenUsage) -> Self {
        Self { token_usage }
    }
}

impl fmt::Display for FinalOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token_usage = &self.token_usage;

        write!(
            f,
            "Token usage: total={} input={}{} output={}{}",
            format_with_separators(token_usage.blended_total()),
            format_with_separators(token_usage.non_cached_input()),
            if token_usage.cached_input() > 0 {
                format!(
                    " (+ {} cached)",
                    format_with_separators(token_usage.cached_input())
                )
            } else {
                String::new()
            },
            format_with_separators(token_usage.output_tokens),
            if token_usage.reasoning_output_tokens > 0 {
                format!(
                    " (reasoning {})",
                    format_with_separators(token_usage.reasoning_output_tokens)
                )
            } else {
                String::new()
            }
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentMessageEvent {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct UserMessageEvent {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentMessageDeltaEvent {
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentReasoningEvent {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentReasoningRawContentEvent {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentReasoningRawContentDeltaEvent {
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentReasoningSectionBreakEvent {
    // load with default value so it's backward compatible with the old format.
    #[serde(default)]
    pub item_id: String,
    #[serde(default)]
    pub summary_index: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct AgentReasoningDeltaEvent {
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS, PartialEq)]
pub struct McpInvocation {
    /// Name of the MCP server as defined in the config.
    pub server: String,
    /// Name of the tool as given by the MCP server.
    pub tool: String,
    /// Arguments to the tool call.
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS, PartialEq)]
pub struct McpToolCallBeginEvent {
    /// Identifier so this can be paired with the McpToolCallEnd event.
    pub call_id: String,
    pub invocation: McpInvocation,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS, PartialEq)]
pub struct McpToolCallEndEvent {
    /// Identifier for the corresponding McpToolCallBegin that finished.
    pub call_id: String,
    pub invocation: McpInvocation,
    #[ts(type = "string")]
    pub duration: Duration,
    /// Result of the tool call. Note this could be an error.
    pub result: Result<CallToolResult, String>,
}

impl McpToolCallEndEvent {
    pub fn is_success(&self) -> bool {
        match &self.result {
            Ok(result) => !result.is_error.unwrap_or(false),
            Err(_) => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct WebSearchBeginEvent {
    pub call_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct WebSearchEndEvent {
    pub call_id: String,
    pub query: String,
}

// Conversation kept for backward compatibility.
/// Response payload for `Op::GetHistory` containing the current session's
/// in-memory transcript.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ConversationPathResponseEvent {
    pub conversation_id: ThreadId,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ResumedHistory {
    pub conversation_id: ThreadId,
    pub history: Vec<RolloutItem>,
    pub rollout_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub enum InitialHistory {
    New,
    Resumed(ResumedHistory),
    Forked(Vec<RolloutItem>),
}

impl InitialHistory {
    pub fn get_rollout_items(&self) -> Vec<RolloutItem> {
        match self {
            InitialHistory::New => Vec::new(),
            InitialHistory::Resumed(resumed) => resumed.history.clone(),
            InitialHistory::Forked(items) => items.clone(),
        }
    }

    pub fn get_event_msgs(&self) -> Option<Vec<EventMsg>> {
        match self {
            InitialHistory::New => None,
            InitialHistory::Resumed(resumed) => Some(
                resumed
                    .history
                    .iter()
                    .filter_map(|ri| match ri {
                        RolloutItem::EventMsg(ev) => Some(ev.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
            InitialHistory::Forked(items) => Some(
                items
                    .iter()
                    .filter_map(|ri| match ri {
                        RolloutItem::EventMsg(ev) => Some(ev.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum SessionSource {
    Cli,
    #[default]
    VSCode,
    Exec,
    Mcp,
    SubAgent(SubAgentSource),
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SubAgentSource {
    Review,
    Compact,
    Other(String),
}

impl fmt::Display for SessionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionSource::Cli => f.write_str("cli"),
            SessionSource::VSCode => f.write_str("vscode"),
            SessionSource::Exec => f.write_str("exec"),
            SessionSource::Mcp => f.write_str("mcp"),
            SessionSource::SubAgent(sub_source) => write!(f, "subagent_{sub_source}"),
            SessionSource::Unknown => f.write_str("unknown"),
        }
    }
}

impl fmt::Display for SubAgentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubAgentSource::Review => f.write_str("review"),
            SubAgentSource::Compact => f.write_str("compact"),
            SubAgentSource::Other(other) => f.write_str(other),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, TS)]
pub struct SessionMeta {
    pub id: ThreadId,
    pub timestamp: String,
    pub cwd: PathBuf,
    pub originator: String,
    pub cli_version: String,
    pub instructions: Option<String>,
    #[serde(default)]
    pub source: SessionSource,
    pub model_provider: Option<String>,
}

impl Default for SessionMeta {
    fn default() -> Self {
        SessionMeta {
            id: ThreadId::default(),
            timestamp: String::new(),
            cwd: PathBuf::new(),
            originator: String::new(),
            cli_version: String::new(),
            instructions: None,
            source: SessionSource::default(),
            model_provider: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
pub struct SessionMetaLine {
    #[serde(flatten)]
    pub meta: SessionMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum RolloutItem {
    SessionMeta(SessionMetaLine),
    ResponseItem(ResponseItem),
    Compacted(CompactedItem),
    TurnContext(TurnContextItem),
    EventMsg(EventMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, TS)]
pub struct CompactedItem {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_history: Option<Vec<ResponseItem>>,
}

impl From<CompactedItem> for ResponseItem {
    fn from(value: CompactedItem) -> Self {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: value.message,
            }],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, TS)]
pub struct TurnContextItem {
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffortConfig>,
    pub summary: ReasoningSummaryConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_output_json_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation_policy: Option<TruncationPolicy>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "mode", content = "limit", rename_all = "snake_case")]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct RolloutLine {
    pub timestamp: String,
    #[serde(flatten)]
    pub item: RolloutItem,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, TS)]
pub struct GitInfo {
    /// Current commit hash (SHA)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Current branch name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Repository URL (if available from remote)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
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

    /// Arbitrary instructions provided by the user.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Custom { instructions: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
/// Review request sent to the review session.
pub struct ReviewRequest {
    pub target: ReviewTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub user_facing_hint: Option<String>,
}

/// Structured review result produced by a child review session.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ReviewOutputEvent {
    pub findings: Vec<ReviewFinding>,
    pub overall_correctness: String,
    pub overall_explanation: String,
    pub overall_confidence_score: f32,
}

impl Default for ReviewOutputEvent {
    fn default() -> Self {
        Self {
            findings: Vec::new(),
            overall_correctness: String::default(),
            overall_explanation: String::default(),
            overall_confidence_score: 0.0,
        }
    }
}

/// A single review finding describing an observed issue or recommendation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ReviewFinding {
    pub title: String,
    pub body: String,
    pub confidence_score: f32,
    pub priority: i32,
    pub code_location: ReviewCodeLocation,
}

/// Location of the code related to a review finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ReviewCodeLocation {
    pub absolute_file_path: PathBuf,
    pub line_range: ReviewLineRange,
}

/// Inclusive line range in a file associated with the finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ReviewLineRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, Display, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExecCommandSource {
    Agent,
    UserShell,
    UnifiedExecStartup,
    UnifiedExecInteraction,
}

impl Default for ExecCommandSource {
    fn default() -> Self {
        Self::Agent
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ExecCommandBeginEvent {
    /// Identifier so this can be paired with the ExecCommandEnd event.
    pub call_id: String,
    /// Identifier for the underlying PTY process (when available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub process_id: Option<String>,
    /// Turn ID that this command belongs to.
    pub turn_id: String,
    /// The command to be executed.
    pub command: Vec<String>,
    /// The command's working directory if not the default cwd for the agent.
    pub cwd: PathBuf,
    pub parsed_cmd: Vec<ParsedCommand>,
    /// Where the command originated. Defaults to Agent for backward compatibility.
    #[serde(default)]
    pub source: ExecCommandSource,
    /// Raw input sent to a unified exec session (if this is an interaction event).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interaction_input: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ExecCommandEndEvent {
    /// Identifier for the ExecCommandBegin that finished.
    pub call_id: String,
    /// Identifier for the underlying PTY process (when available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub process_id: Option<String>,
    /// Turn ID that this command belongs to.
    pub turn_id: String,
    /// The command that was executed.
    pub command: Vec<String>,
    /// The command's working directory if not the default cwd for the agent.
    pub cwd: PathBuf,
    pub parsed_cmd: Vec<ParsedCommand>,
    /// Where the command originated. Defaults to Agent for backward compatibility.
    #[serde(default)]
    pub source: ExecCommandSource,
    /// Raw input sent to a unified exec session (if this is an interaction event).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interaction_input: Option<String>,

    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
    /// Captured aggregated output
    #[serde(default)]
    pub aggregated_output: String,
    /// The command's exit code.
    pub exit_code: i32,
    /// The duration of the command execution.
    #[ts(type = "string")]
    pub duration: Duration,
    /// Formatted output from the command, as seen by the model.
    pub formatted_output: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ViewImageToolCallEvent {
    /// Identifier for the originating tool call.
    pub call_id: String,
    /// Local filesystem path provided to the tool.
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExecOutputStream {
    Stdout,
    Stderr,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ExecCommandOutputDeltaEvent {
    /// Identifier for the ExecCommandBegin that produced this chunk.
    pub call_id: String,
    /// Which stream produced this chunk.
    pub stream: ExecOutputStream,
    /// Raw bytes from the stream (may not be valid UTF-8).
    #[serde_as(as = "serde_with::base64::Base64")]
    #[schemars(with = "String")]
    #[ts(type = "string")]
    pub chunk: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct TerminalInteractionEvent {
    /// Identifier for the ExecCommandBegin that produced this chunk.
    pub call_id: String,
    /// Process id associated with the running command.
    pub process_id: String,
    /// Stdin sent to the running session.
    pub stdin: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct BackgroundEventEvent {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct DeprecationNoticeEvent {
    /// Concise summary of what is deprecated.
    pub summary: String,
    /// Optional extra guidance, such as migration steps or rationale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct UndoStartedEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct UndoCompletedEvent {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ThreadRolledBackEvent {
    /// Number of user turns that were removed from context.
    pub num_turns: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct StreamErrorEvent {
    pub message: String,
    #[serde(default)]
    pub codex_error_info: Option<CodexErrorInfo>,
    /// Optional details about the underlying stream failure (often the same
    /// human-readable message that is surfaced as the terminal error if retries
    /// are exhausted).
    #[serde(default)]
    pub additional_details: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct StreamInfoEvent {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PatchApplyBeginEvent {
    /// Identifier so this can be paired with the PatchApplyEnd event.
    pub call_id: String,
    /// Turn ID that this patch belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    /// If true, there was no ApplyPatchApprovalRequest for this patch.
    pub auto_approved: bool,
    /// The changes to be applied.
    pub changes: HashMap<PathBuf, FileChange>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PatchApplyEndEvent {
    /// Identifier for the PatchApplyBegin that finished.
    pub call_id: String,
    /// Turn ID that this patch belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    /// Captured stdout (summary printed by apply_patch).
    pub stdout: String,
    /// Captured stderr (parser errors, IO failures, etc.).
    pub stderr: String,
    /// Whether the patch was applied successfully.
    pub success: bool,
    /// The changes that were applied (mirrors PatchApplyBeginEvent::changes).
    #[serde(default)]
    pub changes: HashMap<PathBuf, FileChange>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct TurnDiffEvent {
    pub unified_diff: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct GetHistoryEntryResponseEvent {
    pub offset: usize,
    pub log_id: u64,
    /// The entry at the requested offset, if available and parseable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<HistoryEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct McpListToolsResponseEvent {
    /// Fully qualified tool name -> tool definition.
    pub tools: std::collections::HashMap<String, McpTool>,
    /// Known resources grouped by server name.
    pub resources: std::collections::HashMap<String, Vec<McpResource>>,
    /// Known resource templates grouped by server name.
    pub resource_templates: std::collections::HashMap<String, Vec<McpResourceTemplate>>,
    /// Authentication status for each configured MCP server.
    pub auth_statuses: std::collections::HashMap<String, McpAuthStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct McpStartupUpdateEvent {
    /// Server name being started.
    pub server: String,
    /// Current startup status.
    pub status: McpStartupStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", tag = "state")]
#[ts(rename_all = "snake_case", tag = "state")]
pub enum McpStartupStatus {
    Starting,
    Ready,
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS, Default)]
pub struct McpStartupCompleteEvent {
    pub ready: Vec<String>,
    pub failed: Vec<McpStartupFailure>,
    pub cancelled: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct McpStartupFailure {
    pub server: String,
    pub error: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum McpAuthStatus {
    Unsupported,
    NotLoggedIn,
    BearerToken,
    OAuth,
}

impl fmt::Display for McpAuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            McpAuthStatus::Unsupported => "Unsupported",
            McpAuthStatus::NotLoggedIn => "Not logged in",
            McpAuthStatus::BearerToken => "Bearer token",
            McpAuthStatus::OAuth => "OAuth",
        };
        f.write_str(text)
    }
}

/// Response payload for `Op::ListCustomPrompts`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ListCustomPromptsResponseEvent {
    pub custom_prompts: Vec<CustomPrompt>,
}

/// Response payload for `Op::ListSkills`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ListSkillsResponseEvent {
    pub skills: Vec<SkillsListEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SkillScope {
    User,
    Repo,
    System,
    Admin,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    pub path: PathBuf,
    pub scope: SkillScope,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct SkillErrorInfo {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct SkillsListEntry {
    pub cwd: PathBuf,
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillErrorInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct SessionConfiguredEvent {
    /// Name left as session_id instead of thread_id for backwards compatibility.
    pub session_id: ThreadId,

    /// Tell the client what model is being queried.
    pub model: String,

    pub model_provider_id: String,

    /// When to escalate for approval for execution
    pub approval_policy: AskForApproval,

    /// How to sandbox commands executed in the system
    pub sandbox_policy: SandboxPolicy,

    /// Working directory that should be treated as the *root* of the
    /// session.
    pub cwd: PathBuf,

    /// The effort the model is putting into reasoning about the user's request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffortConfig>,

    /// Identifier of the history log file (inode on Unix, 0 otherwise).
    pub history_log_id: u64,

    /// Current number of entries in the history log.
    pub history_entry_count: usize,

    /// Optional initial messages (as events) for resumed sessions.
    /// When present, UIs can use these to seed the history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_messages: Option<Vec<EventMsg>>,

    pub rollout_path: PathBuf,
}

/// User's decision in response to an ExecApprovalRequest.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, Display, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    /// User has approved this command and the agent should execute it.
    Approved,

    /// User has approved this command and wants to apply the proposed execpolicy
    /// amendment so future matching commands are permitted.
    ApprovedExecpolicyAmendment {
        proposed_execpolicy_amendment: ExecPolicyAmendment,
    },

    /// User has approved this command and wants to automatically approve any
    /// future identical instances (`command` and `cwd` match exactly) for the
    /// remainder of the session.
    ApprovedForSession,

    /// User has denied this command and the agent should not execute it, but
    /// it should continue the session and try something else.
    #[default]
    Denied,

    /// User has denied this command and the agent should not do anything until
    /// the user's next command.
    Abort,
}

impl ReviewDecision {
    /// Returns an opaque version of the decision without PII. We can't use an ignored flag
    /// on `serde` because the serialization is required by some surfaces.
    pub fn to_opaque_string(&self) -> &'static str {
        match self {
            ReviewDecision::Approved => "approved",
            ReviewDecision::ApprovedExecpolicyAmendment { .. } => "approved_with_amendment",
            ReviewDecision::ApprovedForSession => "approved_for_session",
            ReviewDecision::Denied => "denied",
            ReviewDecision::Abort => "abort",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type")]
pub enum FileChange {
    Add {
        content: String,
    },
    Delete {
        content: String,
    },
    Update {
        unified_diff: String,
        move_path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct Chunk {
    /// 1-based line index of the first line in the original file
    pub orig_index: u32,
    pub deleted_lines: Vec<String>,
    pub inserted_lines: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct TurnAbortedEvent {
    pub reason: TurnAbortReason,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TurnAbortReason {
    Interrupted,
    Replaced,
    ReviewEnded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::UserMessageItem;
    use crate::items::WebSearchItem;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[test]
    fn external_sandbox_reports_full_access_flags() {
        let restricted = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        };
        assert!(restricted.has_full_disk_write_access());
        assert!(!restricted.has_full_network_access());

        let enabled = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        };
        assert!(enabled.has_full_disk_write_access());
        assert!(enabled.has_full_network_access());
    }

    #[test]
    fn item_started_event_from_web_search_emits_begin_event() {
        let event = ItemStartedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".into(),
            item: TurnItem::WebSearch(WebSearchItem {
                id: "search-1".into(),
                query: "find docs".into(),
            }),
        };

        let legacy_events = event.as_legacy_events(false);
        assert_eq!(legacy_events.len(), 1);
        match &legacy_events[0] {
            EventMsg::WebSearchBegin(event) => assert_eq!(event.call_id, "search-1"),
            _ => panic!("expected WebSearchBegin event"),
        }
    }

    #[test]
    fn item_started_event_from_non_web_search_emits_no_legacy_events() {
        let event = ItemStartedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".into(),
            item: TurnItem::UserMessage(UserMessageItem::new(&[])),
        };

        assert!(event.as_legacy_events(false).is_empty());
    }

    #[test]
    fn user_input_serialization_omits_final_output_json_schema_when_none() -> Result<()> {
        let op = Op::UserInput {
            items: Vec::new(),
            final_output_json_schema: None,
        };

        let json_op = serde_json::to_value(op)?;
        assert_eq!(json_op, json!({ "type": "user_input", "items": [] }));

        Ok(())
    }

    #[test]
    fn user_input_deserializes_without_final_output_json_schema_field() -> Result<()> {
        let op: Op = serde_json::from_value(json!({ "type": "user_input", "items": [] }))?;

        assert_eq!(
            op,
            Op::UserInput {
                items: Vec::new(),
                final_output_json_schema: None,
            }
        );

        Ok(())
    }

    #[test]
    fn user_input_serialization_includes_final_output_json_schema_when_some() -> Result<()> {
        let schema = json!({
            "type": "object",
            "properties": {
                "answer": { "type": "string" }
            },
            "required": ["answer"],
            "additionalProperties": false
        });
        let op = Op::UserInput {
            items: Vec::new(),
            final_output_json_schema: Some(schema.clone()),
        };

        let json_op = serde_json::to_value(op)?;
        assert_eq!(
            json_op,
            json!({
                "type": "user_input",
                "items": [],
                "final_output_json_schema": schema,
            })
        );

        Ok(())
    }

    /// Serialize Event to verify that its JSON representation has the expected
    /// amount of nesting.
    #[test]
    fn serialize_event() -> Result<()> {
        let conversation_id = ThreadId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8")?;
        let rollout_file = NamedTempFile::new()?;
        let event = Event {
            id: "1234".to_string(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: conversation_id,
                model: "codex-mini-latest".to_string(),
                model_provider_id: "openai".to_string(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::ReadOnly,
                cwd: PathBuf::from("/home/user/project"),
                reasoning_effort: Some(ReasoningEffortConfig::default()),
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: rollout_file.path().to_path_buf(),
            }),
        };

        let expected = json!({
            "id": "1234",
            "msg": {
                "type": "session_configured",
                "session_id": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                "model": "codex-mini-latest",
                "model_provider_id": "openai",
                "approval_policy": "never",
                "sandbox_policy": {
                    "type": "read-only"
                },
                "cwd": "/home/user/project",
                "reasoning_effort": "medium",
                "history_log_id": 0,
                "history_entry_count": 0,
                "rollout_path": format!("{}", rollout_file.path().display()),
            }
        });
        assert_eq!(expected, serde_json::to_value(&event)?);
        Ok(())
    }

    #[test]
    fn vec_u8_as_base64_serialization_and_deserialization() -> Result<()> {
        let event = ExecCommandOutputDeltaEvent {
            call_id: "call21".to_string(),
            stream: ExecOutputStream::Stdout,
            chunk: vec![1, 2, 3, 4, 5],
        };
        let serialized = serde_json::to_string(&event)?;
        assert_eq!(
            r#"{"call_id":"call21","stream":"stdout","chunk":"AQIDBAU="}"#,
            serialized,
        );

        let deserialized: ExecCommandOutputDeltaEvent = serde_json::from_str(&serialized)?;
        assert_eq!(deserialized, event);
        Ok(())
    }

    #[test]
    fn serialize_mcp_startup_update_event() -> Result<()> {
        let event = Event {
            id: "init".to_string(),
            msg: EventMsg::McpStartupUpdate(McpStartupUpdateEvent {
                server: "srv".to_string(),
                status: McpStartupStatus::Failed {
                    error: "boom".to_string(),
                },
            }),
        };

        let value = serde_json::to_value(&event)?;
        assert_eq!(value["msg"]["type"], "mcp_startup_update");
        assert_eq!(value["msg"]["server"], "srv");
        assert_eq!(value["msg"]["status"]["state"], "failed");
        assert_eq!(value["msg"]["status"]["error"], "boom");
        Ok(())
    }

    #[test]
    fn serialize_mcp_startup_complete_event() -> Result<()> {
        let event = Event {
            id: "init".to_string(),
            msg: EventMsg::McpStartupComplete(McpStartupCompleteEvent {
                ready: vec!["a".to_string()],
                failed: vec![McpStartupFailure {
                    server: "b".to_string(),
                    error: "bad".to_string(),
                }],
                cancelled: vec!["c".to_string()],
            }),
        };

        let value = serde_json::to_value(&event)?;
        assert_eq!(value["msg"]["type"], "mcp_startup_complete");
        assert_eq!(value["msg"]["ready"][0], "a");
        assert_eq!(value["msg"]["failed"][0]["server"], "b");
        assert_eq!(value["msg"]["failed"][0]["error"], "bad");
        assert_eq!(value["msg"]["cancelled"][0], "c");
        Ok(())
    }
}
