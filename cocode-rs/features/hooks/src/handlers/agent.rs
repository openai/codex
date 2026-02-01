//! Agent handler for hook verification.
//!
//! Delegates execution to a sub-agent with limited tools (Read, Grep, Glob)
//! to verify whether an action should proceed.
//!
//! ## Design
//!
//! When an agent hook is triggered:
//! 1. A sub-agent is spawned with restricted tools
//! 2. The sub-agent analyzes the request (up to `max_turns` turns)
//! 3. The sub-agent returns a structured JSON response
//! 4. The handler parses the response to determine Continue/Reject
//!
//! ## Response Format
//!
//! The sub-agent should output JSON:
//! ```json
//! { "ok": true }
//! { "ok": false, "reason": "The file contains sensitive data" }
//! ```
//!
//! ## Limitations
//!
//! - Max 50 turns (configurable via `max_turns`)
//! - Limited tools: Read, Grep, Glob only
//! - Timeout is enforced at the registry level
//! - If the agent times out or fails to respond, action continues (fail-open)

use serde::Deserialize;
use serde::Serialize;
use tracing::debug;

use crate::context::HookContext;
use crate::result::HookResult;

/// Response format expected from agent verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVerificationResponse {
    /// Whether the action is approved.
    pub ok: bool,
    /// Reason for rejection (if ok is false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Configuration for agent-based verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVerificationConfig {
    /// Maximum number of agent turns.
    pub max_turns: i32,
    /// System prompt for the verification agent.
    pub system_prompt: String,
    /// Tools available to the agent (subset of all tools).
    pub allowed_tools: Vec<String>,
}

impl Default for AgentVerificationConfig {
    fn default() -> Self {
        Self {
            max_turns: 50,
            system_prompt: String::from(
                "You are a verification agent. Analyze the request and determine if it should proceed. \
                 You have access to Read, Grep, and Glob tools to investigate the codebase. \
                 After analysis, output a JSON response: { \"ok\": true } to approve or \
                 { \"ok\": false, \"reason\": \"...\" } to reject.",
            ),
            allowed_tools: vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()],
        }
    }
}

/// Handles hooks that delegate to a verification sub-agent.
pub struct AgentHandler;

impl AgentHandler {
    /// Synchronous stub implementation. Returns `Continue`.
    ///
    /// This is the fallback when async agent spawning isn't available.
    pub fn execute(max_turns: i32) -> HookResult {
        debug!(max_turns, "Agent hook stub invoked (not yet implemented)");
        HookResult::Continue
    }

    /// Prepares the agent verification request.
    ///
    /// Returns the configuration and initial prompt for the sub-agent.
    pub fn prepare_verification_request(
        ctx: &HookContext,
        max_turns: i32,
    ) -> (AgentVerificationConfig, String) {
        let mut config = AgentVerificationConfig::default();
        config.max_turns = max_turns;

        let ctx_json = serde_json::to_string_pretty(ctx).unwrap_or_else(|_| "{}".to_string());
        let user_message = format!(
            "Please analyze the following hook context and determine if this action should proceed:\n\n```json\n{}\n```",
            ctx_json
        );

        (config, user_message)
    }

    /// Parses an agent verification response.
    ///
    /// The agent's final output should contain JSON with `ok` and optional `reason`.
    pub fn parse_verification_response(response: &str) -> HookResult {
        let trimmed = response.trim();

        // Try to extract JSON from the response
        if let Some(start) = trimmed.rfind('{') {
            if let Some(end) = trimmed.rfind('}') {
                let json_str = &trimmed[start..=end];
                if let Ok(resp) = serde_json::from_str::<AgentVerificationResponse>(json_str) {
                    return Self::response_to_result(resp);
                }
            }
        }

        // Try parsing the entire response as JSON
        if let Ok(resp) = serde_json::from_str::<AgentVerificationResponse>(trimmed) {
            return Self::response_to_result(resp);
        }

        // Failed to parse - fail open with warning
        tracing::warn!(
            response = %response,
            "Failed to parse agent verification response, allowing action"
        );
        HookResult::Continue
    }

    fn response_to_result(resp: AgentVerificationResponse) -> HookResult {
        if resp.ok {
            debug!("Agent verification approved");
            HookResult::Continue
        } else {
            debug!(reason = ?resp.reason, "Agent verification rejected");
            HookResult::Reject {
                reason: resp
                    .reason
                    .unwrap_or_else(|| "Verification rejected by agent".to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_returns_continue() {
        let result = AgentHandler::execute(5);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_stub_with_different_turns() {
        let result = AgentHandler::execute(10);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_agent_verification_response_ok() {
        let response = r#"{"ok": true}"#;
        let result = AgentHandler::parse_verification_response(response);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_agent_verification_response_reject() {
        let response = r#"{"ok": false, "reason": "Dangerous operation"}"#;
        let result = AgentHandler::parse_verification_response(response);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "Dangerous operation");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_agent_verification_response_reject_no_reason() {
        let response = r#"{"ok": false}"#;
        let result = AgentHandler::parse_verification_response(response);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "Verification rejected by agent");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_agent_verification_response_at_end() {
        // Agent might output analysis followed by JSON
        let response = "After analyzing the files, I found no issues.\n{\"ok\": true}";
        let result = AgentHandler::parse_verification_response(response);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_agent_verification_response_invalid() {
        let response = "I could not determine a verdict";
        let result = AgentHandler::parse_verification_response(response);
        // Should fail-open
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_agent_verification_config_default() {
        let config = AgentVerificationConfig::default();
        assert_eq!(config.max_turns, 50);
        assert!(!config.system_prompt.is_empty());
        assert_eq!(
            config.allowed_tools,
            vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]
        );
    }

    #[test]
    fn test_agent_verification_response_serde() {
        let resp = AgentVerificationResponse {
            ok: true,
            reason: None,
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"ok\":true"));
        assert!(!json.contains("reason")); // Skipped when None

        let resp2 = AgentVerificationResponse {
            ok: false,
            reason: Some("Test".to_string()),
        };
        let json2 = serde_json::to_string(&resp2).expect("serialize");
        let parsed: AgentVerificationResponse = serde_json::from_str(&json2).expect("parse");
        assert!(!parsed.ok);
        assert_eq!(parsed.reason, Some("Test".to_string()));
    }
}
