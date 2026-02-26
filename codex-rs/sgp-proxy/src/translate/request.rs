use serde_json::Value;

use super::types::TaskMessage;
use super::types::TaskMessageContent;
use crate::error::ProxyError;

/// Translate a Responses API request body into Agentex `TaskMessage` items.
///
/// The function walks the `input` array and the optional `instructions` field,
/// producing content entries that can be sent via `message/send`.
pub fn translate_request(
    body: &Value,
    is_first_turn: bool,
) -> Result<Vec<TaskMessage>, ProxyError> {
    let mut messages: Vec<TaskMessage> = Vec::new();

    // Inject instructions as first user text on first turn (or always in per-request mode).
    if is_first_turn
        && let Some(instructions) = body.get("instructions").and_then(Value::as_str)
        && !instructions.is_empty()
    {
        messages.push(TaskMessage {
            role: "user".to_string(),
            content: vec![TaskMessageContent::Text {
                text: instructions.to_string(),
                author: Some("user".to_string()),
                format: Some("markdown".to_string()),
            }],
        });
    }

    let input = body
        .get("input")
        .and_then(Value::as_array)
        .ok_or_else(|| ProxyError::RequestParse("missing or invalid 'input' array".into()))?;

    for item in input {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");

        match item_type {
            "message" => {
                let role = item
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("user");
                let author = match role {
                    "assistant" => "agent",
                    "developer" => "user",
                    _ => "user",
                };
                let agentex_role = match role {
                    "assistant" => "assistant",
                    _ => "user",
                };

                let mut content_items = Vec::new();
                if let Some(content_arr) = item.get("content").and_then(Value::as_array) {
                    for c in content_arr {
                        let c_type = c.get("type").and_then(Value::as_str).unwrap_or("");
                        match c_type {
                            "input_text" | "output_text" => {
                                if let Some(text) = c.get("text").and_then(Value::as_str) {
                                    let mut tc = TaskMessageContent::Text {
                                        text: text.to_string(),
                                        author: Some(author.to_string()),
                                        format: None,
                                    };
                                    if role == "developer"
                                        && let TaskMessageContent::Text {
                                            ref mut format, ..
                                        } = tc
                                    {
                                        *format = Some("markdown".to_string());
                                    }
                                    content_items.push(tc);
                                }
                            }
                            _ => {
                                // Skip unsupported content types (input_image, etc.)
                            }
                        }
                    }
                }

                if !content_items.is_empty() {
                    messages.push(TaskMessage {
                        role: agentex_role.to_string(),
                        content: content_items,
                    });
                }
            }

            "function_call" => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_string();

                messages.push(TaskMessage {
                    role: "assistant".to_string(),
                    content: vec![TaskMessageContent::ToolRequest {
                        tool_call_id: call_id,
                        name,
                        arguments,
                    }],
                });
            }

            "function_call_output" => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                // The output can be a string or a nested object with "output" field.
                let output = if let Some(s) = item.get("output").and_then(Value::as_str) {
                    s.to_string()
                } else if let Some(obj) = item.get("output") {
                    serde_json::to_string(obj).unwrap_or_default()
                } else {
                    String::new()
                };

                // We need the tool name for Agentex. It may not be in the
                // output item itself, so we use a placeholder that the caller
                // can resolve from session state.
                messages.push(TaskMessage {
                    role: "user".to_string(),
                    content: vec![TaskMessageContent::ToolResponse {
                        tool_call_id: call_id,
                        name: String::new(), // resolved by caller from session state
                        content: output,
                    }],
                });
            }

            "reasoning" => {
                let summary = item
                    .get("summary")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| {
                                Some(super::types::ReasoningSummaryEntry {
                                    entry_type: "summary_text".to_string(),
                                    text: s.get("text").and_then(Value::as_str)?.to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let content = item
                    .get("content")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|c| {
                                Some(super::types::ReasoningContentEntry {
                                    entry_type: "reasoning_text".to_string(),
                                    text: c.get("text").and_then(Value::as_str)?.to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                messages.push(TaskMessage {
                    role: "assistant".to_string(),
                    content: vec![TaskMessageContent::Reasoning { summary, content }],
                });
            }

            // Skip local_shell_call, web_search_call, etc.
            _ => {}
        }
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_translate_user_message() {
        let body = json!({
            "instructions": "You are a helpful assistant.",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Hello"}]
                }
            ]
        });

        let messages = translate_request(&body, true).unwrap();
        assert_eq!(messages.len(), 2);

        // First message: instructions
        match &messages[0].content[0] {
            TaskMessageContent::Text {
                text,
                author,
                format,
            } => {
                assert_eq!(text, "You are a helpful assistant.");
                assert_eq!(author.as_deref(), Some("user"));
                assert_eq!(format.as_deref(), Some("markdown"));
            }
            _ => panic!("expected Text"),
        }

        // Second message: user input
        assert_eq!(messages[1].role, "user");
        match &messages[1].content[0] {
            TaskMessageContent::Text { text, author, .. } => {
                assert_eq!(text, "Hello");
                assert_eq!(author.as_deref(), Some("user"));
            }
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_translate_assistant_message() {
        let body = json!({
            "input": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "Hi there"}]
                }
            ]
        });

        let messages = translate_request(&body, false).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
        match &messages[0].content[0] {
            TaskMessageContent::Text { text, author, .. } => {
                assert_eq!(text, "Hi there");
                assert_eq!(author.as_deref(), Some("agent"));
            }
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_translate_function_call_and_output() {
        let body = json!({
            "input": [
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "read_file",
                    "arguments": "{\"path\":\"/tmp/foo\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "file contents"
                }
            ]
        });

        let messages = translate_request(&body, false).unwrap();
        assert_eq!(messages.len(), 2);

        match &messages[0].content[0] {
            TaskMessageContent::ToolRequest {
                tool_call_id,
                name,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(name, "read_file");
                assert_eq!(arguments, "{\"path\":\"/tmp/foo\"}");
            }
            _ => panic!("expected ToolRequest"),
        }

        match &messages[1].content[0] {
            TaskMessageContent::ToolResponse {
                tool_call_id,
                content,
                ..
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(content, "file contents");
            }
            _ => panic!("expected ToolResponse"),
        }
    }

    #[test]
    fn test_translate_reasoning() {
        let body = json!({
            "input": [
                {
                    "type": "reasoning",
                    "id": "r1",
                    "summary": [{"type": "summary_text", "text": "thinking"}],
                    "content": [{"type": "reasoning_text", "text": "deep thought"}]
                }
            ]
        });

        let messages = translate_request(&body, false).unwrap();
        assert_eq!(messages.len(), 1);
        match &messages[0].content[0] {
            TaskMessageContent::Reasoning { summary, content } => {
                assert_eq!(summary.len(), 1);
                assert_eq!(summary[0].text, "thinking");
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "deep thought");
            }
            _ => panic!("expected Reasoning"),
        }
    }

    #[test]
    fn test_skip_instructions_on_non_first_turn() {
        let body = json!({
            "instructions": "You are a helpful assistant.",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Hello"}]
                }
            ]
        });

        let messages = translate_request(&body, false).unwrap();
        // Instructions should be skipped on non-first turn
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }
}
