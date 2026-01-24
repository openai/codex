//! Error types for the hook system.

use thiserror::Error;

/// Errors that can occur during hook execution.
#[derive(Error, Debug)]
pub enum HookError {
    /// Hook execution timed out.
    #[error("hook execution timed out")]
    Timeout,

    /// Hook was cancelled via abort signal.
    #[error("hook execution cancelled")]
    Cancelled,

    /// Failed to spawn hook process.
    #[error("failed to spawn hook process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// Failed to serialize hook input.
    #[error("failed to serialize hook input: {0}")]
    SerializationFailed(#[from] serde_json::Error),

    /// Hook returned a blocking error (exit code 2).
    #[error("hook blocked execution: {message}")]
    Blocking {
        /// The error message from the hook.
        message: String,
        /// The command that caused the error.
        command: String,
    },

    /// Hook returned a non-blocking error (exit code 1, 3+).
    #[error("hook returned non-blocking error: {message}")]
    NonBlocking {
        /// The error message from the hook.
        message: String,
        /// Exit code from the hook.
        exit_code: i32,
    },

    /// Failed to parse hook output as JSON.
    #[error("failed to parse hook output: {0}")]
    ParseFailed(String),

    /// Hook output validation failed.
    #[error("hook output validation failed: {0}")]
    ValidationFailed(String),

    /// Configuration error.
    #[error("hook configuration error: {0}")]
    ConfigError(String),

    /// Registry error.
    #[error("hook registry error: {0}")]
    RegistryError(String),

    /// Other error.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl HookError {
    /// Check if this error indicates a blocking condition.
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Blocking { .. })
    }

    /// Check if this error indicates cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_error_is_blocking() {
        let blocking = HookError::Blocking {
            message: "test".to_string(),
            command: "cmd".to_string(),
        };
        assert!(blocking.is_blocking());

        let timeout = HookError::Timeout;
        assert!(!timeout.is_blocking());
    }

    #[test]
    fn test_hook_error_is_cancelled() {
        assert!(HookError::Cancelled.is_cancelled());
        assert!(HookError::Timeout.is_cancelled());
        assert!(
            !HookError::Blocking {
                message: "test".to_string(),
                command: "cmd".to_string()
            }
            .is_cancelled()
        );
    }
}
