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
    let initialize_client_info = initialize_client_info(request);
    let method = request.method.as_str();
    let span = app_server_request_span_template(
        method,
        transport_name(transport),
        &request.id,
        connection_id,
    );

    record_client_info(
        &span,
        client_name(initialize_client_info.as_ref(), session),
        client_version(initialize_client_info.as_ref(), session),
    );

    let parent_trace = request.trace.as_ref().and_then(|trace| {
        trace.traceparent.as_ref()?;
        Some(W3cTraceContext {
            traceparent: trace.traceparent.clone(),
            tracestate: trace.tracestate.clone(),
        })
    });
    attach_parent_context(&span, method, &request.id, parent_trace.as_ref());

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
    let method = request.method();
    let span = app_server_request_span_template(&method, "in-process", request.id(), connection_id);

    let client_info = initialize_client_info_from_typed_request(request);
    record_client_info(
        &span,
        client_info
            .map(|(client_name, _)| client_name)
            .or(session.app_server_client_name.as_deref()),
        client_info
            .map(|(_, client_version)| client_version)
            .or(session.client_version.as_deref()),
    );

    attach_parent_context(&span, &method, request.id(), /*parent_trace*/ None);
    span
}

fn transport_name(transport: AppServerTransport) -> &'static str {
    match transport {
        AppServerTransport::Stdio => "stdio",
        AppServerTransport::WebSocket { .. } => "websocket",
        AppServerTransport::Off => "off",
    }
}

fn app_server_request_span_template(
    method: &str,
    transport: &'static str,
    request_id: &impl std::fmt::Display,
    connection_id: ConnectionId,
) -> Span {
    info_span!(
        "app_server.request",
        otel.kind = "server",
        otel.name = method,
        rpc.system = "jsonrpc",
        rpc.method = method,
        rpc.transport = transport,
        rpc.request_id = %request_id,
        app_server.connection_id = %connection_id,
        app_server.api_version = "v2",
        app_server.client_name = field::Empty,
        app_server.client_version = field::Empty,
        turn.id = field::Empty,
    )
}

fn record_client_info(span: &Span, client_name: Option<&str>, client_version: Option<&str>) {
    if let Some(client_name) = client_name {
        span.record("app_server.client_name", client_name);
    }
    if let Some(client_version) = client_version {
        span.record("app_server.client_version", client_version);
    }
}

fn attach_parent_context(
    span: &Span,
    method: &str,
    request_id: &impl std::fmt::Display,
    parent_trace: Option<&W3cTraceContext>,
) {
    if let Some(trace) = parent_trace {
        if !set_parent_from_w3c_trace_context(span, trace) {
            tracing::warn!(
                rpc_method = method,
                rpc_request_id = %request_id,
                "ignoring invalid inbound request trace carrier"
            );
        }
    } else if let Some(context) = traceparent_context_from_env() {
        set_parent_from_context(span, context);
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
