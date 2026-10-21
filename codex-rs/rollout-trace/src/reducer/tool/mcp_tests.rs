//! MCP identity regressions for callable aliases and backend names.

use std::sync::Arc;

use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use crate::ToolDispatchInvocation;
use crate::ToolDispatchPayload;
use crate::ToolDispatchRequester;
use crate::model::ToolCallKind;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::ROOT_THREAD_ID;
use crate::reducer::test_support::append_completed_inference;
use crate::reducer::test_support::create_started_writer;
use crate::reducer::test_support::start_turn;
use crate::reducer::test_support::trace_context;
use crate::replay_bundle;
use crate::tool_dispatch::ToolDispatchTraceContext;

#[test]
fn mcp_calls_keep_callable_identity_and_classify_the_backend() -> anyhow::Result<()> {
    // Both naming styles are aliases: neither the namespace nor the callable
    // tool name determines the backend identity. All test data is invented.
    for (namespace, tool_name, arguments) in [
        ("mcp__catalog", "lookup", "{}"),
        ("catalog_alias", "lookup", "{}"),
        ("catalog_alias", "exec_command", "{}"),
        ("catalog_alias", "send_message", "{}"),
        ("catalog_alias", "spawn_agent", "{}"),
        ("mcp__catalog", "write_stdin", "{}"),
        ("catalog_alias", "write_stdin", r#"{"session_id":123}"#),
    ] {
        let temp = TempDir::new()?;
        let writer = Arc::new(create_started_writer(&temp)?);
        start_turn(&writer, "turn-1")?;
        append_completed_inference(
            &writer,
            ROOT_THREAD_ID,
            "turn-1",
            "inference-1",
            vec![],
            vec![json!({
                "type": "custom_tool_call", "name": "exec",
                "call_id": "call-cell", "input": "await tools.call({})"
            })],
        )?;
        writer.append_with_context(
            trace_context("turn-1"),
            RawTraceEventPayload::CodeCellStarted {
                runtime_cell_id: "cell-1".to_string(),
                model_visible_call_id: "call-cell".to_string(),
                source_js: "await tools.call({})".to_string(),
            },
        )?;
        let _dispatch = ToolDispatchTraceContext::start(
            Arc::clone(&writer),
            ToolDispatchInvocation {
                thread_id: ROOT_THREAD_ID.to_string(),
                codex_turn_id: "turn-1".to_string(),
                tool_call_id: "call-mcp".to_string(),
                tool_name: tool_name.to_string(),
                tool_namespace: Some(namespace.to_string()),
                requester: ToolDispatchRequester::CodeCell {
                    runtime_cell_id: "cell-1".to_string(),
                    runtime_tool_call_id: "runtime-tool-1".to_string(),
                },
                payload: ToolDispatchPayload::Function {
                    arguments: arguments.to_string(),
                },
            },
        );
        let before = replay_bundle(temp.path())?;
        // Namespaced write_stdin calls must neither require terminal arguments
        // nor create terminal sessions when those arguments happen to be valid.
        assert!(before.terminal_operations.is_empty());
        assert!(before.terminal_sessions.is_empty());
        let mut expected = before.tool_calls["call-mcp"].clone();
        expected.kind = ToolCallKind::Mcp {
            server: "inventory_service".to_string(),
            tool: "find_item".to_string(),
        };

        let invocation = json!({
            "server": "inventory_service", "tool": "find_item",
            "arguments": serde_json::from_str::<serde_json::Value>(arguments)?
        });
        let begin = writer.write_json_payload(
            RawPayloadKind::ToolRuntimeEvent,
            &json!({
                "call_id": "call-mcp", "turn_id": "turn-1", "invocation": invocation
            }),
        )?;
        writer.append(RawTraceEventPayload::ToolCallRuntimeStarted {
            tool_call_id: "call-mcp".to_string(),
            runtime_payload: begin.clone(),
        })?;
        expected.raw_runtime_payload_ids.push(begin.raw_payload_id);
        // A live prefix must already have backend identity. Everything else,
        // including the callable label, invocation reference and parent, stays.
        assert_eq!(replay_bundle(temp.path())?.tool_calls["call-mcp"], expected);

        // The producer emits the runtime begin before assigning its MCP
        // correlation ID, even when the callable name matches a built-in.
        writer.append(RawTraceEventPayload::McpToolCallCorrelationAssigned {
            tool_call_id: "call-mcp".to_string(),
            mcp_call_id: "mcp-correlation-1".to_string(),
        })?;
        expected.mcp_call_id = Some("mcp-correlation-1".to_string());

        let end = writer.write_json_payload(
            RawPayloadKind::ToolRuntimeEvent,
            &json!({
                "call_id": "call-mcp", "turn_id": "turn-1", "invocation": invocation,
                "duration": {"secs": 0, "nanos": 1}, "result": {"Ok": {"content": []}}
            }),
        )?;
        writer.append(RawTraceEventPayload::ToolCallRuntimeEnded {
            tool_call_id: "call-mcp".to_string(),
            status: crate::ExecutionStatus::Completed,
            runtime_payload: end.clone(),
        })?;
        expected.raw_runtime_payload_ids.push(end.raw_payload_id);
        // MCP completion must not overwrite the outer dispatch lifecycle.
        assert_eq!(replay_bundle(temp.path())?.tool_calls["call-mcp"], expected);
    }
    Ok(())
}
