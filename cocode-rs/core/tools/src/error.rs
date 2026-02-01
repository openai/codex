//! Error types for tool execution.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
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

impl ToolError {
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
        tool_error::IoSnafu {
            message: err.to_string(),
        }
        .build()
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(err: serde_json::Error) -> Self {
        tool_error::InvalidInputSnafu {
            message: format!("JSON error: {err}"),
        }
        .build()
    }
}

/// Result type for tool operations.
pub type Result<T> = std::result::Result<T, ToolError>;

#[cfg(test)]
mod tests {
    use super::tool_error::*;
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err: ToolError = NotFoundSnafu { name: "test_tool" }.build();
        assert!(err.to_string().contains("test_tool"));

        let err: ToolError = InvalidInputSnafu {
            message: "bad json",
        }
        .build();
        assert!(err.to_string().contains("bad json"));

        let err: ToolError = TimeoutSnafu {
            timeout_secs: 30i64,
        }
        .build();
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_is_retriable() {
        assert!(
            TimeoutSnafu {
                timeout_secs: 30i64
            }
            .build()
            .is_retriable()
        );
        assert!(
            IoSnafu {
                message: "network error"
            }
            .build()
            .is_retriable()
        );
        assert!(!NotFoundSnafu { name: "test" }.build().is_retriable());
        assert!(
            !PermissionDeniedSnafu { message: "denied" }
                .build()
                .is_retriable()
        );
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            NotFoundSnafu { name: "test" }.build().status_code(),
            StatusCode::InvalidArguments
        );
        assert_eq!(
            PermissionDeniedSnafu { message: "test" }
                .build()
                .status_code(),
            StatusCode::PermissionDenied
        );
        assert_eq!(
            TimeoutSnafu {
                timeout_secs: 30i64
            }
            .build()
            .status_code(),
            StatusCode::Timeout
        );
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let tool_err: ToolError = io_err.into();
        assert!(matches!(tool_err, ToolError::Io { .. }));
    }
}
