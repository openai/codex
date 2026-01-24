//! Streaming support for Anthropic Messages API.
//!
//! This module provides SSE (Server-Sent Events) parsing for streaming responses
//! from the `/v1/messages` endpoint with `stream=true`.
//!
//! # Example
//!
//! ```no_run
//! use anthropic_sdk::{Client, MessageCreateParams, MessageParam};
//! use futures::StreamExt;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::from_env()?;
//!
//! let mut stream = client.messages().create_stream(
//!     MessageCreateParams::new(
//!         "claude-sonnet-4-20250514",
//!         1024,
//!         vec![MessageParam::user("Hello!")],
//!     )
//! ).await?;
//!
//! // Process text deltas
//! while let Some(event) = stream.next_event().await {
//!     if let Ok(anthropic_sdk::RawMessageStreamEvent::ContentBlockDelta { delta, .. }) = event {
//!         if let Some(text) = delta.as_text() {
//!             print!("{}", text);
//!         }
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
use futures::StreamExt;
use futures::stream::Stream;

use crate::error::AnthropicError;
use crate::error::Result;
use crate::types::ContentBlock;
use crate::types::ContentBlockDelta;
use crate::types::ContentBlockStartData;
use crate::types::Message;
use crate::types::MessageDeltaUsage;
use crate::types::MessageStartData;
use crate::types::RawMessageStreamEvent;
use crate::types::Role;
use crate::types::StopReason;
use crate::types::StreamError;
use crate::types::Usage;

/// Type alias for a streaming response of raw SSE events.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<RawMessageStreamEvent>> + Send>>;

/// Type alias for a boxed byte stream.
type BoxedByteStream =
    Pin<Box<dyn Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send>>;

/// Parse an SSE byte stream into a stream of `RawMessageStreamEvent`.
pub(crate) fn parse_sse_stream<S>(byte_stream: S) -> EventStream
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let boxed: BoxedByteStream = Box::pin(byte_stream);
    Box::pin(SseParser::new(boxed))
}

// ============================================================================
// SSE Parser
// ============================================================================

/// SSE parser that converts a byte stream into raw message stream events.
struct SseParser {
    inner: BoxedByteStream,
    buffer: String,
    current_event_type: Option<String>,
    current_data: Vec<String>,
}

impl SseParser {
    fn new(inner: BoxedByteStream) -> Self {
        Self {
            inner,
            buffer: String::new(),
            current_event_type: None,
            current_data: Vec::new(),
        }
    }

    /// Try to extract and parse a complete SSE event from the buffer.
    fn try_parse_event(&mut self) -> Option<Result<RawMessageStreamEvent>> {
        // Process complete lines in the buffer
        while let Some(newline_pos) = self.buffer.find('\n') {
            let line = self.buffer[..newline_pos]
                .trim_end_matches('\r')
                .to_string();
            self.buffer.drain(..=newline_pos);

            if line.is_empty() {
                // Empty line = end of event
                if self.current_event_type.is_some() || !self.current_data.is_empty() {
                    let event_type = self.current_event_type.take();
                    let data = std::mem::take(&mut self.current_data).join("\n");
                    if let Some(event_type) = event_type {
                        return Some(self.parse_event(&event_type, &data));
                    }
                }
                continue;
            }

            // Skip comment lines
            if line.starts_with(':') {
                continue;
            }

            // Parse field: value
            if let Some(colon_pos) = line.find(':') {
                let field = &line[..colon_pos];
                let value = line[colon_pos + 1..].trim_start_matches(' ');

                match field {
                    "event" => {
                        self.current_event_type = Some(value.to_string());
                    }
                    "data" => {
                        self.current_data.push(value.to_string());
                    }
                    "id" => {
                        // Per SSE spec, id fields containing null bytes must be ignored.
                        // The Anthropic API doesn't use the id field for message tracking,
                        // so we don't store it, but we validate it for spec compliance.
                        if value.contains('\0') {
                            // Ignore id with null bytes (SSE spec requirement)
                        }
                        // Note: We don't use the id field, but it's acknowledged
                    }
                    // Ignore "retry" and unknown fields
                    _ => {}
                }
            } else {
                // Line without colon - treat as field with empty value
                match line.as_str() {
                    "event" => self.current_event_type = Some(String::new()),
                    "data" => self.current_data.push(String::new()),
                    _ => {}
                }
            }
        }

        None
    }

