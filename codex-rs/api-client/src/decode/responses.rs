use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::trace;

use crate::error::Error;
use crate::error::Result;
use crate::stream::ResponseEvent;

#[derive(Debug, Deserialize)]
pub struct StreamResponseCompleted {
    pub id: String,
    pub usage: Option<TokenUsagePartial>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorBody {
    pub r#type: Option<String>,
    pub code: Option<String>,
    pub message: Option<String>,
    pub plan_type: Option<String>,
    pub resets_at: Option<i64>,
}

// legacy helper removed; decoupled error handling in core

#[derive(Debug, Deserialize)]
pub struct StreamEvent {
    pub r#type: String,
    pub response: Option<Value>,
    pub item: Option<Value>,
    pub error: Option<Value>,
    #[serde(default)]
    pub delta: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenUsagePartial {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub cached_input_tokens: i64,
    #[serde(default)]
    pub input_tokens_details: Option<TokenUsageInputDetails>,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub output_tokens_details: Option<TokenUsageOutputDetails>,
    #[serde(default)]
    pub reasoning_output_tokens: i64,
    #[serde(default)]
    pub total_tokens: i64,
}

impl From<TokenUsagePartial> for TokenUsage {
    fn from(value: TokenUsagePartial) -> Self {
        let cached_input_tokens = if value.cached_input_tokens > 0 {
            Some(value.cached_input_tokens)
        } else {
            value
                .input_tokens_details
                .and_then(|d| d.cached_tokens)
                .filter(|v| *v > 0)
        };
        let reasoning_output_tokens = if value.reasoning_output_tokens > 0 {
            Some(value.reasoning_output_tokens)
        } else {
            value
                .output_tokens_details
                .and_then(|d| d.reasoning_tokens)
                .filter(|v| *v > 0)
        };
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: cached_input_tokens.unwrap_or(0),
            output_tokens: value.output_tokens,
            reasoning_output_tokens: reasoning_output_tokens.unwrap_or(0),
            total_tokens: value.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenUsageInputDetails {
    #[serde(default)]
    pub cached_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TokenUsageOutputDetails {
    #[serde(default)]
    pub reasoning_tokens: Option<i64>,
}

pub async fn handle_sse_payload(
    payload: sse::Payload,
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
    otel_event_manager: &OtelEventManager,
) -> Result<()> {
    if let Some(responses) = payload.responses {
        for ev in responses {
            let event = match ev {
                sse::Response::Completed(complete) => {
                    if let Some(usage) = &complete.usage {
                        otel_event_manager.sse_event_completed(
                            usage.input_tokens,
                            usage.output_tokens,
                            Some(usage.cached_input_tokens),
                            Some(usage.reasoning_output_tokens),
                            usage.total_tokens,
                        );
                    } else {
                        otel_event_manager
                            .see_event_completed_failed(&"missing token usage".to_string());
                    }
                    ResponseEvent::Completed {
                        response_id: complete.id,
                        token_usage: complete.usage,
                    }
                }
                sse::Response::Error(err) => {
                    let retry_after = err
                        .retry_after
                        .map(|secs| Duration::from_secs(if secs < 0 { 0 } else { secs as u64 }));
                    return Err(Error::Stream(
                        err.message.unwrap_or_else(|| "fatal error".to_string()),
                        retry_after,
                    ));
                }
            };
            tx_event.send(Ok(event)).await.ok();
        }
    }

    if let Some(message_delta) = payload.response_message_delta {
        let ev = ResponseEvent::OutputTextDelta(message_delta.text.clone());
        tx_event.send(Ok(ev)).await.ok();
    }

    if let Some(_response_content) = payload.response_content {
        // Not used currently
    }

    if let Some(ev) = payload.response_event {
        debug!("Unhandled response_event: {ev:?}");
    }

    if let Some(item) = payload.response_output_item {
        match item.r#type {
            sse::OutputItem::Created => {
                tx_event.send(Ok(ResponseEvent::Created)).await.ok();
                otel_event_manager.sse_event_kind("response.output_item.done");
            }
        }
    }

    if let Some(done) = payload.response_output_text_delta {
        tx_event
            .send(Ok(ResponseEvent::OutputTextDelta(done.text)))
            .await
            .ok();
    }

    if let Some(completed) = payload.response_output_item_done {
        let response_item =
            serde_json::from_value::<ResponseItem>(completed.item).map_err(Error::Json)?;
        tx_event
            .send(Ok(ResponseEvent::OutputItemDone(response_item)))
            .await
            .ok();
        otel_event_manager.sse_event_kind("response.output_item.done");
    }

    if let Some(reasoning_content_delta) = payload.response_output_reasoning_delta {
        tx_event
            .send(Ok(ResponseEvent::ReasoningContentDelta(
                reasoning_content_delta.text,
            )))
            .await
            .ok();
    }

    if let Some(reasoning_summary_delta) = payload.response_output_reasoning_summary_delta {
        tx_event
            .send(Ok(ResponseEvent::ReasoningSummaryDelta(
                reasoning_summary_delta.text,
            )))
            .await
            .ok();
    }

    if let Some(ev) = payload.response_error
        && ev.code.as_deref() == Some("max_response_tokens")
    {
        let _ = tx_event
            .send(Err(Error::Stream(
                "context window exceeded".to_string(),
                None,
            )))
            .await;
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct TextDelta {
    pub delta: String,
}

pub async fn handle_stream_event(
    event: StreamEvent,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    otel_event_manager: &OtelEventManager,
) {
    trace!("response event: {}", event.r#type);
    match event.r#type.as_str() {
        "response.created" => {
            let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
        }
        "response.output_text.delta" => {
            if let Some(item_val) = event.item {
                let resp = serde_json::from_value::<TextDelta>(item_val);
                if let Ok(delta) = resp {
                    let event = ResponseEvent::OutputTextDelta(delta.delta);
                    let _ = tx_event.send(Ok(event)).await;
                }
            } else if let Some(delta) = event.delta {
                let _ = tx_event
                    .send(Ok(ResponseEvent::OutputTextDelta(delta)))
                    .await;
            }
        }
        "response.reasoning_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::ReasoningContentDelta(delta);
                let _ = tx_event.send(Ok(event)).await;
            }
        }
        "response.reasoning_summary_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::ReasoningSummaryDelta(delta);
                let _ = tx_event.send(Ok(event)).await;
            }
        }
        "response.output_item.done" => {
            if let Some(item_val) = event.item
                && let Ok(item) = serde_json::from_value::<ResponseItem>(item_val)
            {
                let event = ResponseEvent::OutputItemDone(item);
                if tx_event.send(Ok(event)).await.is_err() {}
            }
        }
        "response.failed" => {
            if let Some(resp_val) = event.response {
                otel_event_manager.sse_event_failed(
                    Some(&"response.failed".to_string()),
                    Duration::from_millis(0),
                    &resp_val,
                );

                if let Some(err) = resp_val
                    .get("error")
                    .cloned()
                    .and_then(|v| serde_json::from_value::<ErrorBody>(v).ok())
                {
                    let msg = if err.code.as_deref() == Some("context_length_exceeded") {
                        "context window exceeded".to_string()
                    } else if err.code.as_deref() == Some("insufficient_quota") {
                        "quota exceeded".to_string()
                    } else {
                        err.message.unwrap_or_else(|| "fatal error".to_string())
                    };
                    let _ = tx_event.send(Err(Error::Stream(msg, None))).await;
                }
            }
        }
        "response.error" => {
            if let Some(err_val) = event.error {
                let err_resp = serde_json::from_value::<ErrorResponse>(err_val);
                if let Ok(err) = err_resp {
                    let retry_after = try_parse_retry_after(&err);
                    let _ = tx_event
                        .send(Err(Error::Stream(
                            err.error
                                .message
                                .unwrap_or_else(|| "unknown error".to_string()),
                            retry_after,
                        )))
                        .await;
                }
            }
        }
        "response.completed" => {
            if let Some(resp_val) = event.response
                && let Ok(resp) = serde_json::from_value::<StreamResponseCompleted>(resp_val)
            {
                let usage = resp.usage.map(TokenUsage::from);
                let ev = ResponseEvent::Completed {
                    response_id: resp.id,
                    token_usage: usage.clone(),
                };
                let _ = tx_event.send(Ok(ev)).await;
                if let Some(usage) = &usage {
                    otel_event_manager.sse_event_completed(
                        usage.input_tokens,
                        usage.output_tokens,
                        Some(usage.cached_input_tokens),
                        Some(usage.reasoning_output_tokens),
                        usage.total_tokens,
                    );
                } else {
                    otel_event_manager
                        .see_event_completed_failed(&"missing token usage".to_string());
                }
            }
        }
        "response.output_item.added" => {
            if let Some(item_val) = event.item
                && let Ok(item) = serde_json::from_value::<ResponseItem>(item_val)
            {
                let event = ResponseEvent::OutputItemAdded(item);
                if tx_event.send(Ok(event)).await.is_err() {}
            }
        }
        "response.reasoning_summary_part.added" => {
            let event = ResponseEvent::ReasoningSummaryPartAdded;
            let _ = tx_event.send(Ok(event)).await;
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
pub struct ResponseErrorBody {
    pub code: Option<String>,
}

fn try_parse_retry_after(err: &ErrorResponse) -> Option<Duration> {
    if err.error.r#type.as_deref() == Some("rate_limit_exceeded") {
        let retry_after = serde_json::to_value(&err.error)
            .ok()
            .and_then(|v| v.get("retry_after").cloned())
            .and_then(|v| serde_json::from_value::<ResponseErrorBody>(v).ok())
            .and_then(|v| v.code)
            .and_then(parse_retry_after);
        return retry_after;
    }
    None
}

fn parse_retry_after(s: String) -> Option<Duration> {
    let minutes_pattern = regex_lite::Regex::new(r"^(\d+)m$").ok()?;
    if let Some(cap) = minutes_pattern.captures(&s)
        && let Some(m) = cap.get(1).and_then(|m| m.as_str().parse::<u64>().ok())
    {
        return Some(Duration::from_secs(m * 60));
    }
    s.parse::<u64>().ok().map(Duration::from_secs)
}

pub mod sse {
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Debug, Deserialize)]
    pub struct Payload {
        pub responses: Option<Vec<Response>>,
        pub response_content: Option<Value>,
        pub response_error: Option<ResponseError>,
        pub response_event: Option<String>,
        pub response_message_delta: Option<ResponseMessageDelta>,
        pub response_output_item: Option<ResponseOutputItem>,
        pub response_output_text_delta: Option<ResponseOutputTextDelta>,
        pub response_output_item_done: Option<ResponseOutputItemDone>,
        pub response_output_reasoning_delta: Option<ResponseOutputReasoningDelta>,
        pub response_output_reasoning_summary_delta: Option<ResponseOutputReasoningSummaryDelta>,
    }

