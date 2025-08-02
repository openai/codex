use mcp_types::RequestId;

use crate::mcp_protocol::ConversationStreamArgs;
use crate::mcp_protocol::ConversationStreamResult;
use crate::mcp_protocol::ToolCallResponseResult;
use crate::message_processor::MessageProcessor;
use crate::tool_handlers::send_message::get_session;

pub(crate) async fn handle_stream_conversation(
    message_processor: &MessageProcessor,
    id: RequestId,
    arguments: ConversationStreamArgs,
) {
    let ConversationStreamArgs { conversation_id } = arguments;

    let session_id = conversation_id.0;

    // Ensure the session exists
    let session_exists = get_session(session_id, message_processor.session_map())
        .await
        .is_some();

    if !session_exists {
        // Return an error with no result payload per MCP error pattern
        message_processor
            .send_response_with_optional_error(id, None, Some(true))
            .await;
        return;
    }

    // Toggle streaming to enabled via the per-session watch channel
    let senders_map = message_processor.streaming_session_senders();
    let tx = {
        let guard = senders_map.lock().await;
        guard.get(&session_id).cloned()
    };
    match tx {
        Some(tx) => {
            let _ = tx.send(true);
        }
        None => {
            // No channel found for the session; treat as error
            message_processor
                .send_response_with_optional_error(id, None, Some(true))
                .await;
            return;
        }
    }

    // Acknowledge the stream request
    message_processor
        .send_response_with_optional_error(
            id,
            Some(ToolCallResponseResult::ConversationStream(
                ConversationStreamResult {},
            )),
            Some(false),
        )
        .await;
}

pub(crate) async fn handle_cancel(
    message_processor: &MessageProcessor,
    args: &ConversationStreamArgs,
) {
    let session_id = args.conversation_id.0;
    let sender_opt: Option<tokio::sync::watch::Sender<bool>> = {
        let senders = message_processor.streaming_session_senders();
        let guard = senders.lock().await;
        guard.get(&session_id).cloned()
    };
    if let Some(tx) = sender_opt {
        let _ = tx.send(false);
    }
}
