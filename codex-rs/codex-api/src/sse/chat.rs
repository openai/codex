use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use codex_client::StreamResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

pub(crate) fn spawn_chat_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<std::sync::Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_chat_sse(stream_response.bytes, tx_event, idle_timeout, telemetry).await;
    });
    ResponseStream { rx_event }
}

pub async fn process_chat_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<std::sync::Arc<dyn SseTelemetry>>,
) where
    S: Stream<Item = Result<bytes::Bytes, codex_client::TransportError>> + Unpin,
{
    let mut stream = stream.eventsource();

    #[derive(Default)]
    struct FunctionCallState {
        name: Option<String>,
        arguments: String,
        call_id: Option<String>,
        active: bool,
    }

    let mut fn_call_state = FunctionCallState::default();
    let mut assistant_item: Option<ResponseItem> = None;
    let mut reasoning_item: Option<ResponseItem> = None;
    let mut completed_sent = false;

    loop {
        let response = timeout(idle_timeout, stream.next()).await;
        if let Some(t) = telemetry.as_ref() {
            t.on_sse_poll(&response, idle_timeout);
        }
        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                let _ = tx_event.send(Err(ApiError::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                if let Some(reasoning) = reasoning_item {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(reasoning)))
                        .await;
                }

                if let Some(assistant) = assistant_item {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                        .await;
                }
                if !completed_sent {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::Completed {
                            response_id: String::new(),
                            token_usage: None,
                        }))
                        .await;
                }
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        trace!("SSE event: {}", sse.data);

        if sse.data.trim().is_empty() {
            continue;
        }

        let cooked = sse.data.replace("\\", "\\\\");
        let value: serde_json::Value = match serde_json::from_str(&cooked) {
            Ok(val) => val,
            Err(err) => {
                debug!(
                    "Failed to parse ChatCompletions SSE event: {err}, data: {}",
                    &sse.data
                );
                continue;
            }
        };

        let Some(choices) = value.get("choices").and_then(|c| c.as_array()) else {
            continue;
        };

        for choice in choices {
            let finish_reason = choice.get("finish_reason").and_then(|r| r.as_str());
            if finish_reason == Some("stop") {
                if let Some(reasoning) = reasoning_item.take() {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(reasoning)))
                        .await;
                }

                if let Some(assistant) = assistant_item.take() {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                        .await;
                }
                if !completed_sent {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::Completed {
                            response_id: String::new(),
                            token_usage: None,
                        }))
                        .await;
                    completed_sent = true;
                }
                continue;
            }

            if finish_reason == Some("length") {
                let _ = tx_event
                    .send(Err(ApiError::Stream("context window exceeded".to_string())))
                    .await;
                return;
            }

            if let Some(delta) = choice.get("delta") {
                if let Some(reasoning) = delta.get("reasoning")
                    && reasoning.is_string()
                {
                    let text = reasoning.as_str().unwrap_or_default().to_string();
                    append_reasoning_text(&tx_event, &mut reasoning_item, text).await;
                }

                if let Some(content) = delta.get("content") {
                    if content.is_array() {
                        for item in content.as_array().unwrap_or(&vec![]) {
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                append_assistant_text(
                                    &tx_event,
                                    &mut assistant_item,
                                    text.to_string(),
                                )
                                .await;
                            }
                        }
                    } else if let Some(text) = content.as_str() {
                        append_assistant_text(&tx_event, &mut assistant_item, text.to_string())
                            .await;
                    }
                }

                if let Some(tool_calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                    for tool_call in tool_calls {
                        let mut name = None;
                        if let Some(func) = tool_call.get("function") {
                            name = func
                                .get("name")
                                .and_then(|n| n.as_str())
                                .map(str::to_string);
                        }
                        if let Some(id) = tool_call.get("id").and_then(|i| i.as_str()) {
                            fn_call_state.call_id = Some(id.to_string());
                        }
                        if let Some(arguments) = tool_call
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                        {
                            fn_call_state.arguments.push_str(arguments);
                        }
                        if let Some(fname) = name {
                            fn_call_state.name = Some(fname);
                        }
                        fn_call_state.active = true;
                    }
                }
            }

            if choice.get("finish_reason").and_then(|v| v.as_str()) == Some("tool_calls") {
                let Some(call_id) = fn_call_state.call_id.take() else {
                    continue;
                };

                let item = ResponseItem::FunctionCall {
                    id: None,
                    name: fn_call_state.name.take().unwrap_or_default(),
                    arguments: fn_call_state.arguments.clone(),
                    call_id,
                };
                fn_call_state = FunctionCallState::default();
                let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }
        }
    }
}

async fn append_assistant_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
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
        content.push(ContentItem::OutputText { text: text.clone() });
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputTextDelta(text.clone())))
            .await;
    }
}

async fn append_reasoning_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
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
        let content_index = content.len() as i64;
        content.push(ReasoningItemContent::ReasoningText { text: text.clone() });

        let _ = tx_event
            .send(Ok(ResponseEvent::ReasoningContentDelta {
                delta: text.clone(),
                content_index,
            }))
            .await;
    }
}
