use crate::codex::Session;
use crate::codex::TurnContext;
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

    // Preserve correct ordering when an assistant message and a tool call/output
    // happen in the same turn: any non-empty assistant content must precede the
    // tool call object. This mirrors the upstream Responses ordering expectations
    // and avoids sending `{role:"assistant", content:null, tool_calls:[...]}`
    // before a `{role:"assistant", content:"..."}` message.
    let mut all_items_to_record = [outputs_to_record, new_inputs_to_record].concat();

    // Fix order: move the last assistant Message with non-empty text so it appears
    // immediately before the first tool call object if needed.
    // We only adjust relative order within the items collected this turn to avoid
    // perturbing prior history.
    if all_items_to_record.len() > 1 {
        // Find indices relevant to reordering within this batch.
        let first_tool_call_idx = all_items_to_record.iter().position(|it| {
            matches!(
                it,
                codex_protocol::models::ResponseItem::FunctionCall { .. }
                    | codex_protocol::models::ResponseItem::LocalShellCall { .. }
                    | codex_protocol::models::ResponseItem::CustomToolCall { .. }
            )
        });
        let last_assistant_nonempty_idx =
            all_items_to_record
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, it)| match it {
                    codex_protocol::models::ResponseItem::Message { role, content, .. }
                        if role == "assistant" =>
                    {
                        let has_text = content.iter().any(|c| match c {
                            codex_protocol::models::ContentItem::OutputText { text } => {
                                !text.is_empty()
                            }
                            _ => false,
                        });
                        if has_text { Some(i) } else { None }
                    }
                    _ => None,
                });

        if let (Some(tool_idx), Some(msg_idx)) = (first_tool_call_idx, last_assistant_nonempty_idx)
            && msg_idx > tool_idx
        {
            // Move the assistant message so it sits before the first tool call.
            let msg = all_items_to_record.remove(msg_idx);
            all_items_to_record.insert(tool_idx, msg);
        }
    }

    // Only attempt to take the lock if there is something to record.
    if !all_items_to_record.is_empty() {
        sess.record_conversation_items(turn_context, &all_items_to_record)
            .await;
    }
    (responses, all_items_to_record)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::process_items;
    use crate::codex::ProcessedResponseItem;
    use crate::codex::make_session_and_context;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::models::ResponseItem;

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
