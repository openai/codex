//! Subagent-specific error types.

use crate::error::CodexErr;
use thiserror::Error;

/// Errors specific to subagent execution.
#[derive(Debug, Error)]
pub enum SubagentErr {
    #[error("Unknown agent type: {0}")]
    UnknownAgentType(String),

    #[error("Agent definition parse error: {0}")]
    ParseError(String),

    #[error("Tool '{0}' is not available in this subagent context")]
    ToolRejected(String),

    #[error("Model error: {0}")]
    ModelError(String),

    #[error("Transcript not found for agent: {0}")]
    TranscriptNotFound(String),

    #[error("Approval request timed out")]
    ApprovalTimeout,

    #[error("Subagent execution was cancelled")]
    Cancelled,

    #[error("Output validation failed: {0}")]
    OutputValidationError(String),

    #[error("Max turns ({0}) exceeded")]
    MaxTurnsExceeded(i32),

    #[error("Timeout after {0} seconds")]
    Timeout(i32),

    #[error("Agent did not call complete_task")]
    NoCompleteTaskCall,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<SubagentErr> for CodexErr {
    fn from(err: SubagentErr) -> Self {
        match err {
            // Cancellation maps to TurnAborted (consistent with CancelErr)
            SubagentErr::Cancelled => CodexErr::TurnAborted,

            // Timeout maps to Timeout
            SubagentErr::Timeout(_) => CodexErr::Timeout,

            // Model errors with special cases
            SubagentErr::ModelError(ref msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("context_length") || msg_lower.contains("context window") {
                    CodexErr::ContextWindowExceeded
                } else if msg_lower.contains("quota")
                    || msg_lower.contains("rate_limit")
                    || msg_lower.contains("insufficient_quota")
                {
                    CodexErr::QuotaExceeded
                } else {
                    CodexErr::Fatal(err.to_string())
                }
            }

            // All other errors become Fatal
            _ => CodexErr::Fatal(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SubagentErr::UnknownAgentType("foo".to_string());
        assert_eq!(err.to_string(), "Unknown agent type: foo");
    }

    #[test]
    fn test_conversion_cancelled() {
        let err = SubagentErr::Cancelled;
        let codex_err: CodexErr = err.into();
        assert!(matches!(codex_err, CodexErr::TurnAborted));
    }

    #[test]
    fn test_conversion_timeout() {
        let err = SubagentErr::Timeout(60);
        let codex_err: CodexErr = err.into();
        assert!(matches!(codex_err, CodexErr::Timeout));
    }

    #[test]
    fn test_conversion_context_window() {
        let err = SubagentErr::ModelError("context_length_exceeded".to_string());
        let codex_err: CodexErr = err.into();
        assert!(matches!(codex_err, CodexErr::ContextWindowExceeded));
    }

    #[test]
    fn test_conversion_quota() {
        let err = SubagentErr::ModelError("insufficient_quota".to_string());
        let codex_err: CodexErr = err.into();
        assert!(matches!(codex_err, CodexErr::QuotaExceeded));
    }

    #[test]
    fn test_conversion_generic_model_error() {
        let err = SubagentErr::ModelError("some other error".to_string());
        let codex_err: CodexErr = err.into();
        assert!(matches!(codex_err, CodexErr::Fatal(_)));
    }
}
