//! Error types for the hook system.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
use snafu::Snafu;

#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum HookError {
    #[snafu(display("Invalid matcher pattern: {message}"))]
    InvalidMatcher {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Hook execution failed: {message}"))]
    ExecutionFailed {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Hook timed out after {timeout_secs}s"))]
    Timeout {
        timeout_secs: i32,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid hook config: {message}"))]
    InvalidConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("IO error: {message}"))]
    Io {
        message: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for HookError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidMatcher { .. } => StatusCode::InvalidArguments,
            Self::ExecutionFailed { .. } => StatusCode::Internal,
            Self::Timeout { .. } => StatusCode::Timeout,
            Self::InvalidConfig { .. } => StatusCode::InvalidConfig,
            Self::Io { .. } => StatusCode::IoError,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
