//! Streaming support for Google Generative AI API.
//!
//! This module provides SSE (Server-Sent Events) parsing for streaming responses
//! from the `streamGenerateContent` endpoint.
//!
//! ## SSE Specification
//!
//! This implementation follows the [SSE specification](https://html.spec.whatwg.org/multipage/server-sent-events.html):
//! - Lines starting with `:` are comments (ignored)
//! - Fields: `event`, `data`, `id`, `retry`
//! - Multiple `data:` lines are joined with `\n`
//! - Empty line triggers event emission
//! - `id` persists across events (per spec)

use crate::error::GenAiError;
use crate::error::Result;
use crate::types::ErrorResponse;
use crate::types::GenerateContentResponse;
use bytes::Bytes;
use futures::stream::Stream;
use serde::de::DeserializeOwned;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

// =============================================================================
// Type Aliases
// =============================================================================

/// Type alias for a streaming response of raw SSE events.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<ServerSentEvent>> + Send>>;

/// Type alias for a streaming response of parsed GenerateContentResponse chunks.
///
/// Each item in the stream is a `GenerateContentResponse` chunk containing
/// partial content that should be accumulated by the caller.
pub type ContentStream = Pin<Box<dyn Stream<Item = Result<GenerateContentResponse>> + Send>>;

/// Type alias for a boxed byte stream (pinned for polling).
type BoxedByteStream =
    Pin<Box<dyn Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send>>;

// =============================================================================
// ServerSentEvent
// =============================================================================

/// A parsed Server-Sent Event following the SSE specification.
///
/// This aligns with Python SDK's `ServerSentEvent` class.
///
/// ## SSE Wire Format
///
/// ```text
/// event: message
/// data: {"key": "value"}
/// id: 123
/// retry: 5000
///
/// ```
#[derive(Debug, Clone, Default)]
pub struct ServerSentEvent {
    /// Event type (from "event:" field).
    pub event: Option<String>,
    /// Event data (from "data:" fields, joined with newlines for multi-line data).
    pub data: String,
    /// Event ID (from "id:" field). Persists across events per SSE spec.
    pub id: Option<String>,
    /// Retry timeout in milliseconds (from "retry:" field).
    pub retry: Option<i32>,
}

impl ServerSentEvent {
    /// Create a new empty SSE.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new SSE with data.
    pub fn with_data(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            ..Default::default()
        }
    }

    /// Check if this event has non-empty data.
    pub fn has_data(&self) -> bool {
        !self.data.is_empty()
    }

    /// Check if this is the [DONE] marker.
    pub fn is_done(&self) -> bool {
        self.data.starts_with("[DONE]")
    }

    /// Parse the data as JSON.
    ///
    /// # Errors
    ///
    /// Returns `GenAiError::Parse` if the data is not valid JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.data).map_err(|e| {
            GenAiError::Parse(format!(
                "Failed to parse SSE data as JSON: {e}\nData: {}",
                self.data
            ))
        })
    }
}

// =============================================================================
// SSEDecoder
// =============================================================================

/// Server-Sent Events decoder following the full SSE specification.
///
/// This aligns with Python SDK's `SSEDecoder` class.
///
/// ## Implementation Notes
///
/// Per the [SSE specification](https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation):
/// - Lines starting with `:` are comments (ignored)
/// - Supported fields: `event`, `data`, `id`, `retry`
/// - Unknown fields are ignored
/// - Multiple `data:` lines are joined with `\n`
/// - Empty line triggers event emission
/// - `id` persists across events (do NOT reset on event emission)
/// - `id` containing null character (`\0`) is ignored
#[derive(Debug, Default)]
pub struct SSEDecoder {
    /// Current event type (reset on event emission).
    event: Option<String>,
    /// Accumulated data lines (reset on event emission).
    data: Vec<String>,
    /// Last event ID (persists across events per SSE spec).
    last_event_id: Option<String>,
    /// Retry timeout (reset on event emission).
    retry: Option<i32>,
}

