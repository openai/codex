use crate::events::AppServerRpcTransport;
use crate::events::CodexRuntimeMetadata;
use crate::events::GuardianReviewEventParams;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone)]
pub struct TrackEventsContext {
    pub model_slug: String,
    pub thread_id: String,
    pub turn_id: String,
}

pub fn build_track_events_context(
    model_slug: String,
    thread_id: String,
    turn_id: String,
) -> TrackEventsContext {
    TrackEventsContext {
        model_slug,
        thread_id,
        turn_id,
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSubmissionType {
    Default,
    Queued,
}

#[derive(Clone)]
pub struct TurnResolvedConfigFact {
    pub turn_id: String,
    pub thread_id: String,
    pub num_input_images: usize,
    pub submission_type: Option<TurnSubmissionType>,
    pub ephemeral: bool,
    pub session_source: SessionSource,
    pub model: String,
    pub model_provider: String,
    pub sandbox_policy: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub service_tier: Option<ServiceTier>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox_network_access: bool,
    pub collaboration_mode: ModeKind,
    pub personality: Option<Personality>,
    pub is_first_turn: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadInitializationMode {
    New,
    Forked,
    Resumed,
}

#[derive(Clone)]
pub struct TurnTokenUsageFact {
    pub turn_id: String,
    pub thread_id: String,
    pub token_usage: TokenUsage,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSteerResult {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSteerRejectionReason {
    NoActiveTurn,
    ExpectedTurnMismatch,
    NonSteerableReview,
    NonSteerableCompact,
    EmptyInput,
    InputTooLarge,
}

#[derive(Clone)]
pub struct CodexTurnSteerEvent {
    pub expected_turn_id: Option<String>,
    pub accepted_turn_id: Option<String>,
    pub num_input_images: usize,
    pub result: TurnSteerResult,
    pub rejection_reason: Option<TurnSteerRejectionReason>,
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum AnalyticsJsonRpcError {
    TurnSteer(TurnSteerRequestError),
    Input(InputError),
}

#[derive(Clone, Copy, Debug)]
pub enum TurnSteerRequestError {
    NoActiveTurn,
    ExpectedTurnMismatch,
    NonSteerableReview,
    NonSteerableCompact,
}

#[derive(Clone, Copy, Debug)]
pub enum InputError {
    Empty,
    TooLarge,
}

impl From<TurnSteerRequestError> for TurnSteerRejectionReason {
    fn from(error: TurnSteerRequestError) -> Self {
        match error {
            TurnSteerRequestError::NoActiveTurn => Self::NoActiveTurn,
            TurnSteerRequestError::ExpectedTurnMismatch => Self::ExpectedTurnMismatch,
            TurnSteerRequestError::NonSteerableReview => Self::NonSteerableReview,
            TurnSteerRequestError::NonSteerableCompact => Self::NonSteerableCompact,
        }
    }
}

impl From<InputError> for TurnSteerRejectionReason {
    fn from(error: InputError) -> Self {
        match error {
            InputError::Empty => Self::EmptyInput,
            InputError::TooLarge => Self::InputTooLarge,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkillInvocation {
    pub skill_name: String,
    pub skill_scope: SkillScope,
    pub skill_path: PathBuf,
    pub invocation_type: InvocationType,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvocationType {
    Explicit,
    Implicit,
}

pub struct AppInvocation {
    pub connector_id: Option<String>,
    pub app_name: Option<String>,
    pub invocation_type: Option<InvocationType>,
}

#[derive(Clone)]
pub struct SubAgentThreadStartedInput {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub product_client_id: String,
    pub client_name: String,
    pub client_version: String,
    pub model: String,
    pub ephemeral: bool,
    pub subagent_source: SubAgentSource,
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    Manual,
    Auto,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionReason {
    UserRequested,
    ContextLimit,
    ModelDownshift,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionImplementation {
    Responses,
    ResponsesCompact,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionPhase {
    StandaloneTurn,
    PreTurn,
    MidTurn,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    Memento,
    PrefixCompaction,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone)]
pub struct CodexCompactionEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub trigger: CompactionTrigger,
    pub reason: CompactionReason,
    pub implementation: CompactionImplementation,
    pub phase: CompactionPhase,
    pub strategy: CompactionStrategy,
    pub status: CompactionStatus,
    pub error: Option<String>,
    pub active_context_tokens_before: i64,
    pub active_context_tokens_after: i64,
    pub started_at: u64,
    pub completed_at: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexResponsesApiCallStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodexResponsesApiItemPhase {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexResponseItemType {
    Message,
    Reasoning,
    LocalShellCall,
    FunctionCall,
    FunctionCallOutput,
    CustomToolCall,
    CustomToolCallOutput,
    ToolSearchCall,
    ToolSearchOutput,
    WebSearchCall,
    ImageGenerationCall,
    GhostSnapshot,
    Compaction,
    Other,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CodexResponsesApiItemMetadata {
    pub item_phase: CodexResponsesApiItemPhase,
    pub item_index: usize,
    pub response_item_type: CodexResponseItemType,
    pub role: Option<String>,
    pub status: Option<String>,
    pub message_phase: Option<MessagePhase>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub payload_bytes: Option<i64>,
    pub text_part_count: Option<usize>,
    pub image_part_count: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct CodexResponsesApiCallInput {
    pub responses_id: Option<String>,
    pub turn_responses_call_index: u64,
    pub status: CodexResponsesApiCallStatus,
    pub error: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub input_items: Vec<ResponseItem>,
    pub output_items: Vec<ResponseItem>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Clone, Debug)]
pub struct CodexResponsesApiCallFact {
    pub thread_id: String,
    pub turn_id: String,
    pub responses_id: Option<String>,
    pub turn_responses_call_index: u64,
    pub status: CodexResponsesApiCallStatus,
    pub error: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub input_item_count: usize,
    pub output_item_count: usize,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub items: Vec<CodexResponsesApiItemMetadata>,
}

#[allow(dead_code)]
pub(crate) enum AnalyticsFact {
    Initialize {
        connection_id: u64,
        params: InitializeParams,
        product_client_id: String,
        runtime: CodexRuntimeMetadata,
        rpc_transport: AppServerRpcTransport,
    },
    Request {
        connection_id: u64,
        request_id: RequestId,
        request: Box<ClientRequest>,
    },
    Response {
        connection_id: u64,
        response: Box<ClientResponse>,
    },
    ErrorResponse {
        connection_id: u64,
        request_id: RequestId,
        error: JSONRPCErrorError,
        error_type: Option<AnalyticsJsonRpcError>,
    },
    Notification(Box<ServerNotification>),
    // Facts that do not naturally exist on the app-server protocol surface, or
    // would require non-trivial protocol reshaping on this branch.
    Custom(CustomAnalyticsFact),
}

pub(crate) enum CustomAnalyticsFact {
    SubAgentThreadStarted(SubAgentThreadStartedInput),
    Compaction(Box<CodexCompactionEvent>),
    ResponsesApiCall(Box<CodexResponsesApiCallFact>),
    GuardianReview(Box<GuardianReviewEventParams>),
    TurnResolvedConfig(Box<TurnResolvedConfigFact>),
    TurnTokenUsage(Box<TurnTokenUsageFact>),
    SkillInvoked(SkillInvokedInput),
    AppMentioned(AppMentionedInput),
    AppUsed(AppUsedInput),
    HookRun(HookRunInput),
    PluginUsed(PluginUsedInput),
    PluginStateChanged(PluginStateChangedInput),
}

pub(crate) struct SkillInvokedInput {
    pub tracking: TrackEventsContext,
    pub invocations: Vec<SkillInvocation>,
}

pub(crate) struct AppMentionedInput {
    pub tracking: TrackEventsContext,
    pub mentions: Vec<AppInvocation>,
}

pub(crate) struct AppUsedInput {
    pub tracking: TrackEventsContext,
    pub app: AppInvocation,
}

pub(crate) struct HookRunInput {
    pub tracking: TrackEventsContext,
    pub hook: HookRunFact,
}

pub struct HookRunFact {
    pub event_name: HookEventName,
    pub hook_source: HookSource,
    pub status: HookRunStatus,
}

pub(crate) struct PluginUsedInput {
    pub tracking: TrackEventsContext,
    pub plugin: PluginTelemetryMetadata,
}

pub(crate) struct PluginStateChangedInput {
    pub plugin: PluginTelemetryMetadata,
    pub state: PluginState,
}

#[derive(Clone, Copy)]
pub(crate) enum PluginState {
    Installed,
    Uninstalled,
    Enabled,
    Disabled,
}
