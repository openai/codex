use std::collections::HashMap;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::Verbosity;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::ByteRange as CoreByteRange;
use codex_protocol::user_input::TextElement as CoreTextElement;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

// Reuse shared types defined in `common.rs`.
use crate::protocol::common::AuthMode;
use crate::protocol::common::GitSha;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_info: ClientInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<InitializeCapabilities>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub title: Option<String>,
    pub version: String,
}

/// Client-declared capabilities negotiated during initialize.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeCapabilities {
    /// Opt into receiving experimental API methods and fields.
    #[serde(default)]
    pub experimental_api: bool,
    /// Exact notification method names that should be suppressed for this
    /// connection (for example `codex/event/session_configured`).
    #[ts(optional = nullable)]
    pub opt_out_notification_methods: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub user_agent: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct NewConversationParams {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub profile: Option<String>,
    pub cwd: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox: Option<SandboxMode>,
    pub config: Option<HashMap<String, serde_json::Value>>,
    pub base_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_prompt: Option<String>,
    pub include_apply_patch_tool: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct NewConversationResponse {
    pub conversation_id: ThreadId,
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResumeConversationResponse {
    pub conversation_id: ThreadId,
    pub model: String,
    pub initial_messages: Option<Vec<EventMsg>>,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ForkConversationResponse {
    pub conversation_id: ThreadId,
    pub model: String,
    pub initial_messages: Option<Vec<EventMsg>>,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(untagged)]
pub enum GetConversationSummaryParams {
    RolloutPath {
        #[serde(rename = "rolloutPath")]
        rollout_path: PathBuf,
    },
    ThreadId {
        #[serde(rename = "conversationId")]
        conversation_id: ThreadId,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetConversationSummaryResponse {
    pub summary: ConversationSummary,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsParams {
    pub page_size: Option<usize>,
    pub cursor: Option<String>,
    pub model_providers: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub conversation_id: ThreadId,
    pub path: PathBuf,
    pub preview: String,
    pub timestamp: Option<String>,
    pub updated_at: Option<String>,
    pub model_provider: String,
    pub cwd: PathBuf,
    pub cli_version: String,
    pub source: SessionSource,
    pub git_info: Option<ConversationGitInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub struct ConversationGitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsResponse {
    pub items: Vec<ConversationSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResumeConversationParams {
    pub path: Option<PathBuf>,
    pub conversation_id: Option<ThreadId>,
    pub history: Option<Vec<ResponseItem>>,
    pub overrides: Option<NewConversationParams>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ForkConversationParams {
    pub path: Option<PathBuf>,
    pub conversation_id: Option<ThreadId>,
    pub overrides: Option<NewConversationParams>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddConversationSubscriptionResponse {
    #[schemars(with = "String")]
    pub subscription_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveConversationParams {
    pub conversation_id: ThreadId,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveConversationResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoveConversationSubscriptionResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginApiKeyParams {
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginApiKeyResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginChatGptResponse {
    #[schemars(with = "String")]
    pub login_id: Uuid,
    pub auth_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteResponse {
    pub sha: GitSha,
    pub diff: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPatchApprovalParams {
    pub conversation_id: ThreadId,
    /// Use to correlate this with [codex_protocol::protocol::PatchApplyBeginEvent]
    /// and [codex_protocol::protocol::PatchApplyEndEvent].
    pub call_id: String,
    pub file_changes: HashMap<PathBuf, FileChange>,
    /// Optional explanatory reason (e.g. request for extra write access).
    pub reason: Option<String>,
    /// When set, the agent is asking the user to allow writes under this root
    /// for the remainder of the session (unclear if this is honored today).
    pub grant_root: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPatchApprovalResponse {
    pub decision: ReviewDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecCommandApprovalParams {
    pub conversation_id: ThreadId,
    /// Use to correlate this with [codex_protocol::protocol::ExecCommandBeginEvent]
    /// and [codex_protocol::protocol::ExecCommandEndEvent].
    pub call_id: String,
    /// Identifier for this specific approval callback.
    pub approval_id: Option<String>,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub reason: Option<String>,
    pub parsed_cmd: Vec<ParsedCommand>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct ExecCommandApprovalResponse {
    pub decision: ReviewDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelLoginChatGptParams {
    #[schemars(with = "String")]
    pub login_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteParams {
    pub cwd: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelLoginChatGptResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogoutChatGptParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogoutChatGptResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusParams {
    pub include_token: Option<bool>,
    pub refresh_token: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecOneOffCommandParams {
    pub command: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub cwd: Option<PathBuf>,
    pub sandbox_policy: Option<SandboxPolicy>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecOneOffCommandResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusResponse {
    pub auth_method: Option<AuthMode>,
    pub auth_token: Option<String>,
    pub requires_openai_auth: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUserAgentResponse {
    pub user_agent: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoResponse {
    pub alleged_user_email: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUserSavedConfigResponse {
    pub config: UserSavedConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetDefaultModelParams {
    pub model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetDefaultModelResponse {}

#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserSavedConfig {
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub sandbox_settings: Option<SandboxSettings>,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub model: Option<String>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub tools: Option<Tools>,
    pub profile: Option<String>,
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub chatgpt_base_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub web_search: Option<bool>,
    pub view_image: Option<bool>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SandboxSettings {
    #[serde(default)]
    pub writable_roots: Vec<AbsolutePathBuf>,
    pub network_access: Option<bool>,
    pub exclude_tmpdir_env_var: Option<bool>,
    pub exclude_slash_tmp: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageParams {
    pub conversation_id: ThreadId,
    pub items: Vec<InputItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserTurnParams {
    pub conversation_id: ThreadId,
    pub items: Vec<InputItem>,
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub model: String,
    pub effort: Option<ReasoningEffort>,
    pub summary: ReasoningSummary,
    /// Optional JSON Schema used to constrain the final assistant message for this turn.
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserTurnResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InterruptConversationParams {
    pub conversation_id: ThreadId,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct InterruptConversationResponse {
    pub abort_reason: TurnAbortReason,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddConversationListenerParams {
    pub conversation_id: ThreadId,
    #[serde(default)]
    pub experimental_raw_events: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoveConversationListenerParams {
    #[schemars(with = "String")]
    pub subscription_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "data")]
pub enum InputItem {
    Text {
        text: String,
        /// UI-defined spans within `text` used to render or persist special elements.
        #[serde(default)]
        text_elements: Vec<V1TextElement>,
    },
    Image {
        image_url: String,
    },
    LocalImage {
        path: PathBuf,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "ByteRange")]
pub struct V1ByteRange {
    /// Start byte offset (inclusive) within the UTF-8 text buffer.
    pub start: usize,
    /// End byte offset (exclusive) within the UTF-8 text buffer.
    pub end: usize,
}

impl From<CoreByteRange> for V1ByteRange {
    fn from(value: CoreByteRange) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

impl From<V1ByteRange> for CoreByteRange {
    fn from(value: V1ByteRange) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "TextElement")]
pub struct V1TextElement {
    /// Byte range in the parent `text` buffer that this element occupies.
    pub byte_range: V1ByteRange,
    /// Optional human-readable placeholder for the element, displayed in the UI.
    pub placeholder: Option<String>,
}

impl From<CoreTextElement> for V1TextElement {
    fn from(value: CoreTextElement) -> Self {
        Self {
            byte_range: value.byte_range.into(),
            placeholder: value._placeholder_for_conversion_only().map(str::to_string),
        }
    }
}

impl From<V1TextElement> for CoreTextElement {
    fn from(value: V1TextElement) -> Self {
        Self::new(value.byte_range.into(), value.placeholder)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
/// Deprecated in favor of AccountLoginCompletedNotification.
pub struct LoginChatGptCompleteNotification {
    #[schemars(with = "String")]
    pub login_id: Uuid,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfiguredNotification {
    pub session_id: ThreadId,
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub history_log_id: u64,
    #[ts(type = "number")]
    pub history_entry_count: usize,
    pub initial_messages: Option<Vec<EventMsg>>,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
/// Deprecated notification. Use AccountUpdatedNotification instead.
pub struct AuthStatusChangeNotification {
    pub auth_method: Option<AuthMode>,
}
