use crate::events::AppServerRpcTransport;
use crate::events::CodexRuntimeMetadata;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::SubAgentSource;
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
    Notification(Box<ServerNotification>),
    // Facts that do not naturally exist on the app-server protocol surface, or
    // would require non-trivial protocol reshaping on this branch.
    Custom(CustomAnalyticsFact),
}

pub(crate) enum CustomAnalyticsFact {
    SubAgentThreadStarted(SubAgentThreadStartedInput),
    Compaction(Box<CodexCompactionEvent>),
    GuardianReview(Box<GuardianReviewEventParams>),
    SkillInvoked(SkillInvokedInput),
    AppMentioned(AppMentionedInput),
    AppUsed(AppUsedInput),
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

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewTrigger {
    Shell,
    UnifiedExec,
    Execve,
    ApplyPatch,
    NetworkAccess,
    McpToolCall,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewDecision {
    Approved,
    Denied,
    Aborted,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewTerminalStatus {
    Approved,
    Denied,
    Aborted,
    TimedOut,
    FailedClosed,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewFailureKind {
    Timeout,
    Cancelled,
    PromptBuildError,
    SessionError,
    ParseError,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewSessionKind {
    TrunkSpawned,
    TrunkReused,
    EphemeralForked,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianReviewRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GuardianToolCallCounts {
    pub shell: u64,
    pub unified_exec: u64,
    pub mcp: u64,
    pub dynamic: u64,
    pub apply_patch: u64,
    pub web_search: u64,
    pub image_generation: u64,
    pub view_image: u64,
}

impl GuardianToolCallCounts {
    pub fn total(&self) -> u64 {
        self.shell
            + self.unified_exec
            + self.mcp
            + self.dynamic
            + self.apply_patch
            + self.web_search
            + self.image_generation
            + self.view_image
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuardianReviewedAction {
    Shell {
        command: Vec<String>,
        command_display: String,
        cwd: String,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<PermissionProfile>,
        justification: Option<String>,
    },
    UnifiedExec {
        command: Vec<String>,
        command_display: String,
        cwd: String,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<PermissionProfile>,
        justification: Option<String>,
        tty: bool,
    },
    Execve {
        source: GuardianCommandSource,
        program: String,
        argv: Vec<String>,
        cwd: String,
        additional_permissions: Option<PermissionProfile>,
    },
    ApplyPatch {
        cwd: String,
        files: Vec<String>,
        patch: Option<String>,
    },
    NetworkAccess {
        target: String,
        host: String,
        protocol: NetworkApprovalProtocol,
        port: u16,
    },
    McpToolCall {
        server: String,
        tool_name: String,
        arguments: Option<serde_json::Value>,
        connector_id: Option<String>,
        connector_name: Option<String>,
        tool_title: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianCommandSource {
    Shell,
    UnifiedExec,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuardianReviewEventParams {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    pub target_item_id: String,
    pub product_client_id: Option<String>,
    pub trigger: GuardianReviewTrigger,
    pub retry_reason: Option<String>,
    pub delegated_review: bool,
    pub reviewed_action: GuardianReviewedAction,
    pub reviewed_action_truncated: bool,
    pub decision: GuardianReviewDecision,
    pub terminal_status: GuardianReviewTerminalStatus,
    pub failure_kind: Option<GuardianReviewFailureKind>,
    pub risk_score: Option<u8>,
    pub risk_level: Option<GuardianReviewRiskLevel>,
    pub rationale: Option<String>,
    pub guardian_thread_id: Option<String>,
    pub guardian_session_kind: Option<GuardianReviewSessionKind>,
    pub guardian_model: Option<String>,
    pub guardian_reasoning_effort: Option<String>,
    pub had_prior_review_context: Option<bool>,
    pub review_timeout_ms: u64,
    pub guardian_tool_call_count: u64,
    pub guardian_tool_call_counts: GuardianToolCallCounts,
    pub guardian_time_to_first_token_ms: Option<u64>,
    pub guardian_completion_latency_ms: Option<u64>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}
