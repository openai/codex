//! Streaming support for OpenAI Responses API.
//!
//! This module provides SSE (Server-Sent Events) decoding and a stream wrapper
//! for processing streaming responses from the OpenAI API.

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
use futures::stream::Stream;
use futures::stream::StreamExt;
use tokio::sync::mpsc;

use crate::error::OpenAIError;
use crate::error::Result;
use crate::types::Response;
use crate::types::ResponseStreamEvent;

// ============================================================================
// Server-Sent Event types
// ============================================================================

/// A parsed Server-Sent Event.
#[derive(Debug, Clone)]
pub struct ServerSentEvent {
    /// Event type (from "event:" field).
    pub event: Option<String>,
    /// Event data (from "data:" fields, joined with newlines).
    pub data: String,
    /// Event ID (from "id:" field).
    pub id: Option<String>,
    /// Retry timeout in milliseconds (from "retry:" field).
    pub retry: Option<i32>,
}

impl ServerSentEvent {
    /// Create a new empty SSE.
    fn new() -> Self {
        Self {
            event: None,
            data: String::new(),
            id: None,
            retry: None,
        }
    }

    /// Parse the data as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.data)
            .map_err(|e| OpenAIError::Parse(format!("Failed to parse SSE data as JSON: {e}")))
    }

    /// Check if this event has data.
    pub fn has_data(&self) -> bool {
        !self.data.is_empty()
    }
}

// ============================================================================
// SSE Decoder (following Python SDK pattern)
// ============================================================================

/// Server-Sent Events decoder.
///
/// Follows the SSE specification as implemented in the Python SDK:
/// - Handles event:, data:, id:, retry: fields
/// - Comments (lines starting with :) are ignored
/// - Empty line triggers event emission
/// - Multiple data: lines are joined with newlines
#[derive(Debug, Default)]
pub struct SSEDecoder {
    /// Current event type.
    event: Option<String>,
    /// Accumulated data lines.
    data: Vec<String>,
    /// Last event ID.
    last_event_id: Option<String>,
    /// Retry timeout.
    retry: Option<i32>,
}

impl SSEDecoder {
    /// Create a new SSE decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode a single line of SSE data.
    ///
    /// Returns `Some(ServerSentEvent)` when an event is complete (empty line received),
    /// or `None` if more data is needed.
    pub fn decode(&mut self, line: &str) -> Option<ServerSentEvent> {
        // Empty line triggers event emission
        if line.is_empty() {
            // Only emit if we have data
            if self.event.is_none()
                && self.data.is_empty()
                && self.last_event_id.is_none()
                && self.retry.is_none()
            {
                return None;
            }

            let event = ServerSentEvent {
                event: self.event.take(),
                data: self.data.join("\n"),
                id: self.last_event_id.clone(), // Keep last_event_id per SSE spec
                retry: self.retry.take(),
            };

            // Reset state (except last_event_id per SSE spec)
            self.data.clear();

            return Some(event);
        }

        // Comments start with ':'
        if line.starts_with(':') {
            return None;
        }

        // Parse field:value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let mut value = &line[colon_pos + 1..];
            // Strip leading space if present
            if value.starts_with(' ') {
                value = &value[1..];
            }
            (field, value)
        } else {
            // Field with no value
            (line, "")
        };

        match field {
            "event" => {
                self.event = Some(value.to_string());
            }
            "data" => {
                self.data.push(value.to_string());
            }
            "id" => {
                // Ignore IDs containing null character per spec
                if !value.contains('\0') {
                    self.last_event_id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(ms) = value.parse::<i32>() {
                    self.retry = Some(ms);
                }
            }
            _ => {
                // Unknown field, ignore
            }
        }

        None
    }

    /// Process a chunk of bytes and yield events.
    ///
    /// This handles the buffering of partial lines and chunks.
    pub fn decode_chunk(&mut self, chunk: &[u8], buffer: &mut Vec<u8>) -> Vec<ServerSentEvent> {
        let mut events = Vec::new();
        buffer.extend_from_slice(chunk);

        // Process complete lines
        loop {
            // Find line ending
            let line_end = buffer
                .iter()
                .position(|&b| b == b'\n' || b == b'\r')
                .map(|pos| {
                    // Check for \r\n
                    if pos + 1 < buffer.len() && buffer[pos] == b'\r' && buffer[pos + 1] == b'\n' {
                        (pos, 2)
                    } else {
                        (pos, 1)
                    }
                });

            match line_end {
                Some((pos, skip)) => {
                    let line_bytes = buffer.drain(..pos).collect::<Vec<_>>();
                    buffer.drain(..skip); // Remove line ending

                    // Decode as UTF-8
                    if let Ok(line) = std::str::from_utf8(&line_bytes) {
                        if let Some(event) = self.decode(line) {
                            events.push(event);
                        }
                    }
                }
                None => break,
            }
        }

        events
    }
}

// ============================================================================
// Response Stream
// ============================================================================

/// A stream of response events from the OpenAI API.
///
/// This struct wraps the SSE byte stream and provides a convenient interface
/// for iterating over parsed events.
pub struct ResponseStream {
    rx: mpsc::Receiver<Result<ResponseStreamEvent>>,
    /// Handle to the background task processing the stream.
    #[allow(dead_code)]
    task_handle: tokio::task::JoinHandle<()>,
}

