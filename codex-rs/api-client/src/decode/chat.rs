use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use tokio::sync::mpsc;
use tracing::debug;

use crate::error::Result;
use crate::stream::ResponseEvent;

pub struct ChatSseDecoder {
    fn_call_state: FunctionCallState,
    assistant_item: Option<ResponseItem>,
    reasoning_item: Option<ResponseItem>,
}

#[derive(Default)]
struct FunctionCallState {
    name: Option<String>,
    arguments: String,
    call_id: Option<String>,
    active: bool,
}

impl ChatSseDecoder {
    pub fn new() -> Self {
        Self {
            fn_call_state: FunctionCallState::default(),
            assistant_item: None,
            reasoning_item: None,
        }
    }
}

#[async_trait]
impl crate::client::ResponseDecoder for ChatSseDecoder {
    async fn on_frame(
        &mut self,
        json: &str,
        tx: &mpsc::Sender<Result<ResponseEvent>>,
        _otel: &OtelEventManager,
    ) -> Result<()> {
        // Chat sends a terminal "[DONE]" frame; we ignore it here. Caller should handle end-of-stream.
        let Ok(parsed_chunk) = serde_json::from_str::<serde_json::Value>(json) else {
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
                            append_assistant_text(tx, &mut self.assistant_item, text.to_string())
                                .await;
                            let _ = tx
                                .send(Ok(ResponseEvent::OutputTextDelta(text.to_string())))
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
                            append_reasoning_text(tx, &mut self.reasoning_item, text.to_string())
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

                let item = ResponseItem::FunctionCall {
                    id: Some(call_id.clone()),
                    call_id,
                    name: function_name,
                    arguments,
                };
                let _ = tx.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }
        }

        Ok(())
    }
}

async fn append_assistant_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
    assistant_item: &mut Option<ResponseItem>,
    text: String,
) {
    if assistant_item.is_none() {
        let item = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![],
        };
        *assistant_item = Some(item.clone());
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemAdded(item)))
            .await;
    }

    if let Some(ResponseItem::Message { content, .. }) = assistant_item {
        content.push(ContentItem::OutputText { text });
    }
}

async fn append_reasoning_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
    reasoning_item: &mut Option<ResponseItem>,
    text: String,
) {
    if reasoning_item.is_none() {
        let item = ResponseItem::Reasoning {
            id: String::new(),
            summary: Vec::new(),
            content: Some(vec![]),
            encrypted_content: None,
        };
        *reasoning_item = Some(item.clone());
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemAdded(item)))
            .await;
    }

    if let Some(ResponseItem::Reasoning {
        content: Some(content),
        ..
    }) = reasoning_item
    {
        content.push(ReasoningItemContent::ReasoningText { text });
    }
}
