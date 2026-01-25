//! Builder extensions for non-streaming Responses API requests.
//!
//! This module adds methods to `ResponsesRequestBuilder` for:
//! - Setting `previous_response_id` for conversation continuity
//! - Setting `stream` mode explicitly
//! - Building non-streaming requests with input filtering
//! - Filtering input items for cross-adapter encrypted_content compatibility

use crate::common_ext::ENCRYPTED_CONTENT_PREFIX;
use crate::common_ext::filter_incremental_input;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::ResponsesRequest;
use crate::requests::ResponsesRequestBuilder;
use codex_protocol::models::ResponseItem;

impl<'a> ResponsesRequestBuilder<'a> {
    /// Set the previous response ID for conversation continuity.
    ///
    /// When set, the server uses stored history up to this response,
    /// and the client sends only new items after the last LLM response.
    pub fn previous_response_id(mut self, id: Option<String>) -> Self {
        self.previous_response_id = id;
        self
    }

    /// Set streaming mode explicitly.
    ///
    /// By default, streaming is enabled (`true`). Set to `false` for
    /// non-streaming requests.
    pub fn stream(mut self, enabled: bool) -> Self {
        self.stream = enabled;
        self
    }

    /// Build a non-streaming request with tweakcc input filtering.
    ///
    /// This method:
    /// 1. Sets `stream: false`
    /// 2. Applies tweakcc input filtering if `previous_response_id` is set
    /// 3. Builds the request
    ///
    /// # Incremental Filtering Logic
    ///
    /// When `previous_response_id` is present:
    /// - Finds the last LLM-generated item in the input
    /// - Sends only items after that point (user inputs since last response)
    /// - Returns error if no user input exists after last LLM response
    ///
    /// When `previous_response_id` is `None`:
    /// - Sends all input items (full history)
    pub fn build_nonstream(mut self, provider: &Provider) -> Result<ResponsesRequest, ApiError> {
        self.stream = false;

        // Apply tweakcc filtering if previous_response_id is set
        if self.previous_response_id.is_some() {
            if let Some(input) = self.input {
                match filter_incremental_input(input) {
                    None => {
                        // First turn, no LLM items - use full input
                        tracing::debug!(
                            input_len = input.len(),
                            "First turn (no LLM items) - using full input"
                        );
                    }
                    Some(slice) if slice.is_empty() => {
                        // LLM item is last - error state
                        return Err(ApiError::Stream(
                            "No user input after last LLM response".into(),
                        ));
                    }
                    Some(slice) => {
                        // Normal tweakcc mode - use filtered slice
                        tracing::debug!(
                            original_len = input.len(),
                            filtered_len = slice.len(),
                            "Using tweakcc input mode"
                        );
                        self.input = Some(slice);
                    }
                }
            }
        }

        self.build(provider)
    }
}

