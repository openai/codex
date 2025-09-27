use std::time::Instant;

use tracing::error;
use tracing::warn;

use crate::codex::Session;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::InputItem;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;

/// Handles the specified tool call dispatches the appropriate
/// `McpToolCallBegin` and `McpToolCallEnd` events to the `Session`.
pub(crate) async fn handle_mcp_tool_call(
    sess: &Session,
    sub_id: &str,
    call_id: String,
    server: String,
    tool_name: String,
    arguments: String,
) -> ResponseInputItem {
    // Parse the `arguments` as JSON. An empty string is OK, but invalid JSON
    // is not.
    let arguments_value = if arguments.trim().is_empty() {
        None
    } else {
        match serde_json::from_str::<serde_json::Value>(&arguments) {
            Ok(value) => Some(value),
            Err(e) => {
                error!("failed to parse tool call arguments: {e}");
                return ResponseInputItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: FunctionCallOutputPayload {
                        content: format!("err: {e}"),
                        success: Some(false),
                    },
                };
            }
        }
    };

    let invocation = McpInvocation {
        server: server.clone(),
        tool: tool_name.clone(),
        arguments: arguments_value.clone(),
    };

    let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.clone(),
        invocation: invocation.clone(),
    });
    notify_mcp_tool_call_event(sess, sub_id, tool_call_begin_event).await;

    let start = Instant::now();
    // Perform the tool call.
    let result = sess
        .call_tool(&server, &tool_name, arguments_value.clone())
        .await
        .map_err(|e| format!("tool call error: {e}"));
    let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id: call_id.clone(),
        invocation,
        duration: start.elapsed(),
        result: result.clone(),
    });

    notify_mcp_tool_call_event(sess, sub_id, tool_call_end_event.clone()).await;

    if let Ok(call_tool_result) = &result
        && let Some(items) = build_input_items_for_call_tool_result(call_tool_result)
            && let Err(unqueued) = sess.inject_input(items).await {
                warn!(
                    "failed to queue MCP tool output for model consumption ({} items)",
                    unqueued.len()
                );
            }

    ResponseInputItem::McpToolCallOutput { call_id, result }
}

async fn notify_mcp_tool_call_event(sess: &Session, sub_id: &str, event: EventMsg) {
    sess.send_event(Event {
        id: sub_id.to_string(),
        msg: event,
    })
    .await;
}

fn build_input_items_for_call_tool_result(result: &CallToolResult) -> Option<Vec<InputItem>> {
    let mut items: Vec<InputItem> = Vec::new();
    let mut saw_image = false;

    for block in &result.content {
        match block {
            ContentBlock::TextContent(text) if !text.text.is_empty() => {
                items.push(InputItem::Text {
                    text: text.text.clone(),
                });
            }
            ContentBlock::ImageContent(image) => {
                let image_url = format!("data:{};base64,{}", image.mime_type, image.data);
                items.push(InputItem::Image { image_url });
                saw_image = true;
            }
            _ => {}
        }
    }

    if saw_image { Some(items) } else { None }
}
