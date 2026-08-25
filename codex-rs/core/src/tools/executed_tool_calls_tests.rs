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
        call_id: Some("bounded-output".to_string()),
        name: None,
        namespace: None,
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
    let expected_calls = calls.clone();

    {
        let state = recorder
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(state.pending_nested_calls, 0);
        assert!(!state.cells.contains_key(&cell_id));
        assert_eq!(retry_cache.len(), 1);
    }

    let mut replayed_items = [ResponseItem::FunctionCallOutput {
        id: None,
        call_id: Some("bounded-output".to_string()),
        name: None,
        namespace: None,
        output: FunctionCallOutputPayload::from_text(String::new()),
        internal_chat_message_metadata_passthrough: None,
    }];
    let mut replay_retry_cache = HashMap::new();
    assert!(recorder.attach_pending_to_prompt(&mut replayed_items, &mut replay_retry_cache));
    assert_eq!(
        replayed_items[0]
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.executed_tool_calls.as_ref()),
        Some(&expected_calls),
    );

    let mut compacted_retry_cache = HashMap::new();
    assert!(!recorder.attach_pending_to_prompt(&mut [], &mut compacted_retry_cache));
    let state = recorder
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(state.retained_calls.is_empty());
}

#[test]
fn executed_tool_call_recorder_bounds_retained_history_and_reports_omissions() {
    let recorder = ExecutedToolCallRecorder::default();
    let mut history = Vec::new();
    let arguments = serde_json::to_string(&json!({ "payload": "x".repeat(1024) }))
        .expect("tool arguments must serialize");
    let mut prompt = Vec::new();

    for index in 0..512 {
        let call_id = format!("retained-{index}");
        recorder.record_tool_call(
            &ToolCall {
                tool_name: codex_tools::ToolName::plain(format!("retained_tool_{index}")),
                call_id: call_id.clone(),
                payload: ToolPayload::Function {
                    arguments: arguments.clone(),
                },
                encrypted_function_args: None,
            },
            &ToolCallSource::Direct,
            ToolMode::Direct,
        );
        history.push(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text(String::new()),
            internal_chat_message_metadata_passthrough: None,
        });
        prompt = history.clone();
        assert!(recorder.attach_pending_to_prompt(&mut prompt, &mut HashMap::new()));
        codex_protocol::models::bound_executed_tool_calls_for_prompt(&mut prompt);
        let latest_call = prompt
            .last()
            .and_then(ResponseItem::executed_tool_call_metadata)
            .and_then(|metadata| metadata.executed_tool_calls.as_ref())
            .and_then(|calls| calls.first())
            .map(serde_json::to_value)
            .transpose()
            .expect("latest tool call must serialize")
            .expect("latest tool call must remain in retained metadata");
        assert_eq!(latest_call["name"], format!("retained_tool_{index}"));
        assert_eq!(
            latest_call["arguments"],
            json!({ "payload": "x".repeat(1024) }),
        );
    }

    let state = recorder
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let retained_bytes = state
        .retained_calls
        .values()
        .map(serialized_json_bytes)
        .sum::<serde_json::Result<usize>>()
        .expect("retained calls must serialize");
    assert!(retained_bytes <= MAX_EXECUTED_TOOL_CALL_FULL_ARGUMENT_BYTES_PER_OUTPUT);

    let metadata = prompt
        .iter()
        .filter_map(ResponseItem::executed_tool_call_metadata)
        .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
        .flatten()
        .map(|call| serde_json::to_value(call).expect("retained call must serialize"))
        .collect::<Vec<_>>();
    let omitted_calls = metadata
        .iter()
        .filter_map(|call| {
            call["arguments"]["_codex_executed_tool_call_truncated"]["omitted_calls"].as_u64()
        })
        .sum::<u64>();
    assert!(omitted_calls > 0);
    assert_eq!(metadata.len() as u64 + omitted_calls, 512);
}
