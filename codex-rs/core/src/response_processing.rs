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

    let all_items_to_record = [outputs_to_record, new_inputs_to_record].concat();
    // Only attempt to take the lock if there is something to record.
    if !all_items_to_record.is_empty() {
        sess.record_conversation_items(turn_context, &all_items_to_record)
            .await;
    }
    (responses, all_items_to_record)
}
