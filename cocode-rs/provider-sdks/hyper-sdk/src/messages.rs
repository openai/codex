//! Message types for conversations.

use crate::options::ProviderOptions;
use crate::tools::ToolResultContent;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System instructions/context.
    System,
    /// User input.
    User,
    /// Assistant response.
    Assistant,
    /// Tool/function result.
    Tool,
}

/// Source for an image in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64-encoded image data.
    Base64 {
        /// Base64-encoded data.
        data: String,
        /// MIME type (e.g., "image/png", "image/jpeg").
        media_type: String,
    },
    /// URL to an image.
    Url {
        /// Image URL.
        url: String,
    },
}

/// Image detail level for vision models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    /// Low detail mode (faster, uses fewer tokens).
    Low,
    /// High detail mode (slower, uses more tokens).
    High,
    /// Auto-select detail level.
    #[default]
    Auto,
}

/// A block of content within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content for vision models.
    Image {
        /// Image source (base64 or URL).
        source: ImageSource,
        /// Optional detail level.
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
    /// Tool/function call from assistant.
    ToolUse {
        /// Unique ID for this tool call.
        id: String,
        /// Name of the tool being called.
        name: String,
        /// Arguments as JSON.
        input: Value,
    },
    /// Result of a tool call.
    ToolResult {
        /// ID of the tool call this is responding to.
        tool_use_id: String,
        /// Result content.
        content: ToolResultContent,
        /// Whether this represents an error.
        #[serde(default)]
        is_error: bool,
        /// Whether this result is for a custom tool (vs a function tool).
        /// OpenAI requires `custom_tool_call_output` for custom tools.
        #[serde(default)]
        is_custom: bool,
    },
    /// Thinking/reasoning content (for extended thinking models).
    Thinking {
        /// The thinking content.
        content: String,
        /// Optional signature for verification.
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
}

impl ContentBlock {
    /// Create a text content block.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_sdk::ContentBlock;
    ///
    /// let block = ContentBlock::text("Hello, world!");
    /// assert_eq!(block.as_text(), Some("Hello, world!"));
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text { text: text.into() }
    }

    /// Create an image content block from base64 data.
    pub fn image_base64(data: impl Into<String>, media_type: impl Into<String>) -> Self {
        ContentBlock::Image {
            source: ImageSource::Base64 {
                data: data.into(),
                media_type: media_type.into(),
            },
            detail: None,
        }
    }

    /// Create an image content block from URL.
    pub fn image_url(url: impl Into<String>) -> Self {
        ContentBlock::Image {
            source: ImageSource::Url { url: url.into() },
            detail: None,
        }
    }

    /// Create a tool use content block.
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a tool result content block (for function tools).
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: ToolResultContent,
        is_error: bool,
    ) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content,
            is_error,
            is_custom: false,
        }
    }

    /// Create a custom tool result content block.
    ///
    /// Custom tool results use `custom_tool_call_output` when sent to OpenAI,
    /// instead of `function_call_output`.
    pub fn custom_tool_result(
        tool_use_id: impl Into<String>,
        content: ToolResultContent,
        is_error: bool,
    ) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content,
            is_error,
            is_custom: true,
        }
    }

    /// Create a thinking content block.
    pub fn thinking(content: impl Into<String>) -> Self {
        ContentBlock::Thinking {
            content: content.into(),
            signature: None,
        }
    }

    /// Extract text if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Check if this is a tool use block.
    pub fn is_tool_use(&self) -> bool {
        matches!(self, ContentBlock::ToolUse { .. })
    }

    /// Check if this is a thinking block.
    pub fn is_thinking(&self) -> bool {
        matches!(self, ContentBlock::Thinking { .. })
    }
}

