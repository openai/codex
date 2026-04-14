use crate::facts::AppInvocation;
use crate::facts::InvocationType;
use crate::facts::PluginState;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use codex_login::default_client::originator;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppServerRpcTransport {
    Stdio,
    Websocket,
    InProcess,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadInitializationMode {
    New,
    Forked,
    Resumed,
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
    AppMentioned(CodexAppMentionedEventRequest),
    AppUsed(CodexAppUsedEventRequest),
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
    pub(crate) process_id: Option<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) command_action_count: u64,
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
    pub(crate) receiver_thread_ids: Vec<String>,
    pub(crate) requested_model: Option<String>,
    pub(crate) requested_reasoning_effort: Option<String>,
    pub(crate) agent_state_count: u64,
    pub(crate) completed_agent_count: u64,
    pub(crate) failed_agent_count: u64,
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

pub(crate) fn thread_source_name(thread_source: &SessionSource) -> Option<&'static str> {
    match thread_source {
        SessionSource::Cli | SessionSource::VSCode | SessionSource::Exec => Some("user"),
        SessionSource::SubAgent(_) => Some("subagent"),
        SessionSource::Mcp | SessionSource::Custom(_) | SessionSource::Unknown => None,
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
        parent_thread_id: subagent_parent_thread_id(&input.subagent_source),
        created_at: input.created_at,
    };
    ThreadInitializedEvent {
        event_type: "codex_thread_initialized",
        event_params,
    }
}

fn subagent_source_name(subagent_source: &SubAgentSource) -> String {
    match subagent_source {
        SubAgentSource::Review => "review".to_string(),
        SubAgentSource::Compact => "compact".to_string(),
        SubAgentSource::ThreadSpawn { .. } => "thread_spawn".to_string(),
        SubAgentSource::MemoryConsolidation => "memory_consolidation".to_string(),
        SubAgentSource::Other(other) => other.clone(),
    }
}

fn subagent_parent_thread_id(subagent_source: &SubAgentSource) -> Option<String> {
    match subagent_source {
        SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        } => Some(parent_thread_id.to_string()),
        _ => None,
    }
}