    /// Parse a complete event given its type and data.
    fn parse_event(&self, event_type: &str, data: &str) -> Result<RawMessageStreamEvent> {
        match event_type {
            "message_start" => {
                let wrapper: MessageStartWrapper = serde_json::from_str(data).map_err(|e| {
                    AnthropicError::Parse(format!("Failed to parse message_start: {e}"))
                })?;
                Ok(RawMessageStreamEvent::MessageStart {
                    message: wrapper.message,
                })
            }
            "content_block_start" => {
                let wrapper: ContentBlockStartWrapper =
                    serde_json::from_str(data).map_err(|e| {
                        AnthropicError::Parse(format!("Failed to parse content_block_start: {e}"))
                    })?;
                Ok(RawMessageStreamEvent::ContentBlockStart {
                    index: wrapper.index,
                    content_block: wrapper.content_block,
                })
            }
            "content_block_delta" => {
                let wrapper: ContentBlockDeltaWrapper =
                    serde_json::from_str(data).map_err(|e| {
                        AnthropicError::Parse(format!("Failed to parse content_block_delta: {e}"))
                    })?;
                Ok(RawMessageStreamEvent::ContentBlockDelta {
                    index: wrapper.index,
                    delta: wrapper.delta,
                })
            }
            "content_block_stop" => {
                let wrapper: ContentBlockStopWrapper = serde_json::from_str(data).map_err(|e| {
                    AnthropicError::Parse(format!("Failed to parse content_block_stop: {e}"))
                })?;
                Ok(RawMessageStreamEvent::ContentBlockStop {
                    index: wrapper.index,
                })
            }
            "message_delta" => {
                let wrapper: MessageDeltaWrapper = serde_json::from_str(data).map_err(|e| {
                    AnthropicError::Parse(format!("Failed to parse message_delta: {e}"))
                })?;
                Ok(RawMessageStreamEvent::MessageDelta {
                    delta: wrapper.delta,
                    usage: wrapper.usage,
                })
            }
            "message_stop" => Ok(RawMessageStreamEvent::MessageStop),
            "ping" => Ok(RawMessageStreamEvent::Ping),
            "error" => {
                let wrapper: ErrorWrapper = serde_json::from_str(data)
                    .map_err(|e| AnthropicError::Parse(format!("Failed to parse error: {e}")))?;
                Ok(RawMessageStreamEvent::Error {
                    error: wrapper.error,
                })
            }
            _ => Err(AnthropicError::Parse(format!(
                "Unknown event type: {event_type}"
            ))),
        }
    }
}

// Wrapper structs for deserialization
#[derive(serde::Deserialize)]
struct MessageStartWrapper {
    message: MessageStartData,
}

#[derive(serde::Deserialize)]
struct ContentBlockStartWrapper {
    index: i32,
    content_block: ContentBlockStartData,
}

#[derive(serde::Deserialize)]
struct ContentBlockDeltaWrapper {
    index: i32,
    delta: ContentBlockDelta,
}

#[derive(serde::Deserialize)]
struct ContentBlockStopWrapper {
    index: i32,
}

#[derive(serde::Deserialize)]
struct MessageDeltaWrapper {
    delta: crate::types::MessageDeltaData,
    usage: MessageDeltaUsage,
}

#[derive(serde::Deserialize)]
struct ErrorWrapper {
    error: StreamError,
}

