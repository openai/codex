//! Common types used across the Volcengine Ark SDK.

use serde::Deserialize;
use serde::Serialize;

use crate::error::ArkError;
use crate::error::Result;

/// Conversation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User role.
    User,
    /// Assistant role.
    Assistant,
    /// System role.
    System,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of turn.
    EndTurn,
    /// Maximum tokens reached.
    MaxTokens,
    /// Stop sequence matched.
    StopSequence,
    /// Tool use requested.
    ToolUse,
}

/// Response status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    /// Response completed successfully.
    Completed,
    /// Response is in progress.
    InProgress,
    /// Response is incomplete.
    Incomplete,
    /// Response failed.
    Failed,
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Type of the tool (always "function").
    #[serde(rename = "type")]
    pub tool_type: String,

    /// Function definition.
    pub function: FunctionDefinition,
}

/// Function definition for a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// Name of the function.
    pub name: String,

    /// Description of the function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// JSON Schema for the function parameters.
    pub parameters: serde_json::Value,
}

impl Tool {
    /// Create a new function tool.
    pub fn function(
        name: impl Into<String>,
        description: Option<String>,
        parameters: serde_json::Value,
    ) -> Result<Self> {
        let name = name.into();
        if name.is_empty() || name.len() > 64 {
            return Err(ArkError::Validation(
                "function name must be 1-64 characters".to_string(),
            ));
        }
        Ok(Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name,
                description,
                parameters,
            },
        })
    }
}

/// Tool choice configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let the model decide whether to use tools.
    Auto,
    /// Do not use any tools.
    None,
    /// Require the model to use a tool.
    Required,
    /// Force use of a specific function.
    Function {
        /// Name of the function to call.
        name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), r#""user""#);
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            r#""assistant""#
        );
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), r#""system""#);
    }

    #[test]
    fn test_tool_creation() {
        let tool = Tool::function(
            "get_weather",
            Some("Get the weather".to_string()),
            serde_json::json!({"type": "object", "properties": {}}),
        );
        assert!(tool.is_ok());

        // Empty name should fail
        let tool = Tool::function(
            "",
            None,
            serde_json::json!({"type": "object", "properties": {}}),
        );
        assert!(tool.is_err());
    }

    #[test]
    fn test_tool_choice_serialization() {
        let auto = serde_json::to_string(&ToolChoice::Auto).unwrap();
        assert!(auto.contains(r#""type":"auto""#));

        let func = serde_json::to_string(&ToolChoice::Function {
            name: "test".to_string(),
        })
        .unwrap();
        assert!(func.contains(r#""type":"function""#));
        assert!(func.contains(r#""name":"test""#));
    }
}
