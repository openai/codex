//! Streaming support for Google Generative AI API.
//!
//! This module provides SSE (Server-Sent Events) parsing for streaming responses
//! from the `streamGenerateContent` endpoint.

use crate::error::GenAiError;
use crate::error::Result;
use crate::types::GenerateContentResponse;
use bytes::Bytes;
use futures::stream::Stream;
use std::pin::Pin;

/// Type alias for a streaming response.
///
/// Each item in the stream is a `GenerateContentResponse` chunk containing
/// partial content that should be accumulated by the caller.
pub type ContentStream = Pin<Box<dyn Stream<Item = Result<GenerateContentResponse>> + Send>>;

/// Type alias for a boxed byte stream (pinned for polling).
type BoxedByteStream =
    Pin<Box<dyn Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send>>;

/// Parse an SSE byte stream into a stream of `GenerateContentResponse` chunks.
///
/// # SSE Wire Format
///
/// The Google Gemini streaming API uses Server-Sent Events format:
/// ```text
/// data: {"candidates":[{"content":{"parts":[{"text":"Hello"}]}}]}
///
/// data: {"candidates":[{"content":{"parts":[{"text":" World"}]}}]}
///
/// data: {"candidates":[...],"usageMetadata":{...}}
/// ```
///
/// Each `data:` line contains a complete JSON `GenerateContentResponse`.
pub(crate) fn parse_sse_stream<S>(byte_stream: S) -> ContentStream
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    // Box the inner stream to make it Unpin
    let boxed: BoxedByteStream = Box::pin(byte_stream);
    Box::pin(SseParser::new(boxed))
}

/// SSE parser that converts a byte stream into response chunks.
struct SseParser {
    inner: BoxedByteStream,
    buffer: String,
}

impl SseParser {
    fn new(inner: BoxedByteStream) -> Self {
        Self {
            inner,
            buffer: String::new(),
        }
    }

    /// Try to extract and parse a complete SSE event from the buffer.
    ///
    /// Returns `Some(result)` if an event was found, `None` if more data is needed.
    fn try_parse_event(&mut self) -> Option<Result<GenerateContentResponse>> {
        // Look for "data: " prefix followed by JSON
        while let Some(data_start) = self.buffer.find("data:") {
            // Find the end of this data line (double newline or end of data)
            let json_start = data_start + 5; // "data:" length
            let remaining = &self.buffer[json_start..];

            // Skip whitespace after "data:"
            let remaining = remaining.trim_start();

            // Handle empty data or [DONE] marker
            if remaining.starts_with("[DONE]") {
                // Remove everything up to and including [DONE]
                if let Some(done_end) = self.buffer[data_start..].find('\n') {
                    self.buffer.drain(..data_start + done_end + 1);
                } else {
                    self.buffer.clear();
                }
                continue;
            }

            // Find JSON boundaries - look for complete JSON object
            if let Some(json_end) = find_json_end(remaining) {
                // Find the actual start of JSON in the buffer
                let json_in_buffer_start = self.buffer[data_start + 5..]
                    .find(|c: char| c == '{' || c == '[')
                    .map(|i| data_start + 5 + i);

                if let Some(start) = json_in_buffer_start {
                    let end = start + json_end;

                    // Extract the JSON
                    let json_str = &self.buffer[start..end];

                    // Parse the JSON
                    let result =
                        serde_json::from_str::<GenerateContentResponse>(json_str).map_err(|e| {
                            GenAiError::Parse(format!(
                                "Failed to parse streaming response: {e}\nJSON: {json_str}"
                            ))
                        });

                    // Remove processed data from buffer (up to next newline after JSON)
                    let drain_to = self.buffer[end..]
                        .find('\n')
                        .map(|i| end + i + 1)
                        .unwrap_or(end);
                    self.buffer.drain(..drain_to);

                    return Some(result);
                }
            }

            // JSON not complete yet - need more data
            // But first, clean up any garbage before the data: prefix
            if data_start > 0 {
                self.buffer.drain(..data_start);
            }
            return None;
        }

        // No "data:" found - clean up buffer if it's just whitespace/newlines
        let trimmed = self.buffer.trim();
        if trimmed.is_empty() {
            self.buffer.clear();
        }

        None
    }
}

impl Stream for SseParser {
    type Item = Result<GenerateContentResponse>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;

        loop {
            // Try to extract a complete SSE event from the buffer
            if let Some(response) = self.try_parse_event() {
                return Poll::Ready(Some(response));
            }

            // Need more data - poll the inner stream
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    // Append new bytes to buffer
                    match std::str::from_utf8(&bytes) {
                        Ok(s) => self.buffer.push_str(s),
                        Err(e) => {
                            return Poll::Ready(Some(Err(GenAiError::Parse(format!(
                                "Invalid UTF-8 in stream: {e}"
                            )))));
                        }
                    }
                    // Continue loop to try parsing
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(GenAiError::Network(e.to_string()))));
                }
                Poll::Ready(None) => {
                    // Stream ended - check if there's remaining data
                    if self.buffer.trim().is_empty() {
                        return Poll::Ready(None);
                    }
                    // Try to parse any remaining data
                    if let Some(response) = self.try_parse_event() {
                        return Poll::Ready(Some(response));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Find the end of a JSON object or array, handling nesting.
fn find_json_end(s: &str) -> Option<usize> {
    let mut chars = s.chars().peekable();
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut pos = 0;
    let mut started = false;

    while let Some(c) = chars.next() {
        if escape_next {
            escape_next = false;
            pos += c.len_utf8();
            continue;
        }

        match c {
            '\\' if in_string => {
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
            }
            '{' | '[' if !in_string => {
                depth += 1;
                started = true;
            }
            '}' | ']' if !in_string => {
                depth -= 1;
                if started && depth == 0 {
                    return Some(pos + c.len_utf8());
                }
            }
            _ => {}
        }
        pos += c.len_utf8();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use futures::stream;

    #[test]
    fn test_find_json_end() {
        assert_eq!(find_json_end(r#"{"text":"hello"}"#), Some(16));
        assert_eq!(find_json_end(r#"{"a":{"b":1}}"#), Some(13));
        assert_eq!(find_json_end(r#"{"text":"he\"llo"}"#), Some(18));
        assert_eq!(find_json_end(r#"[1,2,3]"#), Some(7));
        assert_eq!(find_json_end(r#"{"incomplete"#), None);
    }

    #[tokio::test]
    async fn test_parse_sse_single_event() {
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]}}]}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut content_stream = parse_sse_stream(byte_stream);

        let response = content_stream.next().await.unwrap().unwrap();
        assert_eq!(response.text(), Some("Hello".to_string()));
    }

    #[tokio::test]
    async fn test_parse_sse_multiple_events() {
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
    async fn test_parse_sse_chunked_delivery() {
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
    async fn test_parse_sse_with_done_marker() {
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

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].text(), Some("Done".to_string()));
    }
}
