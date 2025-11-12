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
            if !self.created_emitted {
                let _ = tx.send(Ok(WireEvent::Created)).await;
                self.created_emitted = true;
            }

            if let Some(delta) = choice.get("delta") {
                if let Some(content) = delta.get("content").and_then(|c| c.as_array()) {
                    for piece in content {
                        if let Some(text) = piece.get("text").and_then(|t| t.as_str()) {
                            if !self.assistant_started {
                                self.assistant_started = true;
                                let message = ResponseItem::Message {
                                    id: None,
                                    role: "assistant".to_string(),
                                    content: vec![ContentItem::OutputText {
                                        text: String::new(),
                                    }],
                                };
                                let value = serde_json::to_value(message)
                                    .unwrap_or_else(|_| Value::String(String::new()));
                                let _ = tx.send(Ok(WireEvent::OutputItemAdded(value))).await;
                            }
                            self.assistant_text.push_str(text);
                            let _ = tx
                                .send(Ok(WireEvent::OutputTextDelta(text.to_string())))
                                .await;
                        }
                    }
                }

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

                if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_array()) {
                    for entry in reasoning {
                        if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                            if !self.reasoning_started {
                                self.reasoning_started = true;
                                let reasoning_item = ResponseItem::Reasoning {
                                    id: String::new(),
                                    summary: vec![],
                                    content: None,
                                    encrypted_content: None,
                                };
                                let value = serde_json::to_value(reasoning_item)
                                    .unwrap_or_else(|_| Value::String(String::new()));
                                let _ = tx.send(Ok(WireEvent::OutputItemAdded(value))).await;
                            }
                            self.reasoning_text.push_str(text);
                            let _ = tx
                                .send(Ok(WireEvent::ReasoningContentDelta(text.to_string())))
                                .await;
                        }
                    }
                }
            }

            if let Some(finish_reason) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                match finish_reason {
                    "tool_calls" if self.fn_call_state.active => {
                        let function_name = self.fn_call_state.name.take().unwrap_or_default();
                        let call_id = self.fn_call_state.call_id.take().unwrap_or_default();
                        let arguments = self.fn_call_state.arguments.clone();
                        self.fn_call_state = FunctionCallState::default();

                        let item = serde_json::json!({
                            "type": "function_call",
                            "id": call_id,
                            "call_id": call_id,
                            "name": function_name,
                            "arguments": arguments,
                        });
                        let _ = tx.send(Ok(WireEvent::OutputItemDone(item))).await;
                    }
                    "stop" | "length" => {
                        if self.reasoning_started {
                            let mut content = Vec::new();
                            if !self.reasoning_text.is_empty() {
                                content.push(ReasoningItemContent::ReasoningText {
                                    text: self.reasoning_text.clone(),
                                });
                            }
                            let reasoning_item = ResponseItem::Reasoning {
                                id: String::new(),
                                summary: vec![],
                                content: Some(content),
                                encrypted_content: None,
                            };
                            let value = serde_json::to_value(reasoning_item)
                                .unwrap_or_else(|_| Value::String(String::new()));
                            let _ = tx.send(Ok(WireEvent::OutputItemDone(value))).await;
                        }

                        if self.assistant_started {
                            let message = ResponseItem::Message {
                                id: None,
                                role: "assistant".to_string(),
                                content: vec![ContentItem::OutputText {
                                    text: self.assistant_text.clone(),
                                }],
                            };
                            let value = serde_json::to_value(message)
                                .unwrap_or_else(|_| Value::String(String::new()));
                            let _ = tx.send(Ok(WireEvent::OutputItemDone(value))).await;
                        }

                        let _ = tx
                            .send(Ok(WireEvent::Completed {
                                response_id: String::new(),
                                token_usage: None,
                            }))
                            .await;

                        self.assistant_started = false;
                        self.assistant_text.clear();
                        self.reasoning_started = false;
                        self.reasoning_text.clear();
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