impl SSEDecoder {
    /// Create a new SSE decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode a single line of SSE data.
    ///
    /// Returns `Some(ServerSentEvent)` when a complete event is ready (empty line received).
    /// Returns `None` if more data is needed.
    ///
    /// # SSE Line Format
    ///
    /// ```text
    /// field:value
    /// field: value  (space after colon is stripped)
    /// :comment      (ignored)
    /// ```
    pub fn decode(&mut self, line: &str) -> Option<ServerSentEvent> {
        // Empty line = emit event
        if line.is_empty() {
            // Don't emit empty event
            if self.event.is_none() && self.data.is_empty() && self.retry.is_none() {
                return None;
            }

            let event = ServerSentEvent {
                event: self.event.take(),
                data: self.data.join("\n"),
                id: self.last_event_id.clone(), // Persists per SSE spec
                retry: self.retry.take(),
            };

            self.data.clear();
            return Some(event);
        }

        // Comment line (starts with ':')
        if line.starts_with(':') {
            return None;
        }

        // Parse field:value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let mut value = &line[colon_pos + 1..];
            // Strip single leading space after colon per spec
            if value.starts_with(' ') {
                value = &value[1..];
            }
            (field, value)
        } else {
            // Line with no colon - treat entire line as field name with empty value
            (line, "")
        };

        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => self.data.push(value.to_string()),
            "id" => {
                // Ignore IDs containing null character per SSE spec
                if !value.contains('\0') {
                    self.last_event_id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(ms) = value.parse::<i32>() {
                    self.retry = Some(ms);
                }
                // Invalid retry values are ignored per spec
            }
            _ => {} // Unknown field, ignore per spec
        }

        None
    }

    /// Process a chunk of bytes and yield complete events.
    ///
    /// This handles incomplete lines across chunk boundaries.
    pub fn decode_chunk(&mut self, chunk: &[u8], buffer: &mut Vec<u8>) -> Vec<ServerSentEvent> {
        buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        // Process complete lines
        loop {
            if let Some(line_info) = find_line_end(buffer) {
                // Extract line bytes
                let line_bytes: Vec<u8> = buffer.drain(..line_info.end).collect();
                // Remove the line ending
                buffer.drain(..line_info.ending_len);

                // Decode line as UTF-8
                if let Ok(line) = std::str::from_utf8(&line_bytes) {
                    if let Some(event) = self.decode(line) {
                        events.push(event);
                    }
                }
            } else {
                break;
            }
        }

        events
    }

    /// Reset the decoder state.
    pub fn reset(&mut self) {
        self.event = None;
        self.data.clear();
        self.last_event_id = None;
        self.retry = None;
    }
}

/// Information about a line ending found in a buffer.
struct LineEnd {
    /// Position of line content end (before line ending).
    end: usize,
    /// Length of line ending characters to skip.
    ending_len: usize,
}

