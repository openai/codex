use crate::facts::AppInvocation;
use crate::facts::CodexTurnSteerEvent;
use crate::facts::InvocationType;
use crate::facts::PluginState;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use crate::facts::TurnStatus;
use crate::facts::TurnSteerRejectionReason;
use crate::facts::TurnSteerResult;
use codex_app_server_protocol::CodexErrorInfo;
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
    TurnEvent(Box<CodexTurnEventRequest>),
    TurnSteer(CodexTurnSteerEventRequest),
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
pub(crate) struct CodexTurnEventParams {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    pub(crate) product_client_id: String,
    pub(crate) submission_type: Option<crate::facts::TurnSubmissionType>,
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
    pub(crate) num_input_images: usize,
    pub(crate) result: TurnSteerResult,
    pub(crate) rejection_reason: Option<TurnSteerRejectionReason>,
    pub(crate) started_at: u64,
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

pub(crate) fn codex_turn_steer_event_params(
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
    tracking: &TrackEventsContext,
    turn_steer: CodexTurnSteerEvent,
) -> CodexTurnSteerEventParams {
    CodexTurnSteerEventParams {
        thread_id: tracking.thread_id.clone(),
        expected_turn_id: turn_steer.expected_turn_id,
        accepted_turn_id: turn_steer.accepted_turn_id,
        app_server_client,
        runtime,
        num_input_images: turn_steer.num_input_images,
        result: turn_steer.result,
        rejection_reason: turn_steer.rejection_reason,
        started_at: turn_steer.started_at,
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
