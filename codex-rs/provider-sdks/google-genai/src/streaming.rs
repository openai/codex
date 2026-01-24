//! High-level streaming wrapper for Google Generative AI API.
//!
//! This module provides `GenerateContentStream`, a convenience wrapper around
//! the raw SSE stream that offers:
//! - Automatic response accumulation
//! - Text-only stream filtering
//! - Final response collection
//! - Stream lifecycle management
//!
//! ## Example
//!
//! ```rust,ignore
//! use google_genai::{Client, GenerateContentStream};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = Client::from_env()?;
//!
//! // Get streaming response with high-level wrapper
//! let mut stream = client.stream("gemini-2.0-flash", vec![], None).await?;
//!
//! // Option 1: Iterate over chunks
//! while let Some(result) = stream.next().await {
//!     let response = result?;
//!     if let Some(text) = response.text() {
//!         print!("{}", text);
//!     }
//! }
//!
//! // Option 2: Get final accumulated text
//! let stream = client.stream("gemini-2.0-flash", vec![], None).await?;
//! let final_text = stream.get_final_text().await?;
//!
//! // Option 3: Use text-only stream
//! let stream = client.stream("gemini-2.0-flash", vec![], None).await?;
//! use futures::StreamExt;
//! let mut text_stream = std::pin::pin!(stream.text_stream());
//! while let Some(result) = text_stream.next().await {
//!     print!("{}", result?);
//! }
//! # Ok(())
//! # }
//! ```

use crate::error::GenAiError;
use crate::error::Result;
use crate::stream::ContentStream;
use crate::types::Candidate;
use crate::types::CitationMetadata;
use crate::types::Content;
use crate::types::FinishReason;
use crate::types::GenerateContentResponse;
use crate::types::GroundingMetadata;
use crate::types::Part;
use crate::types::SafetyRating;
use crate::types::SdkHttpResponse;
use crate::types::UsageMetadata;
use futures::StreamExt;
use futures::stream::Stream;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

// =============================================================================
// GenerateContentStream
// =============================================================================

/// A high-level wrapper for streaming content generation responses.
///
/// This provides:
/// - Iterator interface for async event consumption
/// - Automatic response accumulation
/// - Convenience methods for common patterns (text_stream, get_final_response)
/// - Stream lifecycle management (close)
///
/// Aligns with Python SDK's `AsyncStream[GenerateContentResponse]` pattern.
pub struct GenerateContentStream {
    /// Inner content stream.
    inner: ContentStream,
    /// Accumulated state for building final response.
    state: ResponseAccumulator,
    /// Whether stream has been closed/exhausted.
    closed: bool,
}

impl GenerateContentStream {
    /// Create a new stream wrapper from a raw content stream.
    pub fn new(inner: ContentStream) -> Self {
        Self {
            inner,
            state: ResponseAccumulator::default(),
            closed: false,
        }
    }

    /// Get the next response chunk from the stream.
    ///
    /// Returns `None` when stream is exhausted or closed.
    pub async fn next(&mut self) -> Option<Result<GenerateContentResponse>> {
        if self.closed {
            return None;
        }

        match self.inner.next().await {
            Some(Ok(response)) => {
                // Update accumulator state
                self.state.update(&response);
                Some(Ok(response))
            }
            Some(Err(e)) => Some(Err(e)),
            None => {
                self.closed = true;
                None
            }
        }
    }

