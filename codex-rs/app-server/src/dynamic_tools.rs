use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_core::CodexThread;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem as CoreDynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::protocol::Op;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::error;

use crate::outgoing_message::ClientRequestResult;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;

pub(crate) async fn on_call_response(
    call_id: String,
    turn_id: String,
    thread_id: String,
    tool: String,
    arguments: JsonValue,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
    outgoing: ThreadScopedOutgoingMessageSender,
) {
    let started_at = Instant::now();
    let response = receiver.await;
    let (response, error) = match response {
        Ok(Ok(value)) => decode_response(value),
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            fallback_response("dynamic tool request failed")
        }
        Err(err) => {
            error!("request failed: {err:?}");
            fallback_response("dynamic tool request failed")
        }
    };

    let DynamicToolCallResponse {
        content_items,
        success,
    } = response.clone();
    let core_response = CoreDynamicToolResponse {
        content_items: content_items
            .into_iter()
            .map(CoreDynamicToolCallOutputContentItem::from)
            .collect(),
        success,
    };
    if let Err(err) = conversation
        .submit(Op::DynamicToolResponse {
            id: call_id.clone(),
            response: core_response,
        })
        .await
    {
        error!("failed to submit DynamicToolResponse: {err}");
    }

    let duration_ms = i64::try_from(started_at.elapsed().as_millis()).ok();
    let status = if response.success {
        DynamicToolCallStatus::Completed
    } else {
        DynamicToolCallStatus::Failed
    };
    let item = ThreadItem::DynamicToolCall {
        id: call_id,
        tool,
        arguments,
        status,
        content_items: Some(response.content_items),
        success: Some(response.success),
        error,
        duration_ms,
    };
    let notification = ItemCompletedNotification {
        thread_id,
        turn_id,
        item,
    };
    outgoing
        .send_server_notification(ServerNotification::ItemCompleted(notification))
        .await;
}

fn decode_response(value: serde_json::Value) -> (DynamicToolCallResponse, Option<String>) {
    match serde_json::from_value::<DynamicToolCallResponse>(value) {
        Ok(response) => (response, None),
        Err(err) => {
            error!("failed to deserialize DynamicToolCallResponse: {err}");
            fallback_response("dynamic tool response was invalid")
        }
    }
}

fn fallback_response(message: &str) -> (DynamicToolCallResponse, Option<String>) {
    (
        DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: message.to_string(),
            }],
            success: false,
        },
        Some(message.to_string()),
    )
}
