//! Response API types.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

use super::InputContentBlock;
use super::Metadata;
use super::OutputContentBlock;
use super::ResponseStatus;
use super::Role;
use super::StopReason;
use super::Tool;
use super::ToolChoice;
use super::Usage;
use crate::error::OpenAIError;
use crate::error::Result;

// ============================================================================
// Prompt caching configuration
// ============================================================================

/// Prompt caching retention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCacheRetention {
    /// Session-based cache (in-memory).
    InMemory,
    /// Extended retention up to 24 hours.
    #[serde(rename = "24h")]
    TwentyFourHours,
}

/// Prompt caching configuration for requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCachingConfig {
    /// Cache key for this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    /// Cache retention policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<PromptCacheRetention>,
}

impl PromptCachingConfig {
    /// Create a new prompt caching config with a cache key.
    pub fn with_key(key: impl Into<String>) -> Self {
        Self {
            cache_key: Some(key.into()),
            retention: None,
        }
    }

    /// Set the retention policy.
    pub fn retention(mut self, retention: PromptCacheRetention) -> Self {
        self.retention = Some(retention);
        self
    }
}

// ============================================================================
// Reasoning configuration
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
    pub fn enabled(budget_tokens: i32) -> Self {
        Self::Enabled { budget_tokens }
    }

    /// Create an enabled thinking config with validation.
    pub fn enabled_checked(budget_tokens: i32) -> Result<Self> {
        if budget_tokens < MIN_THINKING_BUDGET_TOKENS {
            return Err(OpenAIError::Validation(format!(
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

/// Reasoning effort level for model inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    /// No reasoning.
    None,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
}

/// Reasoning configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Effort level for reasoning.
    pub effort: ReasoningEffort,
    /// Whether to generate a summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_summary: Option<String>,
}

impl ReasoningConfig {
    /// Create a reasoning config with the given effort level.
    pub fn with_effort(effort: ReasoningEffort) -> Self {
        Self {
            effort,
            generate_summary: None,
        }
    }

    /// Enable summary generation.
    pub fn with_summary(mut self, mode: impl Into<String>) -> Self {
        self.generate_summary = Some(mode.into());
        self
    }
}

// ============================================================================
// Service and configuration types
// ============================================================================

/// Service tier for request processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTier {
    /// Auto-select tier.
    Auto,
    /// Default tier.
    Default,
    /// Flex tier.
    Flex,
    /// Scale tier.
    Scale,
    /// Priority tier.
    Priority,
}

/// Truncation strategy for input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Truncation {
    /// Auto-truncate if needed.
    Auto,
    /// Disable truncation.
    Disabled,
}

/// Items to include in the response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseIncludable {
    /// Include file search call results.
    #[serde(rename = "file_search_call.results")]
    FileSearchCallResults,
    /// Include message input image URL detail.
    #[serde(rename = "message.input_image.image_url.detail")]
    MessageInputImageUrlDetail,
    /// Include computer call output.
    #[serde(rename = "computer_call_output")]
    ComputerCallOutput,
    /// Include reasoning encrypted content.
    #[serde(rename = "reasoning.encrypted_content")]
    ReasoningEncryptedContent,
    /// Include web search call results.
    #[serde(rename = "web_search_call.results")]
    WebSearchCallResults,
    /// Include web search action sources.
    #[serde(rename = "web_search_call.action.sources")]
    WebSearchCallActionSources,
    /// Include code interpreter call outputs.
    #[serde(rename = "code_interpreter_call.outputs")]
    CodeInterpreterCallOutputs,
    /// Include message output text logprobs.
    #[serde(rename = "message.output_text.logprobs")]
    MessageOutputTextLogprobs,
}

/// Text format configuration for structured outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextFormat {
    /// Plain text output.
    Text,
    /// JSON object output.
    JsonObject,
    /// JSON schema output with strict validation.
    JsonSchema {
        /// The JSON schema definition.
        schema: serde_json::Value,
        /// Name of the schema.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Whether to use strict mode.
        #[serde(skip_serializing_if = "Option::is_none")]
        strict: Option<bool>,
    },
}

/// Text/structured output configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextConfig {
    /// Format for text output.
    pub format: TextFormat,
}

impl TextConfig {
    /// Create a plain text config.
    pub fn text() -> Self {
        Self {
            format: TextFormat::Text,
        }
    }

    /// Create a JSON object config.
    pub fn json_object() -> Self {
        Self {
            format: TextFormat::JsonObject,
        }
    }

    /// Create a JSON schema config.
    pub fn json_schema(schema: serde_json::Value) -> Self {
        Self {
            format: TextFormat::JsonSchema {
                schema,
                name: None,
                strict: None,
            },
        }
    }

    /// Create a JSON schema config with name and strict mode.
    pub fn json_schema_strict(schema: serde_json::Value, name: impl Into<String>) -> Self {
        Self {
            format: TextFormat::JsonSchema {
                schema,
                name: Some(name.into()),
                strict: Some(true),
            },
        }
    }
}

/// Reason why a response is incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    /// Hit the maximum output token limit.
    MaxOutputTokens,
    /// Content was filtered.
    ContentFilter,
    /// Interrupted by user or system.
    Interrupted,
    /// Other reason (catch-all).
    #[serde(other)]
    Other,
}

