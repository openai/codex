use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::debug;

use crate::client::WireResponseDecoder;
use crate::error::Error;
use crate::error::Result;
use crate::stream::WireEvent;

async fn send_wire_event(tx: &mpsc::Sender<crate::error::Result<WireEvent>>, event: WireEvent) {
    let _ = tx.send(Ok(event)).await;
}

#[derive(Default)]
struct FunctionCallState {
    active: bool,
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Default, Deserialize)]
struct ChatChunk {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    delta: Option<ChatDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Vec<DeltaText>,
    #[serde(default)]
    reasoning_content: Vec<DeltaText>,
    #[serde(default)]
    tool_calls: Vec<ChatToolCall>,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaText {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Default, Deserialize)]
struct ChatToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChatFunction>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatFunction {
    #[serde(default)]
    name: String,
    #[serde(default)]
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
        delta: &ChatDelta,
        tx: &mpsc::Sender<crate::error::Result<WireEvent>>,
    ) {
        for piece in &delta.content {
            if !piece.text.is_empty() {
                self.push_assistant_text(&piece.text, tx).await;
            }
        }

        for entry in &delta.reasoning_content {
            if !entry.text.is_empty() {
                self.push_reasoning_text(&entry.text, tx).await;
            }
        }

        self.record_tool_calls(&delta.tool_calls);
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
        send_wire_event(tx, WireEvent::OutputItemAdded(message)).await;
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
        send_wire_event(tx, WireEvent::OutputItemAdded(reasoning_item)).await;
    }

    fn record_tool_calls(&mut self, tool_calls: &[ChatToolCall]) {
        for call in tool_calls {
            if let Some(id_val) = &call.id {
                self.fn_call_state.call_id = Some(id_val.clone());
            }
            if let Some(function) = &call.function {
                if !function.name.is_empty() {
                    self.fn_call_state.name = Some(function.name.clone());
                    self.fn_call_state.active = true;
                }
                if !function.arguments.is_empty() {
                    self.fn_call_state.arguments.push_str(&function.arguments);
                }
            }
        }
    }

    fn finish_function_call(&mut self) -> Option<ResponseItem> {
        if !self.fn_call_state.active {
            return None;
        }
        let function_name = self.fn_call_state.name.take().unwrap_or_default();
        let call_id = self.fn_call_state.call_id.take().unwrap_or_default();
        let arguments = std::mem::take(&mut self.fn_call_state.arguments);
        self.fn_call_state = FunctionCallState::default();

        Some(ResponseItem::FunctionCall {
            id: Some(call_id.clone()),
            name: function_name,
            arguments,
            call_id,
        })
    }

    fn finish_reasoning(&mut self) -> Option<ResponseItem> {
        if !self.reasoning_started {
            return None;
        }

        let mut content = Vec::new();
        let text = std::mem::take(&mut self.reasoning_text);
        if !text.is_empty() {
            content.push(ReasoningItemContent::ReasoningText { text });
        }
        self.reasoning_started = false;

        Some(ResponseItem::Reasoning {
            id: String::new(),
            summary: vec![],
            content: Some(content),
            encrypted_content: None,
        })
    }

    fn finish_assistant(&mut self) -> Option<ResponseItem> {
        if !self.assistant_started {
            return None;
        }
        let text = std::mem::take(&mut self.assistant_text);
        self.assistant_started = false;

        Some(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText { text }],
        })
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
        let chunk = serde_json::from_str::<ChatChunk>(json).map_err(|err| {
            debug!("failed to parse Chat SSE JSON: {}", json);
            Error::Other(format!("failed to parse Chat SSE JSON: {err}"))
        })?;

        for choice in chunk.choices {
            self.emit_created_once(tx).await;

            if let Some(delta) = &choice.delta {
                self.handle_content_delta(delta, tx).await;
            }

            match choice.finish_reason.as_deref() {
                Some("tool_calls") => {
                    if let Some(item) = self.finish_function_call() {
                        send_wire_event(tx, WireEvent::OutputItemDone(item)).await;
                    }
                }
                Some("stop") | Some("length") => {
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

        Ok(())
    }
}
