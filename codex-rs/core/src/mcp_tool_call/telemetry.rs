use std::time::Duration;

use codex_mcp::MCP_TOOL_CODEX_APPS_META_KEY;
use codex_otel::SessionTelemetry;
use codex_otel::sanitize_metric_tag_value;
use codex_protocol::mcp::CallToolResult;
use serde_json::Value as JsonValue;
use tracing::Span;

use crate::tools::ApprovalResolutionSource;

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
    approval_source: Option<&'static str>,
}

impl McpCallMetricOutcome {
    pub(super) fn with_request_error_code(mut self, code: Option<&str>) -> Self {
        if self.error_type == Some(MCP_CALL_ERROR_TYPE_MCP_REQUEST)
            && let Some(code) = code
        {
            self.error_code = Some(code.to_string());
        }
        self
    }

    pub(super) fn from_status(status: &'static str) -> Self {
        Self {
            status,
            error_type: None,
            error_code: None,
            approval_source: None,
        }
    }

    pub(super) fn approval_cancelled(source: Option<ApprovalResolutionSource>) -> Self {
        Self {
            status: "error",
            error_type: Some("approval"),
            error_code: Some("cancelled".to_string()),
            approval_source: Some(match source {
                Some(ApprovalResolutionSource::Guardian) => "guardian",
                Some(ApprovalResolutionSource::User) => "user",
                Some(ApprovalResolutionSource::Hook) => "hook",
                None => "unknown",
            }),
        }
    }
}

pub(super) fn emit_mcp_call_metrics(
    session_telemetry: &SessionTelemetry,
    outcome: &McpCallMetricOutcome,
    server_name: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    duration: Option<Duration>,
) {
    let tags = mcp_call_metric_tags(
        outcome.status,
        server_name,
        tool_name,
        connector_id,
        connector_name,
    );
    let tag_refs: Vec<(&str, &str)> = tags
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    session_telemetry.counter(MCP_CALL_COUNT_METRIC, /*inc*/ 1, &tag_refs);
    if let Some(duration) = duration {
        session_telemetry.record_duration(MCP_CALL_DURATION_METRIC, duration, &tag_refs);
    }

    let (Some(error_type), Some(error_code)) = (outcome.error_type, outcome.error_code.as_deref())
    else {
        return;
    };
    let mut error_tags = tags;
    error_tags.push(("error_type", sanitize_metric_tag_value(error_type)));
    error_tags.push(("error_code", error_code.to_string()));
    if let Some(approval_source) = outcome.approval_source {
        error_tags.push(("approval_source", approval_source.to_string()));
    }
    let error_tag_refs: Vec<(&str, &str)> = error_tags
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    session_telemetry.counter(MCP_CALL_ERROR_COUNT_METRIC, /*inc*/ 1, &error_tag_refs);
}

fn mcp_call_metric_tags(
    status: &str,
    server_name: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut tags = vec![
        ("status", sanitize_metric_tag_value(status)),
        ("server", sanitize_metric_tag_value(server_name)),
        ("tool", sanitize_metric_tag_value(tool_name)),
    ];
    if let Some(connector_id) = connector_id.filter(|connector_id| !connector_id.is_empty()) {
        tags.push(("connector_id", sanitize_metric_tag_value(connector_id)));
    }
    if let Some(connector_name) = connector_name.filter(|connector_name| !connector_name.is_empty())
    {
        tags.push(("connector_name", sanitize_metric_tag_value(connector_name)));
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
                        .and_then(|meta| meta.get(MCP_TOOL_CODEX_APPS_META_KEY))
                        .and_then(JsonValue::as_object)
                        .and_then(|codex_apps| codex_apps.get("connector_auth_failure"))
                        .and_then(JsonValue::as_object)
                        .filter(|auth_failure| {
                            auth_failure
                                .get("is_auth_failure")
                                .and_then(JsonValue::as_bool)
                                == Some(true)
                        })
                        .and_then(|auth_failure| auth_failure.get("error_code"))
                        .and_then(JsonValue::as_str)
                        .filter(|error_code| !error_code.is_empty())
                })
                .or_else(|| {
                    result
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.get(super::MCP_RESULT_TELEMETRY_META_KEY))
                        .and_then(|telemetry| telemetry.get("error_code"))
                        .and_then(JsonValue::as_str)
                        .and_then(allowlisted_codex_telemetry_error_code)
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
                approval_source: None,
            }
        }
        Ok(_) => McpCallMetricOutcome::from_status("ok"),
        Err(_) => McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_MCP_REQUEST),
            error_code: Some(MCP_CALL_ERROR_CODE_UNKNOWN.to_string()),
            approval_source: None,
        },
    }
}

fn allowlisted_codex_telemetry_error_code(code: &str) -> Option<&str> {
    matches!(
        code,
        "kernel_start_failed"
            | "kernel_exited"
            | "kernel_io_error"
            | "execution_timeout"
            | "execution_cancelled"
            | "execution_error"
            | "execution_rejected"
            | "execution_reset"
            | "import_error"
            | "syntax_error"
            | "reference_error"
            | "type_error"
            | "uncaught_exception"
            | "unhandled_rejection"
            | "locator_timeout"
            | "guardian_denied"
    )
    .then_some(code)
}

pub(super) fn record_mcp_call_outcome_span_telemetry(span: &Span, outcome: &McpCallMetricOutcome) {
    span.record("codex.mcp.call.outcome", outcome.status);
    // A successful JSON-RPC exchange can still contain a failed tool result. Keep the
    // exported span status consistent with the outcome used by MCP error metrics.
    if outcome.status == "error" {
        span.record("otel.status_code", "ERROR");
    }
    let (Some(error_type), Some(error_code)) = (outcome.error_type, outcome.error_code.as_deref())
    else {
        return;
    };
    span.record(MCP_CALL_ERROR_TYPE_SPAN_ATTR, error_type);
    span.record(MCP_CALL_ERROR_CODE_SPAN_ATTR, error_code);
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
