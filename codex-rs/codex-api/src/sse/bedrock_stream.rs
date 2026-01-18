//! Bedrock streaming response handler using AWS Event Stream format.
//!
//! Parses the binary event stream from invoke-with-response-stream endpoint.

use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use aws_smithy_eventstream::frame::DecodedFrame;
use aws_smithy_eventstream::frame::MessageFrameDecoder;
use bytes::BytesMut;
use codex_client::StreamResponse;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::debug;

/// Tracks an in-progress tool use block.
#[derive(Default)]
struct ToolUseBlock {
    id: String,
    name: String,
    input_json: String,
}

/// Spawn a task to handle Bedrock's streaming event stream response.
pub fn spawn_bedrock_stream(stream_response: StreamResponse) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(64);
    tokio::spawn(async move {
        process_bedrock_stream(stream_response.bytes, tx_event).await;
    });
    ResponseStream { rx_event }
}

struct StreamState {
    response_id: String,
    input_tokens: i64,
    output_tokens: i64,
    /// In-progress tool use blocks, keyed by content block index.
    tool_blocks: HashMap<u64, ToolUseBlock>,
}

async fn process_bedrock_stream<S>(
    mut stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
) where
    S: futures::Stream<Item = Result<bytes::Bytes, codex_client::TransportError>> + Unpin,
{
    let mut decoder = MessageFrameDecoder::new();
    let mut buffer = BytesMut::new();
    let mut state = StreamState {
        response_id: String::new(),
        input_tokens: 0,
        output_tokens: 0,
        tool_blocks: HashMap::new(),
    };

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => buffer.extend_from_slice(&bytes),
            Err(e) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream(format!("Stream error: {e}"))))
                    .await;
                return;
            }
        }

        // Decode frames from buffer
        loop {
            match decoder.decode_frame(&mut buffer) {
                Ok(DecodedFrame::Complete(message)) => {
                    let payload = message.payload();
                    if let Err(e) = process_event_payload(payload, &tx_event, &mut state).await {
                        let _ = tx_event.send(Err(e)).await;
                        return;
                    }
                }
                Ok(DecodedFrame::Incomplete) => break,
                Err(e) => {
                    let _ = tx_event
                        .send(Err(ApiError::Stream(format!("Frame decode error: {e}"))))
                        .await;
                    return;
                }
            }
        }
    }

    // Send completion event
    let _ = tx_event
        .send(Ok(ResponseEvent::Completed {
            response_id: state.response_id,
            token_usage: Some(TokenUsage {
                input_tokens: state.input_tokens,
                cached_input_tokens: 0,
                output_tokens: state.output_tokens,
                reasoning_output_tokens: 0,
                total_tokens: state.input_tokens + state.output_tokens,
            }),
        }))
        .await;
}

async fn process_event_payload(
    payload: &[u8],
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    state: &mut StreamState,
) -> Result<(), ApiError> {
    let wrapper: Value =
        serde_json::from_slice(payload).map_err(|e| ApiError::Stream(format!("JSON error: {e}")))?;

    let bytes_b64 = wrapper
        .get("bytes")
        .and_then(|b| b.as_str())
        .ok_or_else(|| ApiError::Stream("Missing bytes field".to_string()))?;

    let decoded = base64_decode(bytes_b64)?;
    let event: Value = serde_json::from_slice(&decoded)
        .map_err(|e| ApiError::Stream(format!("Inner JSON error: {e}")))?;

    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
    debug!("Bedrock stream event: {event_type}");

    match event_type {
        "message_start" => {
            if let Some(id) = event
                .get("message")
                .and_then(|m| m.get("id"))
                .and_then(|i| i.as_str())
            {
                state.response_id = id.to_string();
            }
            if let Some(usage) = event.get("message").and_then(|m| m.get("usage")) {
                if let Some(tokens) = usage.get("input_tokens").and_then(|t| t.as_i64()) {
                    state.input_tokens = tokens;
                }
            }
        }
        "content_block_start" => {
            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
            if let Some(content_block) = event.get("content_block") {
                let block_type = content_block.get("type").and_then(|t| t.as_str());
                match block_type {
                    Some("tool_use") => {
                        let id = content_block
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = content_block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        debug!("Tool use started: id={id}, name={name}");
                        state.tool_blocks.insert(
                            index,
                            ToolUseBlock {
                                id,
                                name,
                                input_json: String::new(),
                            },
                        );
                    }
                    Some("thinking") => {
                        debug!("Thinking block started");
                    }
                    _ => {}
                }
            }
        }
        "content_block_delta" => {
            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
            if let Some(delta) = event.get("delta") {
                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match delta_type {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                            let _ = tx_event
                                .send(Ok(ResponseEvent::OutputTextDelta(text.to_string())))
                                .await;
                        }
                    }
                    "thinking_delta" => {
                        if let Some(thinking) = delta.get("thinking").and_then(|t| t.as_str()) {
                            let _ = tx_event
                                .send(Ok(ResponseEvent::ReasoningContentDelta {
                                    delta: thinking.to_string(),
                                    content_index: 0,
                                }))
                                .await;
                        }
                    }
                    "input_json_delta" => {
                        if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                            if let Some(block) = state.tool_blocks.get_mut(&index) {
                                block.input_json.push_str(partial);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_stop" => {
            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
            debug!("Content block {index} stopped");

            // If this was a tool_use block, emit the function call
            if let Some(block) = state.tool_blocks.remove(&index) {
                debug!(
                    "Emitting tool call: name={}, id={}, args={}",
                    block.name, block.id, block.input_json
                );
                let tool_call = ResponseItem::FunctionCall {
                    id: None,
                    name: block.name,
                    arguments: block.input_json,
                    call_id: block.id,
                };
                let _ = tx_event
                    .send(Ok(ResponseEvent::OutputItemDone(tool_call)))
                    .await;
            }
        }
        "message_delta" => {
            if let Some(usage) = event.get("usage") {
                if let Some(tokens) = usage.get("output_tokens").and_then(|t| t.as_i64()) {
                    state.output_tokens = tokens;
                }
            }
            if let Some(delta) = event.get("delta") {
                let stop_reason = delta.get("stop_reason").and_then(|r| r.as_str());
                if stop_reason == Some("max_tokens") {
                    return Err(ApiError::ContextWindowExceeded);
                }
            }
        }
        "message_stop" => {
            debug!("Message stopped");
        }
        _ => {
            debug!("Unknown event type: {event_type}");
        }
    }

    Ok(())
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ApiError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| ApiError::Stream(format!("Base64 decode error: {e}")))
}
