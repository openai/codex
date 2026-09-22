//! Carries request-local W3C snapshots through RMCP's background transport queues.
//! Metadata does not retain caller spans or change the SDK's cursor-based cache keys.

use codex_protocol::protocol::W3cTraceContext;
use rmcp::model::CustomRequest;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::RequestMetaObject;
use serde_json::Value;
use tracing::Span;

/// Reads metadata from an SDK-serialized message, where envelope extensions have
/// already been merged with params._meta and take precedence on key conflicts.
/// Unlike RMCP's GetMeta, this also includes metadata supplied in typed params.
fn merged_request_meta(message: &Value) -> Option<&serde_json::Map<String, Value>> {
    message.pointer("/params/_meta").and_then(Value::as_object)
}

pub(crate) fn with_current_trace(mut meta: Option<RequestMetaObject>) -> Option<RequestMetaObject> {
    if let Some(trace) = codex_otel::current_span_w3c_trace_context() {
        let meta = meta.get_or_insert_default();
        if let Some(traceparent) = trace.traceparent {
            meta.set_traceparent(traceparent);
        }
        // A supplied tracestate must not be paired with a different caller's parent.
        meta.remove("tracestate");
        if let Some(tracestate) = trace.tracestate {
            meta.set_tracestate(tracestate);
        }
    }
    meta
}

pub(crate) fn traced_pagination(
    mut params: Option<PaginatedRequestParams>,
) -> Option<PaginatedRequestParams> {
    let meta = with_current_trace(params.as_mut().and_then(|params| params.meta.take()));
    if params.is_none() && meta.is_none() {
        return None;
    }
    let mut params = params.unwrap_or_default();
    params.meta = meta;
    Some(params)
}

pub(crate) fn traced_custom_request(method: &str, params: Option<Value>) -> CustomRequest {
    let mut request = CustomRequest::new(method, params);
    let existing_meta = request
        .params
        .as_mut()
        .and_then(|params| params.get_mut("_meta"))
        .and_then(Value::as_object_mut)
        .map(std::mem::take)
        .map(RequestMetaObject::from);
    if let Some(meta) = with_current_trace(existing_meta) {
        // Move valid metadata into the canonical SDK extension slot. Otherwise
        // its merge could retain an old params.tracestate beside the new parent.
        request.extensions.insert(meta);
    }
    request
}

pub(crate) fn request_span(message: &Value) -> Span {
    let Some(meta) = merged_request_meta(message) else {
        return Span::none();
    };
    let trace = W3cTraceContext {
        traceparent: meta
            .get("traceparent")
            .and_then(Value::as_str)
            .map(str::to_owned),
        tracestate: meta
            .get("tracestate")
            .and_then(Value::as_str)
            .map(str::to_owned),
    };
    let Some(context) = codex_otel::context_from_w3c_trace_context(&trace) else {
        return Span::none();
    };
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let span = tracing::info_span!("mcp.http.request", mcp.method = method);
    // Set the parent before the span is entered; never reparent the shared worker.
    codex_otel::set_parent_from_context(&span, context);
    span
}

#[cfg(test)]
#[path = "trace_context_tests.rs"]
mod tests;
