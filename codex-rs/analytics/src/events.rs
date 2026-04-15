use crate::facts::AppInvocation;
use crate::facts::CodexCompactionEvent;
use crate::facts::InvocationType;
use crate::facts::PluginState;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::ThreadInitializationMode;
use crate::facts::TrackEventsContext;
use crate::facts::TurnStatus;
use crate::facts::TurnSteerRejectionReason;
use crate::facts::TurnSteerResult;
use crate::facts::TurnSubmissionType;
use codex_app_server_protocol::CodexErrorInfo;
use codex_login::default_client::originator;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::protocol::SubAgentSource;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppServerRpcTransport {
    Stdio,
    Websocket,
    InProcess,
}

#[derive(Serialize)]
pub(crate) struct TrackEventsRequest {
    pub(crate) events: Vec<TrackEventRequest>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum TrackEventRequest {
    SkillInvocation(SkillInvocationEventRequest),
    ThreadInitialized(ThreadInitializedEvent),
    GuardianReview(Box<GuardianReviewEventRequest>),
    AppMentioned(CodexAppMentionedEventRequest),
    AppUsed(CodexAppUsedEventRequest),
    Compaction(Box<CodexCompactionEventRequest>),
    TurnEvent(Box<CodexTurnEventRequest>),
    TurnSteer(CodexTurnSteerEventRequest),
    #[allow(dead_code)]
    CommandExecution(CodexCommandExecutionEventRequest),
    FileChange(CodexFileChangeEventRequest),
    McpToolCall(CodexMcpToolCallEventRequest),
    DynamicToolCall(CodexDynamicToolCallEventRequest),
    CollabAgentToolCall(CodexCollabAgentToolCallEventRequest),
    WebSearch(CodexWebSearchEventRequest),
    ImageGeneration(CodexImageGenerationEventRequest),
    PluginUsed(CodexPluginUsedEventRequest),
    PluginInstalled(CodexPluginEventRequest),
    PluginUninstalled(CodexPluginEventRequest),
    PluginEnabled(CodexPluginEventRequest),
    PluginDisabled(CodexPluginEventRequest),
}

#[derive(Serialize)]
pub(crate) struct SkillInvocationEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) skill_id: String,
    pub(crate) skill_name: String,
    pub(crate) event_params: SkillInvocationEventParams,
}

#[derive(Serialize)]
pub(crate) struct SkillInvocationEventParams {
    pub(crate) product_client_id: Option<String>,
    pub(crate) skill_scope: Option<String>,
    pub(crate) repo_url: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) invoke_type: Option<InvocationType>,
    pub(crate) model_slug: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct CodexAppServerClientMetadata {
    pub(crate) product_client_id: String,
    pub(crate) client_name: Option<String>,
    pub(crate) client_version: Option<String>,
    pub(crate) rpc_transport: AppServerRpcTransport,
    pub(crate) experimental_api_enabled: Option<bool>,
}

#[derive(Clone, Serialize)]
pub(crate) struct CodexRuntimeMetadata {
    pub(crate) codex_rs_version: String,
    pub(crate) runtime_os: String,
    pub(crate) runtime_os_version: String,
    pub(crate) runtime_arch: String,
}

#[derive(Serialize)]
pub(crate) struct ThreadInitializedEventParams {
    pub(crate) thread_id: String,
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    pub(crate) model: String,
    pub(crate) ephemeral: bool,
    pub(crate) thread_source: Option<&'static str>,
    pub(crate) initialization_mode: ThreadInitializationMode,
    pub(crate) subagent_source: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) created_at: u64,
}

#[derive(Serialize)]
pub(crate) struct ThreadInitializedEvent {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: ThreadInitializedEventParams,
}

