//! Think Tool Handler
//!
//! A no-op tool that logs thoughts for transparency and complex reasoning.
//! Does not obtain new information or make any changes.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use serde::Deserialize;

/// Think tool arguments
#[derive(Debug, Clone, Deserialize)]
struct ThinkArgs {
    thought: String,
}

/// Think Tool Handler
///
/// This is a read-only, no-op tool that simply logs thoughts.
/// It is safe for concurrent execution and requires no permissions.
pub struct ThinkHandler;

#[async_trait]
impl ToolHandler for ThinkHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for think".to_string(),
                ));
            }
        };

        let args: ThinkArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Validate thought is not empty
        if args.thought.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "Thought must not be empty".to_string(),
            ));
        }

        // Simply acknowledge the thought was logged
        // The thought content is already captured in the tool call itself,
        // so we don't need to repeat it in the response
        Ok(ToolOutput::Function {
            content: "Your thought has been logged.".to_string(),
            content_items: None,
            success: Some(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_kind() {
        let handler = ThinkHandler;
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let handler = ThinkHandler;

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_parse_valid_args() {
        let args: ThinkArgs =
            serde_json::from_str(r#"{"thought": "I should check the database schema first"}"#)
                .expect("should parse");
        assert_eq!(args.thought, "I should check the database schema first");
    }

    #[test]
    fn test_parse_invalid_args_missing_thought() {
        let result: Result<ThinkArgs, _> = serde_json::from_str(r#"{"invalid": "json"}"#);
        assert!(result.is_err());
    }
}