/// Unified provider metadata for a message.
///
/// Tracks message origin and preserves provider-specific extension data.
/// This design consolidates all provider-related information in one place.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProviderMetadata {
    /// Provider that generated this message (e.g., "openai", "anthropic").
    /// Required for assistant messages, None for user messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_provider: Option<String>,

    /// Model that generated this message (e.g., "gpt-4o", "claude-sonnet-4").
    /// Required for assistant messages, None for user messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model: Option<String>,

    /// Provider-specific extensions keyed by provider name.
    ///
    /// Allows preserving metadata from multiple providers across conversation history.
    /// Examples:
    /// - `{"openai": {"finish_reason_detail": "length"}}`
    /// - `{"anthropic": {"cache_hit": true}}`
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extensions: HashMap<String, Value>,
}

impl ProviderMetadata {
    /// Create empty metadata (for user messages).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create metadata with source information.
    pub fn with_source(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            source_provider: Some(provider.into()),
            source_model: Some(model.into()),
            extensions: HashMap::new(),
        }
    }

    /// Check if this message was generated by the specified provider.
    pub fn is_from_provider(&self, provider: &str) -> bool {
        self.source_provider.as_deref() == Some(provider)
    }

    /// Check if this message was generated by the specified provider and model.
    pub fn is_from(&self, provider: &str, model: &str) -> bool {
        self.source_provider.as_deref() == Some(provider)
            && self.source_model.as_deref() == Some(model)
    }

    /// Get extension data for a specific provider.
    pub fn get_extension(&self, provider: &str) -> Option<&Value> {
        self.extensions.get(provider)
    }

    /// Set extension data for a specific provider.
    pub fn set_extension(&mut self, provider: impl Into<String>, data: Value) {
        self.extensions.insert(provider.into(), data);
    }

    /// Remove extension data for a specific provider.
    pub fn remove_extension(&mut self, provider: &str) -> Option<Value> {
        self.extensions.remove(provider)
    }

    /// Check if metadata is empty (no source, no extensions).
    pub fn is_empty(&self) -> bool {
        self.source_provider.is_none() && self.source_model.is_none() && self.extensions.is_empty()
    }
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender.
    pub role: Role,
    /// Content blocks.
    pub content: Vec<ContentBlock>,
    /// Provider-specific options for THIS request (runtime, not persisted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<ProviderOptions>,
    /// Unified provider metadata (source tracking + extensions).
    #[serde(default, skip_serializing_if = "ProviderMetadata::is_empty")]
    pub metadata: ProviderMetadata,
}

impl Message {
    /// Create a new message with the given role and content blocks.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_sdk::{Message, ContentBlock, Role};
    ///
    /// let msg = Message::new(Role::User, vec![ContentBlock::text("Hello")]);
    /// assert_eq!(msg.role, Role::User);
    /// assert_eq!(msg.text(), "Hello");
    /// ```
    pub fn new(role: Role, content: Vec<ContentBlock>) -> Self {
        Self {
            role,
            content,
            provider_options: None,
            metadata: ProviderMetadata::new(),
        }
    }

    /// Create a user message with text content.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_sdk::{Message, Role};
    ///
    /// let msg = Message::user("What is 2 + 2?");
    /// assert_eq!(msg.role, Role::User);
    /// assert_eq!(msg.text(), "What is 2 + 2?");
    /// ```
    pub fn user(text: impl Into<String>) -> Self {
        Self::new(Role::User, vec![ContentBlock::text(text)])
    }

