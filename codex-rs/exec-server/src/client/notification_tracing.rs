use codex_exec_server_protocol::JSONRPCNotification;
use tracing::warn;

use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;

pub(super) fn notification_span(notification: &JSONRPCNotification) -> tracing::Span {
    let method = notification.method.as_str();
    let params = notification
        .params
        .as_ref()
        .unwrap_or(&serde_json::Value::Null);
    let should_trace = match method {
        EXEC_EXITED_METHOD | EXEC_CLOSED_METHOD => true,
        HTTP_REQUEST_BODY_DELTA_METHOD => {
            let Some(params) = params.as_object() else {
                return tracing::Span::none();
            };
            params.get("done").and_then(serde_json::Value::as_bool) == Some(true)
                || params.get("error").is_some_and(|error| !error.is_null())
        }
        _ => false,
    };
    if !should_trace {
        return tracing::Span::none();
    }
    let span = tracing::info_span!(
        "codex.exec_server.notification",
        otel.kind = "server",
        otel.name = method,
        rpc.system = "jsonrpc",
        rpc.method = method,
        method,
        result = tracing::field::Empty,
    );
    if let Some(trace) = &notification.trace
        && !codex_otel::set_parent_from_w3c_trace_context(&span, trace)
    {
        warn!(
            method,
            "ignoring invalid inbound exec-server notification trace carrier"
        );
    }
    span
}

#[cfg(test)]
#[path = "notification_tracing_tests.rs"]
mod tests;
