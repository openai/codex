//! Hook output schemas aligned with Claude Code.

use serde::Deserialize;
use serde::Serialize;

/// Permission decision (aligned with Claude Code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Auto-approve the permission request.
    Allow,
    /// Auto-deny the permission request (blocking).
    Deny,
    /// Prompt user as normal (default behavior).
    #[default]
    Ask,
}

/// Async response marker (aligned with Claude Code asyncResponseSchema).
///
/// When a hook returns this as its first JSON output, the hook is backgrounded
/// and execution continues immediately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncResponse {
    /// Must be `true` to indicate async execution.
    #[serde(rename = "async")]
    pub is_async: bool,

    /// Optional timeout for the background task in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub async_timeout: Option<u64>,
}

impl AsyncResponse {
    /// Check if a JSON value is an async response.
    pub fn is_async_response(value: &serde_json::Value) -> bool {
        value
            .get("async")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// Synchronous hook output (aligned with Claude Code syncResponseSchema).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    // =========================================================================
    // Flow control
    // =========================================================================
    /// Set to `false` to stop execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#continue: Option<bool>,

    /// Set to `true` to hide output from user (stderr still shown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,

    /// Reason for stopping execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    // =========================================================================
    // Permission decision (deprecated in Claude Code, use hookSpecificOutput)
    // =========================================================================
    /// Decision: "approve" or "block".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,

    /// Reason for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    // =========================================================================
    // Context injection
    // =========================================================================
    /// System message to add to conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,

    // =========================================================================
    // Event-specific output
    // =========================================================================
    /// Event-specific output fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

impl HookOutput {
    /// Create a success output with no modifications.
    pub fn success() -> Self {
        Self::default()
    }

    /// Create an output that blocks execution.
    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            r#continue: Some(false),
            stop_reason: Some(reason.into()),
            ..Default::default()
        }
    }

    /// Create an output with a system message.
    pub fn with_system_message(message: impl Into<String>) -> Self {
        Self {
            system_message: Some(message.into()),
            ..Default::default()
        }
    }

    /// Check if this output indicates blocking.
    pub fn is_blocking(&self) -> bool {
        self.r#continue == Some(false) || self.decision.as_deref() == Some("block")
    }

    /// Get the permission decision from this output.
    pub fn permission_decision(&self) -> Option<PermissionDecision> {
        // Check hookSpecificOutput first
        if let Some(ref specific) = self.hook_specific_output {
            match specific {
                HookSpecificOutput::PreToolUse {
                    permission_decision,
                    ..
                } => {
                    return *permission_decision;
                }
                HookSpecificOutput::PermissionRequest { decision } => {
                    return Some(match decision {
                        PermissionRequestDecision::Allow { .. } => PermissionDecision::Allow,
                        PermissionRequestDecision::Deny { .. } => PermissionDecision::Deny,
                    });
                }
                _ => {}
            }
        }

        // Fall back to deprecated decision field
        match self.decision.as_deref() {
            Some("approve") => Some(PermissionDecision::Allow),
            Some("block") => Some(PermissionDecision::Deny),
            _ => None,
        }
    }

    /// Get additional context from this output.
    pub fn additional_context(&self) -> Option<&str> {
        self.hook_specific_output.as_ref().and_then(|s| match s {
            HookSpecificOutput::PostToolUse {
                additional_context, ..
            }
            | HookSpecificOutput::PostToolUseFailure { additional_context }
            | HookSpecificOutput::UserPromptSubmit { additional_context }
            | HookSpecificOutput::SessionStart { additional_context }
            | HookSpecificOutput::SubagentStart { additional_context } => {
                additional_context.as_deref()
            }
            _ => None,
        })
    }

    /// Get updated input from this output.
    pub fn updated_input(&self) -> Option<&serde_json::Value> {
        self.hook_specific_output.as_ref().and_then(|s| match s {
            HookSpecificOutput::PreToolUse { updated_input, .. } => updated_input.as_ref(),
            HookSpecificOutput::PermissionRequest { decision } => match decision {
                PermissionRequestDecision::Allow { updated_input, .. } => updated_input.as_ref(),
                _ => None,
            },
            _ => None,
        })
    }
}

/// Event-specific output fields (aligned with Claude Code hookSpecificOutput).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hookEventName")]
pub enum HookSpecificOutput {
    /// PreToolUse output.
    PreToolUse {
        /// Permission override decision.
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_decision: Option<PermissionDecision>,

        /// Reason for permission decision.
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_decision_reason: Option<String>,

        /// Modified tool input.
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,
    },

    /// PostToolUse output.
    PostToolUse {
        /// Context to inject into conversation.
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,

        /// Modified MCP tool output.
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_mcp_tool_output: Option<serde_json::Value>,
    },

