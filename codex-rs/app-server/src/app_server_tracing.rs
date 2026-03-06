//! Tracing helpers shared by socket and in-process app-server entry points.
//!
//! The in-process path intentionally reuses the same span shape as JSON-RPC
//! transports so request telemetry stays comparable across stdio, websocket,
//! and embedded callers. [`typed_request_span`] is the in-process counterpart
//! of [`request_span`] and stamps `rpc.transport` as `"in-process"` while
//! deriving client identity from the typed [`ClientRequest`] rather than
//! from a parsed JSON envelope.

use crate::message_processor::ConnectionSessionState;
use crate::outgoing_message::ConnectionId;
use crate::transport::AppServerTransport;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_otel::set_parent_from_context;
use codex_otel::set_parent_from_w3c_trace_context;
use codex_otel::traceparent_context_from_env;
use codex_protocol::protocol::W3cTraceContext;
use tracing::Span;
use tracing::field;
use tracing::info_span;

pub(crate) fn request_span(
    request: &JSONRPCRequest,
    transport: AppServerTransport,
    connection_id: ConnectionId,
    session: &ConnectionSessionState,
) -> Span {
    let span = info_span!(
        "app_server.request",
        otel.kind = "server",
        otel.name = request.method.as_str(),
        rpc.system = "jsonrpc",
        rpc.method = request.method.as_str(),
        rpc.transport = transport_name(transport),
        rpc.request_id = ?request.id,
        app_server.connection_id = ?connection_id,
        app_server.api_version = "v2",
        app_server.client_name = field::Empty,
        app_server.client_version = field::Empty,
    );

    let initialize_client_info = initialize_client_info(request);
    if let Some(client_name) = client_name(initialize_client_info.as_ref(), session) {
        span.record("app_server.client_name", client_name);
    }
    if let Some(client_version) = client_version(initialize_client_info.as_ref(), session) {
        span.record("app_server.client_version", client_version);
    }

    if let Some(traceparent) = request
        .trace
        .as_ref()
        .and_then(|trace| trace.traceparent.as_deref())
    {
        let trace = W3cTraceContext {
            traceparent: Some(traceparent.to_string()),
            tracestate: request
                .trace
                .as_ref()
                .and_then(|value| value.tracestate.clone()),
        };
        if !set_parent_from_w3c_trace_context(&span, &trace) {
            tracing::warn!(
                rpc_method = request.method.as_str(),
                rpc_request_id = ?request.id,
                "ignoring invalid inbound request trace carrier"
            );
        }
    } else if let Some(context) = traceparent_context_from_env() {
        set_parent_from_context(&span, context);
    }

    span
}

/// Builds tracing span metadata for typed in-process requests.
///
/// This mirrors `request_span` semantics while stamping transport as
/// `in-process` and deriving client info either from initialize params or
/// from existing connection session state.
pub(crate) fn typed_request_span(
    request: &ClientRequest,
    connection_id: ConnectionId,
    session: &ConnectionSessionState,
) -> Span {
    let span = info_span!(
        "app_server.request",
        otel.kind = "server",
        otel.name = "in_process",
        rpc.system = "jsonrpc",
        rpc.method = "in_process",
        rpc.transport = "in-process",
        rpc.request_id = ?client_request_id(request),
        app_server.connection_id = ?connection_id,
        app_server.api_version = "v2",
        app_server.client_name = field::Empty,
        app_server.client_version = field::Empty,
    );

    if let Some((client_name, client_version)) = initialize_client_info_from_typed_request(request)
    {
        span.record("app_server.client_name", client_name);
        span.record("app_server.client_version", client_version);
    } else {
        if let Some(client_name) = session.app_server_client_name.as_deref() {
            span.record("app_server.client_name", client_name);
        }
        if let Some(client_version) = session.client_version.as_deref() {
            span.record("app_server.client_version", client_version);
        }
    }

    if let Some(context) = traceparent_context_from_env() {
        set_parent_from_context(&span, context);
    }

    span
}

fn transport_name(transport: AppServerTransport) -> &'static str {
    match transport {
        AppServerTransport::Stdio => "stdio",
        AppServerTransport::WebSocket { .. } => "websocket",
    }
}

fn client_name<'a>(
    initialize_client_info: Option<&'a InitializeParams>,
    session: &'a ConnectionSessionState,
) -> Option<&'a str> {
    if let Some(params) = initialize_client_info {
        return Some(params.client_info.name.as_str());
    }
    session.app_server_client_name.as_deref()
}

fn client_version<'a>(
    initialize_client_info: Option<&'a InitializeParams>,
    session: &'a ConnectionSessionState,
) -> Option<&'a str> {
    if let Some(params) = initialize_client_info {
        return Some(params.client_info.version.as_str());
    }
    session.client_version.as_deref()
}

