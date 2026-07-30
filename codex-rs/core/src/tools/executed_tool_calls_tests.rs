use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn executed_tool_call_recorder_bounds_pending_calls_and_preserves_overflow() {
    let recorder = ExecutedToolCallRecorder::default();

    for index in 0..MAX_PENDING_EXECUTED_TOOL_CALLS + 2 {
        recorder.record_tool_call(
            &ToolCall {
                tool_name: codex_tools::ToolName::plain("direct_tool"),
                call_id: format!("direct-{index}"),
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
                encrypted_function_args: None,
            },
            &ToolCallSource::Direct,
            ToolMode::Direct,
        );
    }

    let cell_id = CellId::new("bounded-cell".to_string());
    recorder.register_cell(&cell_id, "bounded-output");
    for _ in 0..MAX_PENDING_EXECUTED_TOOL_CALLS + 2 {
        recorder.record_nested_tool_call(
            cell_id.clone(),
            ExecutedToolCall::new("nested_tool".to_string(), json!({})),
            /*original_bytes*/ 2,
        );
    }

    for index in 0..MAX_PENDING_EXECUTED_TOOL_CALLS + 2 {
        recorder.register_cell(
            &CellId::new(format!("cell-{index}")),
            &format!("output-{index}"),
        );
    }

    {
        let state = recorder
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            state.direct_calls.len(),
            MAX_PENDING_EXECUTED_TOOL_CALLS + 1
        );
        assert_eq!(
            serde_json::to_value(
                state
                    .direct_calls
                    .get(&format!("direct-{MAX_PENDING_EXECUTED_TOOL_CALLS}"))
                    .expect("first excess direct call must be marked"),
            )
            .expect("direct overflow marker must serialize"),
            json!({
                "name": "direct_tool",
                "arguments": {
                    "_codex_executed_tool_call_truncated": {
                        "original_bytes": 2,
                        "max_bytes": 0,
                    },
                },
            }),
        );
        assert_eq!(
            state.pending_nested_calls,
            MAX_PENDING_EXECUTED_TOOL_CALLS + 1
        );
        assert_eq!(state.cells.len(), MAX_PENDING_EXECUTED_TOOL_CALLS);
        assert_eq!(state.output_cells.len(), MAX_PENDING_EXECUTED_TOOL_CALLS);
    }

    let mut items = [ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "bounded-output".to_string(),
        output: FunctionCallOutputPayload::from_text(String::new()),
        internal_chat_message_metadata_passthrough: None,
    }];
    let mut retry_cache = HashMap::new();
    recorder.attach_pending_to_prompt(&mut items, &mut retry_cache);

    let calls = items[0]
        .executed_tool_call_metadata()
        .and_then(|metadata| metadata.executed_tool_calls.as_ref())
        .expect("bounded nested calls must attach to their own output");
    assert_eq!(calls.len(), MAX_PENDING_EXECUTED_TOOL_CALLS + 1);
    assert_eq!(
        serde_json::to_value(calls.last().expect("overflow marker must be retained"))
            .expect("nested overflow marker must serialize"),
        json!({
            "name": "nested_tool",
            "arguments": {
                "_codex_executed_tool_call_truncated": {
                    "original_bytes": 2,
                    "max_bytes": 0,
                },
            },
        }),
    );

    let state = recorder
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(state.pending_nested_calls, 0);
    assert!(!state.cells.contains_key(&cell_id));
    assert_eq!(retry_cache.len(), 1);
}
