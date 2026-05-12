use pretty_assertions::assert_eq;
use tempfile::TempDir;

use crate::model::ToolCallKind;
use crate::raw_event::RawToolCallRequester;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::create_started_writer;
use crate::reducer::test_support::generic_summary;
use crate::reducer::test_support::start_turn;
use crate::reducer::test_support::trace_context;
use crate::replay_bundle;

#[test]
fn mcp_correlation_reduces_onto_the_existing_tool_call() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_writer(&temp)?;
    start_turn(&writer, "turn-1")?;
    writer.append_with_context(
        trace_context("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "tool-1".to_string(),
            model_visible_call_id: None,
            code_mode_runtime_tool_id: None,
            requester: RawToolCallRequester::Model,
            kind: ToolCallKind::Mcp {
                server: "docs".to_string(),
                tool: "search".to_string(),
            },
            summary: generic_summary("mcp"),
            invocation_payload: None,
        },
    )?;
    writer.append(RawTraceEventPayload::McpToolCallCorrelationAssigned {
        tool_call_id: "tool-1".to_string(),
        mcp_call_id: "018f6f0f-4981-72c0-b041-e7f77db64ed2".to_string(),
    })?;

    let rollout = replay_bundle(temp.path())?;

    assert_eq!(
        rollout.tool_calls["tool-1"].mcp_call_id.as_deref(),
        Some("018f6f0f-4981-72c0-b041-e7f77db64ed2")
    );
    Ok(())
}