impl ResponseStream {
    /// Create a new response stream from a byte stream.
    pub(crate) fn new<S>(byte_stream: S) -> Self
    where
        S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(256);
        let task_handle = tokio::spawn(process_sse_stream(byte_stream, tx));
        Self { rx, task_handle }
    }

    /// Receive the next event from the stream.
    ///
    /// Returns `None` when the stream is complete.
    pub async fn next(&mut self) -> Option<Result<ResponseStreamEvent>> {
        self.rx.recv().await
    }

    /// Collect all events and return the final response.
    ///
    /// This consumes the stream and returns the complete Response object
    /// from the `response.completed` event.
    pub async fn collect_response(mut self) -> Result<Response> {
        let mut final_response: Option<Response> = None;

        while let Some(event_result) = self.next().await {
            match event_result? {
                ResponseStreamEvent::ResponseCompleted { response, .. } => {
                    final_response = Some(response);
                    break;
                }
                ResponseStreamEvent::ResponseFailed { response, .. } => {
                    if let Some(error) = &response.error {
                        return Err(OpenAIError::Api {
                            status: 400,
                            message: error.message.clone(),
                            request_id: None,
                        });
                    }
                    return Err(OpenAIError::Api {
                        status: 400,
                        message: "Response failed".to_string(),
                        request_id: None,
                    });
                }
                ResponseStreamEvent::ResponseIncomplete { response, .. } => {
                    // Return incomplete response
                    final_response = Some(response);
                    break;
                }
                ResponseStreamEvent::Error { code, message, .. } => {
                    return Err(map_stream_error(code.as_deref(), &message));
                }
                _ => continue,
            }
        }

        final_response.ok_or_else(|| {
            OpenAIError::Parse("Stream ended without response.completed".to_string())
        })
    }

    /// Convert to a futures Stream for use with StreamExt combinators.
    pub fn into_stream(self) -> ResponseStreamAdapter {
        ResponseStreamAdapter { inner: self }
    }

    /// Get an iterator that yields text deltas only.
    ///
    /// This is a convenience method for simple text streaming use cases.
    pub async fn text_deltas(&mut self) -> Result<String> {
        let mut text = String::new();

        while let Some(event_result) = self.next().await {
            match event_result? {
                ResponseStreamEvent::OutputTextDelta { delta, .. } => {
                    text.push_str(&delta);
                }
                ResponseStreamEvent::ResponseCompleted { .. }
                | ResponseStreamEvent::ResponseFailed { .. }
                | ResponseStreamEvent::ResponseIncomplete { .. } => {
                    break;
                }
                ResponseStreamEvent::Error { code, message, .. } => {
                    return Err(map_stream_error(code.as_deref(), &message));
                }
                _ => continue,
            }
        }

        Ok(text)
    }
}

/// Adapter to implement the futures Stream trait.
pub struct ResponseStreamAdapter {
    inner: ResponseStream,
}

impl Stream for ResponseStreamAdapter {
    type Item = Result<ResponseStreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner.rx).poll_recv(cx)
    }
}

// ============================================================================
// SSE Processing
// ============================================================================

/// Process SSE stream and emit parsed events.
async fn process_sse_stream<S>(byte_stream: S, tx: mpsc::Sender<Result<ResponseStreamEvent>>)
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send,
{
    let mut decoder = SSEDecoder::new();
    let mut buffer = Vec::new();

    tokio::pin!(byte_stream);

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = match chunk_result {
            Ok(chunk) => chunk,
            Err(e) => {
                let _ = tx.send(Err(OpenAIError::Network(e))).await;
                return;
            }
        };

        let events = decoder.decode_chunk(&chunk, &mut buffer);

        for sse in events {
            // Check for stream terminator
            if sse.data.starts_with("[DONE]") {
                return;
            }

            if !sse.has_data() {
                continue;
            }

            // Parse event JSON
            let event_result = match sse.json::<ResponseStreamEvent>() {
                Ok(event) => Ok(event),
                Err(e) => {
                    // Try to extract error from raw JSON
                    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&sse.data) {
                        if let Some(error) = raw.get("error") {
                            let message = error
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("An error occurred during streaming");
                            let code = error.get("code").and_then(|c| c.as_str());
                            Err(map_stream_error(code, message))
                        } else {
                            Err(OpenAIError::Parse(format!(
                                "Failed to parse SSE event: {e}, data: {}",
                                &sse.data
                            )))
                        }
                    } else {
                        Err(OpenAIError::Parse(format!(
                            "Failed to parse SSE event: {e}, data: {}",
                            &sse.data
                        )))
                    }
                }
            };

            if tx.send(event_result).await.is_err() {
                // Receiver dropped
                return;
            }
        }
    }

    // Process any remaining data in buffer
    if !buffer.is_empty() {
        if let Ok(line) = std::str::from_utf8(&buffer) {
            if let Some(sse) = decoder.decode(line) {
                if sse.has_data() && !sse.data.starts_with("[DONE]") {
                    if let Ok(event) = sse.json::<ResponseStreamEvent>() {
                        let _ = tx.send(Ok(event)).await;
                    }
                }
            }
        }
        // Trigger final event emission with empty line
        if let Some(sse) = decoder.decode("") {
            if sse.has_data() && !sse.data.starts_with("[DONE]") {
                if let Ok(event) = sse.json::<ResponseStreamEvent>() {
                    let _ = tx.send(Ok(event)).await;
                }
            }
        }
    }
}

