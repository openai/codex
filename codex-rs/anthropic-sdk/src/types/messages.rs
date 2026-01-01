use serde::Deserialize;
use serde::Serialize;

use super::ContentBlock;
use super::ContentBlockParam;
use super::Metadata;
use super::Role;
use super::StopReason;
use super::SystemPrompt;
use super::Tool;
use super::ToolChoice;
use super::Usage;
use crate::error::AnthropicError;
use crate::error::Result;

// ============================================================================
// Thinking configuration
// ============================================================================

/// Extended thinking configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    /// Enable extended thinking with a budget.
    Enabled {
        /// Maximum tokens for thinking (must be >= 1024).
        budget_tokens: i32,
    },
    /// Disable extended thinking.
    Disabled,
}

/// Minimum budget tokens for extended thinking.
pub const MIN_THINKING_BUDGET_TOKENS: i32 = 1024;

impl ThinkingConfig {
    /// Create an enabled thinking config with the given budget.
    ///
    /// Note: Use `enabled_checked()` for validation that budget_tokens >= 1024.
    pub fn enabled(budget_tokens: i32) -> Self {
        Self::Enabled { budget_tokens }
    }

    /// Create an enabled thinking config with validation.
    ///
    /// Returns an error if budget_tokens < 1024.
    pub fn enabled_checked(budget_tokens: i32) -> Result<Self> {
        if budget_tokens < MIN_THINKING_BUDGET_TOKENS {
            return Err(AnthropicError::Validation(format!(
                "budget_tokens must be >= {MIN_THINKING_BUDGET_TOKENS}, got {budget_tokens}"
            )));
        }
        Ok(Self::Enabled { budget_tokens })
    }

    /// Create a disabled thinking config.
    pub fn disabled() -> Self {
        Self::Disabled
    }
}

// ============================================================================
// Service tier
// ============================================================================

/// Service tier for priority routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTier {
    /// Automatically select the best tier.
    Auto,
    /// Use standard tier only.
    StandardOnly,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageParam {
    /// The role of the message author.
    pub role: Role,

    /// The content of the message.
    #[serde(with = "message_content")]
    pub content: Vec<ContentBlockParam>,
}

impl MessageParam {
    /// Create a user message with text content.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlockParam::text(text)],
        }
    }

    /// Create an assistant message with text content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlockParam::text(text)],
        }
    }

    /// Create a user message with multiple content blocks.
    pub fn user_with_content(content: Vec<ContentBlockParam>) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    /// Create an assistant message with multiple content blocks.
    pub fn assistant_with_content(content: Vec<ContentBlockParam>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }
}

/// Parameters for creating a message.
#[derive(Debug, Clone, Serialize)]
pub struct MessageCreateParams {
    /// The model to use (e.g., "claude-3-5-sonnet-20241022").
    pub model: String,

    /// Maximum number of tokens to generate.
    pub max_tokens: i32,

    /// Input messages for the conversation.
    pub messages: Vec<MessageParam>,

    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,

    /// Sampling temperature (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Nucleus sampling probability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Top-K sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    /// Custom stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Tool definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// How the model should use tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,

    /// Extended thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,

    /// Service tier for priority routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
}

impl MessageCreateParams {
    /// Create new message parameters with required fields.
    pub fn new(model: impl Into<String>, max_tokens: i32, messages: Vec<MessageParam>) -> Self {
        Self {
            model: model.into(),
            max_tokens,
            messages,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            metadata: None,
            thinking: None,
            service_tier: None,
        }
    }

    /// Set the system prompt.
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(SystemPrompt::Text(system.into()));
        self
    }

    /// Set the temperature (unchecked).
    ///
    /// Note: Use `temperature_checked()` for validation that temperature is in [0.0, 1.0].
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the temperature with validation.
    ///
    /// Returns an error if temperature is not in the range [0.0, 1.0].
    pub fn temperature_checked(mut self, temperature: f64) -> Result<Self> {
        if !(0.0..=1.0).contains(&temperature) {
            return Err(AnthropicError::Validation(format!(
                "temperature must be in range [0.0, 1.0], got {temperature}"
            )));
        }
        self.temperature = Some(temperature);
        Ok(self)
    }

    /// Set top_p.
    pub fn top_p(mut self, top_p: f64) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set top_k.
    pub fn top_k(mut self, top_k: i32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set stop sequences.
    pub fn stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }

    /// Set tools.
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice.
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Set metadata.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set thinking configuration.
    pub fn thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.thinking = Some(thinking);
        self
    }

    /// Set service tier.
    pub fn service_tier(mut self, tier: ServiceTier) -> Self {
        self.service_tier = Some(tier);
        self
    }
}

/// Response message from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: String,

    /// Object type (always "message").
    #[serde(rename = "type")]
    pub message_type: String,

    /// The role (always "assistant" for responses).
    pub role: Role,

    /// Generated content blocks.
    pub content: Vec<ContentBlock>,

    /// The model used.
    pub model: String,

    /// Reason the model stopped generating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,

    /// The stop sequence that was matched, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,

    /// Token usage information.
    pub usage: Usage,
}

impl Message {
    /// Get the text content from the message.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text { text, .. } = block {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Get all tool use blocks from the message.
    pub fn tool_uses(&self) -> Vec<(&str, &str, &serde_json::Value)> {
        self.content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    Some((id.as_str(), name.as_str(), input))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if the message contains tool use.
    pub fn has_tool_use(&self) -> bool {
        self.content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }))
    }
}

/// Parameters for counting tokens.
#[derive(Debug, Clone, Serialize)]
pub struct CountTokensParams {
    /// The model to use for counting.
    pub model: String,

    /// Messages to count tokens for.
    pub messages: Vec<MessageParam>,

    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,

    /// Tool definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool choice configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

impl CountTokensParams {
    /// Create new count tokens parameters with required fields.
    pub fn new(model: impl Into<String>, messages: Vec<MessageParam>) -> Self {
        Self {
            model: model.into(),
            messages,
            system: None,
            tools: None,
            tool_choice: None,
        }
    }

    /// Set the system prompt.
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(SystemPrompt::Text(system.into()));
        self
    }

    /// Set tools.
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }
}

/// Custom serialization for message content to support both string and array.
mod message_content {
    use super::ContentBlockParam;
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;
    use serde::Serializer;

    pub fn serialize<S>(content: &[ContentBlockParam], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // If single text block without cache_control, serialize as string for convenience
        if content.len() == 1 {
            if let ContentBlockParam::Text {
                text,
                cache_control: None,
            } = &content[0]
            {
                return text.serialize(serializer);
            }
        }
        content.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ContentBlockParam>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ContentOrString {
            String(String),
            Blocks(Vec<ContentBlockParam>),
        }

        match ContentOrString::deserialize(deserializer)? {
            ContentOrString::String(s) => Ok(vec![ContentBlockParam::Text {
                text: s,
                cache_control: None,
            }]),
            ContentOrString::Blocks(blocks) => Ok(blocks),
        }
    }
}
