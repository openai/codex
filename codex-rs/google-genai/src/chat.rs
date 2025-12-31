//! Chat session management for Google Generative AI.
//!
//! This module provides a stateful chat session that maintains conversation history.

use crate::client::Client;
use crate::error::Result;
use crate::stream::ContentStream;
use crate::types::Content;
use crate::types::FunctionResponse;
use crate::types::GenerateContentConfig;
use crate::types::GenerateContentResponse;
use crate::types::Part;
use crate::types::Tool;
use futures::StreamExt;

/// A chat session that maintains conversation history.
///
/// Maintains two histories following Python SDK pattern:
/// - `curated_history`: Only valid turns, sent to the API
/// - `comprehensive_history`: All turns including invalid responses (for debugging)
#[derive(Debug)]
pub struct Chat {
    /// The client used for API calls.
    client: Client,

    /// The model to use for generation.
    model: String,

    /// Default configuration for requests.
    config: Option<GenerateContentConfig>,

    /// Curated conversation history (valid turns only, sent to API).
    curated_history: Vec<Content>,

    /// Comprehensive history (all turns including invalid responses).
    comprehensive_history: Vec<Content>,
}

/// Check if a response is valid for inclusion in curated history.
///
/// A response is valid if:
/// - Has at least one candidate
/// - Candidate has content
/// - Content has non-empty parts
/// - No part is an empty Part() object
fn is_valid_response(response: &GenerateContentResponse) -> bool {
    response
        .candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|c| c.parts.as_ref())
        .map(|parts| {
            !parts.is_empty()
                && parts.iter().all(|p| {
                    // Part is valid if at least one field is set
                    p.text.is_some()
                        || p.function_call.is_some()
                        || p.function_response.is_some()
                        || p.inline_data.is_some()
                        || p.file_data.is_some()
                        || p.thought == Some(true)
                })
        })
        .unwrap_or(false)
}

