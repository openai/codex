use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use codex_core::Codex;
use codex_core::protocol::{Op, Submission};
use mcp_types::RequestId;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp_protocol::{ConversationSendMessageArgs, ConversationSendMessageResult, ToolCallResponseResult};
use crate::message_processor::MessageProcessor;

#[derive(Debug)]
pub(crate) enum EnsureSessionError {
    NotFound,
    AlreadyRunning,
}

pub(crate) async fn ensure_session(
    session_id: Uuid,
    session_map: Arc<Mutex<HashMap<Uuid, Arc<Codex>>>>,
    running_session_ids: Arc<Mutex<HashSet<Uuid>>>,
) -> Result<Arc<Codex>, EnsureSessionError> {
    let codex = {
        let guard = session_map.lock().await;
        guard.get(&session_id).cloned()
    };

    let Some(codex) = codex else {
        // TODO: check if session exists on disk as well
        return Err(EnsureSessionError::NotFound);
    };

    if running_session_ids.lock().await.contains(&session_id) {
        return Err(EnsureSessionError::AlreadyRunning);
    }

    Ok(codex)
}

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
    let codex = match ensure_session(
        session_id,
        message_processor.session_map(),
        message_processor.running_session_ids(),
    )
    .await
    {
        Ok(c) => c,
        Err(EnsureSessionError::NotFound) => {
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
        }
        Err(EnsureSessionError::AlreadyRunning) => {
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
    };

    message_processor
        .running_session_ids()
        .lock()
        .await
        .insert(session_id);

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