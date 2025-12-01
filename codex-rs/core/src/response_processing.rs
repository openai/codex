use crate::codex::Session;
use crate::codex::TurnContext;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use tracing::warn;

/// Process streamed `ResponseItem`s from the model into the pair of:
/// - items we should record in conversation history; and
/// - `ResponseInputItem`s to send back to the model on the next turn.
pub(crate) async fn process_items(
    processed_items: Vec<crate::codex::ProcessedResponseItem>,
    sess: &Session,
    turn_context: &TurnContext,
) -> (Vec<ResponseInputItem>, Vec<ResponseItem>) {
    let mut outputs_to_record = Vec::<ResponseItem>::new();
    let mut new_inputs_to_record = Vec::<ResponseItem>::new();
    let mut responses = Vec::<ResponseInputItem>::new();
    for processed_response_item in processed_items {
        let crate::codex::ProcessedResponseItem { item, response } = processed_response_item;

        if let Some(response) = &response {
            responses.push(response.clone());
        }

        match response {
            Some(ResponseInputItem::FunctionCallOutput { call_id, output }) => {
                new_inputs_to_record.push(ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }

            Some(ResponseInputItem::CustomToolCallOutput { call_id, output }) => {
                new_inputs_to_record.push(ResponseItem::CustomToolCallOutput {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }
            Some(ResponseInputItem::McpToolCallOutput { call_id, result }) => {
                let output = match result {
                    Ok(call_tool_result) => FunctionCallOutputPayload::from(&call_tool_result),
                    Err(err) => FunctionCallOutputPayload {
                        content: err.clone(),
                        success: Some(false),
                        ..Default::default()
                    },
                };
                new_inputs_to_record.push(ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output,
                });
            }
            None => {}
            _ => {
                warn!("Unexpected response item: {item:?} with response: {response:?}");
            }
        };

        outputs_to_record.push(item);
    }

    let all_items_to_record =
        reorder_items_for_tool_calls([outputs_to_record, new_inputs_to_record].concat());

    // Only attempt to take the lock if there is something to record.
    if !all_items_to_record.is_empty() {
        sess.record_conversation_items(turn_context, &all_items_to_record)
            .await;
    }
    (responses, all_items_to_record)
}

fn reorder_items_for_tool_calls(items: Vec<ResponseItem>) -> Vec<ResponseItem> {
    let Some(first_tool_call_idx) = items.iter().position(is_tool_call_item) else {
        return items;
    };
    let Some(last_assistant_with_text_idx) = items.iter().rposition(is_assistant_message_with_text)
    else {
        return items;
    };
    if last_assistant_with_text_idx <= first_tool_call_idx {
        return items;
    }

    let mut reordered_items = items;
    let message = reordered_items.remove(last_assistant_with_text_idx);
    reordered_items.insert(first_tool_call_idx, message);
    reordered_items
}

fn is_tool_call_item(item: &ResponseItem) -> bool {
    matches!(
        item,
        ResponseItem::FunctionCall { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::CustomToolCall { .. }
    )
}

fn is_assistant_message_with_text(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, content, .. } if role == "assistant" => {
            content.iter().any(|content_item| match content_item {
                ContentItem::OutputText { text } => !text.is_empty(),
                _ => false,
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::reorder_items_for_tool_calls;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;
    use codex_protocol::models::ResponseInputItem;

    use super::process_items;
    use crate::codex::ProcessedResponseItem;
    use crate::codex::make_session_and_context;
    #[test]
    fn assistant_text_precedes_tool_call_in_turn_recording() {
        let function_call = ResponseItem::FunctionCall {
            id: None,
            name: "tool".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        };
        let assistant_message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Here you go".to_string(),
            }],
        };
        let tool_output = ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                content: "output".to_string(),
                ..FunctionCallOutputPayload::default()
            },
        };

        let reordered = reorder_items_for_tool_calls(vec![
            function_call.clone(),
            assistant_message.clone(),
            tool_output.clone(),
        ]);

        assert_eq!(
            reordered,
            vec![assistant_message, function_call, tool_output]
        );
    }

    #[test]
    fn leaves_existing_order_when_assistant_already_first() {
        let assistant_message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "In order".to_string(),
            }],
        };
        let function_call = ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-2".to_string(),
            name: "custom".to_string(),
            input: "{}".to_string(),
        };

        let reordered =
            reorder_items_for_tool_calls(vec![assistant_message.clone(), function_call.clone()]);

        assert_eq!(reordered, vec![assistant_message, function_call]);
    }

    // When assistant produces non-empty content and then a tool call, ensure
    // recorded items place assistant content before the tool call object.
    // Without the ordering fix, this test fails because the tool call would
    // appear before the assistant message in the recorded turn.
    #[tokio::test]
    async fn assistant_content_precedes_tool_call_in_recorded_turn() {
        let (sess, turn_ctx) = make_session_and_context();

        // Simulate streamed order: FunctionCall first (as forwarded immediately),
        // then the final assistant Message content.
        let call_id = "call_1".to_string();

        let fn_call = ResponseItem::FunctionCall {
            id: None,
            name: "run".to_string(),
            arguments: "{}".to_string(),
            call_id: call_id.clone(),
        };
        let fn_output = ResponseInputItem::FunctionCallOutput {
            call_id: call_id.clone(),
            output: FunctionCallOutputPayload {
                content: "THE TOOL CALL RESULT".to_string(),
                success: Some(true),
                ..Default::default()
            },
        };

        let assistant_msg = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "THE STRING CONTENT".to_string(),
            }],
        };

        let processed = vec![
            ProcessedResponseItem {
                item: fn_call.clone(),
                response: Some(fn_output.clone()),
            },
            ProcessedResponseItem {
                item: assistant_msg.clone(),
                response: None,
            },
        ];

        let (_responses, recorded) = process_items(processed, &sess, &turn_ctx).await;

        // Expected order:
        // 1) assistant message content
        // 2) tool call object
        // 3) tool output (input for next turn)
        let expected = vec![
            assistant_msg,
            fn_call,
            ResponseItem::FunctionCallOutput {
                call_id,
                output: match fn_output {
                    ResponseInputItem::FunctionCallOutput { output, .. } => output,
                    _ => unreachable!("constructed above as FunctionCallOutput"),
                },
            },
        ];

        assert_eq!(expected, recorded);
    }
}
