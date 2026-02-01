//! Prompt handler: produces a modified prompt from a template.
//!
//! The template can contain `$ARGUMENTS` which is replaced with the JSON
//! representation of the arguments value.
//!
//! ## Two Modes of Operation
//!
//! 1. **Template Mode** (current implementation): Simply expands `$ARGUMENTS`
//!    in the template and returns `ModifyInput`.
//!
//! 2. **LLM Verification Mode** (future): Queries an LLM to verify whether
//!    the action should proceed, expecting a JSON response like:
//!    ```json
//!    { "ok": true }
//!    { "ok": false, "reason": "Not allowed because..." }
//!    ```
//!
//! LLM verification mode requires hyper-sdk integration and will be
//! implemented when an LLM client interface is available.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tracing::debug;

use crate::context::HookContext;
use crate::result::HookResult;

/// Response format expected from LLM verification.
///
/// When a prompt hook uses LLM verification mode, the model should return
/// a JSON object with this structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmVerificationResponse {
    /// Whether the action is approved.
    pub ok: bool,
    /// Reason for rejection (if ok is false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Configuration for LLM-based prompt verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVerificationConfig {
    /// System prompt to use for the verification LLM call.
    pub system_prompt: String,
    /// Model to use for verification (if None, uses default).
    pub model: Option<String>,
    /// Maximum tokens for the response.
    pub max_tokens: i32,
}

impl Default for PromptVerificationConfig {
    fn default() -> Self {
        Self {
            system_prompt: String::from(
                "You are a verification system. Analyze the request and respond with JSON: \
                 { \"ok\": true } to approve or { \"ok\": false, \"reason\": \"...\" } to reject.",
            ),
            model: None,
            max_tokens: 100,
        }
    }
}

/// Handles hooks that inject prompt templates or perform LLM verification.
pub struct PromptHandler;

impl PromptHandler {
    /// Template-mode execution: replaces `$ARGUMENTS` in the template with the
    /// serialized JSON of `arguments`, then returns a `ModifyInput` result.
    ///
    /// This is the simple, non-LLM mode that just expands placeholders.
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

    /// LLM verification mode: queries an LLM to verify the action.
    ///
    /// This method prepares the verification request but does not actually
    /// call the LLM. The caller must provide an LLM query function.
    ///
    /// # Arguments
    /// * `template` - Template with `$ARGUMENTS` placeholder
    /// * `ctx` - Hook execution context
    /// * `config` - Verification configuration
    ///
    /// # Returns
    /// A tuple of (system_prompt, user_message) to send to the LLM.
    pub fn prepare_verification_request(
        template: &str,
        ctx: &HookContext,
        config: &PromptVerificationConfig,
    ) -> (String, String) {
        let ctx_json = serde_json::to_string_pretty(ctx).unwrap_or_else(|_| "{}".to_string());
        let user_message = template.replace("$ARGUMENTS", &ctx_json);

        (config.system_prompt.clone(), user_message)
    }

    /// Parses an LLM verification response.
    ///
    /// # Arguments
    /// * `response` - The raw response text from the LLM
    ///
    /// # Returns
    /// * `HookResult::Continue` if approved
    /// * `HookResult::Reject` if rejected with reason
    pub fn parse_verification_response(response: &str) -> HookResult {
        // Try to extract JSON from the response
        let trimmed = response.trim();

        // Try to parse as-is first
        if let Ok(resp) = serde_json::from_str::<LlmVerificationResponse>(trimmed) {
            return Self::response_to_result(resp);
        }

        // Try to find JSON in the response (LLM might add explanation around it)
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                let json_str = &trimmed[start..=end];
                if let Ok(resp) = serde_json::from_str::<LlmVerificationResponse>(json_str) {
                    return Self::response_to_result(resp);
                }
            }
        }

        // Failed to parse - log and continue
        tracing::warn!(
            response = %response,
            "Failed to parse LLM verification response, allowing action"
        );
        HookResult::Continue
    }

    fn response_to_result(resp: LlmVerificationResponse) -> HookResult {
        if resp.ok {
            HookResult::Continue
        } else {
            HookResult::Reject {
                reason: resp
                    .reason
                    .unwrap_or_else(|| "Verification rejected by hook".to_string()),
            }
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

    #[test]
    fn test_llm_verification_response_ok() {
        let response = r#"{"ok": true}"#;
        let result = PromptHandler::parse_verification_response(response);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_llm_verification_response_reject() {
        let response = r#"{"ok": false, "reason": "Not allowed"}"#;
        let result = PromptHandler::parse_verification_response(response);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "Not allowed");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_llm_verification_response_reject_no_reason() {
        let response = r#"{"ok": false}"#;
        let result = PromptHandler::parse_verification_response(response);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "Verification rejected by hook");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_llm_verification_response_with_extra_text() {
        let response = "I've analyzed the request and determined: {\"ok\": true}";
        let result = PromptHandler::parse_verification_response(response);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_llm_verification_response_invalid() {
        let response = "This is not JSON at all";
        let result = PromptHandler::parse_verification_response(response);
        // Should fail-open with Continue
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_verification_config_default() {
        let config = PromptVerificationConfig::default();
        assert!(!config.system_prompt.is_empty());
        assert!(config.model.is_none());
        assert_eq!(config.max_tokens, 100);
    }

    #[test]
    fn test_llm_verification_response_serde() {
        let resp = LlmVerificationResponse {
            ok: false,
            reason: Some("Test reason".to_string()),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        let parsed: LlmVerificationResponse = serde_json::from_str(&json).expect("parse");
        assert!(!parsed.ok);
        assert_eq!(parsed.reason, Some("Test reason".to_string()));
    }
}
