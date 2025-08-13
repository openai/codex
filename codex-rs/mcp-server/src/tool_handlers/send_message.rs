use codex_core::protocol::Op;
use codex_core::protocol::Submission;
use mcp_types::RequestId;

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
    let Ok(codex) = message_processor
        .get_conversation_manager()
        .get_conversation(session_id)
        .await
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

    let running = {
        let running_sessions = message_processor.running_session_ids();
        let mut running_sessions = running_sessions.lock().await;
        !running_sessions.insert(session_id)
    };

    if running {
        message_processor
            .send_response_with_optional_error(
                id,
                Some(ToolCallResponseResult::ConversationSendMessage(
                    ConversationSendMessageResult::Error {
                        message: "Session is already running".to_string(),
                    },
                )),
                Some(true),
            )
            .await;
        return;
    }

    let request_id_string = match &id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(i) => i.to_string(),
    };

    let submit_res = codex
        .submit_with_id(Submission {
            id: request_id_string,
            op: Op::UserInput { items },
        })
        .await;

    if let Err(e) = submit_res {
        message_processor
            .send_response_with_optional_error(
                id,
                Some(ToolCallResponseResult::ConversationSendMessage(
                    ConversationSendMessageResult::Error {
                        message: format!("Failed to submit user input: {e}"),
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