#[derive(Serialize)]
pub(crate) struct GuardianReviewEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: GuardianReviewEventPayload,
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
pub enum GuardianReviewFailureReason {
    Timeout,
    Cancelled,
    PromptBuildError,
    SessionError,
    ParseError,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewSessionKind {
    TrunkNew,
    TrunkReused,
    EphemeralForked,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianReviewRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianReviewUserAuthorization {
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianReviewOutcome {
    Allow,
    Deny,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianApprovalRequestSource {
    /// Approval requested directly by the main Codex turn.
    MainTurn,
    /// Approval requested by a delegated subagent and routed through the parent
    /// session for guardian review.
    DelegatedSubagent,
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

#[derive(Clone, Serialize)]
pub struct GuardianReviewEventParams {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    pub target_item_id: String,
    pub retry_reason: Option<String>,
    pub approval_request_source: GuardianApprovalRequestSource,
    pub reviewed_action: GuardianReviewedAction,
    pub reviewed_action_truncated: bool,
    pub decision: GuardianReviewDecision,
    pub terminal_status: GuardianReviewTerminalStatus,
    pub failure_reason: Option<GuardianReviewFailureReason>,
    pub risk_level: Option<GuardianReviewRiskLevel>,
    pub user_authorization: Option<GuardianReviewUserAuthorization>,
    pub outcome: Option<GuardianReviewOutcome>,
    pub rationale: Option<String>,
    pub guardian_thread_id: Option<String>,
    pub guardian_session_kind: Option<GuardianReviewSessionKind>,
    pub guardian_model: Option<String>,
    pub guardian_reasoning_effort: Option<String>,
    pub had_prior_review_context: Option<bool>,
    pub review_timeout_ms: u64,
    pub tool_call_count: u64,
    pub time_to_first_token_ms: Option<u64>,
    pub completion_latency_ms: Option<u64>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

#[derive(Serialize)]
pub(crate) struct GuardianReviewEventPayload {
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    #[serde(flatten)]
    pub(crate) guardian_review: GuardianReviewEventParams,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolItemFinalApprovalOutcome {
    NotNeeded,
    ConfigAllowed,
    PolicyForbidden,
    GuardianApproved,
    GuardianDenied,
    GuardianAborted,
    UserApproved,
    UserApprovedForSession,
    UserDenied,
    UserAborted,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolItemTerminalStatus {
    Completed,
    Failed,
    Rejected,
    Interrupted,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolItemFailureKind {
    ToolError,
    ApprovalDenied,
    ApprovalAborted,
    SandboxDenied,
    PolicyForbidden,
}

#[derive(Serialize)]
pub(crate) struct CodexToolItemEventBase {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    /// App-server ThreadItem.id. For tool-originated items this generally
    /// corresponds to the originating core call_id.
    pub(crate) item_id: String,
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    pub(crate) thread_source: Option<&'static str>,
    pub(crate) subagent_source: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) tool_name: String,
    pub(crate) started_at: u64,
    pub(crate) completed_at: Option<u64>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) execution_started: bool,
    pub(crate) review_count: u64,
    pub(crate) guardian_review_count: u64,
    pub(crate) user_review_count: u64,
    pub(crate) final_approval_outcome: ToolItemFinalApprovalOutcome,
    pub(crate) terminal_status: ToolItemTerminalStatus,
    pub(crate) failure_kind: Option<ToolItemFailureKind>,
    pub(crate) requested_additional_permissions: bool,
    pub(crate) requested_network_access: bool,
    pub(crate) retry_count: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandExecutionFamily {
    Shell,
    UserShell,
    UnifiedExec,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandExecutionSourceKind {
    Agent,
    UserShell,
    UnifiedExecStartup,
    UnifiedExecInteraction,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollabAgentToolKind {
    SpawnAgent,
    SendInput,
    ResumeAgent,
    Wait,
    CloseAgent,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WebSearchActionKind {
    Search,
    OpenPage,
    FindInPage,
    Other,
}

#[derive(Serialize)]
pub(crate) struct CodexCommandExecutionEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) command_execution_source: CommandExecutionSourceKind,
    pub(crate) command_execution_family: CommandExecutionFamily,
    pub(crate) exit_code: Option<i32>,
    pub(crate) command_action_count: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct CodexCommandExecutionEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexCommandExecutionEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexFileChangeEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) file_change_count: u64,
    pub(crate) file_add_count: u64,
    pub(crate) file_update_count: u64,
    pub(crate) file_delete_count: u64,
    pub(crate) file_move_count: u64,
}

#[derive(Serialize)]
pub(crate) struct CodexFileChangeEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexFileChangeEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexMcpToolCallEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) mcp_server_name: String,
    pub(crate) mcp_tool_name: String,
    pub(crate) mcp_error_present: bool,
    pub(crate) mcp_error_code: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CodexMcpToolCallEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexMcpToolCallEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexDynamicToolCallEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) dynamic_tool_name: String,
    pub(crate) success: Option<bool>,
    pub(crate) output_content_item_count: Option<u64>,
    pub(crate) output_text_item_count: Option<u64>,
    pub(crate) output_image_item_count: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct CodexDynamicToolCallEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexDynamicToolCallEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexCollabAgentToolCallEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) collab_agent_tool: CollabAgentToolKind,
    pub(crate) sender_thread_id: String,
    pub(crate) receiver_thread_count: u64,
    pub(crate) receiver_thread_ids: Option<Vec<String>>,
    pub(crate) requested_model: Option<String>,
    pub(crate) requested_reasoning_effort: Option<String>,
    pub(crate) agent_state_count: Option<u64>,
    pub(crate) completed_agent_count: Option<u64>,
    pub(crate) failed_agent_count: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct CodexCollabAgentToolCallEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexCollabAgentToolCallEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexWebSearchEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) web_search_action: Option<WebSearchActionKind>,
    pub(crate) query_present: bool,
    pub(crate) query_count: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct CodexWebSearchEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexWebSearchEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexImageGenerationEventParams {
    #[serde(flatten)]
    pub(crate) base: CodexToolItemEventBase,
    pub(crate) image_generation_status: String,
    pub(crate) revised_prompt_present: bool,
    pub(crate) saved_path_present: bool,
}

#[derive(Serialize)]
pub(crate) struct CodexImageGenerationEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexImageGenerationEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexAppMetadata {
    pub(crate) connector_id: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) turn_id: Option<String>,
    pub(crate) app_name: Option<String>,
    pub(crate) product_client_id: Option<String>,
    pub(crate) invoke_type: Option<InvocationType>,
    pub(crate) model_slug: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CodexAppMentionedEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexAppMetadata,
}

#[derive(Serialize)]
pub(crate) struct CodexAppUsedEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexAppMetadata,
}

#[derive(Serialize)]
pub(crate) struct CodexCompactionEventParams {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    pub(crate) thread_source: Option<&'static str>,
    pub(crate) subagent_source: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) trigger: crate::facts::CompactionTrigger,
    pub(crate) reason: crate::facts::CompactionReason,
    pub(crate) implementation: crate::facts::CompactionImplementation,
    pub(crate) phase: crate::facts::CompactionPhase,
    pub(crate) strategy: crate::facts::CompactionStrategy,
    pub(crate) status: crate::facts::CompactionStatus,
    pub(crate) error: Option<String>,
    pub(crate) active_context_tokens_before: i64,
    pub(crate) active_context_tokens_after: i64,
    pub(crate) started_at: u64,
    pub(crate) completed_at: u64,
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct CodexCompactionEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexCompactionEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexTurnEventParams {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    // TODO(rhan-oai): Populate once queued/default submission type is plumbed from
    // the turn/start callsites instead of always being reported as None.
    pub(crate) submission_type: Option<TurnSubmissionType>,
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    pub(crate) ephemeral: bool,
    pub(crate) thread_source: Option<String>,
    pub(crate) initialization_mode: ThreadInitializationMode,
    pub(crate) subagent_source: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) model_provider: String,
    pub(crate) sandbox_policy: Option<&'static str>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) reasoning_summary: Option<String>,
    pub(crate) service_tier: String,
    pub(crate) approval_policy: String,
    pub(crate) approvals_reviewer: String,
    pub(crate) sandbox_network_access: bool,
    pub(crate) collaboration_mode: Option<&'static str>,
    pub(crate) personality: Option<String>,
    pub(crate) num_input_images: usize,
    pub(crate) is_first_turn: bool,
    pub(crate) status: Option<TurnStatus>,
    pub(crate) turn_error: Option<CodexErrorInfo>,
    pub(crate) steer_count: Option<usize>,
    // TODO(rhan-oai): Populate these once tool-call accounting is emitted from
    // core; the schema is reserved but these fields are currently always None.
    pub(crate) total_tool_call_count: Option<usize>,
    pub(crate) shell_command_count: Option<usize>,
    pub(crate) file_change_count: Option<usize>,
    pub(crate) mcp_tool_call_count: Option<usize>,
    pub(crate) dynamic_tool_call_count: Option<usize>,
    pub(crate) subagent_tool_call_count: Option<usize>,
    pub(crate) web_search_count: Option<usize>,
    pub(crate) image_generation_count: Option<usize>,
    pub(crate) input_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) reasoning_output_tokens: Option<i64>,
    pub(crate) total_tokens: Option<i64>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) started_at: Option<u64>,
    pub(crate) completed_at: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct CodexTurnEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexTurnEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexTurnSteerEventParams {
    pub(crate) thread_id: String,
    pub(crate) expected_turn_id: Option<String>,
    pub(crate) accepted_turn_id: Option<String>,
    pub(crate) app_server_client: CodexAppServerClientMetadata,
    pub(crate) runtime: CodexRuntimeMetadata,
    pub(crate) thread_source: Option<String>,
    pub(crate) subagent_source: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) num_input_images: usize,
    pub(crate) result: TurnSteerResult,
    pub(crate) rejection_reason: Option<TurnSteerRejectionReason>,
    pub(crate) created_at: u64,
}