/// Find the end of the next line in the buffer.
///
/// Handles `\n`, `\r`, and `\r\n` line endings per SSE spec.
fn find_line_end(buffer: &[u8]) -> Option<LineEnd> {
    for (i, &byte) in buffer.iter().enumerate() {
        if byte == b'\n' {
            return Some(LineEnd {
                end: i,
                ending_len: 1,
            });
        }
        if byte == b'\r' {
            // Check for \r\n
            let ending_len = if buffer.get(i + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            return Some(LineEnd { end: i, ending_len });
        }
    }
    None
}

// =============================================================================
// SseStream - Low-level SSE byte stream to event stream
// =============================================================================

/// SSE parser that converts a byte stream into SSE events.
///
/// Implements the `Stream` trait for async iteration.
pub struct SseStream {
    inner: BoxedByteStream,
    decoder: SSEDecoder,
    buffer: Vec<u8>,
    pending_events: Vec<ServerSentEvent>,
}

impl SseStream {
    fn new(inner: BoxedByteStream) -> Self {
        Self {
            inner,
            decoder: SSEDecoder::new(),
            buffer: Vec::new(),
            pending_events: Vec::new(),
        }
    }
}

impl Stream for SseStream {
    type Item = Result<ServerSentEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Return pending events first
            if !self.pending_events.is_empty() {
                return Poll::Ready(Some(Ok(self.pending_events.remove(0))));
            }

            // Poll inner stream for more data
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    // Decode chunk and collect events
                    // Temporarily take buffer to avoid borrow issues
                    let mut buffer = std::mem::take(&mut self.buffer);
                    let events = self.decoder.decode_chunk(&bytes, &mut buffer);
                    self.buffer = buffer;
                    self.pending_events.extend(events);
                    // Continue loop to return first event
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(GenAiError::Network(e.to_string()))));
                }
                Poll::Ready(None) => {
                    // Stream ended - try to parse remaining buffer
                    if !self.buffer.is_empty() {
                        // Take buffer to avoid borrow issues
                        let buffer = std::mem::take(&mut self.buffer);
                        if let Ok(remaining) = std::str::from_utf8(&buffer) {
                            // Try decoding remaining as final line
                            if let Some(event) = self.decoder.decode(remaining) {
                                return Poll::Ready(Some(Ok(event)));
                            }
                            // Try triggering final event with empty line
                            if let Some(event) = self.decoder.decode("") {
                                return Poll::Ready(Some(Ok(event)));
                            }
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// =============================================================================
// ContentStreamParser - SSE to GenerateContentResponse
// =============================================================================

/// Parser that converts SSE events into GenerateContentResponse chunks.
struct ContentStreamParser {
    inner: SseStream,
}

impl ContentStreamParser {
    fn new(inner: SseStream) -> Self {
        Self { inner }
    }
}

impl Stream for ContentStreamParser {
    type Item = Result<GenerateContentResponse>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(sse))) => {
                    // Skip empty data and [DONE] marker
                    if !sse.has_data() || sse.is_done() {
                        continue;
                    }

                    // First check if this is an error response
                    // (Aligns with Python SDK's error handling in streams)
                    if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&sse.data) {
                        return Poll::Ready(Some(Err(GenAiError::Api {
                            code: error_response.error.code,
                            message: error_response.error.message,
                            status: error_response.error.status,
                        })));
                    }

                    // Parse as GenerateContentResponse
                    match sse.json::<GenerateContentResponse>() {
                        Ok(response) => return Poll::Ready(Some(Ok(response))),
                        Err(e) => return Poll::Ready(Some(Err(e))),
                    }
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Parse an SSE byte stream into a stream of raw `ServerSentEvent`.
///
/// Use this for low-level SSE event access.
pub fn parse_sse_events<S>(byte_stream: S) -> EventStream
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let boxed: BoxedByteStream = Box::pin(byte_stream);
    Box::pin(SseStream::new(boxed))
}

/// Parse an SSE byte stream into a stream of `GenerateContentResponse` chunks.
///
/// This is the main entry point for streaming content generation.
///
/// # SSE Wire Format
///
/// The Google Gemini streaming API uses Server-Sent Events format:
/// ```text
/// data: {"candidates":[{"content":{"parts":[{"text":"Hello"}]}}]}
///
/// data: {"candidates":[{"content":{"parts":[{"text":" World"}]}}]}
///
/// data: [DONE]
/// ```
///
/// Each `data:` line contains a complete JSON `GenerateContentResponse`.
pub fn parse_sse_stream<S>(byte_stream: S) -> ContentStream
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let boxed: BoxedByteStream = Box::pin(byte_stream);
    Box::pin(ContentStreamParser::new(SseStream::new(boxed)))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::StreamExt;
    use futures::stream;

    // =========================================================================
    // ServerSentEvent Tests
    // =========================================================================

    #[test]
    fn test_sse_new() {
        let sse = ServerSentEvent::new();
        assert!(sse.event.is_none());
        assert!(sse.data.is_empty());
        assert!(sse.id.is_none());
        assert!(sse.retry.is_none());
    }

    #[test]
    fn test_sse_with_data() {
        let sse = ServerSentEvent::with_data("hello");
        assert_eq!(sse.data, "hello");
        assert!(sse.has_data());
    }

    #[test]
    fn test_sse_is_done() {
        let sse = ServerSentEvent::with_data("[DONE]");
        assert!(sse.is_done());

        let sse2 = ServerSentEvent::with_data("hello");
        assert!(!sse2.is_done());
    }

    #[test]
    fn test_sse_json() {
        let sse = ServerSentEvent::with_data(r#"{"key": "value"}"#);
        let parsed: serde_json::Value = sse.json().unwrap();
        assert_eq!(parsed["key"], "value");
    }

    // =========================================================================
    // SSEDecoder Tests
    // =========================================================================

    #[test]
    fn test_decoder_basic_event() {
        let mut decoder = SSEDecoder::new();
        assert!(decoder.decode("data: hello").is_none());
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "hello");
    }

    #[test]
    fn test_decoder_event_type() {
        let mut decoder = SSEDecoder::new();
        assert!(decoder.decode("event: message").is_none());
        assert!(decoder.decode("data: hello").is_none());
        let event = decoder.decode("").unwrap();
        assert_eq!(event.event, Some("message".to_string()));
        assert_eq!(event.data, "hello");
    }

    #[test]
    fn test_decoder_multiline_data() {
        let mut decoder = SSEDecoder::new();
        assert!(decoder.decode("data: line1").is_none());
        assert!(decoder.decode("data: line2").is_none());
        assert!(decoder.decode("data: line3").is_none());
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "line1\nline2\nline3");
    }

    #[test]
    fn test_decoder_id_field() {
        let mut decoder = SSEDecoder::new();
        assert!(decoder.decode("id: 123").is_none());
        assert!(decoder.decode("data: test").is_none());
        let event = decoder.decode("").unwrap();
        assert_eq!(event.id, Some("123".to_string()));
    }

    #[test]
    fn test_decoder_id_persists() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("id: 123");
        decoder.decode("data: first");
        let event1 = decoder.decode("").unwrap();

        decoder.decode("data: second");
        let event2 = decoder.decode("").unwrap();

        // ID persists across events per SSE spec
        assert_eq!(event1.id, Some("123".to_string()));
        assert_eq!(event2.id, Some("123".to_string()));
    }

    #[test]
    fn test_decoder_id_with_null_ignored() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("id: has\0null");
        decoder.decode("data: test");
        let event = decoder.decode("").unwrap();
        // ID with null character is ignored per SSE spec
        assert!(event.id.is_none());
    }

    #[test]
    fn test_decoder_retry_field() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("retry: 5000");
        decoder.decode("data: test");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.retry, Some(5000));
    }

    #[test]
    fn test_decoder_retry_invalid_ignored() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("retry: invalid");
        decoder.decode("data: test");
        let event = decoder.decode("").unwrap();
        assert!(event.retry.is_none());
    }

    #[test]
    fn test_decoder_comment_ignored() {
        let mut decoder = SSEDecoder::new();
        assert!(decoder.decode(": this is a comment").is_none());
        decoder.decode("data: test");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "test");
    }

    #[test]
    fn test_decoder_space_after_colon() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("data: hello");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "hello"); // Space stripped
    }

    #[test]
    fn test_decoder_no_space_after_colon() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("data:hello");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "hello");
    }

    #[test]
    fn test_decoder_empty_data() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("data:");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "");
    }

    #[test]
    fn test_decoder_unknown_field_ignored() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("unknown: value");
        decoder.decode("data: test");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, "test");
    }

    #[test]
    fn test_decoder_field_without_colon() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("data");
        let event = decoder.decode("").unwrap();
        assert_eq!(event.data, ""); // Treated as empty data
    }

    #[test]
    fn test_decoder_decode_chunk() {
        let mut decoder = SSEDecoder::new();
        let mut buffer = Vec::new();

        let events = decoder.decode_chunk(b"event: test\ndata: hello\n\n", &mut buffer);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("test".to_string()));
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_decoder_decode_chunk_partial() {
        let mut decoder = SSEDecoder::new();
        let mut buffer = Vec::new();

        // First chunk is partial
        let events1 = decoder.decode_chunk(b"event: te", &mut buffer);
        assert!(events1.is_empty());

        // Second chunk completes
        let events2 = decoder.decode_chunk(b"st\ndata: hi\n\n", &mut buffer);
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].event, Some("test".to_string()));
        assert_eq!(events2[0].data, "hi");
    }

    #[test]
    fn test_decoder_decode_chunk_crlf() {
        let mut decoder = SSEDecoder::new();
        let mut buffer = Vec::new();

        let events = decoder.decode_chunk(b"data: hello\r\n\r\n", &mut buffer);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_decoder_reset() {
        let mut decoder = SSEDecoder::new();
        decoder.decode("event: test");
        decoder.decode("data: hello");
        decoder.decode("id: 123");

        decoder.reset();

        decoder.decode("data: new");
        let event = decoder.decode("").unwrap();
        assert!(event.event.is_none());
        assert_eq!(event.data, "new");
        assert!(event.id.is_none());
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[tokio::test]
    async fn test_parse_sse_events_basic() {
        let data = b"event: message\ndata: hello\n\n";
        let byte_stream = stream::iter(vec![Ok(Bytes::from(&data[..]))]);
        let mut event_stream = parse_sse_events(byte_stream);

        let event = event_stream.next().await.unwrap().unwrap();
        assert_eq!(event.event, Some("message".to_string()));
        assert_eq!(event.data, "hello");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_single_event() {
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]}}]}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        let response = content_stream.next().await.unwrap().unwrap();
        assert_eq!(response.text(), Some("Hello".to_string()));
    }

    #[tokio::test]
    async fn test_parse_sse_stream_multiple_events() {
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]}}]}

