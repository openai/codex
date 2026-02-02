use codex_app_server_protocol::DynamicToolCallResponse;
use codex_core::CodexThread;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::protocol::Op;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

pub(crate) async fn on_call_response(
    call_id: String,
    receiver: oneshot::Receiver<serde_json::Value>,
    conversation: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            let fallback = CoreDynamicToolResponse {
                call_id: call_id.clone(),
                output: Some("dynamic tool request failed".to_string()),
                success: false,
                content_items: None,
            };
            if let Err(err) = conversation
                .submit(Op::DynamicToolResponse {
                    id: call_id.clone(),
                    response: fallback,
                })
                .await
            {
                error!("failed to submit DynamicToolResponse: {err}");
            }
            return;
        }
    };

    let mut response =
        serde_json::from_value::<DynamicToolCallResponse>(value).unwrap_or_else(|err| {
            error!("failed to deserialize DynamicToolCallResponse: {err}");
            DynamicToolCallResponse {
                content_items: None,
                output: Some("dynamic tool response was invalid".to_string()),
                success: false,
            }
        });

    if response.content_items.is_none() && response.output.is_none() {
        error!("dynamic tool response must include output or contentItems");
        response.output = Some("dynamic tool response must include output or contentItems".into());
        response.success = false;
    }

    let content_items = response.content_items.map(|items| {
        items
            .into_iter()
            .map(Into::into)
            .collect::<Vec<FunctionCallOutputContentItem>>()
    });
    let response = CoreDynamicToolResponse {
        call_id: call_id.clone(),
        output: response.output,
        success: response.success,
        content_items,
    };
    if let Err(err) = conversation
        .submit(Op::DynamicToolResponse {
            id: call_id,
            response,
        })
        .await
    {
        error!("failed to submit DynamicToolResponse: {err}");
    }
}
