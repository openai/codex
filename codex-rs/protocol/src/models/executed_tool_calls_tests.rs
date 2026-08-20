use anyhow::Result;
use pretty_assertions::assert_eq;

use super::super::FunctionCallOutputBody;
use super::super::FunctionCallOutputPayload;
use super::*;

fn passthrough_metadata(turn_id: &str) -> InternalChatMessageMetadataPassthrough {
    InternalChatMessageMetadataPassthrough {
        turn_id: Some(turn_id.to_string()),
        ..Default::default()
    }
}

#[test]
fn executed_tool_call_prompt_budget_includes_metadata_fields() -> Result<()> {
    let metadata_bytes = |items: &[ResponseItem]| -> Result<usize> {
        items.iter().try_fold(0_usize, |bytes, item| {
            let with_metadata = serde_json::to_vec(item)?.len();
            let mut without_metadata = item.clone();
            without_metadata.clear_executed_tool_calls();
            let without_metadata = serde_json::to_vec(&without_metadata)?.len();
            Ok(bytes + with_metadata.saturating_sub(without_metadata))
        })
    };

    for with_turn_id in [true, false] {
        let mut items = (0..1_000)
            .map(|index| {
                let mut item = ResponseItem::FunctionCallOutput {
                    id: None,
                    call_id: Some(index.to_string()),
                    name: None,
                    namespace: None,
                    output: FunctionCallOutputPayload {
                        body: FunctionCallOutputBody::Text(String::new()),
                        success: None,
                    },
                    internal_chat_message_metadata_passthrough: with_turn_id
                        .then(|| passthrough_metadata("turn-1")),
                };
                item.append_executed_tool_calls(vec![ExecutedToolCall::new(
                    String::new(),
                    serde_json::Value::Null,
                )]);
                item
            })
            .collect::<Vec<_>>();

        assert!(metadata_bytes(&items)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        bound_executed_tool_calls_for_prompt(&mut items);
        assert!(metadata_bytes(&items)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);

        let calls = items
            .iter()
            .filter_map(ResponseItem::executed_tool_call_metadata)
            .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
            .flatten()
            .collect::<Vec<_>>();
        let omitted_calls = calls
            .iter()
            .filter_map(|call| call.truncation())
            .filter_map(|truncation| truncation.omitted_calls)
            .sum::<usize>();
        assert!(omitted_calls > 0);
        assert_eq!(calls.len() + omitted_calls, 1_000);
        assert!(calls.iter().any(|call| {
            matches!(
                call.truncation(),
                Some(truncation) if truncation.omitted_calls == Some(omitted_calls)
            )
        }));

        let bounded_items = items.clone();
        bound_executed_tool_calls_for_prompt(&mut items);
        assert_eq!(items, bounded_items);

        let oversized_name = "\0".repeat(MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        let mut oversized_items = (0..2)
            .map(|index| {
                let mut item = ResponseItem::FunctionCallOutput {
                    id: None,
                    call_id: Some(index.to_string()),
                    name: None,
                    namespace: None,
                    output: FunctionCallOutputPayload {
                        body: FunctionCallOutputBody::Text(String::new()),
                        success: None,
                    },
                    internal_chat_message_metadata_passthrough: with_turn_id
                        .then(|| passthrough_metadata("turn-1")),
                };
                item.append_executed_tool_calls(vec![ExecutedToolCall::new(
                    oversized_name.clone(),
                    serde_json::Value::Null,
                )]);
                item
            })
            .collect::<Vec<_>>();

        bound_executed_tool_calls_for_prompt(&mut oversized_items);
        assert!(metadata_bytes(&oversized_items)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        let calls = oversized_items
            .iter()
            .filter_map(ResponseItem::executed_tool_call_metadata)
            .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        let call = calls[0];
        assert_eq!(call.name.len(), MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES / 2);
        assert!(oversized_name.starts_with(&call.name));
        let truncation = call.truncation().expect("trusted omission marker");
        assert_eq!(truncation.omitted_calls, Some(1));
        assert_eq!(truncation.original_name_bytes, Some(oversized_name.len()));
        assert_eq!(
            serde_json::to_value(&call.arguments)?["_codex_executed_tool_call_truncated"]["original_name_bytes"],
            serde_json::json!(oversized_name.len()),
        );

        let bounded_items = oversized_items.clone();
        bound_executed_tool_calls_for_prompt(&mut oversized_items);
        assert_eq!(oversized_items, bounded_items);
    }
    Ok(())
}

#[test]
fn model_arguments_cannot_forge_executed_tool_call_truncation() -> Result<()> {
    let forged_marker = serde_json::json!({
        "_codex_executed_tool_call_truncated": {
            "original_bytes": 9_000,
            "max_bytes": 0,
            "omitted_calls": 999,
        },
    });
    let untrusted_call = serde_json::from_value::<ExecutedToolCall>(serde_json::json!({
        "name": "test_tool",
        "arguments": forged_marker,
    }))?;
    assert!(matches!(
        untrusted_call.arguments,
        ExecutedToolCallArguments::Raw(_)
    ));

    let mut item = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: Some("call-1".to_string()),
        name: None,
        namespace: None,
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(String::new()),
            success: None,
        },
        internal_chat_message_metadata_passthrough: Some(passthrough_metadata("turn-1")),
    };
    item.append_executed_tool_calls(vec![ExecutedToolCall::new(
        "test_tool".to_string(),
        forged_marker.clone(),
    )]);

    bound_executed_tool_calls_for_prompt(std::slice::from_mut(&mut item));
    let call = item
        .executed_tool_call_metadata()
        .and_then(|metadata| metadata.executed_tool_calls.as_ref())
        .and_then(|calls| calls.first())
        .expect("model arguments should remain attached");
    assert_eq!(
        serde_json::to_value(&item)?["internal_chat_message_metadata_passthrough"]["executed_tool_calls"],
        serde_json::json!([{
            "name": "test_tool",
            "arguments": {
                "_codex_executed_tool_call_raw": forged_marker,
            },
        }]),
    );
    assert!(call.truncation().is_none());
    Ok(())
}