/// Details about why a response is incomplete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncompleteDetails {
    /// The reason the response is incomplete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<IncompleteReason>,
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

    /// Create a developer message with a single text block.
    pub fn developer(text: impl Into<String>) -> Self {
        Self {
            role: Role::Developer,
            content: vec![InputContentBlock::text(text)],
        }
    }
}

// ============================================================================
// Response input (text or messages)
// ============================================================================

/// Input for response creation - can be simple text or messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInput {
    /// Simple text input.
    Text(String),
    /// Array of input messages.
    Messages(Vec<InputMessage>),
}

impl From<String> for ResponseInput {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for ResponseInput {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

impl From<Vec<InputMessage>> for ResponseInput {
    fn from(messages: Vec<InputMessage>) -> Self {
        Self::Messages(messages)
    }
}

// ============================================================================
// Reasoning types
// ============================================================================

/// A summary item in reasoning output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningSummary {
    /// The summary text.
    pub text: String,
    /// The type of summary.
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
    },
    /// File search tool call.
    #[serde(rename = "file_search_call")]
    FileSearchCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Search queries.
        #[serde(default)]
        queries: Vec<String>,
        /// Search results.
        #[serde(skip_serializing_if = "Option::is_none")]
        results: Option<Vec<FileSearchResult>>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Web search tool call.
    #[serde(rename = "web_search_call")]
    WebSearchCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Search query.
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        /// Search results.
        #[serde(skip_serializing_if = "Option::is_none")]
        results: Option<Vec<WebSearchResult>>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Computer tool call for UI automation.
    #[serde(rename = "computer_call")]
    ComputerCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Action to perform.
        action: ComputerAction,
        /// Pending safety checks.
        #[serde(default)]
        pending_safety_checks: Vec<SafetyCheck>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Code interpreter tool call.
    #[serde(rename = "code_interpreter_call")]
    CodeInterpreterCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Code to execute.
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// Execution outputs.
        #[serde(skip_serializing_if = "Option::is_none")]
        outputs: Option<Vec<CodeInterpreterOutput>>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Image generation tool call.
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Generation prompt.
        #[serde(skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        /// Generated image result.
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<ImageGenerationResult>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Local shell tool call.
    #[serde(rename = "local_shell_call")]
    LocalShellCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Shell command.
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        /// Command output.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// MCP tool call.
    #[serde(rename = "mcp_call")]
    McpCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// MCP server label.
        #[serde(skip_serializing_if = "Option::is_none")]
        server_label: Option<String>,
        /// Tool name.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        /// Tool arguments.
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<serde_json::Value>,
        /// Tool output.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Error if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// MCP list tools response.
    #[serde(rename = "mcp_list_tools")]
    McpListTools {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// MCP server label.
        #[serde(skip_serializing_if = "Option::is_none")]
        server_label: Option<String>,
        /// Available tools.
        #[serde(default)]
        tools: Vec<McpToolInfo>,
    },
    /// MCP approval request.
    #[serde(rename = "mcp_approval_request")]
    McpApprovalRequest {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// MCP server label.
        #[serde(skip_serializing_if = "Option::is_none")]
        server_label: Option<String>,
        /// Tool name requiring approval.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        /// Arguments for the tool.
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<serde_json::Value>,
    },
    /// Apply patch tool call.
    #[serde(rename = "apply_patch_call")]
    ApplyPatchCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Patch content.
        #[serde(skip_serializing_if = "Option::is_none")]
        patch: Option<String>,
        /// Output result.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Function shell tool call.
    #[serde(rename = "function_shell_call")]
    FunctionShellCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Shell command.
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        /// Command output.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Custom tool call.
    #[serde(rename = "custom_tool_call")]
    CustomToolCall {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Call ID.
        call_id: String,
        /// Tool name.
        name: String,
        /// Tool input (free-form text).
        input: String,
        /// Status of the call.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Response compaction item.
    #[serde(rename = "compaction")]
    Compaction {
        /// Unique ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Compacted data.
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
}

// ============================================================================
// Tool call result types
// ============================================================================

/// File search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    /// File ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    /// File name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Relevance score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Web search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    /// Result title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Result URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Result snippet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Computer action for UI automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComputerAction {
    /// Click action.
    Click {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Button (left, right, middle).
        #[serde(skip_serializing_if = "Option::is_none")]
        button: Option<String>,
    },
    /// Double click action.
    DoubleClick {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
    },
    /// Scroll action.
    Scroll {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Scroll direction.
        direction: String,
        /// Scroll amount.
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<i32>,
    },
    /// Type text action.
    Type {
        /// Text to type.
        text: String,
    },
    /// Key press action.
    KeyPress {
        /// Key to press.
        key: String,
    },
    /// Screenshot action.
    Screenshot,
    /// Wait action.
    Wait {
        /// Milliseconds to wait.
        #[serde(skip_serializing_if = "Option::is_none")]
        ms: Option<i32>,
    },
    /// Drag action.
    Drag {
        /// Start X coordinate.
        start_x: i32,
        /// Start Y coordinate.
        start_y: i32,
        /// End X coordinate.
        end_x: i32,
        /// End Y coordinate.
        end_y: i32,
    },
}

/// Safety check for computer actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCheck {
    /// Check ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Check code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Check message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Code interpreter output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodeInterpreterOutput {
    /// Log output.
    Logs {
        /// Log content.
        logs: String,
    },
    /// Image output.
    Image {
        /// Image data (base64 or URL).
        #[serde(skip_serializing_if = "Option::is_none")]
        image: Option<String>,
        /// Image file ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
    },
}

