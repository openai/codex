//! Common types for Z.AI SDK.

use serde::Deserialize;
use serde::Serialize;

/// Conversation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Reason why the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    #[serde(other)]
    Other,
}

/// Function call information (from Python SDK `chat_completion.py:6`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function call arguments (JSON string).
    pub arguments: String,
    /// Function name.
    pub name: String,
}

/// Function definition for tool use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    /// The name of the function.
    pub name: String,
    /// Description of what the function does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the function's parameters.
    pub parameters: serde_json::Value,
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Tool {
    /// Function tool.
    Function { function: FunctionDef },
}

impl Tool {
    /// Create a function tool.
    pub fn function(
        name: impl Into<String>,
        description: Option<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self::Function {
            function: FunctionDef {
                name: name.into(),
                description,
                parameters,
            },
        }
    }
}

/// How the model should use tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// String mode: "auto", "none", "required".
    Mode(String),
    /// Specific function.
    Function {
        #[serde(rename = "type")]
        choice_type: String,
        function: ToolChoiceFunction,
    },
}

/// Function specification for tool choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    /// Function name.
    pub name: String,
}

impl ToolChoice {
    /// Auto mode - model decides whether to use tools.
    pub fn auto() -> Self {
        Self::Mode("auto".into())
    }

    /// None mode - don't use any tools.
    pub fn none() -> Self {
        Self::Mode("none".into())
    }

    /// Required mode - must use at least one tool.
    pub fn required() -> Self {
        Self::Mode("required".into())
    }

    /// Force use of a specific function.
    pub fn function(name: impl Into<String>) -> Self {
        Self::Function {
            choice_type: "function".into(),
            function: ToolChoiceFunction { name: name.into() },
        }
    }
}

// ============================================================================
// SDK HTTP Response (runtime metadata)
// ============================================================================

/// HTTP response metadata for debugging and inspection.
///
/// This struct captures the raw HTTP response information that is not part of
/// the API response body. It's populated by the client after receiving a response.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SdkHttpResponse {
    /// HTTP status code.
    pub status_code: Option<i32>,
    /// Response headers.
    pub headers: Option<std::collections::HashMap<String, String>>,
    /// Raw response body (for debugging).
    pub body: Option<String>,
}

impl SdkHttpResponse {
    /// Create a new SdkHttpResponse with all fields.
    pub fn new(
        status_code: i32,
        headers: std::collections::HashMap<String, String>,
        body: String,
    ) -> Self {
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
