use std::time::Instant;

use tracing::error;
use tracing::warn;

use crate::codex::Session;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;

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
    // is not. If parsing fails because the model omitted a closing delimiter,
    // attempt to synthesize the missing characters so that we can still run
    // the tool call.
    let arguments_value = match parse_tool_arguments(&arguments) {
        Ok(value) => value,
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

    ResponseInputItem::McpToolCallOutput { call_id, result }
}

async fn notify_mcp_tool_call_event(sess: &Session, sub_id: &str, event: EventMsg) {
    sess.send_event(Event {
        id: sub_id.to_string(),
        msg: event,
    })
    .await;
}

fn parse_tool_arguments(arguments: &str) -> Result<Option<serde_json::Value>, serde_json::Error> {
    if arguments.trim().is_empty() {
        return Ok(None);
    }

    match serde_json::from_str(arguments) {
        Ok(value) => Ok(Some(value)),
        Err(original_error) => {
            if let Some(fixed) = fix_unclosed_json_delimiters(arguments) {
                match serde_json::from_str(&fixed) {
                    Ok(value) => {
                        warn!("synthesized missing closing delimiters for tool arguments");
                        Ok(Some(value))
                    }
                    Err(_) => Err(original_error),
                }
            } else {
                Err(original_error)
            }
        }
    }
}

fn fix_unclosed_json_delimiters(arguments: &str) -> Option<String> {
    let trimmed = arguments.trim_end();
    if trimmed.is_empty() {
        return None;
    }

    let trailing_whitespace = &arguments[trimmed.len()..];
    let mut result = trimmed.to_string();
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in trimmed.chars() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else {
            match ch {
                '"' => in_string = true,
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '(' => stack.push(')'),
                '}' | ']' | ')' => {
                    if let Some(expected) = stack.pop() {
                        if expected != ch {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }

    if stack.is_empty() {
        return None;
    }

    while let Some(ch) = stack.pop() {
        result.push(ch);
    }

    result.push_str(trailing_whitespace);

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::fix_unclosed_json_delimiters;
    use super::parse_tool_arguments;

    #[test]
    fn parse_tool_arguments_handles_missing_closing_brace() {
        let args = "{\"foo\": \"bar\"";
        let parsed = parse_tool_arguments(args).expect("should parse");
        let value = parsed.expect("expected some value");
        assert_eq!(value["foo"], "bar");
    }

    #[test]
    fn fix_unclosed_json_delimiters_adds_multiple_missing_braces() {
        let args = "{\"outer\": {\"inner\": 1";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"outer\": {\"inner\": 1}}");
    }

    #[test]
    fn fix_unclosed_json_delimiters_adds_missing_array_bracket() {
        let args = "{\"items\": [1, 2";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"items\": [1, 2]}");
    }

    #[test]
    fn fix_unclosed_json_delimiters_ignores_braces_in_strings() {
        let args = "{\"text\": \"use {curly}\"";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"text\": \"use {curly}\"}");
    }

    #[test]
    fn fix_unclosed_json_delimiters_preserves_trailing_whitespace() {
        let args = "{\"foo\": 1\n";
        let fixed = fix_unclosed_json_delimiters(args).expect("should fix");
        assert_eq!(fixed, "{\"foo\": 1}\n");
    }

    #[test]
    fn fix_unclosed_json_delimiters_returns_none_when_not_needed() {
        let args = "{\"foo\": true}";
        assert!(fix_unclosed_json_delimiters(args).is_none());
    }
}