#[derive(Serialize)]
pub(crate) struct CodexTurnSteerEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexTurnSteerEventParams,
}

#[derive(Serialize)]
pub(crate) struct CodexPluginMetadata {
    pub(crate) plugin_id: Option<String>,
    pub(crate) plugin_name: Option<String>,
    pub(crate) marketplace_name: Option<String>,
    pub(crate) has_skills: Option<bool>,
    pub(crate) mcp_server_count: Option<usize>,
    pub(crate) connector_ids: Option<Vec<String>>,
    pub(crate) product_client_id: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CodexPluginUsedMetadata {
    #[serde(flatten)]
    pub(crate) plugin: CodexPluginMetadata,
    pub(crate) thread_id: Option<String>,
    pub(crate) turn_id: Option<String>,
    pub(crate) model_slug: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CodexPluginEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexPluginMetadata,
}

#[derive(Serialize)]
pub(crate) struct CodexPluginUsedEventRequest {
    pub(crate) event_type: &'static str,
    pub(crate) event_params: CodexPluginUsedMetadata,
}

pub(crate) fn plugin_state_event_type(state: PluginState) -> &'static str {
    match state {
        PluginState::Installed => "codex_plugin_installed",
        PluginState::Uninstalled => "codex_plugin_uninstalled",
        PluginState::Enabled => "codex_plugin_enabled",
        PluginState::Disabled => "codex_plugin_disabled",
    }
}

