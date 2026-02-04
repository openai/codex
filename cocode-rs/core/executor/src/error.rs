//! Error types for the executor module.
//!
//! Provides unified error handling with status codes following the cocode-error pattern.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
use snafu::Snafu;

/// Executor errors for iterative execution.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum ExecutorError {
    /// Git operation failed (e.g., getting HEAD commit, committing changes).
    #[snafu(display("Git operation failed: {message}"))]
    Git {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Iteration execution failed.
    #[snafu(display("Iteration execution failed: {message}"))]
    Execution {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Context initialization failed.
    #[snafu(display("Context initialization failed: {message}"))]
    Context {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Summarization failed.
    #[snafu(display("Summarization failed: {message}"))]
    Summarization {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Spawn blocking task failed.
    #[snafu(display("Task spawn failed: {message}"))]
    TaskSpawn {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for ExecutorError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Git { .. } => StatusCode::IoError,
            Self::Execution { .. } => StatusCode::Internal,
            Self::Context { .. } => StatusCode::InvalidArguments,
            Self::Summarization { .. } => StatusCode::Internal,
            Self::TaskSpawn { .. } => StatusCode::Internal,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Result type for executor operations.
pub type Result<T> = std::result::Result<T, ExecutorError>;

#[cfg(test)]
mod tests {
    use super::executor_error::*;
    use super::*;

    #[test]
    fn test_git_error() {
        let err: ExecutorError = GitSnafu {
            message: "failed to get HEAD",
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::IoError);
        assert!(err.to_string().contains("Git operation failed"));
    }

    #[test]
    fn test_execution_error() {
        let err: ExecutorError = ExecutionSnafu {
            message: "iteration failed",
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::Internal);
        assert!(err.to_string().contains("Iteration execution failed"));
    }

    #[test]
    fn test_context_error() {
        let err: ExecutorError = ContextSnafu {
            message: "invalid config",
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::InvalidArguments);
    }

    #[test]
    fn test_summarization_error() {
        let err: ExecutorError = SummarizationSnafu {
            message: "LLM call failed",
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::Internal);
    }

    #[test]
    fn test_task_spawn_error() {
        let err: ExecutorError = TaskSpawnSnafu {
            message: "spawn_blocking failed",
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::Internal);
    }

    #[test]
    fn test_error_retryable() {
        // Internal errors are retryable
        let exec_err: ExecutorError = ExecutionSnafu { message: "test" }.build();
        assert!(exec_err.status_code().is_retryable());

        // IO errors are not retryable by default
        let git_err: ExecutorError = GitSnafu { message: "test" }.build();
        assert!(!git_err.status_code().is_retryable());
    }
}
