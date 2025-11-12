use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::debug;

use crate::client::WireResponseDecoder;
use crate::error::Error;
use crate::error::Result;
use crate::stream::WireEvent;

async fn send_wire_event(tx: &mpsc::Sender<crate::error::Result<WireEvent>>, event: WireEvent) {
    let _ = tx.send(Ok(event)).await;
}

fn serialize_response_item(item: ResponseItem) -> Value {
    serde_json::to_value(item).unwrap_or_else(|_| Value::String(String::new()))
}

#[derive(Default)]
struct FunctionCallState {
    active: bool,
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Default)]
pub struct WireChatSseDecoder {
    fn_call_state: FunctionCallState,
    created_emitted: bool,
    assistant_started: bool,
    assistant_text: String,
    reasoning_started: bool,
    reasoning_text: String,
}

impl WireChatSseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    async fn emit_created_once(&mut self, tx: &mpsc::Sender<crate::error::Result<WireEvent>>) {
        if self.created_emitted {
            return;
        }
        send_wire_event(tx, WireEvent::Created).await;
        self.created_emitted = true;
    }

    async fn handle_content_delta(
        &mut self,
        delta: &Value,
        tx: &mpsc::Sender<crate::error::Result<WireEvent>>,
    ) {
        if let Some(content) = delta.get("content").and_then(|c| c.as_array()) {
            for piece in content {
                if let Some(text) = piece.get("text").and_then(|t| t.as_str()) {
                    self.push_assistant_text(text, tx).await;
                }
            }
        }

        if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_array()) {
            for entry in reasoning {
                if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                    self.push_reasoning_text(text, tx).await;
                }
            }
        }
    }

    async fn push_assistant_text(
        &mut self,
        text: &str,
        tx: &mpsc::Sender<crate::error::Result<WireEvent>>,
    ) {
        self.start_assistant(tx).await;
        self.assistant_text.push_str(text);
        send_wire_event(tx, WireEvent::OutputTextDelta(text.to_string())).await;
    }

    async fn push_reasoning_text(
        &mut self,
        text: &str,
        tx: &mpsc::Sender<crate::error::Result<WireEvent>>,
    ) {
        self.start_reasoning(tx).await;
        self.reasoning_text.push_str(text);
        send_wire_event(tx, WireEvent::ReasoningContentDelta(text.to_string())).await;
    }

    async fn start_assistant(&mut self, tx: &mpsc::Sender<crate::error::Result<WireEvent>>) {
        if self.assistant_started {
            return;
        }
        self.assistant_started = true;
        let message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: String::new(),
            }],
        };
        send_wire_event(
            tx,
            WireEvent::OutputItemAdded(serialize_response_item(message)),
        )
        .await;
    }

    async fn start_reasoning(&mut self, tx: &mpsc::Sender<crate::error::Result<WireEvent>>) {
        if self.reasoning_started {
            return;
        }
        self.reasoning_started = true;
        let reasoning_item = ResponseItem::Reasoning {
            id: String::new(),
            summary: vec![],
            content: None,
            encrypted_content: None,
        };
        send_wire_event(
            tx,
            WireEvent::OutputItemAdded(serialize_response_item(reasoning_item)),
        )
        .await;
    }

    fn record_tool_calls(&mut self, delta: &Value) {
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
            for call in tool_calls {
                if let Some(id_val) = call.get("id").and_then(|id| id.as_str()) {
                    self.fn_call_state.call_id = Some(id_val.to_string());
                }
                if let Some(function) = call.get("function") {
                    if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                        self.fn_call_state.name = Some(name.to_string());
                        self.fn_call_state.active = true;
                    }
                    if let Some(args) = function.get("arguments").and_then(|a| a.as_str()) {
                        self.fn_call_state.arguments.push_str(args);
                    }
                }
            }
        }
    }

    fn finish_function_call(&mut self) -> Option<Value> {
        if !self.fn_call_state.active {
            return None;
        }
        let function_name = self.fn_call_state.name.take().unwrap_or_default();
        let call_id = self.fn_call_state.call_id.take().unwrap_or_default();
        let arguments = std::mem::take(&mut self.fn_call_state.arguments);
        self.fn_call_state = FunctionCallState::default();

        Some(serde_json::json!({
            "type": "function_call",
            "id": call_id,
            "call_id": call_id,
            "name": function_name,
            "arguments": arguments,
        }))
    }

    fn finish_reasoning(&mut self) -> Option<Value> {
        if !self.reasoning_started {
            return None;
        }

        let mut content = Vec::new();
        let text = std::mem::take(&mut self.reasoning_text);
        if !text.is_empty() {
            content.push(ReasoningItemContent::ReasoningText { text });
        }
        self.reasoning_started = false;

        Some(serialize_response_item(ResponseItem::Reasoning {
            id: String::new(),
            summary: vec![],
            content: Some(content),
            encrypted_content: None,
        }))
    }

    fn finish_assistant(&mut self) -> Option<Value> {
        if !self.assistant_started {
            return None;
        }
        let text = std::mem::take(&mut self.assistant_text);
        self.assistant_started = false;

        Some(serialize_response_item(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText { text }],
        }))
    }

    fn reset_reasoning_and_assistant(&mut self) {
        self.assistant_started = false;
        self.assistant_text.clear();
        self.reasoning_started = false;
        self.reasoning_text.clear();
    }
}

#[async_trait]
impl WireResponseDecoder for WireChatSseDecoder {
    async fn on_frame(
        &mut self,
        json: &str,
        tx: &mpsc::Sender<crate::error::Result<WireEvent>>,
        _otel: &OtelEventManager,
    ) -> Result<()> {
        // Chat sends a terminal "[DONE]" frame; ignore it. Treat other parse errors as failures.
        let parsed_chunk = serde_json::from_str::<Value>(json).map_err(|err| {
            debug!("failed to parse Chat SSE JSON: {}", json);
            Error::Other(format!("failed to parse Chat SSE JSON: {err}"))
        })?;

        let choices = parsed_chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .cloned()
            .unwrap_or_default();

        for choice in choices {
            self.emit_created_once(tx).await;

            if let Some(delta) = choice.get("delta") {
                self.handle_content_delta(delta, tx).await;
                self.record_tool_calls(delta);
            }

            if let Some(finish_reason) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                match finish_reason {
                    "tool_calls" => {
                        if let Some(item) = self.finish_function_call() {
                            send_wire_event(tx, WireEvent::OutputItemDone(item)).await;
                        }
                    }
                    "stop" | "length" => {
                        if let Some(reasoning_item) = self.finish_reasoning() {
                            send_wire_event(tx, WireEvent::OutputItemDone(reasoning_item)).await;
                        }

                        if let Some(message) = self.finish_assistant() {
                            send_wire_event(tx, WireEvent::OutputItemDone(message)).await;
                        }

                        send_wire_event(
                            tx,
                            WireEvent::Completed {
                                response_id: String::new(),
                                token_usage: None,
                            },
                        )
                        .await;

                        self.reset_reasoning_and_assistant();
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
