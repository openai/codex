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
    // Add retry logic
    let max_retries = 3;
    let mut retry_count = 0;
    
    while retry_count < max_retries {
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

        // Perform the tool call with retry logic
        match sess.call_tool(&server, &tool_name, arguments_value, timeout).await {
            Ok(result) => {
                let tool_call_end_event = EventMsg::McpToolCallEnd {
                    call_id: call_id.clone(),
                    success: !result.is_error.unwrap_or(false),
                    result: Some(result),
                };
                notify_mcp_tool_call_event(sess, sub_id, tool_call_end_event.clone()).await;
                
                let EventMsg::McpToolCallEnd {
                    call_id,
                    success,
                    result,
                } = tool_call_end_event else {
                    unimplemented!("unexpected event type");
                };

                return ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload {
                        content: result.map_or_else(
                            || "No result available".to_string(),
                            |result| {
                                serde_json::to_string(&result)
                                    .unwrap_or_else(|e| format!("JSON serialization error: {e}"))
                            },
                        ),
                        success: Some(success),
                    },
                };
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    error!("Tool call failed after {} retries: {}", max_retries, e);
                    let tool_call_end_event = EventMsg::McpToolCallEnd {
                        call_id: call_id.clone(),
                        success: false,
                        result: None,
                    };
                    notify_mcp_tool_call_event(sess, sub_id, tool_call_end_event.clone()).await;
                    
                    let EventMsg::McpToolCallEnd {
                        call_id,
                        success,
                        result,
                    } = tool_call_end_event else {
                        unimplemented!("unexpected event type");
                    };

                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: format!("err: {e}"),
                            success: Some(success),
                        },
                    };
                }
                // Wait before retrying
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    // This should never be reached due to the retry logic above
    ResponseInputItem::FunctionCallOutput {
        call_id,
        output: FunctionCallOutputPayload {
            content: "Unexpected error in tool call handling".to_string(),
            success: Some(false),
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