fn initialize_client_info(request: &JSONRPCRequest) -> Option<InitializeParams> {
    if request.method != "initialize" {
        return None;
    }
    let params = request.params.clone()?;
    serde_json::from_value(params).ok()
}

fn initialize_client_info_from_typed_request(request: &ClientRequest) -> Option<(&str, &str)> {
    match request {
        ClientRequest::Initialize { params, .. } => Some((
            params.client_info.name.as_str(),
            params.client_info.version.as_str(),
        )),
        _ => None,
    }
}

/// Extracts the request id from a typed in-process request.
///
/// This duplicates the `ClientRequest` variant match locally so the protocol
/// crate does not need an in-process-specific helper just for tracing.
pub(crate) fn client_request_id(request: &ClientRequest) -> &RequestId {
    match request {
        ClientRequest::Initialize { request_id, .. }
        | ClientRequest::ThreadStart { request_id, .. }
        | ClientRequest::ThreadResume { request_id, .. }
        | ClientRequest::ThreadFork { request_id, .. }
        | ClientRequest::ThreadArchive { request_id, .. }
        | ClientRequest::ThreadUnsubscribe { request_id, .. }
        | ClientRequest::ThreadSetName { request_id, .. }
        | ClientRequest::ThreadMetadataUpdate { request_id, .. }
        | ClientRequest::ThreadUnarchive { request_id, .. }
        | ClientRequest::ThreadCompactStart { request_id, .. }
        | ClientRequest::ThreadBackgroundTerminalsClean { request_id, .. }
        | ClientRequest::ThreadRollback { request_id, .. }
        | ClientRequest::ThreadList { request_id, .. }
        | ClientRequest::ThreadLoadedList { request_id, .. }
        | ClientRequest::ThreadRead { request_id, .. }
        | ClientRequest::SkillsList { request_id, .. }
        | ClientRequest::SkillsRemoteList { request_id, .. }
        | ClientRequest::SkillsRemoteExport { request_id, .. }
        | ClientRequest::AppsList { request_id, .. }
        | ClientRequest::SkillsConfigWrite { request_id, .. }
        | ClientRequest::PluginList { request_id, .. }
        | ClientRequest::PluginInstall { request_id, .. }
        | ClientRequest::TurnStart { request_id, .. }
        | ClientRequest::TurnSteer { request_id, .. }
        | ClientRequest::TurnInterrupt { request_id, .. }
        | ClientRequest::ThreadRealtimeStart { request_id, .. }
        | ClientRequest::ThreadRealtimeAppendAudio { request_id, .. }
        | ClientRequest::ThreadRealtimeAppendText { request_id, .. }
        | ClientRequest::ThreadRealtimeStop { request_id, .. }
        | ClientRequest::ReviewStart { request_id, .. }
        | ClientRequest::ModelList { request_id, .. }
        | ClientRequest::ExperimentalFeatureList { request_id, .. }
        | ClientRequest::CollaborationModeList { request_id, .. }
        | ClientRequest::MockExperimentalMethod { request_id, .. }
        | ClientRequest::McpServerOauthLogin { request_id, .. }
        | ClientRequest::McpServerRefresh { request_id, .. }
        | ClientRequest::McpServerStatusList { request_id, .. }
        | ClientRequest::WindowsSandboxSetupStart { request_id, .. }
        | ClientRequest::LoginAccount { request_id, .. }
        | ClientRequest::CancelLoginAccount { request_id, .. }
        | ClientRequest::LogoutAccount { request_id, .. }
        | ClientRequest::GetAccountRateLimits { request_id, .. }
        | ClientRequest::FeedbackUpload { request_id, .. }
        | ClientRequest::OneOffCommandExec { request_id, .. }
        | ClientRequest::ConfigRead { request_id, .. }
        | ClientRequest::ExternalAgentConfigDetect { request_id, .. }
        | ClientRequest::ExternalAgentConfigImport { request_id, .. }
        | ClientRequest::ConfigValueWrite { request_id, .. }
        | ClientRequest::ConfigBatchWrite { request_id, .. }
        | ClientRequest::ConfigRequirementsRead { request_id, .. }
        | ClientRequest::GetAccount { request_id, .. }
        | ClientRequest::GetConversationSummary { request_id, .. }
        | ClientRequest::GitDiffToRemote { request_id, .. }
        | ClientRequest::GetAuthStatus { request_id, .. }
        | ClientRequest::FuzzyFileSearch { request_id, .. }
        | ClientRequest::FuzzyFileSearchSessionStart { request_id, .. }
        | ClientRequest::FuzzyFileSearchSessionUpdate { request_id, .. }
        | ClientRequest::FuzzyFileSearchSessionStop { request_id, .. } => request_id,
    }
}
