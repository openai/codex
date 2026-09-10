//! Trace milestones for every direct or code-mode tool call handled by core.
//! These events contain call identifiers and names, never arguments or output.
//! Code-mode calls reach the broker before the dispatching turn is known.

use codex_protocol::DEFAULT_FUNCTION_NAMESPACE;
use codex_protocol::ThreadId;
use codex_tools::ToolName;

#[derive(Clone, Copy)]
pub(crate) enum Source {
    Direct,
    CodeMode,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::CodeMode => "code_mode",
        }
    }
}

pub(crate) enum Receipt<'a> {
    ModelTurn(&'a str),
    CodeModeBroker {
        cell_id: Option<&'a str>,
        runtime_tool_call_id: Option<&'a str>,
    },
}

pub(crate) fn received(
    thread_id: ThreadId,
    tool_name: &ToolName,
    call_id: &str,
    receipt: Receipt<'_>,
) {
    let (turn_id, source, cell_id, runtime_tool_call_id) = match receipt {
        Receipt::ModelTurn(turn_id) => (Some(turn_id), Source::Direct, None, None),
        Receipt::CodeModeBroker {
            cell_id,
            runtime_tool_call_id,
        } => (None, Source::CodeMode, cell_id, runtime_tool_call_id),
    };
    tracing::event!(
        name: "codex.tool_call_received",
        target: "codex_otel.trace_safe",
        tracing::Level::INFO,
        event.name = "codex.tool_call_received",
        conversation.id = %thread_id,
        turn_id,
        call_id,
        tool_name = tool_name.name.as_str(),
        tool_namespace = namespace(tool_name),
        tool_source = source.as_str(),
        cell.id = cell_id,
        runtime_tool_call_id,
    );
}

pub(crate) fn result_ready(
    thread_id: ThreadId,
    turn_id: &str,
    tool_name: &ToolName,
    call_id: &str,
    source: Source,
) {
    tracing::event!(
        name: "codex.tool_result_ready",
        target: "codex_otel.trace_safe",
        tracing::Level::INFO,
        event.name = "codex.tool_result_ready",
        conversation.id = %thread_id,
        turn_id,
        call_id,
        tool_name = tool_name.name.as_str(),
        tool_namespace = namespace(tool_name),
        tool_source = source.as_str(),
    );
}

fn namespace(tool_name: &ToolName) -> &str {
    tool_name
        .namespace
        .as_deref()
        .filter(|namespace| !namespace.is_empty())
        .unwrap_or(DEFAULT_FUNCTION_NAMESPACE)
}
