//! Response API types.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

use super::InputContentBlock;
use super::OutputContentBlock;
use super::ResponseStatus;
use super::Role;
use super::StopReason;
use super::Tool;
use super::ToolChoice;
use super::Usage;
use crate::error::ArkError;
use crate::error::Result;

// ============================================================================
// Caching configuration
// ============================================================================

/// Prompt caching configuration for requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachingConfig {
    /// Whether caching is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Caching information in responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseCaching {
    /// Number of cached tokens used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i32>,
}

// ============================================================================
// Thinking configuration
// ============================================================================

/// Minimum budget tokens for extended thinking.
pub const MIN_THINKING_BUDGET_TOKENS: i32 = 1024;

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
    /// Auto mode - let the model decide.
    Auto,
}

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
            return Err(ArkError::Validation(format!(
                "budget_tokens must be >= {MIN_THINKING_BUDGET_TOKENS}, got {budget_tokens}"
            )));
        }
        Ok(Self::Enabled { budget_tokens })
    }

    /// Create a disabled thinking config.
    pub fn disabled() -> Self {
        Self::Disabled
    }

    /// Create an auto thinking config.
    pub fn auto() -> Self {
        Self::Auto
    }
}

// ============================================================================
// Input message
// ============================================================================

/// Input message for the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessage {
    /// Role of the message author.
    pub role: Role,

    /// Content blocks of the message.
    pub content: Vec<InputContentBlock>,
}

impl InputMessage {
    /// Create a user message with content blocks.
    pub fn user(content: Vec<InputContentBlock>) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    /// Create a user message with a single text block.
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![InputContentBlock::text(text)],
        }
    }

    /// Create an assistant message with content blocks.
    pub fn assistant(content: Vec<InputContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }

    /// Create an assistant message with a single text block.
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![InputContentBlock::text(text)],
        }
    }

    /// Create a system message with a single text block.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![InputContentBlock::text(text)],
        }
    }
}

// ============================================================================
// Reasoning types
// ============================================================================

/// Status of a reasoning item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningStatus {
    /// Reasoning is in progress.
    InProgress,
    /// Reasoning is completed.
    Completed,
    /// Reasoning is incomplete.
    Incomplete,
}

/// A summary item in reasoning output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningSummary {
    /// The summary text.
    pub text: String,
    /// The type of summary (always "summary_text").
    #[serde(rename = "type")]
    pub summary_type: String,
}

impl ReasoningSummary {
    /// Create a new reasoning summary.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            summary_type: "summary_text".to_string(),
        }
    }
}

/// Reasoning effort level for model inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
}

// ============================================================================
// Output item
// ============================================================================

/// Output item from a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    /// Message output.
    Message {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Role (always "assistant").
        role: String,
        /// Content blocks.
        content: Vec<OutputContentBlock>,
    },
    /// Function call output.
    FunctionCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID to reference this call.
        call_id: String,
        /// Function name.
        name: String,
        /// Arguments as JSON string.
        arguments: String,
    },
    /// Reasoning output from reasoning models.
    Reasoning {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Reasoning content.
        content: String,
        /// Reasoning summaries.
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<Vec<ReasoningSummary>>,
        /// Status of the reasoning.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<ReasoningStatus>,
    },
}

// ============================================================================
// Request parameters
// ============================================================================