    /// Close the stream, preventing further iteration.
    ///
    /// This is useful for early termination without consuming remaining data.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Check if the stream is closed or exhausted.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Convert to a stream of text deltas only.
    ///
    /// This filters out non-text content and yields only text strings.
    /// Thought parts are excluded (same behavior as Python SDK's `.text` property).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use google_genai::GenerateContentStream;
    /// use futures::StreamExt;
    ///
    /// # async fn example(stream: GenerateContentStream) -> anyhow::Result<()> {
    /// let mut text_stream = std::pin::pin!(stream.text_stream());
    /// while let Some(result) = text_stream.next().await {
    ///     print!("{}", result?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn text_stream(self) -> impl Stream<Item = Result<String>> + Send {
        futures::stream::unfold(self, |mut stream| async move {
            loop {
                match stream.next().await {
                    Some(Ok(response)) => {
                        if let Some(text) = response.text() {
                            return Some((Ok(text), stream));
                        }
                        // Continue to next chunk (no text in this one)
                    }
                    Some(Err(e)) => return Some((Err(e), stream)),
                    None => return None,
                }
            }
        })
    }

    /// Consume the stream and return the accumulated final response.
    ///
    /// This combines all chunks into a single complete `GenerateContentResponse`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use google_genai::GenerateContentStream;
    ///
    /// # async fn example(stream: GenerateContentStream) -> anyhow::Result<()> {
    /// let final_response = stream.get_final_response().await?;
    /// println!("Total tokens: {:?}", final_response.usage_metadata);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_final_response(mut self) -> Result<GenerateContentResponse> {
        // Consume all remaining chunks
        while let Some(result) = self.next().await {
            result?; // Propagate errors
        }

        self.state.build_response()
    }

    /// Consume the stream and return the final accumulated text.
    ///
    /// This is a convenience method combining all text parts.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use google_genai::GenerateContentStream;
    ///
    /// # async fn example(stream: GenerateContentStream) -> anyhow::Result<()> {
    /// let final_text = stream.get_final_text().await?;
    /// println!("Response: {}", final_text);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_final_text(self) -> Result<String> {
        let response = self.get_final_response().await?;
        Ok(response.text().unwrap_or_default())
    }

    /// Get a snapshot of the current accumulated response without consuming the stream.
    ///
    /// This allows peeking at the current state while continuing to iterate.
    /// Returns `None` if no chunks have been received yet.
    pub fn current_snapshot(&self) -> Option<GenerateContentResponse> {
        self.state.build_snapshot()
    }

    /// Get the current accumulated text without consuming the stream.
    pub fn current_text(&self) -> String {
        self.state.accumulated_text()
    }

    /// Convert to a futures Stream for use with StreamExt combinators.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use google_genai::GenerateContentStream;
    /// use futures::StreamExt;
    ///
    /// # async fn example(stream: GenerateContentStream) -> anyhow::Result<()> {
    /// let responses: Vec<_> = stream.into_stream()
    ///     .filter_map(|r| async { r.ok() })
    ///     .collect()
    ///     .await;
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_stream(self) -> GenerateContentStreamAdapter {
        GenerateContentStreamAdapter { inner: self }
    }
}

// =============================================================================
// GenerateContentStreamAdapter
// =============================================================================

/// Adapter to implement the futures Stream trait for GenerateContentStream.
///
/// This allows using GenerateContentStream with StreamExt combinators.
pub struct GenerateContentStreamAdapter {
    inner: GenerateContentStream,
}

