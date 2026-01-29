//! Prompt handler: produces a modified prompt from a template.
//!
//! The template can contain `$ARGUMENTS` which is replaced with the JSON
//! representation of the arguments value.

use serde_json::Value;
use tracing::debug;

use crate::result::HookResult;

/// Handles hooks that inject prompt templates.
pub struct PromptHandler;

impl PromptHandler {
    /// Replaces `$ARGUMENTS` in the template with the serialized JSON of
    /// `arguments`, then returns a `ModifyInput` result with the expanded
    /// text.
    pub fn execute(template: &str, arguments: &Value) -> HookResult {
        let args_str = match serde_json::to_string(arguments) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to serialize arguments for prompt template: {e}");
                String::from("null")
            }
        };

        let expanded = template.replace("$ARGUMENTS", &args_str);
        debug!(template, expanded = %expanded, "Prompt hook expanded");

        HookResult::ModifyInput {
            new_input: Value::String(expanded),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_substitution() {
        let result =
            PromptHandler::execute("Review: $ARGUMENTS", &serde_json::json!({"file": "a.rs"}));
        if let HookResult::ModifyInput { new_input } = result {
            let text = new_input.as_str().expect("should be a string");
            assert!(text.starts_with("Review: "));
            assert!(text.contains("a.rs"));
        } else {
            panic!("Expected ModifyInput");
        }
    }

    #[test]
    fn test_no_placeholder() {
        let result = PromptHandler::execute("no placeholder here", &serde_json::json!({}));
        if let HookResult::ModifyInput { new_input } = result {
            assert_eq!(new_input.as_str().expect("string"), "no placeholder here");
        } else {
            panic!("Expected ModifyInput");
        }
    }

    #[test]
    fn test_null_arguments() {
        let result = PromptHandler::execute("args=$ARGUMENTS", &Value::Null);
        if let HookResult::ModifyInput { new_input } = result {
            assert_eq!(new_input.as_str().expect("string"), "args=null");
        } else {
            panic!("Expected ModifyInput");
        }
    }

    #[test]
    fn test_multiple_placeholders() {
        let result = PromptHandler::execute(
            "first=$ARGUMENTS second=$ARGUMENTS",
            &serde_json::json!("data"),
        );
        if let HookResult::ModifyInput { new_input } = result {
            let text = new_input.as_str().expect("string");
            // Both occurrences should be replaced
            assert_eq!(text, "first=\"data\" second=\"data\"");
        } else {
            panic!("Expected ModifyInput");
        }
    }
}
