use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::ServerResponse;
use codex_core::CodexThread;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem as CoreDynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::protocol::Op;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

use crate::outgoing_message::ClientRequestResult;
use crate::server_request_error::is_turn_transition_server_request_error;

pub(crate) async fn on_call_response(
    call_id: String,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
) {
    let response = receiver.await;
    let (response, _error) = match response {
        Ok(Ok(ServerResponse::DynamicToolCall { response, .. })) => (response, None),
        Ok(Ok(response)) => {
            error!("dynamic tool request returned an unexpected response: {response:?}");
            fallback_response("dynamic tool response was invalid")
        }
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return,
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
