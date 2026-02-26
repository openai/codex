use bytes::Bytes;
use serde_json::Value;
use serde_json::json;

use crate::translate::types::ReasoningContentEntry;
use crate::translate::types::ReasoningSummaryEntry;

/// A single SSE event ready for serialization.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: Value,
}

impl SseEvent {
    /// Serialize to SSE wire format: `event: <type>\ndata: <json>\n\n`
    pub fn to_bytes(&self) -> Bytes {
        let line = format!(
            "event: {}\ndata: {}\n\n",
            self.event_type,
            serde_json::to_string(&self.data).unwrap_or_default()
        );
        Bytes::from(line)
    }

    /// Return the data as a `serde_json::Value` (for tests).
    pub fn data_json(&self) -> &Value {
        &self.data
    }

    // ------------------------------------------------------------------
    // Builder methods
    // ------------------------------------------------------------------

    pub fn response_created(response_id: &str) -> Self {
        Self {
            event_type: "response.created".to_string(),
            data: json!({
                "type": "response.created",
                "response": {
                    "id": response_id,
                }
            }),
        }
    }

    pub fn response_completed(response_id: &str) -> Self {
        Self {
            event_type: "response.completed".to_string(),
            data: json!({
                "type": "response.completed",
                "response": {
                    "id": response_id,
                    "usage": {
                        "input_tokens": 0,
                        "input_tokens_details": null,
                        "output_tokens": 0,
                        "output_tokens_details": null,
                        "total_tokens": 0
                    }
                }
            }),
        }
    }

    pub fn response_failed(code: &str, message: &str) -> Self {
        Self {
            event_type: "response.failed".to_string(),
            data: json!({
                "type": "response.failed",
                "response": {
                    "error": {
                        "code": code,
                        "message": message
                    }
                }
            }),
        }
    }

    pub fn output_item_done_message(item_id: &str, role: &str, text: &str) -> Self {
        Self {
            event_type: "response.output_item.done".to_string(),
            data: json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "role": role,
                    "id": item_id,
                    "content": [{"type": "output_text", "text": text}]
                }
            }),
        }
    }

    pub fn output_item_done_function_call(call_id: &str, name: &str, arguments: &str) -> Self {
        Self {
            event_type: "response.output_item.done".to_string(),
            data: json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments
                }
            }),
        }
    }

    pub fn output_item_done_reasoning(
        item_id: &str,
        summary: &[ReasoningSummaryEntry],
        content: &[ReasoningContentEntry],
    ) -> Self {
        let summary_entries: Vec<Value> = summary
            .iter()
            .map(|s| json!({"type": "summary_text", "text": s.text}))
            .collect();

        let mut item = json!({
            "type": "reasoning",
            "id": item_id,
            "summary": summary_entries,
        });

        if !content.is_empty() {
            let content_entries: Vec<Value> = content
                .iter()
                .map(|c| json!({"type": "reasoning_text", "text": c.text}))
                .collect();
            item["content"] = Value::Array(content_entries);
        }

        Self {
            event_type: "response.output_item.done".to_string(),
            data: json!({
                "type": "response.output_item.done",
                "item": item,
            }),
        }
    }

    pub fn output_text_delta(delta: &str) -> Self {
        Self {
            event_type: "response.output_text.delta".to_string(),
            data: json!({
                "type": "response.output_text.delta",
                "delta": delta,
            }),
        }
    }

    pub fn reasoning_summary_text_delta(delta: &str, summary_index: u32) -> Self {
        Self {
            event_type: "response.reasoning_summary_text.delta".to_string(),
            data: json!({
                "type": "response.reasoning_summary_text.delta",
                "delta": delta,
                "summary_index": summary_index,
            }),
        }
    }

    pub fn reasoning_text_delta(delta: &str, content_index: u32) -> Self {
        Self {
            event_type: "response.reasoning_text.delta".to_string(),
            data: json!({
                "type": "response.reasoning_text.delta",
                "delta": delta,
                "content_index": content_index,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_wire_format() {
        let event = SseEvent::output_text_delta("Hello");
        let bytes = event.to_bytes();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.starts_with("event: response.output_text.delta\n"));
        assert!(s.contains("data: "));
        assert!(s.ends_with("\n\n"));

        let data_line = s.lines().nth(1).unwrap();
        let json_str = data_line.strip_prefix("data: ").unwrap();
        let parsed: Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["type"], "response.output_text.delta");
        assert_eq!(parsed["delta"], "Hello");
    }

    #[test]
    fn test_response_created() {
        let event = SseEvent::response_created("resp_123");
        let data = event.data_json();
        assert_eq!(data["type"], "response.created");
        assert_eq!(data["response"]["id"], "resp_123");
    }

    #[test]
    fn test_response_completed() {
        let event = SseEvent::response_completed("resp_123");
        let data = event.data_json();
        assert_eq!(data["type"], "response.completed");
        assert_eq!(data["response"]["id"], "resp_123");
        assert_eq!(data["response"]["usage"]["total_tokens"], 0);
    }

    #[test]
    fn test_function_call_event() {
        let event = SseEvent::output_item_done_function_call("call_1", "read_file", "{\"path\":\"/tmp\"}");
        let data = event.data_json();
        assert_eq!(data["item"]["type"], "function_call");
        assert_eq!(data["item"]["call_id"], "call_1");
        assert_eq!(data["item"]["name"], "read_file");
        assert_eq!(data["item"]["arguments"], "{\"path\":\"/tmp\"}");
    }

    #[test]
    fn test_assistant_message_event() {
        let event = SseEvent::output_item_done_message("item_0", "assistant", "Hi there");
        let data = event.data_json();
        assert_eq!(data["item"]["type"], "message");
        assert_eq!(data["item"]["role"], "assistant");
        assert_eq!(data["item"]["content"][0]["type"], "output_text");
        assert_eq!(data["item"]["content"][0]["text"], "Hi there");
    }

    #[test]
    fn test_reasoning_event() {
        use crate::translate::types::ReasoningContentEntry;
        use crate::translate::types::ReasoningSummaryEntry;

        let event = SseEvent::output_item_done_reasoning(
            "item_0",
            &[ReasoningSummaryEntry {
                entry_type: "summary_text".to_string(),
                text: "thinking".to_string(),
            }],
            &[ReasoningContentEntry {
                entry_type: "reasoning_text".to_string(),
                text: "deep thought".to_string(),
            }],
        );
        let data = event.data_json();
        assert_eq!(data["item"]["type"], "reasoning");
        assert_eq!(data["item"]["summary"][0]["text"], "thinking");
        assert_eq!(data["item"]["content"][0]["text"], "deep thought");
    }

    #[test]
    fn test_response_failed() {
        let event = SseEvent::response_failed("server_error", "something went wrong");
        let data = event.data_json();
        assert_eq!(data["type"], "response.failed");
        assert_eq!(data["response"]["error"]["code"], "server_error");
        assert_eq!(data["response"]["error"]["message"], "something went wrong");
    }

    #[test]
    fn test_reasoning_summary_delta() {
        let event = SseEvent::reasoning_summary_text_delta("thinking", 0);
        let data = event.data_json();
        assert_eq!(data["type"], "response.reasoning_summary_text.delta");
        assert_eq!(data["delta"], "thinking");
        assert_eq!(data["summary_index"], 0);
    }

    #[test]
    fn test_reasoning_text_delta() {
        let event = SseEvent::reasoning_text_delta("deep", 0);
        let data = event.data_json();
        assert_eq!(data["type"], "response.reasoning_text.delta");
        assert_eq!(data["delta"], "deep");
        assert_eq!(data["content_index"], 0);
    }
}
