use std::time::Duration;

use tracing::error;

use crate::codex::Session;
use crate::models::FunctionCallOutputPayload;
use crate::models::ResponseInputItem;
use crate::protocol::Event;
use crate::protocol::EventMsg;

/// Handles the specified tool call dispatches the appropriate
/// `McpToolCallBegin` and `McpToolCallEnd` events to the `Session`.
pub(crate) async fn handle_mcp_tool_call(
    sess: &Session,
    sub_id: &str,
    call_id: String,
    server: String,
    tool_name: String,
    arguments: String,
    timeout: Option<Duration>,
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

    let tool_call_begin_event = EventMsg::McpToolCallBegin {
        call_id: call_id.clone(),
        server: server.clone(),
        tool: tool_name.clone(),
        arguments: arguments_value.clone(),
    };
    notify_mcp_tool_call_event(sess, sub_id, tool_call_begin_event).await;

    // Perform the tool call.
    let (tool_call_end_event, tool_call_err) = match sess
        .call_tool(&server, &tool_name, arguments_value, timeout)
        .await
    {
        Ok(result) => (
            EventMsg::McpToolCallEnd {
                call_id,
                success: !result.is_error.unwrap_or(false),
                result: Some(result),
            },
            None,
        ),
        Err(e) => (
            EventMsg::McpToolCallEnd {
                call_id,
                success: false,
                result: None,
            },
            Some(e),
        ),
    };

    notify_mcp_tool_call_event(sess, sub_id, tool_call_end_event.clone()).await;
    let EventMsg::McpToolCallEnd {
        call_id,
        success,
        result,
    } = tool_call_end_event
    else {
        unimplemented!("unexpected event type");
    };

    ResponseInputItem::FunctionCallOutput {
        call_id,
        output: FunctionCallOutputPayload {
            content: result.map_or_else(
                || format!("err: {tool_call_err:?}"),
                |result| {
                    serde_json::to_string(&result)
                        .unwrap_or_else(|e| format!("JSON serialization error: {e}"))
                },
            ),
            success: Some(success),
        },
    }
}

async fn notify_mcp_tool_call_event(sess: &Session, sub_id: &str, event: EventMsg) {
    sess.send_event(Event {
        id: sub_id.to_string(),
        msg: event,
    })
    .await;
}