    #[derive(Debug, Deserialize)]
    pub enum Response {
        #[serde(rename = "response.completed")]
        Completed(ResponseCompleted),
        #[serde(rename = "response.error")]
        Error(ResponseError),
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseCompleted {
        pub id: String,
        pub usage: Option<codex_protocol::protocol::TokenUsage>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseError {
        pub code: Option<String>,
        pub message: Option<String>,
        pub retry_after: Option<i64>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseMessageDelta {
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub enum OutputItem {
        #[serde(rename = "response.output_item.created")]
        Created,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputItem {
        pub r#type: OutputItem,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputTextDelta {
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputItemDone {
        pub item: Value,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputReasoningDelta {
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputReasoningSummaryDelta {
        pub text: String,
    }
}

pub struct ResponsesSseDecoder;

impl Default for ResponsesSseDecoder {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl crate::client::ResponseDecoder for ResponsesSseDecoder {
    async fn on_frame(
        &mut self,
        json: &str,
        tx: &mpsc::Sender<Result<ResponseEvent>>,
        otel_event_manager: &OtelEventManager,
    ) -> Result<()> {
        if let Ok(event) = serde_json::from_str::<StreamEvent>(json) {
            otel_event_manager.sse_event_kind(&event.r#type);
            handle_stream_event(event, tx.clone(), otel_event_manager).await;
            return Ok(());
        }

        otel_event_manager.sse_event_failed(
            None,
            Duration::from_millis(0),
            &format!("Cannot parse SSE JSON: {json}"),
        );

        match serde_json::from_str::<sse::Payload>(json) {
            Ok(payload) => handle_sse_payload(payload, tx, otel_event_manager).await,
            Err(err) => {
                otel_event_manager.sse_event_failed(
                    None,
                    Duration::from_millis(0),
                    &format!("Cannot parse SSE JSON: {err}"),
                );
                Err(Error::Other(format!("Cannot parse SSE JSON: {err}")))
            }
        }
    }
}
