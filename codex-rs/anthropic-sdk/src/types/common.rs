use serde::Deserialize;
use serde::Serialize;

use super::CacheControl;
use crate::error::AnthropicError;
use crate::error::Result;

/// Conversation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Reason why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of turn.
    EndTurn,
    /// Reached max_tokens limit.
    MaxTokens,
    /// Custom stop sequence matched.
    StopSequence,
    /// Model invoked a tool.
    ToolUse,
    /// Long-running turn paused.
    PauseTurn,
    /// Content was refused due to policy.
    Refusal,
}

/// Request metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    /// External identifier for the user making the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Maximum length for tool names.
pub const MAX_TOOL_NAME_LENGTH: usize = 64;

/// Tool definition for tool use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// The name of the tool (must match regex `^[a-zA-Z0-9_-]{1,64}$`).
    pub name: String,

    /// Description of what the tool does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,

    /// Cache control settings for the tool definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl Tool {
    /// Validate a tool name against the pattern `^[a-zA-Z0-9_-]{1,64}$`.
    fn is_valid_tool_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= MAX_TOOL_NAME_LENGTH
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    /// Create a new tool with validation.
    ///
    /// Returns an error if the name doesn't match `^[a-zA-Z0-9_-]{1,64}$`.
    pub fn new(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: serde_json::Value,
    ) -> Result<Self> {
        let name = name.into();
        if !Self::is_valid_tool_name(&name) {
            return Err(AnthropicError::Validation(format!(
                "tool name must match pattern ^[a-zA-Z0-9_-]{{1,64}}$, got '{name}'"
            )));
        }
        Ok(Self {
            name,
            description,
            input_schema,
            cache_control: None,
        })
    }

    /// Create a new tool with validation and cache control.
    pub fn new_with_cache(
        name: impl Into<String>,
        description: Option<String>,
        input_schema: serde_json::Value,
        cache_control: CacheControl,
    ) -> Result<Self> {
        let name = name.into();
        if !Self::is_valid_tool_name(&name) {
            return Err(AnthropicError::Validation(format!(
                "tool name must match pattern ^[a-zA-Z0-9_-]{{1,64}}$, got '{name}'"
            )));
        }
        Ok(Self {
            name,
            description,
            input_schema,
            cache_control: Some(cache_control),
        })
    }
}

/// How the model should use tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Model decides whether to use tools.
    Auto {
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// Model must use one of the provided tools.
    Any {
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// Model must use the specified tool.
    Tool {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// Model will not use any tools.
    None,
}
