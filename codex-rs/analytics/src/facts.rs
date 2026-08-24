use crate::events::AppServerRpcTransport;
use crate::events::CodexRuntimeMetadata;
use crate::events::GuardianReviewEventParams;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::error::CodexErr;
pub use codex_protocol::error::CodexErrKind;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use serde::Serialize;
use serde_json::Value; // copybara:strip-for-public
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct TrackEventsContext {
    pub model_slug: String,
    pub thread_id: String,
    pub turn_id: String,
    pub product_client_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactOperationLifecycle {
    Started,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactOperation {
    pub item_id: String,
    pub lifecycle: ArtifactOperationLifecycle,
    pub occurred_at_ms: u64,
    pub plugin_id: String,
    pub script_path: String,
    pub skill: String,
    pub artifact_type: String,
    pub operation_kind: String,
    pub expected_output_count: u32,
    pub output_format: String,
    pub execution_backend: String,
}

#[derive(Clone)]
pub enum CodeModeToolCallFact {
    CellStarted {
        thread_id: String,
        turn_id: String,
        call_id: String,
        cell_id: String,
    },
    ChildStarted {
        thread_id: String,
        turn_id: String,
        call_id: String,
        cell_id: String,
    },
    CellClosed {
        thread_id: String,
        turn_id: String,
        cell_id: String,
    },
    SamplingResponseCompleted {
        thread_id: String,
        turn_id: String,
        response_id: String,
        tool_call_ids: Vec<String>,
    },
    Completed {
        thread_id: String,
        turn_id: String,
        turn_metadata: Arc<dyn TurnAnalyticsMetadata>,
        call_id: String,
        cell_id: Option<String>,
        tool_name: String,
        started_at_ms: u64,
        completed_at_ms: u64,
        status: CodeModeToolCallStatus,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeModeToolCallStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone)]
pub struct ControlToolCallFact {
    pub thread_id: String,
    pub turn_id: String,
    pub turn_metadata: Arc<dyn TurnAnalyticsMetadata>,
    pub call_id: String,
    pub cell_id: Option<String>,
    pub tool_name: String,
    pub started_at_ms: u64,
    pub completed_at_ms: u64,
    pub status: ControlToolCallStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlToolCallStatus {
    Completed,
    Failed,
    Rejected,
    Interrupted,
}
// copybara:strip-for-public begin
pub const INTERNAL_TOOL_INPUT_MAX_COMMANDS: usize = 16;
pub const INTERNAL_TOOL_INPUT_MAX_FLAGS: usize = 32;
pub const INTERNAL_TOOL_INPUT_COMMAND_NAMES: &str =
    "cargo curl docker gh git just kubectl make npm pnpm pytest python python3 rg yarn";
pub const INTERNAL_TOOL_INPUT_SUBCOMMAND_NAMES: &str = concat!(
    "add branch build checkout clean clone commit config describe diff exec fetch get init ",
    "install issue list log login logs merge pr pull push rebase remote reset restore run show ",
    "stash status switch test version view"
);
pub const INTERNAL_TOOL_INPUT_FLAG_NAMES: &str = concat!(
    "--all --api-key --cookie --data --data-binary --data-raw --dry-run --force --header ",
    "--help --json --name-only --password --porcelain --quiet --recursive --request --short ",
    "--stat --token --user --verbose -H -U -X -b -d -f -n -p -q -r -u -v"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InternalToolSourceKind {
    Direct,
    CodeMode,
    UserShell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InternalToolHookOutcome {
    NotApplicable,
    Unchanged,
    Rewritten,
    Blocked,
    RewriteFailed,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InternalToolInputLog {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub tool_call_id: String,
    pub item_id: Option<String>,
    pub tool_name: String,
    pub namespace: Option<String>,
    pub source_kind: InternalToolSourceKind,
    pub arguments_before_hooks: Value,
    pub hook_normalized_input: Option<Value>,
    pub arguments_after_hooks: Option<Value>,
    pub hook_outcome: InternalToolHookOutcome,
    pub code_mode_cell_id: Option<String>,
    pub code_mode_runtime_tool_call_id: Option<String>,
}
// copybara:strip-for-public end

pub fn build_track_events_context(
    model_slug: String,
    thread_id: String,
    turn_id: String,
    product_client_id: String,
) -> TrackEventsContext {
    TrackEventsContext {
        model_slug,
        thread_id,
        turn_id,
        product_client_id,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetailSetting {
    High,
    Original,
}

/// Measurements for one successfully decoded image at the point where Codex prepares it for
/// durable conversation history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImagePreparationMetadata {
    /// Set for images embedded in message content.
    pub message_role: Option<String>,
    /// Set to the originating call ID for tool-output images. This joins to the `item_id` on
    /// existing tool events for tool type and provenance.
    pub item_id: Option<String>,
    pub effective_detail: ImageDetailSetting,
    pub source_width: u32,
    pub source_height: u32,
    pub prepared_width: u32,
    pub prepared_height: u32,
}

#[derive(Clone)]
pub struct ImagePreparationFact {
    pub turn_id: String,
    pub metadata: ImagePreparationMetadata,
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
    pub turn_metadata: Arc<dyn TurnAnalyticsMetadata>,
    pub num_input_images: usize,
    pub submission_type: Option<TurnSubmissionType>,
    pub ephemeral: bool,
    pub session_source: SessionSource,
    pub model: String,
    pub model_provider: String,
    pub permission_profile: PermissionProfile,
    pub permission_profile_cwd: PathBuf,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub service_tier: Option<ServiceTier>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox_network_access: bool,
    pub collaboration_mode: ModeKind,
    pub personality: Option<Personality>,
    pub workspace_kind: Option<String>,
    pub is_first_turn: bool,
}

/// A live, read-only view of a turn's trusted analytics provenance.
///
/// Implementations must return `None` for unknown or ambiguous roots. The reducer
/// reads this when constructing each event because steering can invalidate a root
/// after the turn's configuration has been resolved.
pub trait TurnAnalyticsMetadata: Send + Sync {
    fn root_turn_id(&self) -> Option<String>;
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TurnProfile {
    pub before_first_sampling_ms: u64,
    pub sampling_ms: u64,
    pub compaction_ms: u64,
    pub between_sampling_overhead_ms: u64,
    pub tool_blocking_ms: u64,
    pub after_last_sampling_ms: u64,
    pub sampling_request_count: u32,
    pub sampling_retry_count: u32,
}

#[derive(Clone)]
pub struct TurnProfileFact {
    pub turn_id: String,
    pub profile: TurnProfile,
}

#[derive(Clone)]
pub struct TurnCodexErrorFact {
    pub(crate) turn_id: String,
    pub(crate) thread_id: String,
    pub(crate) error: TurnCodexError,
}

impl TurnCodexErrorFact {
    pub fn from_codex_err(thread_id: String, turn_id: String, error: &CodexErr) -> Self {
        Self {
            turn_id,
            thread_id,
            error: TurnCodexError::from_codex_err(error),
        }
    }
}

#[derive(Clone)]
pub(crate) struct TurnCodexError {
    pub(crate) kind: CodexErrKind,
    pub(crate) http_status_code: Option<u16>,
}

impl TurnCodexError {
    fn from_codex_err(error: &CodexErr) -> Self {
        Self {
            kind: error.into(),
            http_status_code: error.http_status_code_value(),
        }
    }
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
    pub location: SkillInvocationLocation,
    pub plugin_id: Option<String>,
    pub remote_plugin_id: Option<String>,
    pub invocation_type: InvocationType,
}

#[derive(Clone, Debug)]
pub enum SkillInvocationLocation {
    Host {
        path: PathBuf,
        scope: SkillScope,
    },
    Resource {
        id: String,
        skill_id: Option<String>,
        scope: Option<SkillScope>,
    },
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
    pub session_id: String,
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub forked_from_thread_id: Option<String>,
    pub product_client_id: String,
    pub client_name: String,
    pub client_version: String,
    pub model: String,
    pub ephemeral: bool,
    pub thread_source: Option<ThreadSource>,
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
    CompHashChanged,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionImplementation {
    Responses,
    ResponsesCompactionV2,
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
    pub codex_error_kind: Option<CodexErrKind>,
    pub codex_error_http_status_code: Option<u16>,
    pub active_context_tokens_before: i64,
    pub active_context_tokens_after: i64,
    pub retained_image_count: Option<usize>,
    pub compaction_summary_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_write_input_tokens: Option<i64>,
    pub started_at: u64,
    pub completed_at: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalEventKind {
    Created,
    UsageAccounted,
    StatusChanged,
    Cleared,
}

#[derive(Clone)]
pub struct CodexGoalEvent {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub goal_id: String,
    pub event_kind: GoalEventKind,
    pub goal_status: codex_state::ThreadGoalStatus,
    pub has_token_budget: bool,
    pub cumulative_tokens_accounted: Option<i64>,
    pub cumulative_time_accounted_seconds: Option<i64>,
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
    ClientRequest {
        connection_id: u64,
        request_id: RequestId,
        request: Box<ClientRequest>,
    },
    ExplicitClientInterruptRequest {
        connection_id: u64,
        request_id: RequestId,
        turn_id: String,
        requested_at_ms: u64,
    },
    ClientResponse {
        connection_id: u64,
        request_id: RequestId,
        response: Box<ClientResponsePayload>,
        thread_originator: Option<String>,
    },
    ErrorResponse {
        connection_id: u64,
        request_id: RequestId,
        error: JSONRPCErrorError,
        error_type: Option<AnalyticsJsonRpcError>,
    },
    ServerRequest {
        connection_id: u64,
        request: Box<ServerRequest>,
    },
    ServerResponse {
        completed_at_ms: u64,
        response: Box<ServerResponse>,
    },
    EffectivePermissionsApprovalResponse {
        completed_at_ms: u64,
        request_id: RequestId,
        response: Box<RequestPermissionsResponse>,
    },
    ServerRequestAborted {
        completed_at_ms: u64,
        request_id: RequestId,
    },
    Notification(Box<ServerNotification>),
    // Facts that do not naturally exist on the app-server protocol surface, or
    // would require non-trivial protocol reshaping on this branch.
    Custom(CustomAnalyticsFact),
}
// copybara:strip-for-public begin
impl From<InternalToolInputLog> for AnalyticsFact {
    fn from(input: InternalToolInputLog) -> Self {
        Self::Custom(CustomAnalyticsFact::InternalToolInput(Box::new(input)))
    }
}
// copybara:strip-for-public end

pub(crate) enum CustomAnalyticsFact {
    ArtifactOperation(ArtifactOperationInput),
    CodeModeToolCall(CodeModeToolCallFact),
    ControlToolCall(ControlToolCallFact),
    SubAgentThreadStarted(SubAgentThreadStartedInput),
    Compaction(Box<CodexCompactionEvent>),
    Goal(Box<CodexGoalEvent>),
    GuardianReview(Box<GuardianReviewEventParams>),
    TurnResolvedConfig(Box<TurnResolvedConfigFact>),
    TurnTokenUsage(Box<TurnTokenUsageFact>),
    TurnProfile(Box<TurnProfileFact>),
    TurnCodexError(Box<TurnCodexErrorFact>),
    ImagePreparation(Box<ImagePreparationFact>),
    SkillInvoked(SkillInvokedInput),
    AppMentioned(AppMentionedInput),
    AppUsed(AppUsedInput),
    HookRun(HookRunInput),
    PluginUsed(PluginUsedInput),
    PluginInstallRequested(PluginInstallRequestedInput),
    PluginStateChanged(PluginStateChangedInput),
    PluginInstallFailed(PluginInstallFailedInput),
    PluginMeasurements(PluginMeasurementsInput),
    ExternalAgentConfigImportCompleted(ExternalAgentConfigImportCompletedInput),
    ExternalAgentConfigImportFailure(ExternalAgentConfigImportFailureInput),
    // copybara:strip-for-public begin
    InternalToolInput(Box<InternalToolInputLog>),
    // copybara:strip-for-public end
}

pub(crate) struct ArtifactOperationInput {
    pub tracking: TrackEventsContext,
    pub operation: ArtifactOperation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PluginMeasurementRow {
    pub measurement_name: String,
    pub number_value: f64,
    pub dimensions: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PluginMeasurementsInput {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub plugin_id: String,
    pub execution_id: String,
    pub operation: String,
    pub rows: Vec<PluginMeasurementRow>,
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
    pub handler_type: HookHandlerType,
    pub execution_mode: HookExecutionMode,
    pub status: HookRunStatus,
}

pub(crate) struct PluginUsedInput {
    pub tracking: TrackEventsContext,
    pub plugin: PluginTelemetryMetadata,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallRequestSource {
    EndpointRecommendation,
    LegacyDiscovery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInstallRequested {
    pub suggestion_id: String,
    pub plugins: Vec<PluginInstallRequestedPlugin>,
    pub source: PluginInstallRequestSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInstallRequestedPlugin {
    pub plugin_id: String,
    pub remote_plugin_id: Option<String>,
    pub plugin_name: String,
    pub connector_ids: Vec<String>,
}

pub(crate) struct PluginInstallRequestedInput {
    pub tracking: TrackEventsContext,
    pub request: PluginInstallRequested,
}

pub(crate) struct PluginStateChangedInput {
    pub plugin: PluginTelemetryMetadata,
    pub state: PluginState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallSource {
    Manual,
    ExternalAgentMigration,
}

pub(crate) struct PluginInstallFailedInput {
    pub plugin: PluginTelemetryMetadata,
    pub source: PluginInstallSource,
    pub error_type: String,
    pub sub_error_type: Option<String>,
}

pub struct ExternalAgentConfigImportCompletedInput {
    pub import_id: String,
    pub source: String,
    pub provider_id: String,
    pub item_type: String,
    pub success_count: usize,
    pub failed_count: usize,
}

pub struct ExternalAgentConfigImportFailureInput {
    pub import_id: String,
    pub source: String,
    pub provider_id: String,
    pub item_type: String,
    pub failure_stage: String,
    pub error_type: String,
    pub sub_error_type: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum PluginState {
    Installed,
    Uninstalled,
    Enabled,
    Disabled,
}
