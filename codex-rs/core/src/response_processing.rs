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
    let mut items_to_record_in_conversation_history = Vec::<ResponseItem>::new();
    let mut responses = Vec::<ResponseInputItem>::new();
    for processed_response_item in processed_items {
        let crate::codex::ProcessedResponseItem { item, response } = processed_response_item;
        match (&item, &response) {
            (ResponseItem::Message { role, .. }, None) if role == "assistant" => {
                // If the model returned a message, we need to record it.
                items_to_record_in_conversation_history.push(item);
            }
            (
                ResponseItem::LocalShellCall { .. },
                Some(ResponseInputItem::FunctionCallOutput { call_id, output }),
            ) => {
                items_to_record_in_conversation_history.push(item);
                items_to_record_in_conversation_history.push(ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }
            (
                ResponseItem::FunctionCall { .. },
                Some(ResponseInputItem::FunctionCallOutput { call_id, output }),
            ) => {
                items_to_record_in_conversation_history.push(item);
                items_to_record_in_conversation_history.push(ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }
            (
                ResponseItem::CustomToolCall { .. },
                Some(ResponseInputItem::CustomToolCallOutput { call_id, output }),
            ) => {
                items_to_record_in_conversation_history.push(item);
                items_to_record_in_conversation_history.push(ResponseItem::CustomToolCallOutput {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }
            (
                ResponseItem::FunctionCall { .. },
                Some(ResponseInputItem::McpToolCallOutput { call_id, result }),
            ) => {
                items_to_record_in_conversation_history.push(item);
                let output = match result {
                    Ok(call_tool_result) => FunctionCallOutputPayload::from(call_tool_result),
                    Err(err) => FunctionCallOutputPayload {
                        content: err.clone(),
                        success: Some(false),
                        ..Default::default()
                    },
                };
                items_to_record_in_conversation_history.push(ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output,
                });
            }
            (
                ResponseItem::Reasoning {
                    id,
                    summary,
                    content,
                    encrypted_content,
                },
                None,
            ) => {
                items_to_record_in_conversation_history.push(ResponseItem::Reasoning {
                    id: id.clone(),
                    summary: summary.clone(),
                    content: content.clone(),
                    encrypted_content: encrypted_content.clone(),
                });
            }
            _ => {
                warn!("Unexpected response item: {item:?} with response: {response:?}");
            }
        };
        if let Some(response) = response {
            responses.push(response);
        }
    }

    // Only attempt to take the lock if there is something to record.
    if !items_to_record_in_conversation_history.is_empty() {
        sess.record_conversation_items(turn_context, &items_to_record_in_conversation_history)
            .await;
    }
    (responses, items_to_record_in_conversation_history)
}
