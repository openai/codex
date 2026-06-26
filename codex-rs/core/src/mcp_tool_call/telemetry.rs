use std::time::Duration;

use crate::session::turn_context::TurnContext;
use codex_otel::sanitize_metric_tag_value;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::MCP_ERROR_CODE_META_KEY;
use serde_json::Value as JsonValue;
use tracing::Span;

const MCP_CALL_COUNT_METRIC: &str = "codex.mcp.call";
const MCP_CALL_DURATION_METRIC: &str = "codex.mcp.call.duration_ms";
const MCP_CALL_ERROR_COUNT_METRIC: &str = "codex.mcp.call.error";
// No CallToolResult was received. This includes request setup, transport, timeout, protocol, and
// JSON-RPC failures; it does not imply that the request never reached the MCP server.
const MCP_CALL_ERROR_TYPE_MCP_REQUEST: &str = "mcp_request";
// The MCP server returned a CallToolResult with isError=true.
const MCP_CALL_ERROR_TYPE_TOOL_RESULT: &str = "tool_result";
const MCP_CALL_ERROR_CODE_UNKNOWN: &str = "unknown";
const MCP_CALL_ERROR_CODE_MAX_CHARS: usize = 256;
const MCP_CALL_ERROR_TYPE_SPAN_ATTR: &str = "error.type";
const MCP_CALL_ERROR_CODE_SPAN_ATTR: &str = "codex.mcp.error.code";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct McpCallMetricOutcome {
    status: &'static str,
    error_type: Option<&'static str>,
    error_code: Option<String>,
}

impl McpCallMetricOutcome {
    pub(super) fn from_status(status: &'static str) -> Self {
        Self {
            status,
            error_type: None,
            error_code: None,
        }
    }
}

pub(super) fn emit_mcp_call_metrics(
    turn_context: &TurnContext,
    outcome: &McpCallMetricOutcome,
    server_name: &str,
    tool_name: &str,
    runtime_labels: &[(String, String)],
    duration: Option<Duration>,
) {
    let tags = mcp_call_metric_tags(outcome.status, server_name, tool_name, runtime_labels);
    let tag_refs: Vec<(&str, &str)> = tags
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    turn_context
        .session_telemetry
        .counter(MCP_CALL_COUNT_METRIC, /*inc*/ 1, &tag_refs);
    if let Some(duration) = duration {
        turn_context.session_telemetry.record_duration(
            MCP_CALL_DURATION_METRIC,
            duration,
            &tag_refs,
        );
    }

    let (Some(error_type), Some(error_code)) = (outcome.error_type, outcome.error_code.as_deref())
    else {
        return;
    };
    let mut error_tags = tags;
    error_tags.push((
        "error_type".to_string(),
        sanitize_metric_tag_value(error_type),
    ));
    error_tags.push(("error_code".to_string(), error_code.to_string()));
    let error_tag_refs: Vec<(&str, &str)> = error_tags
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    turn_context.session_telemetry.counter(
        MCP_CALL_ERROR_COUNT_METRIC,
        /*inc*/ 1,
        &error_tag_refs,
    );
}

pub(super) fn mcp_call_metric_tags(
    status: &str,
    server_name: &str,
    tool_name: &str,
    runtime_labels: &[(String, String)],
) -> Vec<(String, String)> {
    let mut tags = vec![
        ("status".to_string(), sanitize_metric_tag_value(status)),
        ("server".to_string(), sanitize_metric_tag_value(server_name)),
        ("tool".to_string(), sanitize_metric_tag_value(tool_name)),
    ];
    for (key, value) in runtime_labels {
        if matches!(
            key.as_str(),
            "status" | "server" | "tool" | "error_type" | "error_code"
        ) || tags.iter().any(|(existing, _)| existing == key)
        {
            continue;
        }
        tags.push((key.clone(), sanitize_metric_tag_value(value)));
    }
    tags
}

pub(super) fn mcp_call_metric_outcome(
    result: &Result<CallToolResult, String>,
) -> McpCallMetricOutcome {
    match result {
        Ok(result) if result.is_error.unwrap_or(false) => {
            let error_code = result
                .structured_content
                .as_ref()
                .and_then(JsonValue::as_object)
                .and_then(|structured_content| structured_content.get("error_code"))
                .and_then(JsonValue::as_str)
                .filter(|error_code| !error_code.is_empty())
                .or_else(|| {
                    result
                        .meta
                        .as_ref()
                        .and_then(JsonValue::as_object)
                        .and_then(|meta| meta.get(MCP_ERROR_CODE_META_KEY))
                        .and_then(JsonValue::as_str)
                        .filter(|error_code| !error_code.is_empty())
                });
            let error_code: String = error_code
                .unwrap_or(MCP_CALL_ERROR_CODE_UNKNOWN)
                .chars()
                .take(MCP_CALL_ERROR_CODE_MAX_CHARS)
                .collect();
            McpCallMetricOutcome {
                status: "error",
                error_type: Some(MCP_CALL_ERROR_TYPE_TOOL_RESULT),
                error_code: Some(sanitize_metric_tag_value(&error_code)),
            }
        }
        Ok(_) => McpCallMetricOutcome::from_status("ok"),
        Err(_) => McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_MCP_REQUEST),
            error_code: Some(MCP_CALL_ERROR_CODE_UNKNOWN.to_string()),
        },
    }
}

pub(super) fn record_mcp_call_outcome_span_telemetry(
    span: &Span,
    result: &Result<CallToolResult, String>,
) {
    let outcome = mcp_call_metric_outcome(result);
    let (Some(error_type), Some(error_code)) = (outcome.error_type, outcome.error_code) else {
        return;
    };
    span.record(MCP_CALL_ERROR_TYPE_SPAN_ATTR, error_type);
    span.record(MCP_CALL_ERROR_CODE_SPAN_ATTR, error_code);
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
