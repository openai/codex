use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use codex_client::ByteStream;
use codex_client::StreamResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

const OPENAI_MODEL_HEADER: &str = "openai-model";

#[derive(Debug, Default)]
struct AggregatedToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    delta: ChatDelta,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ChatToolCallDelta {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChatFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

impl From<ChatUsage> for TokenUsage {
    fn from(usage: ChatUsage) -> Self {
        TokenUsage {
            input_tokens: usage.prompt_tokens,
            cached_input_tokens: 0,
            output_tokens: usage.completion_tokens,
            reasoning_output_tokens: 0,
            total_tokens: usage.total_tokens,
        }
    }
}

pub fn spawn_chat_completions_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let server_model = stream_response
        .headers
        .get(OPENAI_MODEL_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);

    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        if let Some(model) = server_model {
            let _ = tx_event.send(Ok(ResponseEvent::ServerModel(model))).await;
        }
        process_chat_sse(stream_response.bytes, tx_event, idle_timeout, telemetry).await;
    });

    ResponseStream { rx_event }
}

async fn process_chat_sse(
    stream: ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) {
    let mut stream = stream.eventsource();
    let mut created_emitted = false;
    let mut response_id: Option<String> = None;
    let mut usage: Option<TokenUsage> = None;
    let mut assistant_text = String::new();
    let mut tool_calls: Vec<AggregatedToolCall> = Vec::new();
    let mut last_server_model: Option<String> = None;

    loop {
        let start = Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        if let Some(t) = telemetry.as_ref() {
            t.on_sse_poll(&response, start.elapsed());
        }
        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let _ = tx_event.send(Err(ApiError::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream(
                        "stream closed before chat completion finished".into(),
                    )))
                    .await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        if sse.data.trim() == "[DONE]" {
            emit_chat_completion_items(
                &tx_event,
                &assistant_text,
                &tool_calls,
                response_id.unwrap_or_default(),
                usage,
            )
            .await;
            return;
        }

        let event: ChatChunk = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(err) => {
                trace!("ignoring non-json chat chunk: {err}");
                continue;
            }
        };

        if !created_emitted {
            let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
            created_emitted = true;
        }

        if response_id.is_none() {
            response_id = event.id.clone();
        }
        if let Some(chunk_usage) = event.usage {
            usage = Some(chunk_usage.into());
        }
        if let Some(model) = event.model {
            let changed = match last_server_model.as_ref() {
                Some(last) => last != &model,
                None => true,
            };
            if changed {
                let _ = tx_event
                    .send(Ok(ResponseEvent::ServerModel(model.clone())))
                    .await;
                last_server_model = Some(model);
            }
        }

        for choice in event.choices {
            let _ = choice.index;
            if let Some(content) = choice.delta.content {
                assistant_text.push_str(&content);
                let _ = tx_event.send(Ok(ResponseEvent::OutputTextDelta(content))).await;
            }
            if let Some(delta_tool_calls) = choice.delta.tool_calls {
                merge_tool_call_deltas(&mut tool_calls, delta_tool_calls);
            }
            let _ = choice.finish_reason;
        }
    }
}

fn merge_tool_call_deltas(
    aggregated: &mut Vec<AggregatedToolCall>,
    deltas: Vec<ChatToolCallDelta>,
) {
    for delta in deltas {
        let index = delta.index.unwrap_or(0);
        if aggregated.len() <= index {
            aggregated.resize_with(index + 1, AggregatedToolCall::default);
        }
        let entry = &mut aggregated[index];
        if let Some(id) = delta.id {
            entry.id = Some(id);
        }
        if let Some(function) = delta.function {
            if let Some(name) = function.name {
                if entry.name.is_empty() {
                    entry.name = name;
                } else {
                    entry.name.push_str(&name);
                }
            }
            if let Some(arguments) = function.arguments {
                entry.arguments.push_str(&arguments);
            }
        }
    }
}

async fn emit_chat_completion_items(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    assistant_text: &str,
    tool_calls: &[AggregatedToolCall],
    response_id: String,
    usage: Option<TokenUsage>,
) {
    for (index, call) in tool_calls.iter().enumerate() {
        if call.name.trim().is_empty() {
            continue;
        }
        let call_id = call
            .id
            .clone()
            .unwrap_or_else(|| format!("chat_tool_call_{index}"));
        let item = ResponseItem::FunctionCall {
            id: None,
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            call_id,
        };
        if tx_event
            .send(Ok(ResponseEvent::OutputItemDone(item)))
            .await
            .is_err()
        {
            return;
        }
    }

    if !assistant_text.trim().is_empty() {
        let message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: assistant_text.to_string(),
            }],
            end_turn: None,
            phase: None,
        };
        if tx_event
            .send(Ok(ResponseEvent::OutputItemDone(message)))
            .await
            .is_err()
        {
            return;
        }
    }

    let _ = tx_event
        .send(Ok(ResponseEvent::Completed {
            response_id,
            token_usage: usage,
            can_append: false,
        }))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_tool_call_deltas_concatenates_partial_chunks() {
        let mut aggregated = Vec::new();
        merge_tool_call_deltas(
            &mut aggregated,
            vec![ChatToolCallDelta {
                index: Some(0),
                id: Some("call_1".to_string()),
                function: Some(ChatFunctionDelta {
                    name: Some("exec_".to_string()),
                    arguments: Some("{\"cmd\":\"".to_string()),
                }),
            }],
        );
        merge_tool_call_deltas(
            &mut aggregated,
            vec![ChatToolCallDelta {
                index: Some(0),
                id: None,
                function: Some(ChatFunctionDelta {
                    name: Some("command".to_string()),
                    arguments: Some("pwd\"}".to_string()),
                }),
            }],
        );

        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].id.as_deref(), Some("call_1"));
        assert_eq!(aggregated[0].name, "exec_command");
        assert_eq!(aggregated[0].arguments, "{\"cmd\":\"pwd\"}");
    }

    #[tokio::test]
    async fn emit_chat_completion_items_emits_tool_message_and_completed() {
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent, ApiError>>(8);
        let tool_calls = vec![AggregatedToolCall {
            id: Some("call_1".to_string()),
            name: "exec_command".to_string(),
            arguments: "{\"cmd\":\"pwd\"}".to_string(),
        }];

        emit_chat_completion_items(&tx, "done", &tool_calls, "resp_1".to_string(), None).await;

        let first = rx.recv().await.expect("event").expect("ok event");
        let second = rx.recv().await.expect("event").expect("ok event");
        let third = rx.recv().await.expect("event").expect("ok event");

        match first {
            ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
                name,
                call_id,
                arguments,
                ..
            }) => {
                assert_eq!(name, "exec_command");
                assert_eq!(call_id, "call_1");
                assert_eq!(arguments, "{\"cmd\":\"pwd\"}");
            }
            other => panic!("unexpected first event: {other:?}"),
        }

        match second {
            ResponseEvent::OutputItemDone(ResponseItem::Message { role, content, .. }) => {
                assert_eq!(role, "assistant");
                assert_eq!(
                    content,
                    vec![ContentItem::OutputText {
                        text: "done".to_string(),
                    }]
                );
            }
            other => panic!("unexpected second event: {other:?}"),
        }

        match third {
            ResponseEvent::Completed {
                response_id,
                can_append,
                ..
            } => {
                assert_eq!(response_id, "resp_1");
                assert!(!can_append);
            }
            other => panic!("unexpected third event: {other:?}"),
        }
    }
}