impl Stream for SseParser {
    type Item = Result<RawMessageStreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to extract a complete SSE event from the buffer
            if let Some(event) = self.try_parse_event() {
                return Poll::Ready(Some(event));
            }

            // Need more data - poll the inner stream
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => match std::str::from_utf8(&bytes) {
                    Ok(s) => self.buffer.push_str(s),
                    Err(e) => {
                        return Poll::Ready(Some(Err(AnthropicError::Parse(format!(
                            "Invalid UTF-8 in stream: {e}"
                        )))));
                    }
                },
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(AnthropicError::Network(e))));
                }
                Poll::Ready(None) => {
                    // Stream ended - try to parse remaining data
                    if let Some(event) = self.try_parse_event() {
                        return Poll::Ready(Some(event));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// ============================================================================
// MessageStream - High-level wrapper
// ============================================================================

/// A high-level wrapper for streaming message responses.
///
/// This accumulates events into a complete `Message` and provides
/// convenience methods for common use cases.
///
/// # Example
///
/// ```no_run
/// use anthropic_sdk::{Client, MessageCreateParams, MessageParam, RawMessageStreamEvent, ContentBlockDelta};
/// use futures::StreamExt;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = Client::from_env()?;
///
/// let mut stream = client.messages().create_stream(
///     MessageCreateParams::new(
///         "claude-sonnet-4-20250514",
///         1024,
///         vec![MessageParam::user("Write a haiku")],
///     )
/// ).await?;
///
/// // Option 1: Process individual events
/// while let Some(event) = stream.next_event().await {
///     match event? {
///         RawMessageStreamEvent::ContentBlockDelta { delta, .. } => {
///             if let ContentBlockDelta::TextDelta { text } = delta {
///                 print!("{}", text);
///             }
///         }
///         _ => {}
///     }
/// }
///
/// // Option 2: Get the final accumulated message
/// // let message = stream.get_final_message().await?;
/// # Ok(())
/// # }
/// ```
pub struct MessageStream {
    inner: EventStream,
    /// Accumulated state
    state: MessageStreamState,
}

#[derive(Default)]
struct MessageStreamState {
    id: Option<String>,
    model: Option<String>,
    role: Role,
    content_blocks: HashMap<i32, ContentBlockBuilder>,
    stop_reason: Option<StopReason>,
    stop_sequence: Option<String>,
    usage: Usage,
}

#[derive(Default)]
struct ContentBlockBuilder {
    block_type: ContentBlockType,
    text: String,
    citations: Vec<serde_json::Value>,
    tool_id: Option<String>,
    tool_name: Option<String>,
    tool_input_json: String,
    thinking: String,
    signature: String,
    redacted_data: String,
    // For WebSearchToolResult
    web_search_tool_use_id: Option<String>,
    web_search_content: serde_json::Value,
}

#[derive(Default, Clone, Copy)]
enum ContentBlockType {
    #[default]
    Unknown,
    Text,
    ToolUse,
    ServerToolUse,
    WebSearchToolResult,
    Thinking,
    RedactedThinking,
}

impl Default for Role {
    fn default() -> Self {
        Role::Assistant
    }
}

impl MessageStream {
    /// Create a new MessageStream from an event stream.
    pub fn new(inner: EventStream) -> Self {
        Self {
            inner,
            state: MessageStreamState::default(),
        }
    }

    /// Get the next raw event from the stream.
    ///
    /// Also updates internal state for `get_final_message()`.
    pub async fn next_event(&mut self) -> Option<Result<RawMessageStreamEvent>> {
        let event = self.inner.next().await?;

        // Update state on success
        if let Ok(ref evt) = event {
            self.update_state(evt);
        }

        Some(event)
    }

    /// Consume the stream and return a stream of text deltas only.
    ///
    /// This filters out all non-text events and yields only the text content.
    pub fn text_stream(self) -> impl Stream<Item = Result<String>> + Send {
        self.inner.filter_map(|event| async move {
            match event {
                Ok(RawMessageStreamEvent::ContentBlockDelta {
                    delta: ContentBlockDelta::TextDelta { text },
                    ..
                }) => Some(Ok(text)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })
    }

    /// Wait for the stream to complete and return the accumulated message.
    ///
    /// This consumes the stream and returns a complete `Message` with all
    /// content blocks assembled from the streaming deltas.
    pub async fn get_final_message(mut self) -> Result<Message> {
        while let Some(event) = self.inner.next().await {
            match event {
                Ok(evt) => self.update_state(&evt),
                Err(e) => return Err(e),
            }
        }

        self.build_message()
    }

    /// Wait for completion and return concatenated text content.
    ///
    /// This is a convenience method that calls `get_final_message()` and
    /// extracts only the text content.
    pub async fn get_final_text(self) -> Result<String> {
        let message = self.get_final_message().await?;
        Ok(message.text())
    }

    /// Get the current message snapshot without consuming the stream.
    ///
    /// Returns `None` if no `message_start` event has been received yet.
    /// The returned message represents the accumulated state so far.
    pub fn current_message_snapshot(&self) -> Option<Result<Message>> {
        // Only return a snapshot if we have received message_start
        if self.state.id.is_none() {
            return None;
        }
        Some(self.build_message())
    }

    fn update_state(&mut self, event: &RawMessageStreamEvent) {
        match event {
            RawMessageStreamEvent::MessageStart { message } => {
                self.state.id = Some(message.id.clone());
                self.state.model = Some(message.model.clone());
                self.state.role = message.role;
                self.state.usage = message.usage.clone();
            }
            RawMessageStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let builder = self.state.content_blocks.entry(*index).or_default();
                match content_block {
                    ContentBlockStartData::Text { text } => {
                        builder.block_type = ContentBlockType::Text;
                        builder.text = text.clone();
                    }
                    ContentBlockStartData::ToolUse { id, name, input } => {
                        builder.block_type = ContentBlockType::ToolUse;
                        builder.tool_id = Some(id.clone());
                        builder.tool_name = Some(name.clone());
                        // Only set initial input if it's a non-empty object
                        // (deltas will be accumulated via input_json_delta events)
                        if !input.is_null() && input.as_object().map_or(true, |obj| !obj.is_empty())
                        {
                            builder.tool_input_json = input.to_string();
                        }
                    }
                    ContentBlockStartData::ServerToolUse { id, name, input } => {
                        builder.block_type = ContentBlockType::ServerToolUse;
                        builder.tool_id = Some(id.clone());
                        builder.tool_name = Some(name.clone());
                        if !input.is_null() && input.as_object().map_or(true, |obj| !obj.is_empty())
                        {
                            builder.tool_input_json = input.to_string();
                        }
                    }
                    ContentBlockStartData::WebSearchToolResult {
                        tool_use_id,
                        content,
                    } => {
                        builder.block_type = ContentBlockType::WebSearchToolResult;
                        builder.web_search_tool_use_id = Some(tool_use_id.clone());
                        builder.web_search_content = content.clone();
                    }
                    ContentBlockStartData::Thinking { thinking } => {
                        builder.block_type = ContentBlockType::Thinking;
                        builder.thinking = thinking.clone();
                    }
                    ContentBlockStartData::RedactedThinking { data } => {
                        builder.block_type = ContentBlockType::RedactedThinking;
                        builder.redacted_data = data.clone();
                    }
                }
            }
            RawMessageStreamEvent::ContentBlockDelta { index, delta } => {
                let builder = self.state.content_blocks.entry(*index).or_default();
                match delta {
                    ContentBlockDelta::TextDelta { text } => {
                        builder.text.push_str(text);
                    }
                    ContentBlockDelta::InputJsonDelta { partial_json } => {
                        builder.tool_input_json.push_str(partial_json);
                    }
                    ContentBlockDelta::ThinkingDelta { thinking } => {
                        builder.thinking.push_str(thinking);
                    }
                    ContentBlockDelta::SignatureDelta { signature } => {
                        builder.signature.push_str(signature);
                    }
                    ContentBlockDelta::CitationsDelta { citation } => {
                        builder.citations.push(citation.clone());
                    }
                }
            }
            RawMessageStreamEvent::MessageDelta { delta, usage } => {
                self.state.stop_reason = delta.stop_reason;
                self.state.stop_sequence.clone_from(&delta.stop_sequence);
                self.state.usage.output_tokens = usage.output_tokens;
                // Update optional fields if present
                if let Some(input_tokens) = usage.input_tokens {
                    self.state.usage.input_tokens = input_tokens;
                }
                if let Some(cache_creation) = usage.cache_creation_input_tokens {
                    self.state.usage.cache_creation_input_tokens = Some(cache_creation);
                }
                if let Some(cache_read) = usage.cache_read_input_tokens {
                    self.state.usage.cache_read_input_tokens = Some(cache_read);
                }
                if let Some(server_tool_use) = usage.server_tool_use.clone() {
                    self.state.usage.server_tool_use = Some(server_tool_use);
                }
            }
            RawMessageStreamEvent::ContentBlockStop { .. }
            | RawMessageStreamEvent::MessageStop
            | RawMessageStreamEvent::Ping => {}
            RawMessageStreamEvent::Error { .. } => {
                // Errors are handled by the caller
            }
        }
    }

    fn build_message(&self) -> Result<Message> {
        let id = self.state.id.clone().unwrap_or_default();
        let model = self.state.model.clone().unwrap_or_default();

        // Build content blocks in order
        let mut indices: Vec<_> = self.state.content_blocks.keys().copied().collect();
        indices.sort();

        let content: Vec<ContentBlock> = indices
            .into_iter()
            .filter_map(|idx| self.state.content_blocks.get(&idx).map(|b| b.build()))
            .collect::<Result<Vec<_>>>()?;

        Ok(Message {
            id,
            message_type: "message".to_string(),
            role: self.state.role,
            content,
            model,
            stop_reason: self.state.stop_reason,
            stop_sequence: self.state.stop_sequence.clone(),
            usage: self.state.usage.clone(),
            sdk_http_response: None,
        })
    }
}

impl ContentBlockBuilder {
    fn build(&self) -> Result<ContentBlock> {
        match self.block_type {
            ContentBlockType::Text => {
                let citations = if self.citations.is_empty() {
                    None
                } else {
                    // Convert accumulated citations to TextCitation
                    let text_citations: Vec<crate::types::TextCitation> = self
                        .citations
                        .iter()
                        .filter_map(|c| serde_json::from_value(c.clone()).ok())
                        .collect();
                    if text_citations.is_empty() {
                        None
                    } else {
                        Some(text_citations)
                    }
                };
                Ok(ContentBlock::Text {
                    text: self.text.clone(),
                    citations,
                })
            }
            ContentBlockType::ToolUse => {
                let input: serde_json::Value = if self.tool_input_json.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(&self.tool_input_json).map_err(|e| {
                        AnthropicError::Parse(format!("Failed to parse tool input JSON: {e}"))
                    })?
                };
                Ok(ContentBlock::ToolUse {
                    id: self.tool_id.clone().unwrap_or_default(),
                    name: self.tool_name.clone().unwrap_or_default(),
                    input,
                })
            }
            ContentBlockType::ServerToolUse => {
                let input: serde_json::Value = if self.tool_input_json.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(&self.tool_input_json).map_err(|e| {
                        AnthropicError::Parse(format!(
                            "Failed to parse server tool input JSON: {e}"
                        ))
                    })?
                };
                Ok(ContentBlock::ServerToolUse {
                    id: self.tool_id.clone().unwrap_or_default(),
                    name: self.tool_name.clone().unwrap_or_default(),
                    input,
                })
            }
            ContentBlockType::WebSearchToolResult => Ok(ContentBlock::WebSearchToolResult {
                tool_use_id: self.web_search_tool_use_id.clone().unwrap_or_default(),
                content: self.web_search_content.clone(),
            }),
            ContentBlockType::Thinking => Ok(ContentBlock::Thinking {
                thinking: self.thinking.clone(),
                signature: self.signature.clone(),
            }),
            ContentBlockType::RedactedThinking => Ok(ContentBlock::RedactedThinking {
                data: self.redacted_data.clone(),
            }),
            ContentBlockType::Unknown => Err(AnthropicError::Parse(
                "Unknown content block type".to_string(),
            )),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[test]
    fn test_parse_text_delta() {
        let delta: ContentBlockDelta =
            serde_json::from_str(r#"{"type": "text_delta", "text": "Hello"}"#).unwrap();

        assert!(matches!(
            delta,
            ContentBlockDelta::TextDelta { text } if text == "Hello"
        ));
    }

    #[test]
    fn test_parse_input_json_delta() {
        let delta: ContentBlockDelta =
            serde_json::from_str(r#"{"type": "input_json_delta", "partial_json": "{\"key\":"}"#)
                .unwrap();

        assert!(matches!(
            delta,
            ContentBlockDelta::InputJsonDelta { partial_json }
            if partial_json == "{\"key\":"
        ));
    }

    #[test]
    fn test_parse_thinking_delta() {
        let delta: ContentBlockDelta =
            serde_json::from_str(r#"{"type": "thinking_delta", "thinking": "Let me think..."}"#)
                .unwrap();

        assert!(matches!(
            delta,
            ContentBlockDelta::ThinkingDelta { thinking } if thinking == "Let me think..."
        ));
    }

    #[tokio::test]
    async fn test_parse_sse_message_start() {
        let data = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg-123","type":"message","role":"assistant","model":"claude-3-5-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut event_stream = parse_sse_stream(byte_stream);

        let event = event_stream.next().await.unwrap().unwrap();
        assert!(matches!(event, RawMessageStreamEvent::MessageStart { .. }));
    }

    #[tokio::test]
    async fn test_parse_sse_content_block_delta() {
        let data = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut event_stream = parse_sse_stream(byte_stream);

        let event = event_stream.next().await.unwrap().unwrap();
        match event {
            RawMessageStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                assert!(matches!(
                    delta,
                    ContentBlockDelta::TextDelta { text } if text == "Hello"
                ));
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[tokio::test]
    async fn test_parse_sse_multiple_events() {
        let data = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg-123","type":"message","role":"assistant","model":"claude-3-5-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" World"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}

event: message_stop
data: {"type":"message_stop"}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let event_stream = parse_sse_stream(byte_stream);
        let events: Vec<_> = event_stream.collect().await;

        assert_eq!(events.len(), 7);
        assert!(matches!(
            events[0].as_ref().unwrap(),
            RawMessageStreamEvent::MessageStart { .. }
        ));
        assert!(matches!(
            events[1].as_ref().unwrap(),
            RawMessageStreamEvent::ContentBlockStart { .. }
        ));
        assert!(matches!(
            events[6].as_ref().unwrap(),
            RawMessageStreamEvent::MessageStop
        ));
    }

    #[tokio::test]
    async fn test_message_stream_accumulation() {
        let data = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg-123","type":"message","role":"assistant","model":"claude-3-5-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" World"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}

event: message_stop
data: {"type":"message_stop"}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let event_stream = parse_sse_stream(byte_stream);
        let stream = MessageStream::new(event_stream);

        let message = stream.get_final_message().await.unwrap();

        assert_eq!(message.id, "msg-123");
        assert_eq!(message.text(), "Hello World");
        assert_eq!(message.stop_reason, Some(StopReason::EndTurn));
        assert_eq!(message.usage.output_tokens, 5);
    }

    #[tokio::test]
    async fn test_text_stream() {
        let data = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg-123","type":"message","role":"assistant","model":"claude-3-5-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" World"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_stop
data: {"type":"message_stop"}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let event_stream = parse_sse_stream(byte_stream);
        let stream = MessageStream::new(event_stream);

        let texts: Vec<String> = stream
            .text_stream()
            .filter_map(|r| async { r.ok() })
            .collect()
            .await;

        assert_eq!(texts, vec!["Hello", " World"]);
    }

    #[tokio::test]
    async fn test_parse_ping_event() {
        let data = r#"event: ping
data: {"type":"ping"}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut event_stream = parse_sse_stream(byte_stream);

        let event = event_stream.next().await.unwrap().unwrap();
        assert!(matches!(event, RawMessageStreamEvent::Ping));
    }

    #[tokio::test]
    async fn test_parse_error_event() {
        let data = r#"event: error
data: {"type":"error","error":{"type":"overloaded_error","message":"Server is overloaded"}}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut event_stream = parse_sse_stream(byte_stream);

        let event = event_stream.next().await.unwrap().unwrap();
        match event {
            RawMessageStreamEvent::Error { error } => {
                assert_eq!(error.error_type, "overloaded_error");
                assert_eq!(error.message, "Server is overloaded");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[tokio::test]
    async fn test_chunked_sse_data() {
        // Test that SSE parsing works when data is split across multiple chunks
        let chunk1 = Bytes::from("event: message_start\ndata: {\"type\":");
        let chunk2 = Bytes::from("\"message_start\",\"message\":{\"id\":\"msg-123\",");
        let chunk3 = Bytes::from("\"type\":\"message\",\"role\":\"assistant\",");
        let chunk4 = Bytes::from("\"model\":\"claude-3-5-sonnet\",\"content\":[],");
        let chunk5 = Bytes::from(
            "\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
        );

        let byte_stream = stream::iter(vec![
            Ok(chunk1),
            Ok(chunk2),
            Ok(chunk3),
            Ok(chunk4),
            Ok(chunk5),
        ]);
        let mut event_stream = parse_sse_stream(byte_stream);

        let event = event_stream.next().await.unwrap().unwrap();
        assert!(matches!(event, RawMessageStreamEvent::MessageStart { .. }));
    }

    #[tokio::test]
    async fn test_multi_line_data() {
        // Test multi-line data concatenation (per SSE spec, multiple data lines are joined with \n)
        let data = r#"event: content_block_delta
data: {"type":"content_block_delta",
data: "index":0,
data: "delta":{"type":"text_delta","text":"Hello"}}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let mut event_stream = parse_sse_stream(byte_stream);

        // This tests that multi-line data is properly joined
        let event = event_stream.next().await.unwrap().unwrap();
        match event {
            RawMessageStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                assert!(matches!(delta, ContentBlockDelta::TextDelta { text } if text == "Hello"));
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[tokio::test]
    async fn test_tool_use_accumulation() {
        let data = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg-456","type":"message","role":"assistant","model":"claude-3-5-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":50,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call_123","name":"get_weather","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"location\":"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"San Francisco\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":25}}

event: message_stop
data: {"type":"message_stop"}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let event_stream = parse_sse_stream(byte_stream);
        let stream = MessageStream::new(event_stream);

        let message = stream.get_final_message().await.unwrap();

        assert_eq!(message.id, "msg-456");
        assert!(message.has_tool_use());

        let tool_uses = message.tool_uses();
        assert_eq!(tool_uses.len(), 1);
        assert_eq!(tool_uses[0].0, "call_123");
        assert_eq!(tool_uses[0].1, "get_weather");
        assert_eq!(
            tool_uses[0].2,
            &serde_json::json!({"location": "San Francisco"})
        );
    }
}
