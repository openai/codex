use std::collections::HashMap;
use std::collections::HashSet;

use super::types::StreamDelta;
use super::types::TaskMessage;
use super::types::TaskMessageContent;
use super::types::TaskMessageUpdate;
use crate::sse_writer::SseEvent;
use crate::tool_routing::ToolRoute;
use crate::tool_routing::route_tool;

/// State buffer for accumulating streamed tool-call deltas until the full
/// tool request is available.
#[derive(Debug, Default)]
pub struct ToolDeltaBuffer {
    /// tool_call_id -> (name, arguments_so_far)
    pub pending: HashMap<String, (String, String)>,
}

/// Translate a complete (non-streaming) Agentex response into SSE events.
pub fn translate_task_messages(
    messages: &[TaskMessage],
    agent_tools: &HashSet<String>,
    response_id: &str,
) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut item_index: u32 = 0;

    for msg in messages {
        for content in &msg.content {
            match content {
                TaskMessageContent::Text { text, author, .. } => {
                    if author.as_deref() == Some("agent")
                        || author.as_deref() == Some("assistant")
                        || msg.role == "assistant"
                    {
                        let item_id = format!("{response_id}_item_{item_index}");
                        events.push(SseEvent::output_item_done_message(
                            &item_id, "assistant", text,
                        ));
                        item_index += 1;
                    }
                }

                TaskMessageContent::ToolRequest {
                    tool_call_id,
                    name,
                    arguments,
                } => match route_tool(name, agent_tools) {
                    ToolRoute::CodexLocal => {
                        events.push(SseEvent::output_item_done_function_call(
                            tool_call_id,
                            name,
                            arguments,
                        ));
                        item_index += 1;
                    }
                    ToolRoute::Agent => {
                        // Suppressed — agent handles internally
                    }
                },

                TaskMessageContent::Reasoning { summary, content } => {
                    let item_id = format!("{response_id}_item_{item_index}");
                    events.push(SseEvent::output_item_done_reasoning(
                        &item_id, summary, content,
                    ));
                    item_index += 1;
                }

                TaskMessageContent::ToolResponse { .. } => {
                    // Tool responses from the agent are not emitted as SSE events.
                }
            }
        }
    }

    events
}