pub(crate) fn codex_app_metadata(
    tracking: &TrackEventsContext,
    app: AppInvocation,
) -> CodexAppMetadata {
    CodexAppMetadata {
        connector_id: app.connector_id,
        thread_id: Some(tracking.thread_id.clone()),
        turn_id: Some(tracking.turn_id.clone()),
        app_name: app.app_name,
        product_client_id: Some(originator().value),
        invoke_type: app.invocation_type,
        model_slug: Some(tracking.model_slug.clone()),
    }
}

pub(crate) fn codex_plugin_metadata(plugin: PluginTelemetryMetadata) -> CodexPluginMetadata {
    let capability_summary = plugin.capability_summary;
    CodexPluginMetadata {
        plugin_id: Some(plugin.plugin_id.as_key()),
        plugin_name: Some(plugin.plugin_id.plugin_name),
        marketplace_name: Some(plugin.plugin_id.marketplace_name),
        has_skills: capability_summary
            .as_ref()
            .map(|summary| summary.has_skills),
        mcp_server_count: capability_summary
            .as_ref()
            .map(|summary| summary.mcp_server_names.len()),
        connector_ids: capability_summary.map(|summary| {
            summary
                .app_connector_ids
                .into_iter()
                .map(|connector_id| connector_id.0)
                .collect()
        }),
        product_client_id: Some(originator().value),
    }
}