data: {"candidates":[{"content":{"role":"model","parts":[{"text":" World"}]}}]}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let content_stream = parse_sse_stream(byte_stream);

        let responses: Vec<_> = content_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].text(), Some("Hello".to_string()));
        assert_eq!(responses[1].text(), Some(" World".to_string()));
    }

    #[tokio::test]
    async fn test_parse_sse_stream_chunked_delivery() {
        // Simulate data arriving in chunks
        let chunks = vec![
            Ok(Bytes::from("data: {\"candi")),
            Ok(Bytes::from(
                "dates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hi\"}]}}]}\n\n",
            )),
        ];
        let byte_stream = stream::iter(chunks);
        let mut content_stream = parse_sse_stream(byte_stream);

        let response = content_stream.next().await.unwrap().unwrap();
        assert_eq!(response.text(), Some("Hi".to_string()));
    }

    #[tokio::test]
    async fn test_parse_sse_stream_with_done_marker() {
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Done"}]}}]}

data: [DONE]

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let content_stream = parse_sse_stream(byte_stream);

        let responses: Vec<_> = content_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        // [DONE] marker should not produce a response
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].text(), Some("Done".to_string()));
    }

    #[tokio::test]
    async fn test_parse_sse_stream_with_comments() {
        let data = r#": this is a comment
data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hi"}]}}]}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        let response = content_stream.next().await.unwrap().unwrap();
        assert_eq!(response.text(), Some("Hi".to_string()));
    }

    #[tokio::test]
    async fn test_parse_sse_stream_parse_error() {
        // Test with invalid JSON in SSE data
        let data = "data: {invalid json}\n\n";
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        let result = content_stream.next().await.unwrap();
        assert!(result.is_err());
    }

    // =========================================================================
    // Error Handling Tests (Aligned with Python SDK)
    // =========================================================================

    #[tokio::test]
    async fn test_parse_sse_stream_with_error_response() {
        // Aligns with Python SDK test: test_error_event_in_generate_content_stream
        // First chunk is valid, second chunk is an error
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"test"}]}}]}

