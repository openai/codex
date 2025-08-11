use std::collections::HashMap;
use std::sync::Arc;

use mcp_types::RequestId;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::conversation_loop::Conversation;
use crate::mcp_protocol::ConversationSendMessageArgs;
use crate::mcp_protocol::ConversationSendMessageResult;
use crate::mcp_protocol::ToolCallResponseResult;
use crate::message_processor::MessageProcessor;

pub(crate) async fn handle_send_message(
    message_processor: &MessageProcessor,
    id: RequestId,
    arguments: ConversationSendMessageArgs,
) {
    let ConversationSendMessageArgs {
        conversation_id,
        content: items,
        parent_message_id: _,
        conversation_overrides: _,
    } = arguments;

    if items.is_empty() {
        message_processor
            .send_response_with_optional_error(
                id,
                Some(ToolCallResponseResult::ConversationSendMessage(
                    ConversationSendMessageResult::Error {
                        message: "No content items provided".to_string(),
                    },
                )),
                Some(true),
            )
            .await;
        return;
    }

    let session_id = conversation_id.0;
    let Some(conversation) = get_session(session_id, message_processor.conversation_map()).await
    else {
        message_processor
            .send_response_with_optional_error(
                id,
                Some(ToolCallResponseResult::ConversationSendMessage(
                    ConversationSendMessageResult::Error {
                        message: "Session does not exist".to_string(),
                    },
                )),
                Some(true),
            )
            .await;
        return;
    };

    let res = conversation.try_submit_user_input(id.clone(), items).await;

    if let Err(e) = res {
        message_processor
            .send_response_with_optional_error(
                id,
                Some(ToolCallResponseResult::ConversationSendMessage(
                    ConversationSendMessageResult::Error {
                        message: e.to_string(),
                    },
                )),
                Some(true),
            )
            .await;
        return;
    }

    message_processor
        .send_response_with_optional_error(
            id,
            Some(ToolCallResponseResult::ConversationSendMessage(
                ConversationSendMessageResult::Ok,
            )),
            Some(false),
        )
        .await;
}

pub(crate) async fn get_session(
    session_id: Uuid,
    conversation_map: Arc<Mutex<HashMap<Uuid, Arc<Conversation>>>>,
) -> Option<Arc<Conversation>> {
    let guard = conversation_map.lock().await;
    guard.get(&session_id).cloned()
}
