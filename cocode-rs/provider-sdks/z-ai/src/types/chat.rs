//! Chat completion types for Z.AI SDK.
//!
//! Types aligned with Python SDK `chat_completion.py`.

use serde::Deserialize;
use serde::Serialize;

use super::CompletionUsage;
use super::ContentBlock;
use super::Function;
use super::MessageParam;
use super::SdkHttpResponse;
use super::ThinkingConfig;
use super::Tool;
use super::ToolChoice;

/// Parameters for creating a chat completion.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionsCreateParams {
    /// Model name (e.g., "glm-4.7").
    pub model: String,
    /// Input messages.
    pub messages: Vec<MessageParam>,
    /// Whether to stream the response (not implemented yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Sampling temperature (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Nucleus sampling probability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    /// Random seed for reproducibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
    /// Whether to use sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub do_sample: Option<bool>,
    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Tool definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Tool choice strategy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Extended thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    /// Response format specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
    /// Request ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// User ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Additional metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::HashMap<String, String>>,

    /// Extra parameters passed through to the API request body.
    #[serde(flatten, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl ChatCompletionsCreateParams {
    /// Create new chat completion parameters.
    pub fn new(model: impl Into<String>, messages: Vec<MessageParam>) -> Self {
        Self {
            model: model.into(),
            messages,
            stream: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            seed: None,
            do_sample: None,
            stop: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            response_format: None,
            request_id: None,
            user_id: None,
            meta: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Set temperature.
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set top_p.
    pub fn top_p(mut self, top_p: f64) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set max_tokens.
    pub fn max_tokens(mut self, max_tokens: i32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set seed.
    pub fn seed(mut self, seed: i32) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set do_sample.
    pub fn do_sample(mut self, do_sample: bool) -> Self {
        self.do_sample = Some(do_sample);
        self
    }

    /// Set stop sequences.
    pub fn stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
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
    pub fn thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.thinking = Some(thinking);
        self
    }

    /// Set response format.
    pub fn response_format(mut self, format: serde_json::Value) -> Self {
        self.response_format = Some(format);
        self
    }

    /// Set request ID.
    pub fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Set user ID.
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }
}

/// Tool call information in completion message.
///
/// From Python SDK `chat_completion.py:19`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionMessageToolCall {
    /// Unique identifier for the tool call.
    pub id: String,
    /// Function call information.
    pub function: Function,
    /// Type of the tool call (always "function").
    #[serde(rename = "type")]
    pub call_type: String,
}

/// Completion message information.
///
/// From Python SDK `chat_completion.py:34`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionMessage {
    /// Message content.
    #[serde(default)]
    pub content: Option<String>,
    /// Role of the message sender.
    pub role: String,
    /// Reasoning content (for thinking models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// List of tool calls in the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<CompletionMessageToolCall>>,
}

/// Completion choice information.
///
/// From Python SDK `chat_completion.py:92`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    /// Index of the choice.
    pub index: i32,
    /// Reason why the completion finished.
    pub finish_reason: String,
    /// Completion message.
    pub message: CompletionMessage,
}

/// Chat completion response.
///
/// From Python SDK `chat_completion.py:107`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    /// Model used for the completion.
    #[serde(default)]
    pub model: Option<String>,
    /// Timestamp when the completion was created.
    #[serde(default)]
    pub created: Option<i64>,
    /// List of completion choices.
    pub choices: Vec<CompletionChoice>,
    /// Request identifier.
    #[serde(default)]
    pub request_id: Option<String>,
    /// Unique identifier for the completion.
    #[serde(default)]
    pub id: Option<String>,
    /// Token usage information.
    pub usage: CompletionUsage,
    /// HTTP response metadata (populated by SDK, not from API).
    #[serde(skip)]
    pub sdk_http_response: Option<SdkHttpResponse>,
}

impl Completion {
    /// Get the text content from the first choice.
    pub fn text(&self) -> String {
        self.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default()
    }

    /// Get reasoning content from the first choice.
    pub fn reasoning(&self) -> Option<String> {
        self.choices
            .first()
            .and_then(|c| c.message.reasoning_content.clone())
    }

    /// Get tool calls from the first choice.
    pub fn tool_calls(&self) -> Option<&[CompletionMessageToolCall]> {
        self.choices
            .first()
            .and_then(|c| c.message.tool_calls.as_deref())
    }

    /// Check if the response contains tool calls.
    pub fn has_tool_calls(&self) -> bool {
        self.choices
            .first()
            .is_some_and(|c| c.message.tool_calls.is_some())
    }
}

/// Helper to create a message from a completion for multi-turn conversations.
impl From<&Completion> for MessageParam {
    fn from(completion: &Completion) -> Self {
        let content = completion.text();
        if content.is_empty() {
            // If no text content, return empty content vec
            Self {
                role: super::Role::Assistant,
                content: vec![],
                tool_call_id: None,
                name: None,
            }
        } else {
            Self {
                role: super::Role::Assistant,
                content: vec![ContentBlock::text(content)],
                tool_call_id: None,
                name: None,
            }
        }
    }
}
