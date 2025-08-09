use mcp_types::RequestId;

use crate::mcp_protocol::ConversationStreamArgs;
use crate::mcp_protocol::ConversationStreamResult;
use crate::mcp_protocol::ToolCallResponseResult;
use crate::message_processor::MessageProcessor;
use crate::tool_handlers::send_message::get_session;

/// Handles the ConversationStream tool call: verifies the session and
/// enables streaming for the session, replying with an OK result.
pub(crate) async fn handle_stream_conversation(
    message_processor: &MessageProcessor,
    id: RequestId,
    arguments: ConversationStreamArgs,
) {
    let ConversationStreamArgs { conversation_id } = arguments;

    let session_id = conversation_id.0;

    // Ensure the session exists
    let conv = get_session(session_id, message_processor.conversation_map()).await;

    if conv.is_none() {
        // Return an error with no result payload per MCP error pattern
        message_processor
            .send_response_with_optional_error(id, None, Some(true))
            .await;
        return;
    }

    message_processor
        .send_response_with_optional_error(
            id,
            Some(ToolCallResponseResult::ConversationStream(
                ConversationStreamResult {},
            )),
            Some(false),
        )
        .await;

    if let Some(conv) = conv {
        tokio::spawn(async move {
            conv.set_streaming(true).await;
        });
    }
}

/// Handles cancellation for ConversationStream by disabling streaming for the session.
pub(crate) async fn handle_cancel(
    message_processor: &MessageProcessor,
    args: &ConversationStreamArgs,
) {
    let session_id = args.conversation_id.0;
    if let Some(conv) = get_session(session_id, message_processor.conversation_map()).await {
        conv.set_streaming(false).await;
    }
}