/// Map stream error codes to OpenAIError variants.
fn map_stream_error(code: Option<&str>, message: &str) -> OpenAIError {
    if let Some(code) = code {
        if code.contains("context_length_exceeded") {
            return OpenAIError::ContextWindowExceeded;
        }
        if code.contains("insufficient_quota") {
            return OpenAIError::QuotaExceeded;
        }
        if code.contains("rate_limit_exceeded") {
            return OpenAIError::RateLimited { retry_after: None };
        }
    }

    OpenAIError::Api {
        status: 400,
        message: message.to_string(),
        request_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_decoder_basic() {
        let mut decoder = SSEDecoder::new();

        // No event yet
        assert!(decoder.decode("event: test").is_none());
        assert!(decoder.decode("data: hello").is_none());

        // Empty line triggers emission
        let event = decoder.decode("").unwrap();
        assert_eq!(event.event, Some("test".to_string()));
        assert_eq!(event.data, "hello");
    }

    #[test]
    fn test_sse_decoder_multiline_data() {
        let mut decoder = SSEDecoder::new();

        decoder.decode("data: line1");
        decoder.decode("data: line2");
        decoder.decode("data: line3");

        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "line1\nline2\nline3");
    }

    #[test]
    fn test_sse_decoder_comment() {
        let mut decoder = SSEDecoder::new();

        // Comment should be ignored
        assert!(decoder.decode(": this is a comment").is_none());
        assert!(decoder.decode("data: actual data").is_none());

        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "actual data");
    }

    #[test]
    fn test_sse_decoder_colon_in_value() {
        let mut decoder = SSEDecoder::new();

        decoder.decode("data: {\"key\": \"value\"}");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "{\"key\": \"value\"}");
    }

    #[test]
    fn test_sse_decoder_no_space_after_colon() {
        let mut decoder = SSEDecoder::new();

        decoder.decode("data:no space");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "no space");
    }

    #[test]
    fn test_sse_decoder_retry() {
        let mut decoder = SSEDecoder::new();

        decoder.decode("retry: 5000");
        decoder.decode("data: test");

        let event = decoder.decode("").unwrap();
        assert_eq!(event.retry, Some(5000));
    }

    #[test]
    fn test_sse_decoder_id() {
        let mut decoder = SSEDecoder::new();

        decoder.decode("id: event-123");
        decoder.decode("data: test");

        let event = decoder.decode("").unwrap();
        assert_eq!(event.id, Some("event-123".to_string()));

        // ID persists per SSE spec
        decoder.decode("data: test2");
        let event2 = decoder.decode("").unwrap();
        assert_eq!(event2.id, Some("event-123".to_string()));
    }

    #[test]
    fn test_sse_decoder_chunk() {
        let mut decoder = SSEDecoder::new();
        let mut buffer = Vec::new();

        let chunk = b"event: test\ndata: hello\n\n";
        let events = decoder.decode_chunk(chunk, &mut buffer);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("test".to_string()));
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_sse_decoder_partial_chunk() {
        let mut decoder = SSEDecoder::new();
        let mut buffer = Vec::new();

        // First chunk is partial
        let events1 = decoder.decode_chunk(b"event: te", &mut buffer);
        assert!(events1.is_empty());

        // Second chunk completes the event
        let events2 = decoder.decode_chunk(b"st\ndata: hello\n\n", &mut buffer);
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].event, Some("test".to_string()));
        assert_eq!(events2[0].data, "hello");
    }

    #[test]
    fn test_server_sent_event_json() {
        let event = ServerSentEvent {
            event: None,
            data: r#"{"type": "response.output_text.delta", "sequence_number": 1, "item_id": "x", "output_index": 0, "content_index": 0, "delta": "hi", "logprobs": []}"#.to_string(),
            id: None,
            retry: None,
        };

        let parsed: ResponseStreamEvent = event.json().unwrap();
        assert!(matches!(
            parsed,
            ResponseStreamEvent::OutputTextDelta { .. }
        ));
    }

    #[test]
    fn test_map_stream_error() {
        assert!(matches!(
            map_stream_error(Some("context_length_exceeded"), "test"),
            OpenAIError::ContextWindowExceeded
        ));

        assert!(matches!(
            map_stream_error(Some("insufficient_quota"), "test"),
            OpenAIError::QuotaExceeded
        ));

        assert!(matches!(
            map_stream_error(Some("rate_limit_exceeded"), "test"),
            OpenAIError::RateLimited { .. }
        ));

        assert!(matches!(
            map_stream_error(Some("unknown_error"), "test message"),
            OpenAIError::Api { message, .. } if message == "test message"
        ));
    }
}