    /// Create an assistant message with text content.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_sdk::{Message, Role};
    ///
    /// let msg = Message::assistant("The answer is 4.");
    /// assert_eq!(msg.role, Role::Assistant);
    /// assert_eq!(msg.text(), "The answer is 4.");
    /// ```
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::new(Role::Assistant, vec![ContentBlock::text(text)])
    }

    /// Create a system message with text content.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_sdk::{Message, Role};
    ///
    /// let msg = Message::system("You are a helpful assistant.");
    /// assert_eq!(msg.role, Role::System);
    /// ```
    pub fn system(text: impl Into<String>) -> Self {
        Self::new(Role::System, vec![ContentBlock::text(text)])
    }

    /// Create a user message with text and an image.
    pub fn user_with_image(text: impl Into<String>, image: ImageSource) -> Self {
        Self::new(
            Role::User,
            vec![
                ContentBlock::text(text),
                ContentBlock::Image {
                    source: image,
                    detail: None,
                },
            ],
        )
    }

    /// Create a tool result message.
    pub fn tool_result(tool_use_id: impl Into<String>, content: ToolResultContent) -> Self {
        Self::new(
            Role::Tool,
            vec![ContentBlock::tool_result(tool_use_id, content, false)],
        )
    }

    /// Create a tool error message.
    pub fn tool_error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::new(
            Role::Tool,
            vec![ContentBlock::tool_result(
                tool_use_id,
                ToolResultContent::Text(error.into()),
                true,
            )],
        )
    }

    /// Set provider-specific options for this message.
    pub fn with_provider_options(mut self, options: ProviderOptions) -> Self {
        self.provider_options = Some(options);
        self
    }

    /// Get all text content from this message concatenated.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| b.as_text())
            .collect::<Vec<_>>()
            .join("")
    }

    /// Get all tool use blocks from this message.
    pub fn tool_uses(&self) -> Vec<&ContentBlock> {
        self.content.iter().filter(|b| b.is_tool_use()).collect()
    }

    /// Set the source provider and model for this message.
    pub fn with_source(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.metadata = ProviderMetadata::with_source(provider, model);
        self
    }

    /// Get source provider (convenience accessor).
    pub fn source_provider(&self) -> Option<&str> {
        self.metadata.source_provider.as_deref()
    }

    /// Get source model (convenience accessor).
    pub fn source_model(&self) -> Option<&str> {
        self.metadata.source_model.as_deref()
    }

    /// Strip all thinking signatures from this message.
    ///
    /// This is useful when switching providers, as thinking signatures
    /// are provider-specific and cannot be verified by other providers.
    pub fn strip_thinking_signatures(&mut self) {
        for block in &mut self.content {
            if let ContentBlock::Thinking { signature, .. } = block {
                *signature = None;
            }
        }
    }

    /// Sanitize this message for use with a target provider and model.
    ///
    /// If the message was generated by a different provider or model,
    /// this will strip thinking signatures to avoid verification errors.
    /// Both provider AND model must match to preserve signatures, since
    /// different models from the same provider may have incompatible signatures.
    pub fn sanitize_for_target(&mut self, target_provider: &str, target_model: &str) {
        if !self.metadata.is_from(target_provider, target_model) {
            self.strip_thinking_signatures();
        }
    }

    /// Convert message content to be compatible with target provider.
    ///
    /// This method sanitizes the message for cross-provider compatibility:
    /// 1. Strips thinking signatures if source differs from target
    /// 2. Clears provider-specific options
    /// 3. Preserves source tracking in metadata for debugging
    pub fn convert_for_provider(&mut self, target_provider: &str, target_model: &str) {
        let is_same_provider = self.metadata.is_from_provider(target_provider);
        let is_same_model = self.metadata.is_from(target_provider, target_model);

        // 1. Strip thinking signatures if provider/model differs
        if !is_same_model {
            self.strip_thinking_signatures();
        }

        // 2. Clear provider-specific options that won't be understood
        if !is_same_provider {
            self.provider_options = None;
        }

        // 3. Clear extensions from other providers (optional, configurable)
        // Keep source tracking, but remove runtime extensions from different providers
        if !is_same_provider {
            // Preserve extensions for the target provider only
            let target_ext = self.metadata.extensions.remove(target_provider);
            self.metadata.extensions.clear();
            if let Some(ext) = target_ext {
                self.metadata
                    .extensions
                    .insert(target_provider.to_string(), ext);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_constructors() {
        let user_msg = Message::user("Hello!");
        assert_eq!(user_msg.role, Role::User);
        assert_eq!(user_msg.text(), "Hello!");

        let assistant_msg = Message::assistant("Hi there!");
        assert_eq!(assistant_msg.role, Role::Assistant);
        assert_eq!(assistant_msg.text(), "Hi there!");

        let system_msg = Message::system("You are helpful.");
        assert_eq!(system_msg.role, Role::System);
    }

    #[test]
    fn test_user_with_image() {
        let msg = Message::user_with_image(
            "What's in this image?",
            ImageSource::Url {
                url: "https://example.com/image.png".to_string(),
            },
        );

        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 2);
        assert!(msg.content[0].as_text().is_some());
        assert!(matches!(msg.content[1], ContentBlock::Image { .. }));
    }

    #[test]
    fn test_content_block_serde() {
        let block = ContentBlock::text("Hello");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));

        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_text(), Some("Hello"));
    }

    #[test]
    fn test_tool_use_block() {
        let block = ContentBlock::tool_use(
            "call_123",
            "get_weather",
            serde_json::json!({"location": "NYC"}),
        );

        assert!(block.is_tool_use());
        assert!(!block.is_thinking());
    }

    // ============================================================
    // Cross-Provider Tests
    // ============================================================

    #[test]
    fn test_provider_metadata_new() {
        let meta = ProviderMetadata::new();
        assert!(meta.source_provider.is_none());
        assert!(meta.source_model.is_none());
        assert!(meta.extensions.is_empty());
        assert!(meta.is_empty());
    }

    #[test]
    fn test_provider_metadata_with_source() {
        let meta = ProviderMetadata::with_source("openai", "gpt-4o");
        assert_eq!(meta.source_provider, Some("openai".to_string()));
        assert_eq!(meta.source_model, Some("gpt-4o".to_string()));
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_provider_metadata_is_from() {
        let meta = ProviderMetadata::with_source("openai", "gpt-4o");
        assert!(meta.is_from_provider("openai"));
        assert!(!meta.is_from_provider("anthropic"));
        assert!(meta.is_from("openai", "gpt-4o"));
        assert!(!meta.is_from("openai", "gpt-4o-mini"));
    }

    #[test]
    fn test_provider_metadata_extensions() {
        let mut meta = ProviderMetadata::new();
        meta.set_extension("openai", serde_json::json!({"cache_hit": true}));

        assert!(meta.get_extension("openai").is_some());
        assert!(meta.get_extension("anthropic").is_none());

        let removed = meta.remove_extension("openai");
        assert!(removed.is_some());
        assert!(meta.get_extension("openai").is_none());
    }

    #[test]
    fn test_message_with_source() {
        let msg = Message::assistant("Response from OpenAI").with_source("openai", "gpt-4o");

        assert_eq!(msg.source_provider(), Some("openai"));
        assert_eq!(msg.source_model(), Some("gpt-4o"));
        assert!(msg.metadata.is_from_provider("openai"));
    }

    #[test]
    fn test_openai_history_to_anthropic() {
        // Create messages that came from OpenAI
        let mut openai_msg =
            Message::assistant("I can help with that.").with_source("openai", "gpt-4o");

        // Add thinking block (no signature from OpenAI)
        openai_msg.content.push(ContentBlock::Thinking {
            content: "Let me think...".to_string(),
            signature: None,
        });

        // Convert for Anthropic
        openai_msg.convert_for_provider("anthropic", "claude-sonnet-4-20250514");

        // Source tracking should be preserved
        assert_eq!(
            openai_msg.metadata.source_provider,
            Some("openai".to_string())
        );
        // Provider options should be cleared
        assert!(openai_msg.provider_options.is_none());
    }

    #[test]
    fn test_anthropic_thinking_to_openai() {
        // Create message with Claude thinking signature
        let mut claude_msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Deep reasoning here...".to_string(),
                    signature: Some("base64-encrypted-signature-from-claude".to_string()),
                },
                ContentBlock::text("The answer is 42."),
            ],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        // Convert for OpenAI
        claude_msg.convert_for_provider("openai", "gpt-4o");

        // Signature should be stripped
        if let ContentBlock::Thinking { signature, content } = &claude_msg.content[0] {
            assert!(
                signature.is_none(),
                "Signature should be stripped for OpenAI"
            );
            assert_eq!(content, "Deep reasoning here...");
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[test]
    fn test_tool_calls_cross_provider() {
        // OpenAI tool call
        let openai_tool_msg = Message::new(
            Role::Assistant,
            vec![ContentBlock::tool_use(
                "call_abc123",
                "get_weather",
                serde_json::json!({"location": "NYC"}),
            )],
        )
        .with_source("openai", "gpt-4o");

        // Tool result
        let tool_result = Message::tool_result(
            "call_abc123",
            crate::tools::ToolResultContent::text("Weather: Sunny, 72°F"),
        );

        let mut history = vec![
            Message::user("What's the weather in NYC?"),
            openai_tool_msg,
            tool_result,
        ];

        // Convert for Anthropic
        for msg in &mut history {
            msg.convert_for_provider("anthropic", "claude-3-opus");
        }

        // ToolUse/ToolResult structure should be preserved
        if let ContentBlock::ToolUse { id, name, .. } = &history[1].content[0] {
            assert_eq!(id, "call_abc123");
            assert_eq!(name, "get_weather");
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_provider_options_handling() {
        // Message with OpenAI-specific options
        let openai_opts: crate::options::ProviderOptions =
            Box::new(crate::options::OpenAIOptions {
                previous_response_id: Some("resp_123".to_string()),
                ..Default::default()
            });

        let mut msg = Message::assistant("Response")
            .with_source("openai", "gpt-4o")
            .with_provider_options(openai_opts);

        // Convert for Anthropic - options should be cleared
        msg.convert_for_provider("anthropic", "claude-3-opus");
        assert!(msg.provider_options.is_none());

        // Same provider, different model - options should be preserved
        let openai_opts2: crate::options::ProviderOptions =
            Box::new(crate::options::OpenAIOptions::default());
        let mut msg2 = Message::assistant("Response")
            .with_source("openai", "gpt-4o")
            .with_provider_options(openai_opts2);
        msg2.convert_for_provider("openai", "gpt-4o-mini");
        assert!(msg2.provider_options.is_some()); // Same provider, preserved
    }

    #[test]
    fn test_multi_turn_cross_provider() {
        // Simulate: User -> OpenAI -> User -> Anthropic -> User -> Gemini
        let mut conversation = vec![
            // Turn 1: User to OpenAI
            Message::user("Explain quantum computing"),
            Message::assistant("Quantum computing uses qubits...").with_source("openai", "gpt-4o"),
            // Turn 2: User to Anthropic (with OpenAI history)
            Message::user("Can you elaborate on superposition?"),
            Message::new(
                Role::Assistant,
                vec![
                    ContentBlock::Thinking {
                        content: "The user wants details on superposition...".to_string(),
                        signature: Some("anthropic-sig-xyz".to_string()),
                    },
                    ContentBlock::text("Superposition is a quantum principle where..."),
                ],
            )
            .with_source("anthropic", "claude-sonnet-4-20250514"),
        ];

        // Convert all for Gemini
        for msg in &mut conversation {
            msg.convert_for_provider("gemini", "gemini-1.5-pro");
        }

        // All thinking signatures should be stripped
        for msg in &conversation {
            for block in &msg.content {
                if let ContentBlock::Thinking { signature, .. } = block {
                    assert!(
                        signature.is_none(),
                        "All thinking signatures should be stripped for Gemini"
                    );
                }
            }
        }

        // Source tracking should be preserved (for debugging)
        assert_eq!(
            conversation[1].metadata.source_provider,
            Some("openai".to_string())
        );
        assert_eq!(
            conversation[3].metadata.source_provider,
            Some("anthropic".to_string())
        );
    }

    #[test]
    fn test_sanitize_for_target() {
        let mut msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Thinking content".to_string(),
                    signature: Some("sig".to_string()),
                },
                ContentBlock::text("Response"),
            ],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        // Same provider/model - signature preserved
        msg.sanitize_for_target("anthropic", "claude-sonnet-4-20250514");
        if let ContentBlock::Thinking { signature, .. } = &msg.content[0] {
            assert!(
                signature.is_some(),
                "Signature should be preserved for same provider/model"
            );
        }

        // Different model - signature stripped
        msg.sanitize_for_target("anthropic", "claude-opus-4-20250514");
        if let ContentBlock::Thinking { signature, .. } = &msg.content[0] {
            assert!(
                signature.is_none(),
                "Signature should be stripped for different model"
            );
        }
    }

    // ============================================================
    // Cross-Provider Integration Tests (from design document)
    // ============================================================

    /// Test: OpenAI-generated history with tool calls can be sent to Anthropic.
    /// Verifies that ToolUse/ToolResult IDs are preserved across providers.
    #[test]
    fn test_openai_tool_history_to_anthropic() {
        // Build OpenAI response history with tool call
        let openai_response = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::text("I'll help you with that task."),
                ContentBlock::tool_use(
                    "call_abc123",
                    "read_file",
                    serde_json::json!({"path": "/tmp/test.txt"}),
                ),
            ],
        )
        .with_source("openai", "gpt-4o");

        let tool_result =
            Message::tool_result("call_abc123", ToolResultContent::text("File content here"));

        let mut history = vec![
            Message::user("Please read /tmp/test.txt"),
            openai_response,
            tool_result,
            Message::user("Now summarize it"),
        ];

        // Sanitize for Anthropic
        for msg in &mut history {
            msg.convert_for_provider("anthropic", "claude-sonnet-4-20250514");
        }

        // Verify: ToolUse ID preserved (critical for tool call correlation)
        if let ContentBlock::ToolUse { id, name, .. } = &history[1].content[1] {
            assert_eq!(
                id, "call_abc123",
                "ToolUse ID must be preserved across providers"
            );
            assert_eq!(name, "read_file");
        } else {
            panic!("Expected ToolUse block");
        }

        // Verify: ToolResult ID matches ToolUse ID
        if let ContentBlock::ToolResult { tool_use_id, .. } = &history[2].content[0] {
            assert_eq!(
                tool_use_id, "call_abc123",
                "ToolResult ID must match ToolUse ID"
            );
        } else {
            panic!("Expected ToolResult block");
        }

        // Verify: Source tracking preserved (for debugging)
        assert_eq!(
            history[1].metadata.source_provider,
            Some("openai".to_string())
        );
    }

    /// Test: Anthropic thinking with signature sent to OpenAI has signature stripped.
    #[test]
    fn test_anthropic_thinking_signature_to_openai() {
        let mut anthropic_response = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Let me analyze this step by step...".to_string(),
                    signature: Some("base64-anthropic-signature-xyz".to_string()),
                },
                ContentBlock::text("Based on my analysis, the answer is 42."),
            ],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        // Sanitize for OpenAI
        anthropic_response.convert_for_provider("openai", "gpt-4o");

        // Verify: Signature stripped (OpenAI cannot verify Anthropic signatures)
        if let ContentBlock::Thinking { signature, content } = &anthropic_response.content[0] {
            assert!(signature.is_none(), "Signature must be stripped for OpenAI");
            assert_eq!(
                content, "Let me analyze this step by step...",
                "Thinking content preserved"
            );
        } else {
            panic!("Expected Thinking block");
        }

        // Verify: Text content preserved
        assert_eq!(
            anthropic_response.content[1].as_text(),
            Some("Based on my analysis, the answer is 42.")
        );
    }

    /// Test: Multi-hop provider switching (OpenAI → Anthropic → Gemini → OpenAI).
    #[test]
    fn test_multi_hop_provider_conversation() {
        // Turn 1: OpenAI response
        let openai_msg = Message::assistant("OpenAI response").with_source("openai", "gpt-4o");

        // Turn 2: Anthropic response with thinking
        let anthropic_msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Anthropic thinking".to_string(),
                    signature: Some("ant-sig".to_string()),
                },
                ContentBlock::text("Anthropic response"),
            ],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        // Turn 3: Gemini response with thinking (no signature - Gemini doesn't use signatures)
        let gemini_msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Gemini thinking".to_string(),
                    signature: None,
                },
                ContentBlock::text("Gemini response"),
            ],
        )
        .with_source("gemini", "gemini-2.5-pro");

        // Build full history
        let mut history = vec![
            Message::user("Question 1"),
            openai_msg,
            Message::user("Question 2"),
            anthropic_msg,
            Message::user("Question 3"),
            gemini_msg,
            Message::user("Question 4"),
        ];

        // Now switch back to OpenAI for the next turn
        for msg in &mut history {
            msg.convert_for_provider("openai", "gpt-4o");
        }

        // Verify: All thinking signatures stripped
        for msg in &history {
            for block in &msg.content {
                if let ContentBlock::Thinking { signature, .. } = block {
                    assert!(
                        signature.is_none(),
                        "All signatures should be stripped for OpenAI"
                    );
                }
            }
        }

        // Verify: Source tracking preserved for all assistant messages
        assert_eq!(
            history[1].metadata.source_provider,
            Some("openai".to_string())
        );
        assert_eq!(
            history[3].metadata.source_provider,
            Some("anthropic".to_string())
        );
        assert_eq!(
            history[5].metadata.source_provider,
            Some("gemini".to_string())
        );
    }

    /// Test: Tool call continuity across provider switch with follow-up.
    #[test]
    fn test_tool_call_continuity_across_providers() {
        // OpenAI makes a tool call
        let openai_tool_call = Message::new(
            Role::Assistant,
            vec![ContentBlock::tool_use(
                "call_001",
                "get_weather",
                serde_json::json!({"city": "NYC"}),
            )],
        )
        .with_source("openai", "gpt-4o");

        // User provides tool result
        let tool_result =
            Message::tool_result("call_001", ToolResultContent::text("Weather: Sunny, 72°F"));

        // OpenAI continues with tool result
        let openai_followup = Message::assistant("The weather in NYC is sunny and 72°F.")
            .with_source("openai", "gpt-4o");

        // Now switch to Anthropic for the next turn
        let mut history = vec![
            Message::user("What's the weather in NYC?"),
            openai_tool_call,
            tool_result,
            openai_followup,
            Message::user("What about tomorrow?"),
        ];

        for msg in &mut history {
            msg.convert_for_provider("anthropic", "claude-sonnet-4-20250514");
        }

        // Verify: ToolUse ID preserved
        if let ContentBlock::ToolUse { id, name, .. } = &history[1].content[0] {
            assert_eq!(id, "call_001");
            assert_eq!(name, "get_weather");
        } else {
            panic!("Expected ToolUse block");
        }

        // Verify: ToolResult ID matches
        if let ContentBlock::ToolResult { tool_use_id, .. } = &history[2].content[0] {
            assert_eq!(tool_use_id, "call_001");
        } else {
            panic!("Expected ToolResult block");
        }

        // Verify: Text content preserved
        assert_eq!(history[3].text(), "The weather in NYC is sunny and 72°F.");
    }

    /// Test: Same provider, different model sanitization.
    /// Model-specific signatures should be stripped even within the same provider.
    #[test]
    fn test_same_provider_different_model_sanitization() {
        let mut claude_sonnet_msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Thinking from Sonnet".to_string(),
                    signature: Some("sonnet-4-specific-signature".to_string()),
                },
                ContentBlock::text("Response from Sonnet"),
            ],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        // Sanitize for Claude Opus (same provider, different model)
        claude_sonnet_msg.sanitize_for_target("anthropic", "claude-opus-4-20250514");

        // Signature should be stripped (different models may have incompatible signatures)
        if let ContentBlock::Thinking { signature, content } = &claude_sonnet_msg.content[0] {
            assert!(
                signature.is_none(),
                "Signature must be stripped for different model"
            );
            assert_eq!(
                content, "Thinking from Sonnet",
                "Thinking content preserved"
            );
        } else {
            panic!("Expected Thinking block");
        }

        // Text content preserved
        assert_eq!(
            claude_sonnet_msg.content[1].as_text(),
            Some("Response from Sonnet")
        );
    }
}