/// Parameters for creating a response.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseCreateParams {
    /// Model or endpoint ID to use.
    pub model: String,

    /// Input messages.
    pub input: Vec<InputMessage>,

    /// System instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Maximum output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    /// Tool definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool choice configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Extended thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,

    /// Reasoning effort level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// Previous response ID for multi-turn conversations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,

    /// Sampling temperature (0.0 to 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Nucleus sampling probability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Whether to store the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,

    /// Prompt caching configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caching: Option<CachingConfig>,

    /// Extra parameters passed through to the API request body.
    #[serde(flatten, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl ResponseCreateParams {
    /// Create new response parameters with required fields.
    pub fn new(model: impl Into<String>, input: Vec<InputMessage>) -> Self {
        Self {
            model: model.into(),
            input,
            instructions: None,
            max_output_tokens: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            reasoning_effort: None,
            previous_response_id: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            store: None,
            caching: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Set system instructions.
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    /// Set maximum output tokens.
    pub fn max_output_tokens(mut self, tokens: i32) -> Self {
        self.max_output_tokens = Some(tokens);
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

    /// Set thinking configuration.
    pub fn thinking(mut self, config: ThinkingConfig) -> Self {
        self.thinking = Some(config);
        self
    }

    /// Set reasoning effort level.
    pub fn reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }

    /// Set previous response ID.
    pub fn previous_response_id(mut self, id: impl Into<String>) -> Self {
        self.previous_response_id = Some(id.into());
        self
    }

    /// Set temperature (unchecked).
    ///
    /// Note: Use `temperature_checked()` for validation that temperature is in [0.0, 2.0].
    pub fn temperature(mut self, temp: f64) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set temperature with validation.
    ///
    /// Returns an error if temperature is not in the range [0.0, 2.0].
    pub fn temperature_checked(mut self, temp: f64) -> Result<Self> {
        if !(0.0..=2.0).contains(&temp) {
            return Err(ArkError::Validation(format!(
                "temperature must be in range [0.0, 2.0], got {temp}"
            )));
        }
        self.temperature = Some(temp);
        Ok(self)
    }

    /// Set top_p.
    pub fn top_p(mut self, top_p: f64) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set stop sequences.
    pub fn stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }

    /// Set whether to store the response.
    pub fn store(mut self, store: bool) -> Self {
        self.store = Some(store);
        self
    }

    /// Set caching configuration.
    pub fn caching(mut self, config: CachingConfig) -> Self {
        self.caching = Some(config);
        self
    }
}

// ============================================================================
// SDK HTTP Response (for round-trip preservation)
// ============================================================================

/// HTTP response metadata (not serialized, populated by client).
/// Used to retain the full HTTP response for debugging/round-trip preservation.
#[derive(Debug, Clone, Default)]
pub struct SdkHttpResponse {
    /// HTTP status code.
    pub status_code: Option<i32>,
    /// Response headers.
    pub headers: Option<HashMap<String, String>>,
    /// Raw response body.
    pub body: Option<String>,
}

impl SdkHttpResponse {
    /// Create a new SdkHttpResponse with all fields.
    pub fn new(status_code: i32, headers: HashMap<String, String>, body: String) -> Self {
        Self {
            status_code: Some(status_code),
            headers: Some(headers),
            body: Some(body),
        }
    }

    /// Create from status code and body only.
    pub fn from_status_and_body(status_code: i32, body: String) -> Self {
        Self {
            status_code: Some(status_code),
            headers: None,
            body: Some(body),
        }
    }
}

// ============================================================================
// Response
// ============================================================================

/// Error details in a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

/// Response from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Unique response ID.
    pub id: String,

    /// Response status.
    pub status: ResponseStatus,

    /// Output items.
    pub output: Vec<OutputItem>,

    /// Token usage.
    pub usage: Usage,

    /// Creation timestamp (Unix seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,

    /// Model or endpoint used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Object type (always "response").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,

    /// Caching information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caching: Option<ResponseCaching>,

    /// Error details if status is Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,

    /// Reason generation stopped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,

    /// HTTP response metadata (not serialized, populated by client).
    /// Used to retain the full HTTP response for round-trip preservation.
    #[serde(skip)]
    pub sdk_http_response: Option<SdkHttpResponse>,
}