/// Filter input items for OpenAI Responses API requests.
///
/// Handles cross-adapter encrypted_content compatibility when sending to native OpenAI:
///
/// - **Adapter format** (`"codex-ec:*"` prefix): Strip `encrypted_content` field.
///   OpenAI cannot verify adapter-generated encrypted content, so we remove it.
///   OpenAI will regenerate reasoning content if needed.
///
/// - **Native OpenAI format** (no prefix): Preserve `encrypted_content` as-is.
///   OpenAI can verify its own encrypted content format.
///
/// - **No encrypted_content**: Pass through unchanged.
///
/// # Background
///
/// Adapters (genai, anthropic, volcengine_ark, zai) store their full response body
/// in `encrypted_content` with `"codex-ec:"` prefix for cross-adapter switching.
/// When sending back to native OpenAI Responses API, this adapter format must be
/// stripped because OpenAI's verification would fail on non-native formats.
pub fn filter_input_for_openai(input: &[ResponseItem]) -> Vec<ResponseItem> {
    input
        .iter()
        .map(|item| match item {
            ResponseItem::Reasoning {
                encrypted_content: Some(ec),
                ..
            } if ec.starts_with(ENCRYPTED_CONTENT_PREFIX) => {
                // Adapter format detected - clone item and strip encrypted_content
                // OpenAI cannot verify adapter's format, so we remove it
                let ResponseItem::Reasoning {
                    id,
                    summary,
                    content,
                    ..
                } = item
                else {
                    unreachable!()
                };
                ResponseItem::Reasoning {
                    id: id.clone(),
                    summary: summary.clone(),
                    content: content.clone(),
                    encrypted_content: None,
                }
            }
            // Native OpenAI encrypted_content or no encrypted_content - keep as-is
            _ => item.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RetryConfig;
    use crate::provider::WireApi;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseItem;
    use http::HeaderMap;
    use std::time::Duration;

    fn test_provider() -> Provider {
        Provider {
            name: "test".to_string(),
            base_url: "https://api.example.com".to_string(),
            query_params: None,
            wire: WireApi::Responses,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(50),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(5),
            adapter: None,
            model_parameters: None,
            interceptors: Vec::new(),
            request_timeout: None,
            streaming: true,
        }
    }

    #[test]
    fn test_build_nonstream_sets_stream_false() {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hello".to_string(),
            }],
            end_turn: None,
        }];

        let request = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .build_nonstream(&test_provider())
            .unwrap();

        let stream = request.body.get("stream").and_then(|v| v.as_bool());
        assert_eq!(stream, Some(false));
    }

    #[test]
    fn test_build_nonstream_with_previous_response_id() {
        let input = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![],
                end_turn: None,
            },
            ResponseItem::Message {
                id: Some("msg_1".to_string()),
                role: "assistant".to_string(),
                content: vec![],
                end_turn: None,
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_1".to_string(),
                output: FunctionCallOutputPayload {
                    content: "output".to_string(),
                    content_items: None,
                    success: Some(true),
                },
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Continue".to_string(),
                }],
                end_turn: None,
            },
        ];

        let request = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .previous_response_id(Some("resp-prev".to_string()))
            .build_nonstream(&test_provider())
            .unwrap();

        // Verify previous_response_id is in the body
        let prev_id = request
            .body
            .get("previous_response_id")
            .and_then(|v| v.as_str());
        assert_eq!(prev_id, Some("resp-prev"));

        // Verify input was filtered (only items after last LLM response)
        let input_array = request.body.get("input").and_then(|v| v.as_array());
        assert!(input_array.is_some());
        // Should have 2 items: FunctionCallOutput + user message
        assert_eq!(input_array.unwrap().len(), 2);
    }

    #[test]
    fn test_build_nonstream_no_previous_response_id_full_input() {
        let input = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![],
                end_turn: None,
            },
            ResponseItem::Message {
                id: Some("msg_1".to_string()),
                role: "assistant".to_string(),
                content: vec![],
                end_turn: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![],
                end_turn: None,
            },
        ];

        let request = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .build_nonstream(&test_provider())
            .unwrap();

        // No previous_response_id - should have full input
        let input_array = request.body.get("input").and_then(|v| v.as_array());
        assert!(input_array.is_some());
        assert_eq!(input_array.unwrap().len(), 3);
    }

    #[test]
    fn test_build_nonstream_error_llm_is_last() {
        let input = vec![ResponseItem::Message {
            id: Some("msg_1".to_string()),
            role: "assistant".to_string(),
            content: vec![],
            end_turn: None,
        }];

        let result = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .previous_response_id(Some("resp-prev".to_string()))
            .build_nonstream(&test_provider());

        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::Stream(msg) => {
                assert!(msg.contains("No user input after last LLM response"));
            }
            other => panic!("Expected Stream error, got {:?}", other),
        }
    }

    #[test]
    fn test_stream_method() {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![],
            end_turn: None,
        }];

        // Test setting stream to false explicitly
        let request = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .stream(false)
            .build(&test_provider())
            .unwrap();

        let stream = request.body.get("stream").and_then(|v| v.as_bool());
        assert_eq!(stream, Some(false));

        // Test setting stream to true explicitly
        let request = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .stream(true)
            .build(&test_provider())
            .unwrap();

        let stream = request.body.get("stream").and_then(|v| v.as_bool());
        assert_eq!(stream, Some(true));
    }

    #[test]
    fn test_previous_response_id_method() {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![],
            end_turn: None,
        }];

        let request = ResponsesRequestBuilder::new("gpt-4", "You are helpful", &input)
            .previous_response_id(Some("resp-123".to_string()))
            .build(&test_provider())
            .unwrap();

        let prev_id = request
            .body
            .get("previous_response_id")
            .and_then(|v| v.as_str());
        assert_eq!(prev_id, Some("resp-123"));
    }

    // Tests for filter_input_for_openai

    #[test]
    fn test_filter_input_for_openai_strips_adapter_encrypted_content() {
        use codex_protocol::models::ReasoningItemReasoningSummary;

        let input = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hello".to_string(),
                }],
                end_turn: None,
            },
            ResponseItem::Reasoning {
                id: "reasoning-1".to_string(),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: "Thinking...".to_string(),
                }],
                content: None,
                // Adapter format - should be stripped
                encrypted_content: Some("codex-ec:{\"provider_sdk\":\"genai\"}".to_string()),
            },
            ResponseItem::Message {
                id: Some("msg-1".to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Hi!".to_string(),
                }],
                end_turn: None,
            },
        ];

        let filtered = filter_input_for_openai(&input);

        assert_eq!(filtered.len(), 3);

        // First item unchanged
        assert!(matches!(&filtered[0], ResponseItem::Message { role, .. } if role == "user"));

        // Reasoning item should have encrypted_content cleared
        match &filtered[1] {
            ResponseItem::Reasoning {
                id,
                summary,
                encrypted_content,
                ..
            } => {
                assert_eq!(id, "reasoning-1");
                assert_eq!(summary.len(), 1);
                assert_eq!(*encrypted_content, None); // Stripped!
            }
            _ => panic!("Expected Reasoning item"),
        }

        // Last item unchanged
        assert!(matches!(&filtered[2], ResponseItem::Message { role, .. } if role == "assistant"));
    }

    #[test]
    fn test_filter_input_for_openai_preserves_native_encrypted_content() {
        use codex_protocol::models::ReasoningItemReasoningSummary;

        let input = vec![ResponseItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Thinking...".to_string(),
            }],
            content: None,
            // Native OpenAI format (no codex-ec: prefix) - should be preserved
            encrypted_content: Some("native-openai-encrypted-content".to_string()),
        }];

        let filtered = filter_input_for_openai(&input);

        assert_eq!(filtered.len(), 1);
        match &filtered[0] {
            ResponseItem::Reasoning {
                encrypted_content, ..
            } => {
                // Native format preserved
                assert_eq!(
                    *encrypted_content,
                    Some("native-openai-encrypted-content".to_string())
                );
            }
            _ => panic!("Expected Reasoning item"),
        }
    }

    #[test]
    fn test_filter_input_for_openai_handles_no_encrypted_content() {
        use codex_protocol::models::ReasoningItemReasoningSummary;

        let input = vec![ResponseItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Thinking...".to_string(),
            }],
            content: None,
            encrypted_content: None, // No encrypted_content
        }];

        let filtered = filter_input_for_openai(&input);

        assert_eq!(filtered.len(), 1);
        match &filtered[0] {
            ResponseItem::Reasoning {
                encrypted_content, ..
            } => {
                assert_eq!(*encrypted_content, None);
            }
            _ => panic!("Expected Reasoning item"),
        }
    }
}
