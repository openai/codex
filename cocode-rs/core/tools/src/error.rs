//! Error types for tool execution.

use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
use snafu::Snafu;

/// Tool execution errors.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum ToolError {
    /// Tool not found in registry.
    #[snafu(display("Tool not found: {name}"))]
    NotFound {
        name: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Invalid input for tool.
    #[snafu(display("Invalid input: {message}"))]
    InvalidInput {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Tool execution failed.
    #[snafu(display("Execution failed: {message}"))]
    ExecutionFailed {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Permission denied for tool.
    #[snafu(display("Permission denied: {message}"))]
    PermissionDenied {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Tool execution timed out.
    #[snafu(display("Timeout after {timeout_secs}s"))]
    Timeout {
        timeout_secs: i64,
        #[snafu(implicit)]
        location: Location,
    },

    /// Tool execution was aborted.
    #[snafu(display("Aborted: {reason}"))]
    Aborted {
        reason: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// IO error during tool execution.
    #[snafu(display("IO error: {message}"))]
    Io {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Internal error.
    #[snafu(display("Internal error: {message}"))]
    Internal {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Tool call rejected by a hook.
    #[snafu(display("Hook rejected: {reason}"))]
    HookRejected {
        reason: String,
        #[snafu(implicit)]
        location: Location,
    },
}

/// Create a Location from the caller's position.
#[track_caller]
fn caller_location() -> Location {
    let loc = std::panic::Location::caller();
    Location::new(loc.file(), loc.line(), loc.column())
}

impl ToolError {
    /// Create a not found error.
    #[track_caller]
    pub fn not_found(name: impl Into<String>) -> Self {
        Self::NotFound {
            name: name.into(),
            location: caller_location(),
        }
    }

    /// Create an invalid input error.
    #[track_caller]
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create an execution failed error.
    #[track_caller]
    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a permission denied error.
    #[track_caller]
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a timeout error.
    #[track_caller]
    pub fn timeout(timeout_secs: i64) -> Self {
        Self::Timeout {
            timeout_secs,
            location: caller_location(),
        }
    }

    /// Create an aborted error.
    #[track_caller]
    pub fn aborted(reason: impl Into<String>) -> Self {
        Self::Aborted {
            reason: reason.into(),
            location: caller_location(),
        }
    }

    /// Create an IO error.
    #[track_caller]
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create an internal error.
    #[track_caller]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a hook rejected error.
    #[track_caller]
    pub fn hook_rejected(reason: impl Into<String>) -> Self {
        Self::HookRejected {
            reason: reason.into(),
            location: caller_location(),
        }
    }

    /// Check if this is a retriable error.
    pub fn is_retriable(&self) -> bool {
        matches!(self, ToolError::Timeout { .. } | ToolError::Io { .. })
    }

    /// Convert to tool output error message.
    pub fn to_output_message(&self) -> String {
        self.to_string()
    }
}

impl ErrorExt for ToolError {
    fn status_code(&self) -> StatusCode {
        match self {
            ToolError::NotFound { .. } => StatusCode::InvalidArguments, // Tool not found
            ToolError::InvalidInput { .. } => StatusCode::InvalidArguments,
            ToolError::ExecutionFailed { .. } => StatusCode::External, // External tool failure
            ToolError::PermissionDenied { .. } => StatusCode::PermissionDenied,
            ToolError::Timeout { .. } => StatusCode::Timeout,
            ToolError::Aborted { .. } => StatusCode::Cancelled,
            ToolError::Io { .. } => StatusCode::IoError,
            ToolError::Internal { .. } => StatusCode::Internal,
            ToolError::HookRejected { .. } => StatusCode::PermissionDenied, // Hook rejection is a form of denial
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl From<std::io::Error> for ToolError {
    fn from(err: std::io::Error) -> Self {
        ToolError::io(err.to_string())
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(err: serde_json::Error) -> Self {
        ToolError::invalid_input(format!("JSON error: {err}"))
    }
}

/// Result type for tool operations.
pub type Result<T> = std::result::Result<T, ToolError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err = ToolError::not_found("test_tool");
        assert!(err.to_string().contains("test_tool"));

        let err = ToolError::invalid_input("bad json");
        assert!(err.to_string().contains("bad json"));

        let err = ToolError::timeout(30);
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_is_retriable() {
        assert!(ToolError::timeout(30).is_retriable());
        assert!(ToolError::io("network error").is_retriable());
        assert!(!ToolError::not_found("test").is_retriable());
        assert!(!ToolError::permission_denied("denied").is_retriable());
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            ToolError::not_found("test").status_code(),
            StatusCode::InvalidArguments
        );
        assert_eq!(
            ToolError::permission_denied("test").status_code(),
            StatusCode::PermissionDenied
        );
        assert_eq!(ToolError::timeout(30).status_code(), StatusCode::Timeout);
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let tool_err: ToolError = io_err.into();
        assert!(matches!(tool_err, ToolError::Io { .. }));
    }
}