/// Translate a single streaming update into zero or more SSE events.
pub fn translate_stream_event(
    update: &TaskMessageUpdate,
    agent_tools: &HashSet<String>,
    buffer: &mut ToolDeltaBuffer,
    response_id: &str,
    item_index: &mut u32,
) -> Vec<SseEvent> {
    match update {
        TaskMessageUpdate::Delta { delta } => match delta {
            StreamDelta::TextDelta { text } => {
                vec![SseEvent::output_text_delta(text)]
            }

            StreamDelta::ReasoningSummaryDelta { text } => {
                vec![SseEvent::reasoning_summary_text_delta(text, 0)]
            }

            StreamDelta::ReasoningContentDelta { text } => {
                vec![SseEvent::reasoning_text_delta(text, 0)]
            }

            StreamDelta::ToolRequestDelta {
                tool_call_id,
                name,
                arguments,
            } => {
                let entry = buffer
                    .pending
                    .entry(tool_call_id.clone())
                    .or_insert_with(|| (String::new(), String::new()));
                if let Some(n) = name {
                    entry.0.clone_from(n);
                }
                if let Some(args) = arguments {
                    entry.1.push_str(args);
                }
                vec![]
            }
        },

        TaskMessageUpdate::Full { message } | TaskMessageUpdate::Done { message } => {
            let mut events = Vec::new();

            for content in &message.content {
                match content {
                    TaskMessageContent::Text { text, author, .. } => {
                        if author.as_deref() == Some("agent")
                            || author.as_deref() == Some("assistant")
                            || message.role == "assistant"
                        {
                            let item_id = format!("{response_id}_item_{item_index}");
                            events.push(SseEvent::output_item_done_message(
                                &item_id, "assistant", text,
                            ));
                            *item_index += 1;
                        }
                    }

                    TaskMessageContent::ToolRequest {
                        tool_call_id,
                        name,
                        arguments,
                    } => {
                        // Flush any buffered delta for this tool call.
                        buffer.pending.remove(tool_call_id);

                        match route_tool(name, agent_tools) {
                            ToolRoute::CodexLocal => {
                                events.push(SseEvent::output_item_done_function_call(
                                    tool_call_id,
                                    name,
                                    arguments,
                                ));
                                *item_index += 1;
                            }
                            ToolRoute::Agent => {}
                        }
                    }

                    TaskMessageContent::Reasoning { summary, content } => {
                        let item_id = format!("{response_id}_item_{item_index}");
                        events.push(SseEvent::output_item_done_reasoning(
                            &item_id, summary, content,
                        ));
                        *item_index += 1;
                    }

                    TaskMessageContent::ToolResponse { .. } => {}
                }
            }

            // Flush any remaining buffered tool-call deltas on Done.
            if matches!(update, TaskMessageUpdate::Done { .. }) {
                for (call_id, (name, arguments)) in buffer.pending.drain() {
                    match route_tool(&name, agent_tools) {
                        ToolRoute::CodexLocal => {
                            events.push(SseEvent::output_item_done_function_call(
                                &call_id,
                                &name,
                                &arguments,
                            ));
                            *item_index += 1;
                        }
                        ToolRoute::Agent => {}
                    }
                }
            }

            events
        }

        TaskMessageUpdate::Start { .. } => {
            // Start events don't produce SSE output.
            vec![]
        }

        TaskMessageUpdate::Error { message } => {
            vec![SseEvent::response_failed("proxy_error", message)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::types::ReasoningContentEntry;
    use crate::translate::types::ReasoningSummaryEntry;

    #[test]
    fn test_translate_text_message() {
        let messages = vec![TaskMessage {
            role: "assistant".to_string(),
            content: vec![TaskMessageContent::Text {
                text: "Hello!".to_string(),
                author: Some("agent".to_string()),
                format: None,
            }],
        }];

        let events = translate_task_messages(&messages, &HashSet::new(), "resp_1");
        assert_eq!(events.len(), 1);
        let data = events[0].data_json();
        assert_eq!(data["type"], "response.output_item.done");
        assert_eq!(data["item"]["type"], "message");
        assert_eq!(data["item"]["content"][0]["text"], "Hello!");
    }

    #[test]
    fn test_translate_function_call_codex_local() {
        let messages = vec![TaskMessage {
            role: "assistant".to_string(),
            content: vec![TaskMessageContent::ToolRequest {
                tool_call_id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: "{\"path\":\"/tmp\"}".to_string(),
            }],
        }];

        let events = translate_task_messages(&messages, &HashSet::new(), "resp_1");
        assert_eq!(events.len(), 1);
        let data = events[0].data_json();
        assert_eq!(data["item"]["type"], "function_call");
        assert_eq!(data["item"]["call_id"], "call_1");
    }

    #[test]
    fn test_agent_tool_suppressed() {
        let mut agent_tools = HashSet::new();
        agent_tools.insert("search".to_string());

        let messages = vec![TaskMessage {
            role: "assistant".to_string(),
            content: vec![TaskMessageContent::ToolRequest {
                tool_call_id: "call_1".to_string(),
                name: "search".to_string(),
                arguments: "{}".to_string(),
            }],
        }];

        let events = translate_task_messages(&messages, &agent_tools, "resp_1");
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_translate_reasoning() {
        let messages = vec![TaskMessage {
            role: "assistant".to_string(),
            content: vec![TaskMessageContent::Reasoning {
                summary: vec![ReasoningSummaryEntry {
                    entry_type: "summary_text".to_string(),
                    text: "thinking".to_string(),
                }],
                content: vec![ReasoningContentEntry {
                    entry_type: "reasoning_text".to_string(),
                    text: "deep thought".to_string(),
                }],
            }],
        }];

        let events = translate_task_messages(&messages, &HashSet::new(), "resp_1");
        assert_eq!(events.len(), 1);
        let data = events[0].data_json();
        assert_eq!(data["item"]["type"], "reasoning");
    }

    #[test]
    fn test_stream_text_delta() {
        let update = TaskMessageUpdate::Delta {
            delta: StreamDelta::TextDelta {
                text: "Hello".to_string(),
            },
        };

        let mut buffer = ToolDeltaBuffer::default();
        let mut idx = 0;
        let events =
            translate_stream_event(&update, &HashSet::new(), &mut buffer, "resp_1", &mut idx);
        assert_eq!(events.len(), 1);
        let data = events[0].data_json();
        assert_eq!(data["type"], "response.output_text.delta");
        assert_eq!(data["delta"], "Hello");
    }

    #[test]
    fn test_stream_tool_delta_buffering() {
        let agent_tools = HashSet::new();
        let mut buffer = ToolDeltaBuffer::default();
        let mut idx = 0;

        // First delta — accumulates, no output.
        let update1 = TaskMessageUpdate::Delta {
            delta: StreamDelta::ToolRequestDelta {
                tool_call_id: "call_1".to_string(),
                name: Some("read_file".to_string()),
                arguments: Some("{\"pa".to_string()),
            },
        };
        let events =
            translate_stream_event(&update1, &agent_tools, &mut buffer, "resp_1", &mut idx);
        assert!(events.is_empty());

        // Done with full message — flushes.
        let update2 = TaskMessageUpdate::Done {
            message: TaskMessage {
                role: "assistant".to_string(),
                content: vec![TaskMessageContent::ToolRequest {
                    tool_call_id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: "{\"path\":\"/tmp\"}".to_string(),
                }],
            },
        };
        let events =
            translate_stream_event(&update2, &agent_tools, &mut buffer, "resp_1", &mut idx);
        assert_eq!(events.len(), 1);
        let data = events[0].data_json();
        assert_eq!(data["item"]["type"], "function_call");
    }
}