/// Image generation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationResult {
    /// Generated image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Generated image base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,
    /// Revised prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
}

/// MCP tool information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Tool name.
    pub name: String,
    /// Tool description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tool input schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

// ============================================================================
// Request parameters
// ============================================================================

/// Parameters for creating a response.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseCreateParams {
    /// Model ID to use (e.g., "gpt-4o", "o3").
    pub model: String,

    /// Input (text string or message array).
    pub input: ResponseInput,

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

    /// Reasoning configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,

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
    pub stop: Option<Vec<String>>,

    /// Whether to store the response server-side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,

    /// Prompt caching configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_caching: Option<PromptCachingConfig>,

    /// Request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,

    /// User identifier for abuse monitoring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Items to include in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<ResponseIncludable>>,

    /// Maximum number of tool calls per turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i32>,

    /// Whether to allow parallel tool calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,

    /// Service tier for processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,

    /// Text/structured output configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextConfig>,

    /// Truncation strategy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,

    /// Number of top logprobs to return (0-20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i32>,

    /// Conversation state for multi-turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<ConversationParam>,

    /// Run model response in background.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,

    /// Safety identifier for policy violation detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,

    /// Prompt template reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PromptParam>,

    /// Stable cache identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,

    /// Extra parameters passed through to the API request body.
    #[serde(flatten, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Conversation parameter for multi-turn state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConversationParam {
    /// Reference by ID.
    Id(String),
    /// Inline conversation items.
    Items {
        /// Items to prepend.
        #[serde(default)]
        items: Vec<serde_json::Value>,
    },
}

/// Prompt template parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptParam {
    /// Prompt template ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Template variables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<std::collections::HashMap<String, String>>,
}