    /// PostToolUseFailure output.
    PostToolUseFailure {
        /// Context about the failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },

    /// UserPromptSubmit output.
    UserPromptSubmit {
        /// Context to inject before processing.
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },

    /// SessionStart output.
    SessionStart {
        /// Initial context for the session.
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },

    /// SessionEnd output (no additional fields - informational only).
    SessionEnd,

    /// SubagentStart output.
    SubagentStart {
        /// Context for the subagent.
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },

    /// SubagentStop output (no additional fields).
    SubagentStop,

    /// Stop output (control via continue/stopReason).
    Stop,

    /// Notification output (no additional output fields).
    Notification,

    /// PreCompact output.
    PreCompact {
        /// Instructions to append to existing custom instructions.
        #[serde(skip_serializing_if = "Option::is_none")]
        new_custom_instructions: Option<String>,

        /// Message to show during compaction.
        #[serde(skip_serializing_if = "Option::is_none")]
        user_display_message: Option<String>,
    },

    /// PermissionRequest output.
    PermissionRequest {
        /// The permission decision.
        decision: PermissionRequestDecision,
    },
}

/// PermissionRequest decision (aligned with Claude Code).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "lowercase")]
pub enum PermissionRequestDecision {
    /// Allow the permission request.
    Allow {
        /// Modified tool input.
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<serde_json::Value>,

        /// Modified permissions.
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<serde_json::Value>>,
    },

    /// Deny the permission request.
    Deny {
        /// Error message for the denial.
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,

        /// Whether to interrupt execution.
        #[serde(skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
    },
}

/// Hook execution outcome (aligned with Claude Code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookOutcome {
    /// Exit code 0, or callback returned successfully.
    Success,
    /// Exit code 2, or {ok: false} from prompt/agent.
    Blocking,
    /// Exit code 1, 3+, or validation error.
    NonBlockingError,
    /// Timeout or abort signal.
    Cancelled,
}

impl std::fmt::Display for HookOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Blocking => write!(f, "blocking"),
            Self::NonBlockingError => write!(f, "non_blocking_error"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Internal result structure for hook execution.
#[derive(Debug, Clone)]
pub struct HookResult {
    /// The execution outcome.
    pub outcome: HookOutcome,

    /// The parsed hook output.
    pub output: Option<HookOutput>,

    /// Blocking error details.
    pub blocking_error: Option<BlockingError>,

    /// Raw stdout from command hooks.
    pub stdout: Option<String>,

    /// Raw stderr from command hooks.
    pub stderr: Option<String>,

    /// Exit code from command hooks.
    pub exit_code: Option<i32>,
}

impl HookResult {
    /// Create a success result.
    pub fn success(output: HookOutput) -> Self {
        Self {
            outcome: HookOutcome::Success,
            output: Some(output),
            blocking_error: None,
            stdout: None,
            stderr: None,
            exit_code: None,
        }
    }

    /// Create a blocking result.
    pub fn blocking(error: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            outcome: HookOutcome::Blocking,
            output: None,
            blocking_error: Some(BlockingError {
                error: error.into(),
                command: command.into(),
            }),
            stdout: None,
            stderr: None,
            exit_code: None,
        }
    }

    /// Create a cancelled result.
    pub fn cancelled() -> Self {
        Self {
            outcome: HookOutcome::Cancelled,
            output: None,
            blocking_error: None,
            stdout: None,
            stderr: None,
            exit_code: None,
        }
    }

    /// Create a non-blocking error result.
    pub fn non_blocking_error(stderr: impl Into<String>) -> Self {
        Self {
            outcome: HookOutcome::NonBlockingError,
            output: None,
            blocking_error: None,
            stdout: None,
            stderr: Some(stderr.into()),
            exit_code: None,
        }
    }
}

/// Blocking error details.
#[derive(Debug, Clone)]
pub struct BlockingError {
    /// The error message.
    pub error: String,

    /// The command or hook that caused the error.
    pub command: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_output_default() {
        let output = HookOutput::default();
        assert!(!output.is_blocking());
        assert!(output.permission_decision().is_none());
    }

    #[test]
    fn test_hook_output_block() {
        let output = HookOutput::block("Security violation");
        assert!(output.is_blocking());
        assert_eq!(output.stop_reason, Some("Security violation".to_string()));
    }

    #[test]
    fn test_async_response_detection() {
        let async_json = serde_json::json!({"async": true, "asyncTimeout": 15000});
        assert!(AsyncResponse::is_async_response(&async_json));

        let sync_json = serde_json::json!({"continue": true});
        assert!(!AsyncResponse::is_async_response(&sync_json));
    }

    #[test]
    fn test_permission_decision_extraction() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some(PermissionDecision::Allow),
                permission_decision_reason: Some("Whitelisted".to_string()),
                updated_input: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            output.permission_decision(),
            Some(PermissionDecision::Allow)
        );
    }

    #[test]
    fn test_hook_outcome_display() {
        assert_eq!(HookOutcome::Success.to_string(), "success");
        assert_eq!(HookOutcome::Blocking.to_string(), "blocking");
    }
}
