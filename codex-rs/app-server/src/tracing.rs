use crate::message_processor::ConnectionSessionState;
use crate::outgoing_message::ConnectionId;
use crate::transport::AppServerTransport;
use ::tracing::Span;
use ::tracing::field;
use ::tracing::info_span;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCRequest;
use codex_otel::set_parent_from_context;
use codex_otel::set_parent_from_w3c_trace_context;
use codex_otel::traceparent_context_from_env;

pub(crate) fn request_span(
    request: &JSONRPCRequest,
    transport: AppServerTransport,
    connection_id: ConnectionId,
    session: &ConnectionSessionState,
) -> Span {
    let span = info_span!(
        "app_server.request",
        rpc.system = "jsonrpc",
        rpc.service = "codex-app-server",
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

    if let Some(trace) = &request.trace {
        if !set_parent_from_w3c_trace_context(&span, trace) {
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
    session.client_name.as_deref()
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