impl ResponseCreateParams {
    /// Create new response parameters with message input.
    pub fn new(model: impl Into<String>, input: Vec<InputMessage>) -> Self {
        Self {
            model: model.into(),
            input: ResponseInput::Messages(input),
            instructions: None,
            max_output_tokens: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            reasoning: None,
            previous_response_id: None,
            temperature: None,
            top_p: None,
            stop: None,
            store: None,
            prompt_caching: None,
            metadata: None,
            user: None,
            include: None,
            max_tool_calls: None,
            parallel_tool_calls: None,
            service_tier: None,
            text: None,
            truncation: None,
            top_logprobs: None,
            conversation: None,
            background: None,
            safety_identifier: None,
            prompt: None,
            prompt_cache_key: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Create new response parameters with simple text input.
    pub fn with_text(model: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            input: ResponseInput::Text(text.into()),
            instructions: None,
            max_output_tokens: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            reasoning: None,
            previous_response_id: None,
            temperature: None,
            top_p: None,
            stop: None,
            store: None,
            prompt_caching: None,
            metadata: None,
            user: None,
            include: None,
            max_tool_calls: None,
            parallel_tool_calls: None,
            service_tier: None,
            text: None,
            truncation: None,
            top_logprobs: None,
            conversation: None,
            background: None,
            safety_identifier: None,
            prompt: None,
            prompt_cache_key: None,
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

    /// Set reasoning configuration.
    pub fn reasoning(mut self, config: ReasoningConfig) -> Self {
        self.reasoning = Some(config);
        self
    }

    /// Set previous response ID for multi-turn conversations.
    pub fn previous_response_id(mut self, id: impl Into<String>) -> Self {
        self.previous_response_id = Some(id.into());
        self
    }

    /// Set temperature (unchecked).
    pub fn temperature(mut self, temp: f64) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set temperature with validation.
    pub fn temperature_checked(mut self, temp: f64) -> Result<Self> {
        if !(0.0..=2.0).contains(&temp) {
            return Err(OpenAIError::Validation(format!(
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
    pub fn stop(mut self, sequences: Vec<String>) -> Self {
        self.stop = Some(sequences);
        self
    }

    /// Set whether to store the response.
    pub fn store(mut self, store: bool) -> Self {
        self.store = Some(store);
        self
    }

    /// Set prompt caching configuration.
    pub fn prompt_caching(mut self, config: PromptCachingConfig) -> Self {
        self.prompt_caching = Some(config);
        self
    }

    /// Set metadata.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set user identifier.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set items to include in the response.
    pub fn include(mut self, items: Vec<ResponseIncludable>) -> Self {
        self.include = Some(items);
        self
    }

    /// Set maximum tool calls per turn.
    pub fn max_tool_calls(mut self, max: i32) -> Self {
        self.max_tool_calls = Some(max);
        self
    }

    /// Set whether to allow parallel tool calls.
    pub fn parallel_tool_calls(mut self, enabled: bool) -> Self {
        self.parallel_tool_calls = Some(enabled);
        self
    }

    /// Set service tier.
    pub fn service_tier(mut self, tier: ServiceTier) -> Self {
        self.service_tier = Some(tier);
        self
    }

    /// Set text/structured output configuration.
    pub fn text_config(mut self, config: TextConfig) -> Self {
        self.text = Some(config);
        self
    }

    /// Set truncation strategy.
    pub fn truncation(mut self, strategy: Truncation) -> Self {
        self.truncation = Some(strategy);
        self
    }

    /// Set top logprobs (unchecked).
    pub fn top_logprobs(mut self, n: i32) -> Self {
        self.top_logprobs = Some(n);
        self
    }

    /// Set top logprobs with validation (0-20).
    pub fn top_logprobs_checked(mut self, n: i32) -> Result<Self> {
        if !(0..=20).contains(&n) {
            return Err(OpenAIError::Validation(format!(
                "top_logprobs must be in range [0, 20], got {n}"
            )));
        }
        self.top_logprobs = Some(n);
        Ok(self)
    }

    /// Set conversation state for multi-turn.
    pub fn conversation(mut self, conv: ConversationParam) -> Self {
        self.conversation = Some(conv);
        self
    }

    /// Set conversation by ID.
    pub fn conversation_id(mut self, id: impl Into<String>) -> Self {
        self.conversation = Some(ConversationParam::Id(id.into()));
        self
    }

    /// Run model response in background.
    pub fn background(mut self, enabled: bool) -> Self {
        self.background = Some(enabled);
        self
    }

    /// Set safety identifier for policy violation detection.
    pub fn safety_identifier(mut self, id: impl Into<String>) -> Self {
        self.safety_identifier = Some(id.into());
        self
    }

    /// Set prompt template reference.
    pub fn prompt(mut self, prompt: PromptParam) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Set stable cache identifier.
    pub fn prompt_cache_key(mut self, key: impl Into<String>) -> Self {
        self.prompt_cache_key = Some(key.into());
        self
    }
}

// ============================================================================
// SDK HTTP Response (for round-trip preservation)
// ============================================================================

/// HTTP response metadata (not serialized, populated by client).
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

/// Prompt template information in response (echoed back).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePrompt {
    /// Prompt template ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Prompt version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Response from the Responses API.
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

    /// Model used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Object type (always "response").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,

    /// Error details if status is Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,

    /// Reason generation stopped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,

    /// Completion timestamp (Unix seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,

    /// Details about why the response is incomplete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_details: Option<IncompleteDetails>,

    /// System instructions (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Service tier used for processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,

    /// Temperature used (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Whether parallel tool calls are allowed (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,

    /// Tools used in this response (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool choice configuration (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Maximum output tokens (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    /// Maximum tool calls per turn (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i32>,

    /// Top-p sampling parameter (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Reasoning configuration (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,

    /// Text configuration (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextConfig>,

    /// Truncation strategy (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,

    /// Top logprobs setting (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i32>,

    /// Prompt template used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<ResponsePrompt>,

    /// Prompt cache key used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,

    /// Prompt cache retention policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,

    /// Safety identifier used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,

    /// HTTP response metadata (not serialized, populated by client).
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

    /// Get cached tokens used (from prompt caching).
    pub fn cached_tokens(&self) -> i32 {
        self.usage.cached_tokens()
    }

    /// Check if response contains any tool calls (including function calls).
    pub fn has_tool_calls(&self) -> bool {
        self.output.iter().any(|item| {
            matches!(
                item,
                OutputItem::FunctionCall { .. }
                    | OutputItem::FileSearchCall { .. }
                    | OutputItem::WebSearchCall { .. }
                    | OutputItem::ComputerCall { .. }
                    | OutputItem::CodeInterpreterCall { .. }
                    | OutputItem::ImageGenerationCall { .. }
                    | OutputItem::LocalShellCall { .. }
                    | OutputItem::McpCall { .. }
                    | OutputItem::ApplyPatchCall { .. }
                    | OutputItem::FunctionShellCall { .. }
                    | OutputItem::CustomToolCall { .. }
            )
        })
    }

    /// Get all web search calls from the response.
    pub fn web_search_calls(&self) -> Vec<(&str, Option<&str>, Option<&Vec<WebSearchResult>>)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::WebSearchCall {
                    call_id,
                    query,
                    results,
                    ..
                } = item
                {
                    Some((call_id.as_str(), query.as_deref(), results.as_ref()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all file search calls from the response.
    pub fn file_search_calls(&self) -> Vec<(&str, &[String], Option<&Vec<FileSearchResult>>)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::FileSearchCall {
                    call_id,
                    queries,
                    results,
                    ..
                } = item
                {
                    Some((call_id.as_str(), queries.as_slice(), results.as_ref()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all computer calls from the response.
    pub fn computer_calls(&self) -> Vec<(&str, &ComputerAction)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::ComputerCall {
                    call_id, action, ..
                } = item
                {
                    Some((call_id.as_str(), action))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all code interpreter calls from the response.
    pub fn code_interpreter_calls(
        &self,
    ) -> Vec<(&str, Option<&str>, Option<&Vec<CodeInterpreterOutput>>)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::CodeInterpreterCall {
                    call_id,
                    code,
                    outputs,
                    ..
                } = item
                {
                    Some((call_id.as_str(), code.as_deref(), outputs.as_ref()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all MCP calls from the response.
    pub fn mcp_calls(&self) -> Vec<MpcCallRef<'_>> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::McpCall {
                    call_id,
                    server_label,
                    tool_name,
                    arguments,
                    output,
                    error,
                    ..
                } = item
                {
                    Some(MpcCallRef {
                        call_id: call_id.as_str(),
                        server_label: server_label.as_deref(),
                        tool_name: tool_name.as_deref(),
                        arguments: arguments.as_ref(),
                        output: output.as_deref(),
                        error: error.as_deref(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all image generation calls from the response.
    pub fn image_generation_calls(
        &self,
    ) -> Vec<(&str, Option<&str>, Option<&ImageGenerationResult>)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::ImageGenerationCall {
                    call_id,
                    prompt,
                    result,
                    ..
                } = item
                {
                    Some((call_id.as_str(), prompt.as_deref(), result.as_ref()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all custom tool calls from the response.
    pub fn custom_tool_calls(&self) -> Vec<(&str, &str, &str)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::CustomToolCall {
                    call_id,
                    name,
                    input,
                    ..
                } = item
                {
                    Some((call_id.as_str(), name.as_str(), input.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all local shell calls from the response.
    pub fn local_shell_calls(&self) -> Vec<(&str, Option<&str>, Option<&str>)> {
        self.output
            .iter()
            .filter_map(|item| {
                if let OutputItem::LocalShellCall {
                    call_id,
                    command,
                    output,
                    ..
                } = item
                {
                    Some((call_id.as_str(), command.as_deref(), output.as_deref()))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Reference to an MCP call in a response.
#[derive(Debug, Clone)]
pub struct MpcCallRef<'a> {
    /// Call ID.
    pub call_id: &'a str,
    /// MCP server label.
    pub server_label: Option<&'a str>,
    /// Tool name.
    pub tool_name: Option<&'a str>,
    /// Tool arguments.
    pub arguments: Option<&'a serde_json::Value>,
    /// Tool output.
    pub output: Option<&'a str>,
    /// Error if any.
    pub error: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::usage::InputTokensDetails;
    use crate::types::usage::OutputTokensDetails;

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
    fn test_response_input_text() {
        let input = ResponseInput::from("Hello");
        let json = serde_json::to_string(&input).unwrap();
        assert_eq!(json, r#""Hello""#);
    }

    #[test]
    fn test_response_input_messages() {
        let input = ResponseInput::from(vec![InputMessage::user_text("Hello")]);
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains(r#""role":"user""#));
    }

    #[test]
    fn test_response_create_params_builder() {
        let params = ResponseCreateParams::new("gpt-4o", vec![InputMessage::user_text("Hello")])
            .instructions("Be helpful")
            .max_output_tokens(1024)
            .temperature(0.7)
            .thinking(ThinkingConfig::enabled(2048));

        assert_eq!(params.model, "gpt-4o");
        assert_eq!(params.instructions, Some("Be helpful".to_string()));
        assert_eq!(params.max_output_tokens, Some(1024));
        assert_eq!(params.temperature, Some(0.7));
        assert!(params.thinking.is_some());
    }

    #[test]
    fn test_response_create_params_with_text() {
        let params = ResponseCreateParams::with_text("gpt-4o", "Hello world");
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains(r#""input":"Hello world""#));
    }

    #[test]
    fn test_prompt_caching_config() {
        let config = PromptCachingConfig::with_key("my-key")
            .retention(PromptCacheRetention::TwentyFourHours);

        assert_eq!(config.cache_key, Some("my-key".to_string()));
        assert_eq!(
            config.retention,
            Some(PromptCacheRetention::TwentyFourHours)
        );

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""cache_key":"my-key""#));
        assert!(json.contains(r#""retention":"24h""#));
    }

    #[test]
    fn test_temperature_checked() {
        let params = ResponseCreateParams::new("gpt-4o", vec![]);
        assert!(params.clone().temperature_checked(0.5).is_ok());
        assert!(params.clone().temperature_checked(0.0).is_ok());
        assert!(params.clone().temperature_checked(2.0).is_ok());
        assert!(params.clone().temperature_checked(-0.1).is_err());
        assert!(params.clone().temperature_checked(2.1).is_err());
    }

    #[test]
    fn test_reasoning_config() {
        let config = ReasoningConfig::with_effort(ReasoningEffort::High).with_summary("auto");

        assert_eq!(config.effort, ReasoningEffort::High);
        assert_eq!(config.generate_summary, Some("auto".to_string()));
    }

    #[test]
    fn test_service_tier() {
        let tier = ServiceTier::Priority;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, r#""priority""#);

        let tier = ServiceTier::Auto;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, r#""auto""#);
    }

    #[test]
    fn test_truncation() {
        let trunc = Truncation::Auto;
        let json = serde_json::to_string(&trunc).unwrap();
        assert_eq!(json, r#""auto""#);

        let trunc = Truncation::Disabled;
        let json = serde_json::to_string(&trunc).unwrap();
        assert_eq!(json, r#""disabled""#);
    }

    #[test]
    fn test_text_config_json_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        let config = TextConfig::json_schema_strict(schema, "person");
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""type":"json_schema""#));
        assert!(json.contains(r#""strict":true"#));
        assert!(json.contains(r#""name":"person""#));
    }

    #[test]
    fn test_text_config_variants() {
        let text = TextConfig::text();
        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains(r#""type":"text""#));

        let json_obj = TextConfig::json_object();
        let json = serde_json::to_string(&json_obj).unwrap();
        assert!(json.contains(r#""type":"json_object""#));
    }

    #[test]
    fn test_response_create_params_new_fields() {
        let params = ResponseCreateParams::new("gpt-4o", vec![])
            .max_tool_calls(10)
            .parallel_tool_calls(true)
            .service_tier(ServiceTier::Priority)
            .truncation(Truncation::Auto)
            .top_logprobs(5);

        assert_eq!(params.max_tool_calls, Some(10));
        assert_eq!(params.parallel_tool_calls, Some(true));
        assert_eq!(params.service_tier, Some(ServiceTier::Priority));
        assert_eq!(params.truncation, Some(Truncation::Auto));
        assert_eq!(params.top_logprobs, Some(5));
    }

    #[test]
    fn test_top_logprobs_checked() {
        let params = ResponseCreateParams::new("gpt-4o", vec![]);
        assert!(params.clone().top_logprobs_checked(0).is_ok());
        assert!(params.clone().top_logprobs_checked(20).is_ok());
        assert!(params.clone().top_logprobs_checked(10).is_ok());
        assert!(params.clone().top_logprobs_checked(-1).is_err());
        assert!(params.clone().top_logprobs_checked(21).is_err());
    }

    #[test]
    fn test_response_includable() {
        let item = ResponseIncludable::FileSearchCallResults;
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, r#""file_search_call.results""#);

        let item = ResponseIncludable::ComputerCallOutput;
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, r#""computer_call_output""#);

        let item = ResponseIncludable::WebSearchCallResults;
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, r#""web_search_call.results""#);

        let item = ResponseIncludable::CodeInterpreterCallOutputs;
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, r#""code_interpreter_call.outputs""#);
    }

    #[test]
    fn test_incomplete_reason() {
        let reason = IncompleteReason::MaxOutputTokens;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#""max_output_tokens""#);

        let reason = IncompleteReason::ContentFilter;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, r#""content_filter""#);
    }

    #[test]
    fn test_new_request_params() {
        let params = ResponseCreateParams::new("gpt-4o", vec![])
            .conversation_id("conv-123")
            .background(true)
            .safety_identifier("safety-456")
            .prompt_cache_key("cache-789");

        assert!(matches!(
            params.conversation,
            Some(ConversationParam::Id(_))
        ));
        assert_eq!(params.background, Some(true));
        assert_eq!(params.safety_identifier, Some("safety-456".to_string()));
        assert_eq!(params.prompt_cache_key, Some("cache-789".to_string()));
    }

    // ========================================================================
    // Response helper method tests
    // ========================================================================

    fn make_test_response(output: Vec<OutputItem>) -> Response {
        Response {
            id: "resp-test".to_string(),
            status: ResponseStatus::Completed,
            output,
            usage: Usage::default(),
            created_at: None,
            model: Some("gpt-4o".to_string()),
            object: Some("response".to_string()),
            error: None,
            stop_reason: None,
            completed_at: None,
            incomplete_details: None,
            instructions: None,
            service_tier: None,
            temperature: None,
            parallel_tool_calls: None,
            tools: None,
            tool_choice: None,
            max_output_tokens: None,
            max_tool_calls: None,
            top_p: None,
            reasoning: None,
            text: None,
            truncation: None,
            top_logprobs: None,
            prompt: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            safety_identifier: None,
            sdk_http_response: None,
        }
    }

    #[test]
    fn test_response_text_single_message() {
        let response = make_test_response(vec![OutputItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![OutputContentBlock::OutputText {
                text: "Hello, world!".to_string(),
                annotations: vec![],
                logprobs: None,
            }],
        }]);
        assert_eq!(response.text(), "Hello, world!");
    }

    #[test]
    fn test_response_text_multiple_messages() {
        let response = make_test_response(vec![
            OutputItem::Message {
                id: Some("msg-1".to_string()),
                role: "assistant".to_string(),
                content: vec![OutputContentBlock::OutputText {
                    text: "Hello".to_string(),
                    annotations: vec![],
                    logprobs: None,
                }],
            },
            OutputItem::Message {
                id: Some("msg-2".to_string()),
                role: "assistant".to_string(),
                content: vec![OutputContentBlock::OutputText {
                    text: " world!".to_string(),
                    annotations: vec![],
                    logprobs: None,
                }],
            },
        ]);
        assert_eq!(response.text(), "Hello world!");
    }

    #[test]
    fn test_response_text_empty() {
        let response = make_test_response(vec![]);
        assert_eq!(response.text(), "");
    }

    #[test]
    fn test_response_function_calls() {
        let response = make_test_response(vec![
            OutputItem::FunctionCall {
                id: Some("fc-1".to_string()),
                call_id: "call-123".to_string(),
                name: "get_weather".to_string(),
                arguments: r#"{"city":"London"}"#.to_string(),
            },
            OutputItem::FunctionCall {
                id: Some("fc-2".to_string()),
                call_id: "call-456".to_string(),
                name: "get_time".to_string(),
                arguments: r#"{"timezone":"UTC"}"#.to_string(),
            },
        ]);
        let calls = response.function_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            ("call-123", "get_weather", r#"{"city":"London"}"#)
        );
        assert_eq!(calls[1], ("call-456", "get_time", r#"{"timezone":"UTC"}"#));
    }

    #[test]
    fn test_response_has_function_calls_true() {
        let response = make_test_response(vec![OutputItem::FunctionCall {
            id: Some("fc-1".to_string()),
            call_id: "call-123".to_string(),
            name: "test_func".to_string(),
            arguments: "{}".to_string(),
        }]);
        assert!(response.has_function_calls());
    }

    #[test]
    fn test_response_has_function_calls_false() {
        let response = make_test_response(vec![OutputItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![],
        }]);
        assert!(!response.has_function_calls());
    }

    #[test]
    fn test_response_has_tool_calls_with_web_search() {
        let response = make_test_response(vec![OutputItem::WebSearchCall {
            id: Some("ws-1".to_string()),
            call_id: "call-ws".to_string(),
            query: Some("test query".to_string()),
            results: None,
            status: Some("completed".to_string()),
        }]);
        assert!(response.has_tool_calls());
    }

    #[test]
    fn test_response_has_tool_calls_with_mcp() {
        let response = make_test_response(vec![OutputItem::McpCall {
            id: Some("mcp-1".to_string()),
            call_id: "call-mcp".to_string(),
            server_label: Some("my-server".to_string()),
            tool_name: Some("my-tool".to_string()),
            arguments: None,
            output: None,
            error: None,
            status: Some("completed".to_string()),
        }]);
        assert!(response.has_tool_calls());
    }

    #[test]
    fn test_response_has_tool_calls_false() {
        let response = make_test_response(vec![OutputItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![],
        }]);
        assert!(!response.has_tool_calls());
    }

    #[test]
    fn test_response_reasoning_present() {
        let response = make_test_response(vec![OutputItem::Reasoning {
            id: Some("r-1".to_string()),
            content: "Let me think about this...".to_string(),
            summary: None,
        }]);
        assert_eq!(response.reasoning(), Some("Let me think about this..."));
    }

    #[test]
    fn test_response_reasoning_absent() {
        let response = make_test_response(vec![OutputItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![],
        }]);
        assert_eq!(response.reasoning(), None);
    }

    #[test]
    fn test_response_cached_tokens() {
        let mut response = make_test_response(vec![]);
        response.usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            input_tokens_details: InputTokensDetails {
                cached_tokens: 75,
                text_tokens: 25,
                image_tokens: 0,
                audio_tokens: 0,
            },
            output_tokens_details: OutputTokensDetails::default(),
        };
        assert_eq!(response.cached_tokens(), 75);
    }

    #[test]
    fn test_response_web_search_calls() {
        let response = make_test_response(vec![OutputItem::WebSearchCall {
            id: Some("ws-1".to_string()),
            call_id: "call-ws".to_string(),
            query: Some("Rust programming".to_string()),
            results: Some(vec![WebSearchResult {
                title: Some("Rust Lang".to_string()),
                url: Some("https://rust-lang.org".to_string()),
                snippet: Some("A language...".to_string()),
            }]),
            status: Some("completed".to_string()),
        }]);
        let calls = response.web_search_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-ws");
        assert_eq!(calls[0].1, Some("Rust programming"));
        assert!(calls[0].2.is_some());
    }

    #[test]
    fn test_response_file_search_calls() {
        let response = make_test_response(vec![OutputItem::FileSearchCall {
            id: Some("fs-1".to_string()),
            call_id: "call-fs".to_string(),
            queries: vec!["config".to_string(), "settings".to_string()],
            results: Some(vec![FileSearchResult {
                file_id: Some("file-123".to_string()),
                filename: Some("config.json".to_string()),
                score: Some(0.95),
                text: Some("config content".to_string()),
            }]),
            status: Some("completed".to_string()),
        }]);
        let calls = response.file_search_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-fs");
        assert_eq!(calls[0].1, &["config".to_string(), "settings".to_string()]);
    }

    #[test]
    fn test_response_computer_calls() {
        let response = make_test_response(vec![OutputItem::ComputerCall {
            id: Some("cc-1".to_string()),
            call_id: "call-cc".to_string(),
            action: ComputerAction::Click {
                x: 100,
                y: 200,
                button: Some("left".to_string()),
            },
            pending_safety_checks: vec![],
            status: Some("completed".to_string()),
        }]);
        let calls = response.computer_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-cc");
        if let ComputerAction::Click { x, y, .. } = calls[0].1 {
            assert_eq!(*x, 100);
            assert_eq!(*y, 200);
        } else {
            panic!("Expected Click action");
        }
    }

    #[test]
    fn test_response_code_interpreter_calls() {
        let response = make_test_response(vec![OutputItem::CodeInterpreterCall {
            id: Some("ci-1".to_string()),
            call_id: "call-ci".to_string(),
            code: Some("print('Hello')".to_string()),
            outputs: Some(vec![CodeInterpreterOutput::Logs {
                logs: "Hello".to_string(),
            }]),
            status: Some("completed".to_string()),
        }]);
        let calls = response.code_interpreter_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-ci");
        assert_eq!(calls[0].1, Some("print('Hello')"));
    }

    #[test]
    fn test_response_mcp_calls() {
        let response = make_test_response(vec![OutputItem::McpCall {
            id: Some("mcp-1".to_string()),
            call_id: "call-mcp".to_string(),
            server_label: Some("my-server".to_string()),
            tool_name: Some("my-tool".to_string()),
            arguments: Some(serde_json::json!({"key": "value"})),
            output: Some("result".to_string()),
            error: None,
            status: Some("completed".to_string()),
        }]);
        let calls = response.mcp_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].call_id, "call-mcp");
        assert_eq!(calls[0].server_label, Some("my-server"));
        assert_eq!(calls[0].tool_name, Some("my-tool"));
        assert_eq!(calls[0].output, Some("result"));
    }

    #[test]
    fn test_response_image_generation_calls() {
        let response = make_test_response(vec![OutputItem::ImageGenerationCall {
            id: Some("ig-1".to_string()),
            call_id: "call-ig".to_string(),
            prompt: Some("A sunset over mountains".to_string()),
            result: Some(ImageGenerationResult {
                url: Some("https://example.com/image.png".to_string()),
                b64_json: None,
                revised_prompt: Some("A beautiful sunset...".to_string()),
            }),
            status: Some("completed".to_string()),
        }]);
        let calls = response.image_generation_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-ig");
        assert_eq!(calls[0].1, Some("A sunset over mountains"));
        assert!(calls[0].2.is_some());
    }

    #[test]
    fn test_response_local_shell_calls() {
        let response = make_test_response(vec![OutputItem::LocalShellCall {
            id: Some("ls-1".to_string()),
            call_id: "call-ls".to_string(),
            command: Some("ls -la".to_string()),
            output: Some("file1.txt\nfile2.txt".to_string()),
            status: Some("completed".to_string()),
        }]);
        let calls = response.local_shell_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-ls");
        assert_eq!(calls[0].1, Some("ls -la"));
        assert_eq!(calls[0].2, Some("file1.txt\nfile2.txt"));
    }

    // ========================================================================
    // Response deserialization tests
    // ========================================================================

    #[test]
    fn test_deserialize_response_completed_with_message() {
        let json = r#"{
            "id": "resp-abc123",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from the API!"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            },
            "model": "gpt-4o"
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "resp-abc123");
        assert_eq!(response.status, ResponseStatus::Completed);
        assert_eq!(response.text(), "Hello from the API!");
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }

    #[test]
    fn test_deserialize_response_with_function_call() {
        let json = r#"{
            "id": "resp-func123",
            "status": "completed",
            "output": [
                {
                    "type": "function_call",
                    "id": "fc-1",
                    "call_id": "call-abc",
                    "name": "get_weather",
                    "arguments": "{\"city\":\"Tokyo\"}"
                }
            ],
            "usage": {
                "input_tokens": 20,
                "output_tokens": 10,
                "total_tokens": 30
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert!(response.has_function_calls());
        let calls = response.function_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call-abc");
        assert_eq!(calls[0].1, "get_weather");
        assert_eq!(calls[0].2, r#"{"city":"Tokyo"}"#);
    }

    #[test]
    fn test_deserialize_response_with_reasoning() {
        let json = r#"{
            "id": "resp-reason123",
            "status": "completed",
            "output": [
                {
                    "type": "reasoning",
                    "id": "r-1",
                    "content": "Let me analyze this step by step..."
                },
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "The answer is 42."
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 50,
                "output_tokens": 100,
                "total_tokens": 150,
                "output_tokens_details": {
                    "reasoning_tokens": 80,
                    "text_tokens": 20
                }
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(
            response.reasoning(),
            Some("Let me analyze this step by step...")
        );
        assert_eq!(response.text(), "The answer is 42.");
        assert_eq!(response.usage.reasoning_tokens(), 80);
    }

    #[test]
    fn test_deserialize_response_with_web_search() {
        let json = r#"{
            "id": "resp-ws123",
            "status": "completed",
            "output": [
                {
                    "type": "web_search_call",
                    "id": "ws-1",
                    "call_id": "call-ws",
                    "query": "Rust programming language",
                    "results": [
                        {
                            "title": "Rust Programming Language",
                            "url": "https://rust-lang.org",
                            "snippet": "A language empowering..."
                        }
                    ],
                    "status": "completed"
                }
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert!(response.has_tool_calls());
        let calls = response.web_search_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, Some("Rust programming language"));
    }

    #[test]
    fn test_deserialize_response_failed() {
        let json = r#"{
            "id": "resp-failed123",
            "status": "failed",
            "output": [],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 0,
                "total_tokens": 10
            },
            "error": {
                "code": "content_filter",
                "message": "Content was filtered"
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, ResponseStatus::Failed);
        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, "content_filter");
    }

    #[test]
    fn test_deserialize_response_incomplete() {
        let json = r#"{
            "id": "resp-incomplete123",
            "status": "incomplete",
            "output": [
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "This response was truncated..."
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 100,
                "output_tokens": 4096,
                "total_tokens": 4196
            },
            "incomplete_details": {
                "reason": "max_output_tokens"
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, ResponseStatus::Incomplete);
        assert!(response.incomplete_details.is_some());
        assert_eq!(
            response.incomplete_details.as_ref().unwrap().reason,
            Some(IncompleteReason::MaxOutputTokens)
        );
    }

    #[test]
    fn test_deserialize_response_with_cached_tokens() {
        let json = r#"{
            "id": "resp-cached123",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Response with cache hit"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 50,
                "total_tokens": 1050,
                "input_tokens_details": {
                    "cached_tokens": 950,
                    "text_tokens": 50
                }
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.cached_tokens(), 950);
        assert_eq!(response.usage.input_text_tokens(), 50);
    }
}
