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
                output: "dynamic tool request failed".to_string(),
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

    let response = serde_json::from_value::<DynamicToolCallResponse>(value).unwrap_or_else(|err| {
        error!("failed to deserialize DynamicToolCallResponse: {err}");
        DynamicToolCallResponse {
            output: Some("dynamic tool response was invalid".to_string()),
            success: false,
            content_items: None,
        }
    });
    let output = normalize_output(response.output, response.content_items.as_deref());
    let response = CoreDynamicToolResponse {
        call_id: call_id.clone(),
        output,
        success: response.success,
        content_items: response.content_items,
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

fn normalize_output(
    output: Option<String>,
    content_items: Option<&[FunctionCallOutputContentItem]>,
) -> String {
    if let Some(output) = output {
        return output;
    }

    if let Some(items) = content_items {
        return match serde_json::to_string(items) {
            Ok(json) => json,
            Err(err) => {
                error!("failed to serialize dynamic tool content_items: {err}");
                String::new()
            }
        };
    }

    String::new()
}