data: {"error":{"code":500,"message":"Internal Server Error","status":"INTERNAL"}}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        // First chunk should succeed
        let response = content_stream.next().await.unwrap().unwrap();
        assert_eq!(response.text(), Some("test".to_string()));

        // Second chunk should be an API error
        let result = content_stream.next().await.unwrap();
        assert!(result.is_err());

        match result.unwrap_err() {
            GenAiError::Api {
                code,
                message,
                status,
            } => {
                assert_eq!(code, 500);
                assert_eq!(message, "Internal Server Error");
                assert_eq!(status, "INTERNAL");
            }
            other => panic!("Expected GenAiError::Api, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_parse_sse_stream_error_only() {
        // Test stream that only returns an error
        let data = r#"data: {"error":{"code":429,"message":"Rate limit exceeded","status":"RESOURCE_EXHAUSTED"}}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        let result = content_stream.next().await.unwrap();
        assert!(result.is_err());

        match result.unwrap_err() {
            GenAiError::Api {
                code,
                message,
                status,
            } => {
                assert_eq!(code, 429);
                assert_eq!(message, "Rate limit exceeded");
                assert_eq!(status, "RESOURCE_EXHAUSTED");
            }
            other => panic!("Expected GenAiError::Api, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_parse_sse_stream_error_with_bad_json() {
        // Aligns with Python SDK test: test_error_event_in_streamed_responses_bad_json
        // First chunk valid, second chunk has malformed JSON (not a valid error or response)
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"test"}]}}]}

data: {"error": bad_json}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        // First chunk should succeed
        let response = content_stream.next().await.unwrap().unwrap();
        assert_eq!(response.text(), Some("test".to_string()));

        // Second chunk should be a parse error (not an API error, since JSON is invalid)
        let result = content_stream.next().await.unwrap();
        assert!(result.is_err());

        match result.unwrap_err() {
            GenAiError::Parse(_) => {} // Expected
            other => panic!("Expected GenAiError::Parse, got: {:?}", other),
        }
    }
}