impl Response {
    /// Get concatenated text from all message outputs.
    pub fn text(&self) -> String {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::Message { content, .. } = item {
                    Some(
                        content
                            .iter()
                            .filter_map(|c| c.as_text())
                            .collect::<Vec<_>>()
                            .join(""),
                    )
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Get all function calls from the response.
    pub fn function_calls(&self) -> Vec<(&str, &str, &str)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                    ..
                } = item
                {
                    Some((call_id.as_str(), name.as_str(), arguments.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if response contains function calls.
    pub fn has_function_calls(&self) -> bool {
        self.output
            .iter()
            .any(|item| matches!(item, OutputItem::FunctionCall { .. }))
    }

    /// Get thinking content if present.
    pub fn thinking(&self) -> Option<String> {
        self.output.iter().find_map(|item| {
            if let OutputItem::Message { content, .. } = item {
                content
                    .iter()
                    .find_map(|c| c.as_thinking().map(String::from))
            } else {
                None
            }
        })
    }

    /// Get reasoning content if present.
    pub fn reasoning(&self) -> Option<&str> {
        self.output.iter().find_map(|item| {
            if let OutputItem::Reasoning { content, .. } = item {
                Some(content.as_str())
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_config() {
        let enabled = ThinkingConfig::enabled(2048);
        let json = serde_json::to_string(&enabled).unwrap();
        assert!(json.contains(r#""type":"enabled""#));
        assert!(json.contains(r#""budget_tokens":2048"#));

        let disabled = ThinkingConfig::disabled();
        let json = serde_json::to_string(&disabled).unwrap();
        assert!(json.contains(r#""type":"disabled""#));

        let auto = ThinkingConfig::auto();
        let json = serde_json::to_string(&auto).unwrap();
        assert!(json.contains(r#""type":"auto""#));
    }

    #[test]
    fn test_thinking_config_checked() {
        assert!(ThinkingConfig::enabled_checked(1024).is_ok());
        assert!(ThinkingConfig::enabled_checked(2048).is_ok());
        assert!(ThinkingConfig::enabled_checked(1023).is_err());
        assert!(ThinkingConfig::enabled_checked(0).is_err());
    }

    #[test]
    fn test_input_message() {
        let msg = InputMessage::user_text("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);

        let msg = InputMessage::system("You are helpful");
        assert_eq!(msg.role, Role::System);
    }

    #[test]
    fn test_response_create_params_builder() {
        let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("Hello")])
            .instructions("Be helpful")
            .max_output_tokens(1024)
            .temperature(0.7)
            .thinking(ThinkingConfig::enabled(2048));

        assert_eq!(params.model, "ep-xxx");
        assert_eq!(params.instructions, Some("Be helpful".to_string()));
        assert_eq!(params.max_output_tokens, Some(1024));
        assert_eq!(params.temperature, Some(0.7));
        assert!(params.thinking.is_some());
    }

    #[test]
    fn test_temperature_checked() {
        let params = ResponseCreateParams::new("ep-xxx", vec![]);
        assert!(params.clone().temperature_checked(0.5).is_ok());
        assert!(params.clone().temperature_checked(0.0).is_ok());
        assert!(params.clone().temperature_checked(2.0).is_ok());
        assert!(params.clone().temperature_checked(-0.1).is_err());
        assert!(params.clone().temperature_checked(2.1).is_err());
    }

    #[test]
    fn test_store_and_caching() {
        let params = ResponseCreateParams::new("ep-xxx", vec![])
            .store(true)
            .caching(CachingConfig {
                enabled: Some(true),
            });

        assert_eq!(params.store, Some(true));
        assert!(params.caching.is_some());
    }

    #[test]
    fn test_reasoning_output_item() {
        let item = OutputItem::Reasoning {
            id: Some("r-1".to_string()),
            content: "Let me think...".to_string(),
            summary: Some(vec![ReasoningSummary::new("Summary")]),
            status: Some(ReasoningStatus::Completed),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains(r#""type":"reasoning""#));
        assert!(json.contains(r#""content":"Let me think...""#));
        assert!(json.contains(r#""status":"completed""#));
    }

    #[test]
    fn test_reasoning_status() {
        let status = ReasoningStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""in_progress""#);

        let status = ReasoningStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""completed""#);

        let status = ReasoningStatus::Incomplete;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""incomplete""#);
    }

    #[test]
    fn test_reasoning_summary() {
        let summary = ReasoningSummary::new("Test summary");
        assert_eq!(summary.text, "Test summary");
        assert_eq!(summary.summary_type, "summary_text");

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains(r#""text":"Test summary""#));
        assert!(json.contains(r#""type":"summary_text""#));
    }

    #[test]
    fn test_reasoning_effort() {
        let effort = ReasoningEffort::High;
        let json = serde_json::to_string(&effort).unwrap();
        assert_eq!(json, r#""high""#);

        let effort = ReasoningEffort::Minimal;
        let json = serde_json::to_string(&effort).unwrap();
        assert_eq!(json, r#""minimal""#);
    }

    #[test]
    fn test_response_create_params_with_reasoning_effort() {
        let params = ResponseCreateParams::new("ep-xxx", vec![])
            .reasoning_effort(ReasoningEffort::High)
            .thinking(ThinkingConfig::auto());

        assert!(params.reasoning_effort.is_some());
        assert!(params.thinking.is_some());
    }

    #[test]
    fn test_response_status_incomplete() {
        use super::super::ResponseStatus;
        let json = r#""incomplete""#;
        let status: ResponseStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, ResponseStatus::Incomplete);
    }
}
