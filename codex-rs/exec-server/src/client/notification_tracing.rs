use codex_exec_server_protocol::JSONRPCNotification;
use tracing::warn;

use crate::rpc::should_trace_server_notification;

pub(super) fn notification_span(notification: &JSONRPCNotification) -> tracing::Span {
    let method = notification.method.as_str();
    let params = notification
        .params
        .as_ref()
        .unwrap_or(&serde_json::Value::Null);
    if !should_trace_server_notification(method, params) {
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