impl Chat {
    /// Create a new chat session.
    pub fn new(client: Client, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
            config: None,
            curated_history: Vec::new(),
            comprehensive_history: Vec::new(),
        }
    }

    /// Create a new chat session with configuration.
    pub fn with_config(
        client: Client,
        model: impl Into<String>,
        config: GenerateContentConfig,
    ) -> Self {
        Self {
            client,
            model: model.into(),
            config: Some(config),
            curated_history: Vec::new(),
            comprehensive_history: Vec::new(),
        }
    }

    /// Create a new chat session with initial history.
    pub fn with_history(client: Client, model: impl Into<String>, history: Vec<Content>) -> Self {
        Self {
            client,
            model: model.into(),
            config: None,
            curated_history: history.clone(),
            comprehensive_history: history,
        }
    }

    /// Get the curated conversation history (valid turns only).
    /// This is the history sent to the API in subsequent requests.
    pub fn history(&self) -> &[Content] {
        &self.curated_history
    }

    /// Get conversation history with option for curated or comprehensive.
    ///
    /// - `curated=true`: Only valid turns (default, sent to API)
    /// - `curated=false`: All turns including invalid responses
    pub fn get_history(&self, curated: bool) -> &[Content] {
        if curated {
            &self.curated_history
        } else {
            &self.comprehensive_history
        }
    }

    /// Clear the conversation history (both curated and comprehensive).
    pub fn clear_history(&mut self) {
        self.curated_history.clear();
        self.comprehensive_history.clear();
    }

    /// Add a message to both histories without sending.
    pub fn add_to_history(&mut self, content: Content) {
        self.curated_history.push(content.clone());
        self.comprehensive_history.push(content);
    }

    /// Send a text message and get a response.
    pub async fn send_message(&mut self, message: &str) -> Result<GenerateContentResponse> {
        self.send_message_with_parts(vec![Part::text(message)], None)
            .await
    }

    /// Send a message with custom configuration.
    pub async fn send_message_with_config(
        &mut self,
        message: &str,
        config: GenerateContentConfig,
    ) -> Result<GenerateContentResponse> {
        self.send_message_with_parts(vec![Part::text(message)], Some(config))
            .await
    }

    /// Send a message with multiple parts (e.g., text + image).
    pub async fn send_message_with_parts(
        &mut self,
        parts: Vec<Part>,
        config: Option<GenerateContentConfig>,
    ) -> Result<GenerateContentResponse> {
        // Create user content
        let user_content = Content::with_parts("user", parts);

        // Build full contents with curated history (only valid turns sent to API)
        let mut contents = self.curated_history.clone();
        contents.push(user_content.clone());

        // Use provided config or default
        let effective_config = config.or_else(|| self.config.clone());

        // Send request
        let response = self
            .client
            .generate_content(&self.model, contents, effective_config)
            .await?;

        // Extract model content from response
        let model_content = response
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .cloned();

        // Check if response is valid
        let is_valid = is_valid_response(&response);

        // Always add to comprehensive history
        self.comprehensive_history.push(user_content.clone());
        if let Some(ref content) = model_content {
            self.comprehensive_history.push(content.clone());
        } else {
            // Add empty model content to comprehensive history for tracking
            self.comprehensive_history.push(Content {
                role: Some("model".to_string()),
                parts: Some(Vec::new()),
            });
        }

        // Only add to curated history if response is valid
        if is_valid {
            self.curated_history.push(user_content);
            if let Some(content) = model_content {
                self.curated_history.push(content);
            }
        }

        Ok(response)
    }

    /// Send a message with an image (bytes).
    pub async fn send_message_with_image(
        &mut self,
        message: &str,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<GenerateContentResponse> {
        self.send_message_with_parts(
            vec![Part::text(message), Part::from_bytes(image_data, mime_type)],
            None,
        )
        .await
    }

    /// Send a message with an image URI.
    pub async fn send_message_with_image_uri(
        &mut self,
        message: &str,
        file_uri: &str,
        mime_type: &str,
    ) -> Result<GenerateContentResponse> {
        self.send_message_with_parts(
            vec![Part::text(message), Part::from_uri(file_uri, mime_type)],
            None,
        )
        .await
    }

    /// Send a function response and continue the conversation.
    pub async fn send_function_response(
        &mut self,
        name: &str,
        response: serde_json::Value,
    ) -> Result<GenerateContentResponse> {
        self.send_message_with_parts(vec![Part::function_response(name, response)], None)
            .await
    }

    /// Send a function response with call ID for proper pairing.
    ///
    /// When the model returns multiple function calls, each has a unique ID.
    /// Use this method to pair your response with the correct call.
    pub async fn send_function_response_with_id(
        &mut self,
        id: &str,
        name: &str,
        response: serde_json::Value,
    ) -> Result<GenerateContentResponse> {
        let fr = FunctionResponse::new(name, response).with_id(id);
        self.send_message_with_parts(
            vec![Part {
                function_response: Some(fr),
                ..Default::default()
            }],
            None,
        )
        .await
    }

    /// Send multiple function responses in a single turn.
    ///
    /// Use this when the model returns multiple function calls in one response.
    /// Each response is a tuple of (name, result).
    pub async fn send_function_responses(
        &mut self,
        responses: Vec<(&str, serde_json::Value)>,
    ) -> Result<GenerateContentResponse> {
        let parts: Vec<Part> = responses
            .into_iter()
            .map(|(name, response)| Part::function_response(name, response))
            .collect();
        self.send_message_with_parts(parts, None).await
    }

    /// Send multiple function responses with IDs in a single turn.
    ///
    /// Use this when the model returns multiple function calls with IDs.
    /// Each response is a tuple of (optional_id, name, result).
    pub async fn send_function_responses_with_ids(
        &mut self,
        responses: Vec<(Option<&str>, &str, serde_json::Value)>,
    ) -> Result<GenerateContentResponse> {
        let parts: Vec<Part> = responses
            .into_iter()
            .map(|(id, name, response)| {
                let mut fr = FunctionResponse::new(name, response);
                if let Some(id) = id {
                    fr = fr.with_id(id);
                }
                Part {
                    function_response: Some(fr),
                    ..Default::default()
                }
            })
            .collect();
        self.send_message_with_parts(parts, None).await
    }

    // ========== Streaming Methods ==========

    /// Send a text message with streaming response.
    ///
    /// **Note**: Streaming does NOT automatically update history.
    /// Use `add_to_history()` manually after consuming the stream if you want
    /// to continue the conversation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use google_genai::Chat;
    /// use futures::StreamExt;
    ///
    /// # async fn example(chat: &mut Chat) -> anyhow::Result<()> {
    /// let mut stream = chat.send_message_stream("Tell me a story").await?;
    /// let mut full_text = String::new();
    ///
    /// while let Some(chunk) = stream.next().await {
    ///     if let Ok(response) = chunk {
    ///         if let Some(text) = response.text() {
    ///             print!("{}", text);
    ///             full_text.push_str(&text);
    ///         }
    ///     }
    /// }
    ///
    /// // Manually update history after streaming
    /// chat.add_to_history(google_genai::Content::user("Tell me a story"));
    /// chat.add_to_history(google_genai::Content::model(&full_text));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_message_stream(&mut self, message: &str) -> Result<ContentStream> {
        self.send_message_stream_with_parts(vec![Part::text(message)], None)
            .await
    }

    /// Send a message with custom configuration and streaming response.
    pub async fn send_message_stream_with_config(
        &mut self,
        message: &str,
        config: GenerateContentConfig,
    ) -> Result<ContentStream> {
        self.send_message_stream_with_parts(vec![Part::text(message)], Some(config))
            .await
    }

    /// Send a message with multiple parts and streaming response.
    ///
    /// **Note**: Streaming does NOT automatically update history.
    pub async fn send_message_stream_with_parts(
        &mut self,
        parts: Vec<Part>,
        config: Option<GenerateContentConfig>,
    ) -> Result<ContentStream> {
        // Create user content
        let user_content = Content::with_parts("user", parts);

        // Build full contents with curated history
        let mut contents = self.curated_history.clone();
        contents.push(user_content);

        // Use provided config or default
        let effective_config = config.or_else(|| self.config.clone());

        // Send streaming request
        self.client
            .generate_content_stream(&self.model, contents, effective_config)
            .await
    }

    // ========== Auto-History Streaming Methods (Python SDK Aligned) ==========

    /// Send a text message with streaming and automatic history update.
    ///
    /// This method matches Python SDK behavior:
    /// - Yields each chunk to the callback as it arrives
    /// - Accumulates all text internally
    /// - Automatically updates history after streaming completes
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use google_genai::Chat;
    ///
    /// # async fn example(chat: &mut Chat) -> anyhow::Result<()> {
    /// // Stream with auto-history (Python SDK style)
    /// let full_text = chat.send_message_stream_auto(
    ///     "Tell me a story",
    ///     |response| {
    ///         // Called for each chunk
    ///         if let Some(text) = response.text() {
    ///             print!("{}", text);
    ///         }
    ///     }
    /// ).await?;
    ///
    /// println!("\n\nFull response: {} chars", full_text.len());
    /// // History is automatically updated - ready for next message!
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_message_stream_auto<F>(
        &mut self,
        message: &str,
        on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&GenerateContentResponse),
    {
        self.send_message_stream_auto_with_parts(vec![Part::text(message)], None, on_chunk)
            .await
    }

    /// Send a message with streaming, callback, and automatic history update.
    ///
    /// Full-featured version with custom parts and config.
    pub async fn send_message_stream_auto_with_parts<F>(
        &mut self,
        parts: Vec<Part>,
        config: Option<GenerateContentConfig>,
        mut on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&GenerateContentResponse),
    {
        // Store user content for history
        let user_content = Content::with_parts("user", parts.clone());

        // Build full contents with curated history
        let mut contents = self.curated_history.clone();
        contents.push(user_content.clone());

        // Use provided config or default
        let effective_config = config.or_else(|| self.config.clone());

        // Get stream
        let mut stream = self
            .client
            .generate_content_stream(&self.model, contents, effective_config)
            .await?;

        // Accumulate response and call callback for each chunk
        let mut accumulated_text = String::new();
        let mut accumulated_parts: Vec<Part> = Vec::new();
        let mut is_valid = true;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(response) => {
                    // Validate chunk (like Python SDK)
                    if !is_valid_response(&response) {
                        is_valid = false;
                    }

                    // Extract and accumulate text
                    if let Some(text) = response.text() {
                        accumulated_text.push_str(&text);
                    }

                    // Accumulate all parts from response
                    if let Some(parts) = response.parts() {
                        accumulated_parts.extend(parts.into_iter().cloned());
                    }

                    // Call user's callback
                    on_chunk(&response);
                }
                Err(e) => {
                    // On error, don't update history
                    return Err(e);
                }
            }
        }

        // Build model content from accumulated parts (or text if no parts)
        let model_content = if accumulated_parts.is_empty() {
            Content::model(&accumulated_text)
        } else {
            Content::with_parts("model", accumulated_parts)
        };

        // Update histories (matching Python SDK behavior)
        // Always add to comprehensive history
        self.comprehensive_history.push(user_content.clone());
        self.comprehensive_history.push(model_content.clone());

        // Only add to curated history if valid
        if is_valid && !accumulated_text.is_empty() {
            self.curated_history.push(user_content);
            self.curated_history.push(model_content);
        }

        Ok(accumulated_text)
    }
}

