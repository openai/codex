use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::debug;

use crate::client::WireResponseDecoder;
use crate::error::Error;
use crate::error::Result;
use crate::stream::WireEvent;
use crate::stream::WireTokenUsage;

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    response: Option<Value>,
    #[serde(default)]
    item: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    delta: Option<String>,
}

#[derive(Default, Deserialize)]
struct WireUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    cached_input_tokens: Option<i64>,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    reasoning_output_tokens: Option<i64>,
    #[serde(default)]
    total_tokens: i64,
    #[serde(default)]
    input_tokens_details: Option<WireInputTokensDetails>,
    #[serde(default)]
    output_tokens_details: Option<WireOutputTokensDetails>,
}

#[derive(Default, Deserialize)]
struct WireInputTokensDetails {
    #[serde(default)]
    cached_tokens: Option<i64>,
}

#[derive(Default, Deserialize)]
struct WireOutputTokensDetails {
    #[serde(default)]
    reasoning_tokens: Option<i64>,
}

pub struct WireResponsesSseDecoder;

#[async_trait]
impl WireResponseDecoder for WireResponsesSseDecoder {
    async fn on_frame(
        &mut self,
        json: &str,
        tx: &mpsc::Sender<Result<WireEvent>>,
        otel: &OtelEventManager,
    ) -> Result<()> {
        let event = serde_json::from_str::<StreamEvent>(json).map_err(|err| {
            debug!("failed to parse Responses SSE JSON: {}", json);
            Error::Other(format!("failed to parse Responses SSE JSON: {err}"))
        })?;

        match event.event_type.as_str() {
            "response.created" => {
                let _ = tx.send(Ok(WireEvent::Created)).await;
            }
            "response.output_text.delta" => {
                if let Some(delta) = event.delta.or_else(|| {
                    event.item.and_then(|v| {
                        v.get("delta")
                            .and_then(|d| d.as_str().map(std::string::ToString::to_string))
                    })
                }) {
                    let _ = tx.send(Ok(WireEvent::OutputTextDelta(delta))).await;
                }
            }
            "response.reasoning_text.delta" => {
                if let Some(delta) = event.delta {
                    let _ = tx.send(Ok(WireEvent::ReasoningContentDelta(delta))).await;
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let Some(delta) = event.delta {
                    let _ = tx.send(Ok(WireEvent::ReasoningSummaryDelta(delta))).await;
                }
            }
            "response.output_item.done" => {
                if let Some(item_val) = event.item {
                    let _ = tx.send(Ok(WireEvent::OutputItemDone(item_val))).await;
                }
            }
            "response.output_item.added" => {
                if let Some(item_val) = event.item {
                    let _ = tx.send(Ok(WireEvent::OutputItemAdded(item_val))).await;
                }
            }
            "response.reasoning_summary_part.added" => {
                let _ = tx.send(Ok(WireEvent::ReasoningSummaryPartAdded)).await;
            }
            "response.completed" => {
                if let Some(resp) = event.response {
                    let response_id = resp
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let usage = parse_wire_usage(&resp);
                    if let Some(u) = &usage {
                        otel.sse_event_completed(
                            u.input_tokens,
                            u.output_tokens,
                            Some(u.cached_input_tokens),
                            Some(u.reasoning_output_tokens),
                            u.total_tokens,
                        );
                    } else {
                        otel.see_event_completed_failed(&"missing token usage".to_string());
                    }
                    let _ = tx
                        .send(Ok(WireEvent::Completed {
                            response_id,
                            token_usage: usage,
                        }))
                        .await;
                }
            }
            "response.error" | "response.failed" => {
                let message = event
                    .error
                    .as_ref()
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string)
                    .unwrap_or_else(|| "unknown error".to_string());
                let _ = tx.send(Err(Error::Stream(message, None))).await;
            }
            _ => {}
        }

        Ok(())
    }
}

fn parse_wire_usage(resp: &Value) -> Option<WireTokenUsage> {
    let usage: WireUsage = serde_json::from_value(resp.get("usage")?.clone()).ok()?;
    let cached_input_tokens = usage
        .cached_input_tokens
        .or_else(|| {
            usage
                .input_tokens_details
                .and_then(|details| details.cached_tokens)
        })
        .unwrap_or(0);
    let reasoning_output_tokens = usage
        .reasoning_output_tokens
        .or_else(|| {
            usage
                .output_tokens_details
                .and_then(|details| details.reasoning_tokens)
        })
        .unwrap_or(0);

    Some(WireTokenUsage {
        input_tokens: usage.input_tokens,
        cached_input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_output_tokens,
        total_tokens: usage.total_tokens,
    })
}
