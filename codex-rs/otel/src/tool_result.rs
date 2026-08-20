use crate::SessionTelemetry;
use crate::events::shared::log_and_trace_event;
use crate::events::shared::log_event;
use crate::events::shared::tool_namespace;
use crate::events::shared::trace_event;
use codex_protocol::ToolName;
use codex_protocol::config_types::ToolResultLogConfig;
use codex_utils_string::take_bytes_at_char_boundary;
use std::borrow::Cow;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

const TRUNCATION_NOTICE: &str = "[... telemetry preview truncated ...]";

/// Result-recording order across all sessions in this process, not tool-start order.
/// Independent of the optional rollout-trace writer and reset on process restart.
fn next_tool_result_seq() -> u64 {
    static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);
    NEXT_SEQ.fetch_add(/*val*/ 1, Ordering::Relaxed)
}

pub(crate) struct ToolResultEvent<'a> {
    pub tool_name: &'a ToolName,
    pub call_id: &'a str,
    pub arguments: &'a str,
    pub mcp_server: &'a str,
    pub mcp_server_origin: &'a str,
    pub duration: Duration,
    pub success: bool,
    pub output: &'a str,
}

/// Emits diagnostic logs and trace-safe metadata without changing tool-call metrics.
pub(crate) fn emit_tool_result(
    telemetry: &SessionTelemetry,
    limits: ToolResultLogConfig,
    event: ToolResultEvent<'_>,
) {
    let ToolResultEvent {
        tool_name,
        call_id,
        arguments,
        mcp_server,
        mcp_server_origin,
        duration,
        success,
        output,
    } = event;
    let tool_namespace = tool_namespace(tool_name);
    let preview = telemetry_preview(output, limits);
    let tool_result_seq = next_tool_result_seq();

    log_and_trace_event!(
        telemetry,
        common: {
            event.name = "codex.tool_result",
            tool_result_seq = tool_result_seq,
            tool_name = %tool_name.name,
            tool_namespace = %tool_namespace,
            call_id = %call_id,
            duration_ms = %duration.as_millis(),
            success = %success,
            output_truncated = preview.truncated,
        },
        log: {
            agent_name = %telemetry.metadata.agent_name,
            arguments = %arguments,
            output = %preview.text,
            mcp_server = %mcp_server,
            mcp_server_origin = %mcp_server_origin,
        },
        trace: {
            arguments_length = arguments.len() as i64,
            output_length = output.len() as i64,
            output_line_count = output.lines().count() as i64,
            tool_origin = if mcp_server.is_empty() { "builtin" } else { "mcp" },
            mcp_tool = !mcp_server.is_empty(),
        },
    );
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ToolResultPreview<'a> {
    pub text: Cow<'a, str>,
    /// Whether this telemetry limit removed text, not whether the tool result was lossless.
    pub truncated: bool,
}

pub(crate) fn telemetry_preview(
    content: &str,
    limits: ToolResultLogConfig,
) -> ToolResultPreview<'_> {
    let prefix = take_bytes_at_char_boundary(content, limits.max_bytes);
    if prefix.len() == content.len() {
        return ToolResultPreview {
            text: Cow::Borrowed(content),
            truncated: false,
        };
    }
    let separator = if prefix.is_empty() || prefix.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    ToolResultPreview {
        text: Cow::Owned(format!("{prefix}{separator}{TRUNCATION_NOTICE}")),
        truncated: true,
    }
}

#[cfg(test)]
#[path = "tool_result_tests.rs"]
mod tests;
