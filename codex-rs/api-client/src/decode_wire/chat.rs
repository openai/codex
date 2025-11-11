use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::debug;

use crate::client::WireResponseDecoder;
use crate::error::Result;
use crate::stream::WireEvent;

#[derive(Default)]
struct FunctionCallState {
    active: bool,
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
}

pub struct WireChatSseDecoder {
    fn_call_state: FunctionCallState,
}

impl WireChatSseDecoder {
    pub fn new() -> Self {
        Self {
            fn_call_state: FunctionCallState::default(),
        }
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
        // Chat sends a terminal "[DONE]" frame; ignore it.
        let Ok(parsed_chunk) = serde_json::from_str::<Value>(json) else {
            debug!("failed to parse Chat SSE JSON: {}", json);
            return Ok(());
        };

        let choices = parsed_chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .cloned()
            .unwrap_or_default();

        for choice in choices {
            if let Some(delta) = choice.get("delta") {
                if let Some(content) = delta.get("content").and_then(|c| c.as_array()) {
                    for piece in content {
                        if let Some(text) = piece.get("text").and_then(|t| t.as_str()) {
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
                            let _ = tx
                                .send(Ok(WireEvent::ReasoningContentDelta(text.to_string())))
                                .await;
                        }
                    }
                }
            }

            if let Some(finish_reason) = choice.get("finish_reason").and_then(|f| f.as_str())
                && finish_reason == "tool_calls"
                && self.fn_call_state.active
            {
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
        }

        Ok(())
    }
}
