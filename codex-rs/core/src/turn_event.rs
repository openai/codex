use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::items::TurnItem;
use futures::stream::FuturesUnordered;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::Result;
use crate::function_tool::FunctionCallError;
use crate::parse_turn_item;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolRouter;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use futures::Future;
use tracing::debug;

/// Handle a completed output item from the model stream, recording it and
/// queuing any tool execution futures. This records items immediately so
/// history and rollout stay in sync even if the turn is later cancelled.
pub(crate) type InFlightFuture<'f> =
    Pin<Box<dyn Future<Output = Result<ResponseInputItem>> + Send + 'f>>;

pub(crate) struct OutputItemResult {
    pub last_agent_message: Option<String>,
    pub needs_follow_up: bool,
}

pub(crate) async fn handle_output_item_done(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    tool_runtime: Arc<ToolCallRuntime>,
    item: ResponseItem,
    previously_active_item: Option<TurnItem>,
    in_flight: &mut FuturesUnordered<InFlightFuture<'_>>,
    cancellation_token: CancellationToken,
) -> Result<OutputItemResult> {
    let mut result = OutputItemResult {
        last_agent_message: None,
        needs_follow_up: false,
    };

    match ToolRouter::build_tool_call(sess.as_ref(), item.clone()).await {
        Ok(Some(call)) => {
            let payload_preview = call.payload.log_payload().into_owned();
            tracing::info!("ToolCall: {} {}", call.tool_name, payload_preview);

            sess.record_conversation_items(turn_context, std::slice::from_ref(&item))
                .await;

            let sess_for_output: Arc<Session> = Arc::clone(sess);
            let turn_for_output: Arc<TurnContext> = Arc::clone(turn_context);
            let tool_runtime = Arc::clone(&tool_runtime);

            in_flight.push(Box::pin(async move {
                let response_input = tool_runtime
                    .handle_tool_call(call, cancellation_token)
                    .await?;
                if let Some(response_item) = response_input_to_response_item(&response_input) {
                    sess_for_output
                        .record_conversation_items(
                            turn_for_output.as_ref(),
                            std::slice::from_ref(&response_item),
                        )
                        .await;
                }
                Ok(response_input)
            }));
            result.needs_follow_up = true;
        }
        Ok(None) => {
            if let Some(turn_item) = handle_non_tool_response_item(&item).await {
                if previously_active_item.is_none() {
                    sess.emit_turn_item_started(turn_context, &turn_item).await;
                }

                sess.emit_turn_item_completed(turn_context, turn_item).await;
            }

            sess.record_conversation_items(turn_context, std::slice::from_ref(&item))
                .await;
            if let Some(agent_message) = last_assistant_message_from_item(&item) {
                result.last_agent_message = Some(agent_message);
            }
        }
        Err(FunctionCallError::MissingLocalShellCallId) => {
            let msg = "LocalShellCall without call_id or id";
            turn_context
                .client
                .get_otel_event_manager()
                .log_tool_failed("local_shell", msg);
            tracing::error!(msg);

            let response = ResponseInputItem::FunctionCallOutput {
                call_id: String::new(),
                output: FunctionCallOutputPayload {
                    content: msg.to_string(),
                    ..Default::default()
                },
            };
            sess.record_conversation_items(turn_context, std::slice::from_ref(&item))
                .await;
            if let Some(response_item) = response_input_to_response_item(&response) {
                sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
                    .await;
            }
            result.needs_follow_up = true;
        }
        Err(FunctionCallError::RespondToModel(message))
        | Err(FunctionCallError::Denied(message)) => {
            let response = ResponseInputItem::FunctionCallOutput {
                call_id: String::new(),
                output: FunctionCallOutputPayload {
                    content: message,
                    ..Default::default()
                },
            };
            sess.record_conversation_items(turn_context, std::slice::from_ref(&item))
                .await;
            if let Some(response_item) = response_input_to_response_item(&response) {
                sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
                    .await;
            }
            result.needs_follow_up = true;
        }
        Err(FunctionCallError::Fatal(message)) => {
            return Err(CodexErr::Fatal(message));
        }
    }

    Ok(result)
}

pub(crate) async fn handle_non_tool_response_item(item: &ResponseItem) -> Option<TurnItem> {
    debug!(?item, "Output item");

    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. } => parse_turn_item(item),
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected tool output from stream");
            None
        }
        _ => None,
    }
}

pub(crate) fn last_assistant_message_from_item(item: &ResponseItem) -> Option<String> {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        return content.iter().rev().find_map(|ci| match ci {
            codex_protocol::models::ContentItem::OutputText { text } => Some(text.clone()),
            _ => None,
        });
    }
    None
}

pub(crate) fn response_input_to_response_item(input: &ResponseInputItem) -> Option<ResponseItem> {
    match input {
        ResponseInputItem::FunctionCallOutput { call_id, output } => {
            Some(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseInputItem::CustomToolCallOutput { call_id, output } => {
            Some(ResponseItem::CustomToolCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseInputItem::McpToolCallOutput { call_id, result } => {
            let output = match result {
                Ok(call_tool_result) => FunctionCallOutputPayload::from(call_tool_result),
                Err(err) => FunctionCallOutputPayload {
                    content: err.clone(),
                    success: Some(false),
                    ..Default::default()
                },
            };
            Some(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output,
            })
        }
        _ => None,
    }
}