impl Stream for GenerateContentStreamAdapter {
    type Item = Result<GenerateContentResponse>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.inner.closed {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.inner.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(response))) => {
                self.inner.state.update(&response);
                Poll::Ready(Some(Ok(response)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                self.inner.closed = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

// =============================================================================
// ResponseAccumulator
// =============================================================================

/// Internal accumulator for building a complete response from streaming chunks.
#[derive(Default)]
struct ResponseAccumulator {
    /// Accumulated candidates.
    candidates: Vec<CandidateBuilder>,
    /// Accumulated usage metadata.
    usage_metadata: Option<UsageMetadata>,
    /// Model version from response.
    model_version: Option<String>,
    /// HTTP response metadata (from first chunk).
    sdk_http_response: Option<SdkHttpResponse>,
    /// Whether any chunks have been received.
    has_data: bool,
}

/// Builder for accumulating candidate data across chunks.
#[derive(Default)]
struct CandidateBuilder {
    /// Accumulated content parts.
    parts: Vec<Part>,
    /// Accumulated text (for convenience).
    text: String,
    /// Finish reason from final chunk.
    finish_reason: Option<FinishReason>,
    /// Safety ratings (last received).
    safety_ratings: Vec<SafetyRating>,
    /// Citation metadata (last received).
    citation_metadata: Option<CitationMetadata>,
    /// Grounding metadata (last received).
    grounding_metadata: Option<GroundingMetadata>,
    /// Candidate index.
    index: Option<i32>,
}

impl ResponseAccumulator {
    /// Update the accumulator with data from a new chunk.
    fn update(&mut self, response: &GenerateContentResponse) {
        self.has_data = true;

        // Update model version if provided
        if response.model_version.is_some() {
            self.model_version.clone_from(&response.model_version);
        }

        // Update usage metadata (take latest, as it accumulates)
        if response.usage_metadata.is_some() {
            self.usage_metadata.clone_from(&response.usage_metadata);
        }

        // Store SDK HTTP response from first chunk
        if self.sdk_http_response.is_none() && response.sdk_http_response.is_some() {
            self.sdk_http_response
                .clone_from(&response.sdk_http_response);
        }

        // Update candidates
        if let Some(candidates) = &response.candidates {
            for (i, candidate) in candidates.iter().enumerate() {
                // Ensure we have a builder for this candidate index
                while self.candidates.len() <= i {
                    self.candidates.push(CandidateBuilder::default());
                }

                self.candidates[i].update(candidate);
            }
        }
    }

    /// Build the final accumulated response.
    fn build_response(&self) -> Result<GenerateContentResponse> {
        if !self.has_data {
            return Err(GenAiError::Parse(
                "No data received from stream".to_string(),
            ));
        }

        Ok(GenerateContentResponse {
            candidates: if self.candidates.is_empty() {
                None
            } else {
                Some(self.candidates.iter().map(|b| b.build()).collect())
            },
            prompt_feedback: None, // Not available in streaming
            usage_metadata: self.usage_metadata.clone(),
            model_version: self.model_version.clone(),
            sdk_http_response: self.sdk_http_response.clone(),
            create_time: None,
            response_id: None,
        })
    }

    /// Build a snapshot without consuming.
    fn build_snapshot(&self) -> Option<GenerateContentResponse> {
        if !self.has_data {
            return None;
        }

        Some(GenerateContentResponse {
            candidates: if self.candidates.is_empty() {
                None
            } else {
                Some(self.candidates.iter().map(|b| b.build()).collect())
            },
            prompt_feedback: None,
            usage_metadata: self.usage_metadata.clone(),
            model_version: self.model_version.clone(),
            sdk_http_response: self.sdk_http_response.clone(),
            create_time: None,
            response_id: None,
        })
    }

    /// Get accumulated text from all candidates.
    fn accumulated_text(&self) -> String {
        self.candidates
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("")
    }
}

impl CandidateBuilder {
    /// Update the builder with data from a new candidate chunk.
    fn update(&mut self, candidate: &Candidate) {
        // Update index
        if candidate.index.is_some() {
            self.index = candidate.index;
        }

        // Update finish reason
        if candidate.finish_reason.is_some() {
            self.finish_reason.clone_from(&candidate.finish_reason);
        }

        // Update safety ratings
        if let Some(ratings) = &candidate.safety_ratings {
            self.safety_ratings.clone_from(ratings);
        }

        // Update citation metadata
        if candidate.citation_metadata.is_some() {
            self.citation_metadata
                .clone_from(&candidate.citation_metadata);
        }

        // Update grounding metadata
        if candidate.grounding_metadata.is_some() {
            self.grounding_metadata
                .clone_from(&candidate.grounding_metadata);
        }

        // Accumulate content parts
        if let Some(content) = &candidate.content {
            if let Some(parts) = &content.parts {
                for part in parts {
                    // Accumulate text (excluding thought parts)
                    if let Some(text) = &part.text {
                        if part.thought != Some(true) {
                            self.text.push_str(text);
                        }
                    }

                    // Store all parts
                    self.parts.push(part.clone());
                }
            }
        }
    }

    /// Build the final candidate.
    fn build(&self) -> Candidate {
        Candidate {
            content: if self.parts.is_empty() {
                None
            } else {
                Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(self.parts.clone()),
                })
            },
            finish_reason: self.finish_reason.clone(),
            safety_ratings: if self.safety_ratings.is_empty() {
                None
            } else {
                Some(self.safety_ratings.clone())
            },
            citation_metadata: self.citation_metadata.clone(),
            grounding_metadata: self.grounding_metadata.clone(),
            index: self.index,
            ..Default::default()
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::parse_sse_stream;
    use bytes::Bytes;
    use futures::stream;

    fn make_response(text: &str) -> GenerateContentResponse {
        GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part::text(text)]),
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }
    }

    fn make_content_stream(responses: Vec<GenerateContentResponse>) -> ContentStream {
        Box::pin(stream::iter(responses.into_iter().map(Ok)))
    }

    #[tokio::test]
    async fn test_stream_next() {
        let responses = vec![make_response("Hello"), make_response(" World")];
        let mut stream = GenerateContentStream::new(make_content_stream(responses));

        let chunk1 = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk1.text(), Some("Hello".to_string()));

        let chunk2 = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk2.text(), Some(" World".to_string()));

        assert!(stream.next().await.is_none());
        assert!(stream.is_closed());
    }

    #[tokio::test]
    async fn test_stream_close() {
        let responses = vec![make_response("Hello"), make_response(" World")];
        let mut stream = GenerateContentStream::new(make_content_stream(responses));

        // Read one chunk
        let _ = stream.next().await;

        // Close early
        stream.close();
        assert!(stream.is_closed());

        // No more chunks
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_get_final_text() {
        let responses = vec![make_response("Hello"), make_response(" World")];
        let stream = GenerateContentStream::new(make_content_stream(responses));

        let final_text = stream.get_final_text().await.unwrap();
        assert_eq!(final_text, "Hello World");
    }

    #[tokio::test]
    async fn test_get_final_response() {
        let responses = vec![
            GenerateContentResponse {
                candidates: Some(vec![Candidate {
                    content: Some(Content {
                        role: Some("model".to_string()),
                        parts: Some(vec![Part::text("Hello")]),
                    }),
                    ..Default::default()
                }]),
                usage_metadata: Some(UsageMetadata {
                    prompt_token_count: Some(10),
                    candidates_token_count: Some(5),
                    ..Default::default()
                }),
                ..Default::default()
            },
            GenerateContentResponse {
                candidates: Some(vec![Candidate {
                    content: Some(Content {
                        role: Some("model".to_string()),
                        parts: Some(vec![Part::text(" World")]),
                    }),
                    finish_reason: Some(FinishReason::Stop),
                    ..Default::default()
                }]),
                usage_metadata: Some(UsageMetadata {
                    prompt_token_count: Some(10),
                    candidates_token_count: Some(10),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ];
        let stream = GenerateContentStream::new(make_content_stream(responses));

        let final_response = stream.get_final_response().await.unwrap();
        assert_eq!(final_response.text(), Some("Hello World".to_string()));

        // Should have latest usage metadata
        let usage = final_response.usage_metadata.unwrap();
        assert_eq!(usage.candidates_token_count, Some(10));
    }

    #[tokio::test]
    async fn test_text_stream() {
        let responses = vec![make_response("A"), make_response("B"), make_response("C")];
        let stream = GenerateContentStream::new(make_content_stream(responses));

        let texts: Vec<String> = stream
            .text_stream()
            .filter_map(|r| async { r.ok() })
            .collect()
            .await;

        assert_eq!(texts, vec!["A", "B", "C"]);
    }

    #[tokio::test]
    async fn test_current_snapshot() {
        let responses = vec![make_response("Hello"), make_response(" World")];
        let mut stream = GenerateContentStream::new(make_content_stream(responses));

        // Before any data
        assert!(stream.current_snapshot().is_none());

        // After first chunk
        let _ = stream.next().await;
        let snapshot = stream.current_snapshot().unwrap();
        assert_eq!(snapshot.text(), Some("Hello".to_string()));

        // After second chunk
        let _ = stream.next().await;
        let snapshot = stream.current_snapshot().unwrap();
        assert_eq!(snapshot.text(), Some("Hello World".to_string()));
    }

    #[tokio::test]
    async fn test_current_text() {
        let responses = vec![make_response("Hello"), make_response(" World")];
        let mut stream = GenerateContentStream::new(make_content_stream(responses));

        assert_eq!(stream.current_text(), "");

        let _ = stream.next().await;
        assert_eq!(stream.current_text(), "Hello");

        let _ = stream.next().await;
        assert_eq!(stream.current_text(), "Hello World");
    }

    #[tokio::test]
    async fn test_into_stream() {
        let responses = vec![make_response("A"), make_response("B")];
        let stream = GenerateContentStream::new(make_content_stream(responses));

        let collected: Vec<_> = stream
            .into_stream()
            .filter_map(|r| async { r.ok() })
            .map(|r| r.text().unwrap_or_default())
            .collect()
            .await;

        assert_eq!(collected, vec!["A", "B"]);
    }

    #[tokio::test]
    async fn test_thought_parts_excluded_from_text() {
        let responses = vec![GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![
                        Part {
                            text: Some("Thinking...".to_string()),
                            thought: Some(true),
                            ..Default::default()
                        },
                        Part {
                            text: Some("Final answer".to_string()),
                            ..Default::default()
                        },
                    ]),
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }];
        let stream = GenerateContentStream::new(make_content_stream(responses));

        let final_text = stream.get_final_text().await.unwrap();
        // Thought parts should be excluded
        assert_eq!(final_text, "Final answer");
    }

    #[tokio::test]
    async fn test_empty_stream_error() {
        let stream = GenerateContentStream::new(make_content_stream(vec![]));
        let result = stream.get_final_response().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_with_sse_stream() {
        let data = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]}}]}

data: {"candidates":[{"content":{"role":"model","parts":[{"text":" World"}]}}]}

"#;
        let byte_stream = stream::iter(vec![Ok(Bytes::from(data))]);
        let content_stream = parse_sse_stream(byte_stream);
        let stream = GenerateContentStream::new(content_stream);

        let final_text = stream.get_final_text().await.unwrap();
        assert_eq!(final_text, "Hello World");
    }
}