pub(crate) fn codex_compaction_event_params(
    input: CodexCompactionEvent,
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
    thread_source: Option<&'static str>,
    subagent_source: Option<String>,
    parent_thread_id: Option<String>,
) -> CodexCompactionEventParams {
    CodexCompactionEventParams {
        thread_id: input.thread_id,
        turn_id: input.turn_id,
        app_server_client,
        runtime,
        thread_source,
        subagent_source,
        parent_thread_id,
        trigger: input.trigger,
        reason: input.reason,
        implementation: input.implementation,
        phase: input.phase,
        strategy: input.strategy,
        status: input.status,
        error: input.error,
        active_context_tokens_before: input.active_context_tokens_before,
        active_context_tokens_after: input.active_context_tokens_after,
        started_at: input.started_at,
        completed_at: input.completed_at,
        duration_ms: input.duration_ms,
    }
}

pub(crate) fn codex_plugin_used_metadata(
    tracking: &TrackEventsContext,
    plugin: PluginTelemetryMetadata,
) -> CodexPluginUsedMetadata {
    CodexPluginUsedMetadata {
        plugin: codex_plugin_metadata(plugin),
        thread_id: Some(tracking.thread_id.clone()),
        turn_id: Some(tracking.turn_id.clone()),
        model_slug: Some(tracking.model_slug.clone()),
    }
}

pub(crate) fn current_runtime_metadata() -> CodexRuntimeMetadata {
    let os_info = os_info::get();
    CodexRuntimeMetadata {
        codex_rs_version: env!("CARGO_PKG_VERSION").to_string(),
        runtime_os: std::env::consts::OS.to_string(),
        runtime_os_version: os_info.version().to_string(),
        runtime_arch: std::env::consts::ARCH.to_string(),
    }
}

pub(crate) fn subagent_thread_started_event_request(
    input: SubAgentThreadStartedInput,
) -> ThreadInitializedEvent {
    let event_params = ThreadInitializedEventParams {
        thread_id: input.thread_id,
        app_server_client: CodexAppServerClientMetadata {
            product_client_id: input.product_client_id,
            client_name: Some(input.client_name),
            client_version: Some(input.client_version),
            rpc_transport: AppServerRpcTransport::InProcess,
            experimental_api_enabled: None,
        },
        runtime: current_runtime_metadata(),
        model: input.model,
        ephemeral: input.ephemeral,
        thread_source: Some("subagent"),
        initialization_mode: ThreadInitializationMode::New,
        subagent_source: Some(subagent_source_name(&input.subagent_source)),
        parent_thread_id: input
            .parent_thread_id
            .or_else(|| subagent_parent_thread_id(&input.subagent_source)),
        created_at: input.created_at,
    };
    ThreadInitializedEvent {
        event_type: "codex_thread_initialized",
        event_params,
    }
}

pub(crate) fn subagent_source_name(subagent_source: &SubAgentSource) -> String {
    match subagent_source {
        SubAgentSource::Review => "review".to_string(),
        SubAgentSource::Compact => "compact".to_string(),
        SubAgentSource::ThreadSpawn { .. } => "thread_spawn".to_string(),
        SubAgentSource::MemoryConsolidation => "memory_consolidation".to_string(),
        SubAgentSource::Other(other) => other.clone(),
    }
}

pub(crate) fn subagent_parent_thread_id(subagent_source: &SubAgentSource) -> Option<String> {
    match subagent_source {
        SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        } => Some(parent_thread_id.to_string()),
        _ => None,
    }
}
