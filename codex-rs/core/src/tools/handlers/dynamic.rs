use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::protocol::DynamicToolCallResponseEvent;
use codex_protocol::protocol::EventMsg;
use codex_tools::ToolName;
use serde_json::Value;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::warn;

pub struct DynamicToolHandler;

impl ToolHandler for DynamicToolHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PreToolUsePayload {
            tool_name: HookToolName::new(invocation.tool_name.display()),
            tool_input: parse_arguments(arguments).ok()?,
        })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PostToolUsePayload {
            tool_name: HookToolName::new(invocation.tool_name.display()),
            tool_use_id: invocation.call_id.clone(),
            tool_input: parse_arguments(arguments).ok()?,
            tool_response: serde_json::json!({
                "contentItems": result.body.clone(),
                "success": result.success_for_logging(),
            }),
        })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "dynamic tool handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: Value = parse_arguments(&arguments)?;
        let response = request_dynamic_tool(&session, turn.as_ref(), call_id, tool_name, args)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "dynamic tool call was cancelled before receiving a response".to_string(),
                )
            })?;

        let DynamicToolResponse {
            content_items,
            success,
        } = response;
        let body = content_items
            .into_iter()
            .map(FunctionCallOutputContentItem::from)
            .collect::<Vec<_>>();
        Ok(FunctionToolOutput::from_content(body, Some(success)))
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "active turn checks and dynamic tool response registration must remain atomic"
)]
async fn request_dynamic_tool(
    session: &Session,
    turn_context: &TurnContext,
    call_id: String,
    tool_name: ToolName,
    arguments: Value,
) -> Option<DynamicToolResponse> {
    let namespace = tool_name.namespace;
    let tool = tool_name.name;
    let turn_id = turn_context.sub_id.clone();
    let (tx_response, rx_response) = oneshot::channel();
    let event_id = call_id.clone();
    let prev_entry = {
        let mut active = session.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.insert_pending_dynamic_tool(call_id.clone(), tx_response)
            }
            None => None,
        }
    };
    if prev_entry.is_some() {
        warn!("Overwriting existing pending dynamic tool call for call_id: {event_id}");
    }

    let started_at = Instant::now();
    let event = EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
        call_id: call_id.clone(),
        turn_id: turn_id.clone(),
        namespace: namespace.clone(),
        tool: tool.clone(),
        arguments: arguments.clone(),
    });
    session.send_event(turn_context, event).await;
    let response = rx_response.await.ok();

    let response_event = match &response {
        Some(response) => EventMsg::DynamicToolCallResponse(DynamicToolCallResponseEvent {
            call_id,
            turn_id,
            namespace,
            tool,
            arguments,
            content_items: response.content_items.clone(),
            success: response.success,
            error: None,
            duration: started_at.elapsed(),
        }),
        None => EventMsg::DynamicToolCallResponse(DynamicToolCallResponseEvent {
            call_id,
            turn_id,
            namespace,
            tool,
            arguments,
            content_items: Vec::new(),
            success: false,
            error: Some("dynamic tool call was cancelled before receiving a response".to_string()),
            duration: started_at.elapsed(),
        }),
    };
    session.send_event(turn_context, response_event).await;

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context;
    use crate::tools::context::ToolCallSource;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    async fn dynamic_invocation(arguments: serde_json::Value) -> ToolInvocation {
        let (session, turn) = make_session_and_context().await;
        ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-dynamic".to_string(),
            tool_name: ToolName::namespaced("openclaw__", "message"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
        }
    }

    #[tokio::test]
    async fn dynamic_pre_tool_use_payload_uses_model_tool_name_and_structured_args() {
        let invocation = dynamic_invocation(json!({
            "channel": "telegram",
            "text": "hello"
        }))
        .await;

        assert_eq!(
            DynamicToolHandler.pre_tool_use_payload(&invocation),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("openclaw__message"),
                tool_input: json!({
                    "channel": "telegram",
                    "text": "hello"
                }),
            })
        );
    }

    #[tokio::test]
    async fn dynamic_post_tool_use_payload_includes_structured_args_and_result() {
        let invocation = dynamic_invocation(json!({ "text": "hello" })).await;
        let output = FunctionToolOutput::from_content(
            vec![
                FunctionCallOutputContentItem::InputText {
                    text: "sent".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,abc".to_string(),
                    detail: None,
                },
            ],
            Some(true),
        );

        assert_eq!(
            DynamicToolHandler.post_tool_use_payload(&invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("openclaw__message"),
                tool_use_id: "call-dynamic".to_string(),
                tool_input: json!({ "text": "hello" }),
                tool_response: json!({
                    "contentItems": [
                        {
                            "type": "input_text",
                            "text": "sent"
                        },
                        {
                            "type": "input_image",
                            "image_url": "data:image/png;base64,abc"
                        }
                    ],
                    "success": true
                }),
            })
        );
    }
}