/// Builder for creating chat sessions.
#[derive(Debug)]
pub struct ChatBuilder {
    client: Client,
    model: String,
    config: Option<GenerateContentConfig>,
    history: Vec<Content>,
    system_instruction: Option<Content>,
    tools: Option<Vec<Tool>>,
}

impl ChatBuilder {
    /// Create a new chat builder.
    pub fn new(client: Client, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
            config: None,
            history: Vec::new(),
            system_instruction: None,
            tools: None,
        }
    }

    /// Set the system instruction.
    pub fn system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(Content {
            parts: Some(vec![Part::text(instruction)]),
            role: Some("user".to_string()),
        });
        self
    }

    /// Set the initial history.
    pub fn history(mut self, history: Vec<Content>) -> Self {
        self.history = history;
        self
    }

    /// Set the tools available for function calling.
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the temperature.
    pub fn temperature(mut self, temperature: f32) -> Self {
        let config = self
            .config
            .get_or_insert_with(GenerateContentConfig::default);
        config.temperature = Some(temperature);
        self
    }

    /// Set the max output tokens.
    pub fn max_output_tokens(mut self, max_tokens: i32) -> Self {
        let config = self
            .config
            .get_or_insert_with(GenerateContentConfig::default);
        config.max_output_tokens = Some(max_tokens);
        self
    }

    /// Build the chat session.
    pub fn build(mut self) -> Chat {
        // Apply system instruction and tools to config
        if self.system_instruction.is_some() || self.tools.is_some() {
            let config = self
                .config
                .get_or_insert_with(GenerateContentConfig::default);
            if let Some(system) = self.system_instruction {
                config.system_instruction = Some(system);
            }
            if let Some(tools) = self.tools {
                config.tools = Some(tools);
            }
        }

        Chat {
            client: self.client,
            model: self.model,
            config: self.config,
            curated_history: self.history.clone(),
            comprehensive_history: self.history,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientConfig;
    use crate::types::Candidate;
    use crate::types::FunctionCall;

    fn create_test_client() -> Client {
        // Use a fake API key for testing - requests will fail but structure tests work
        Client::new(ClientConfig::with_api_key("test-api-key").base_url("https://test.example.com"))
            .expect("Failed to create test client")
    }

    #[test]
    fn test_chat_history_management() {
        let client = create_test_client();
        let mut chat = Chat::new(client, "gemini-2.0-flash");

        assert!(chat.history().is_empty());
        assert!(chat.get_history(true).is_empty());
        assert!(chat.get_history(false).is_empty());

        chat.add_to_history(Content::user("Hello"));
        assert_eq!(chat.history().len(), 1);
        assert_eq!(chat.get_history(true).len(), 1);
        assert_eq!(chat.get_history(false).len(), 1);

        chat.add_to_history(Content::model("Hi there!"));
        assert_eq!(chat.history().len(), 2);

        chat.clear_history();
        assert!(chat.history().is_empty());
        assert!(chat.get_history(false).is_empty());
    }

    #[test]
    fn test_chat_builder() {
        let client = create_test_client();

        let chat = ChatBuilder::new(client, "gemini-2.0-flash")
            .system_instruction("You are a helpful assistant")
            .temperature(0.7)
            .max_output_tokens(1024)
            .build();

        assert!(chat.config.is_some());
        let config = chat.config.as_ref().unwrap();
        assert_eq!(config.max_output_tokens, Some(1024));
        assert!(config.system_instruction.is_some());
        // Check temperature with approximate comparison (f32 precision)
        assert!((config.temperature.unwrap() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_is_valid_response_with_text() {
        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part::text("Hello!")]),
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(is_valid_response(&response));
    }

    #[test]
    fn test_is_valid_response_with_function_call() {
        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part {
                        function_call: Some(FunctionCall::new(
                            "get_weather",
                            serde_json::json!({"city": "Tokyo"}),
                        )),
                        ..Default::default()
                    }]),
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(is_valid_response(&response));
    }

    #[test]
    fn test_is_valid_response_empty_parts() {
        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(Vec::new()), // Empty parts = invalid
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(!is_valid_response(&response));
    }

    #[test]
    fn test_is_valid_response_no_candidates() {
        let response = GenerateContentResponse {
            candidates: None,
            ..Default::default()
        };
        assert!(!is_valid_response(&response));
    }

    #[test]
    fn test_is_valid_response_empty_part() {
        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part::default()]), // Empty Part = invalid
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(!is_valid_response(&response));
    }

    #[test]
    fn test_chat_with_initial_history() {
        let client = create_test_client();
        let initial_history = vec![Content::user("Hello"), Content::model("Hi there!")];

        let chat = Chat::with_history(client, "gemini-2.0-flash", initial_history);

        assert_eq!(chat.history().len(), 2);
        assert_eq!(chat.get_history(true).len(), 2);
        assert_eq!(chat.get_history(false).len(), 2);
    }

    // ========== Python SDK Alignment Tests ==========

    #[test]
    fn test_history_alternates_user_model() {
        // Test that history maintains user -> model alternation
        let client = create_test_client();
        let mut chat = Chat::new(client, "gemini-2.0-flash");

        // Manually add history to test structure
        chat.add_to_history(Content::user("Question 1"));
        chat.add_to_history(Content::model("Answer 1"));
        chat.add_to_history(Content::user("Question 2"));
        chat.add_to_history(Content::model("Answer 2"));

        let history = chat.history();
        assert_eq!(history.len(), 4);

        // Verify alternating roles
        assert_eq!(history[0].role, Some("user".to_string()));
        assert_eq!(history[1].role, Some("model".to_string()));
        assert_eq!(history[2].role, Some("user".to_string()));
        assert_eq!(history[3].role, Some("model".to_string()));
    }

    #[test]
    fn test_function_call_in_history() {
        // Test that function calls can be properly added to history
        let client = create_test_client();
        let mut chat = Chat::new(client, "gemini-2.0-flash");

        // User message
        chat.add_to_history(Content::user("What's the weather?"));

        // Model response with function call
        let model_response = Content {
            role: Some("model".to_string()),
            parts: Some(vec![Part {
                function_call: Some(FunctionCall {
                    id: Some("call_1".to_string()),
                    name: Some("get_weather".to_string()),
                    args: Some(serde_json::json!({"city": "Tokyo"})),
                    partial_args: None,
                    will_continue: None,
                }),
                ..Default::default()
            }]),
        };
        chat.add_to_history(model_response);

        // Function response (as user)
        let fn_response = Content {
            role: Some("user".to_string()),
            parts: Some(vec![Part {
                function_response: Some(FunctionResponse {
                    id: Some("call_1".to_string()),
                    name: Some("get_weather".to_string()),
                    response: Some(serde_json::json!({"temp": 20, "condition": "sunny"})),
                    will_continue: None,
                    scheduling: None,
                    parts: None,
                }),
                ..Default::default()
            }]),
        };
        chat.add_to_history(fn_response);

        // Model's final response
        chat.add_to_history(Content::model("It's 20°C and sunny in Tokyo."));

        let history = chat.history();
        assert_eq!(history.len(), 4);

        // Verify function call is in history
        let model_content = &history[1];
        assert!(
            model_content.parts.as_ref().unwrap()[0]
                .function_call
                .is_some()
        );

        // Verify function response is in history
        let fn_resp_content = &history[2];
        assert!(
            fn_resp_content.parts.as_ref().unwrap()[0]
                .function_response
                .is_some()
        );
    }

    #[test]
    fn test_dual_history_consistency() {
        // Test that curated and comprehensive history maintain consistency
        let client = create_test_client();
        let chat = Chat::new(client, "gemini-2.0-flash");

        // Initially both should be empty
        assert!(chat.get_history(true).is_empty()); // curated
        assert!(chat.get_history(false).is_empty()); // comprehensive

        // After adding valid content, both should match
        let mut chat = Chat::new(create_test_client(), "gemini-2.0-flash");
        chat.add_to_history(Content::user("Hello"));
        chat.add_to_history(Content::model("Hi!"));

        let curated = chat.get_history(true);
        let comprehensive = chat.get_history(false);

        // For valid responses, both histories should be equal
        assert_eq!(curated.len(), comprehensive.len());
        assert_eq!(curated.len(), 2);
    }

    #[test]
    fn test_is_valid_response_with_thought_parts() {
        // Test that thought parts are considered valid
        let response = GenerateContentResponse {
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
        };
        assert!(is_valid_response(&response));
    }

    #[test]
    fn test_is_valid_response_thought_only() {
        // Test that response with only thought parts is valid
        let response = GenerateContentResponse {
            candidates: Some(vec![Candidate {
                content: Some(Content {
                    role: Some("model".to_string()),
                    parts: Some(vec![Part {
                        text: Some("Reasoning...".to_string()),
                        thought: Some(true),
                        thought_signature: Some(b"sig123".to_vec()),
                        ..Default::default()
                    }]),
                }),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(is_valid_response(&response));
    }
}
