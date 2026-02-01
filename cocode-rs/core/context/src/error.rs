//! Error types for context management.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
use snafu::Snafu;

/// Context management errors.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum ContextError {
    /// Budget exceeded for a category.
    #[snafu(display("Budget exceeded: {message}"))]
    BudgetExceeded {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Invalid configuration value.
    #[snafu(display("Invalid configuration: {message}"))]
    InvalidConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Context build error.
    #[snafu(display("Context build error: {message}"))]
    BuildError {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for ContextError {
    fn status_code(&self) -> StatusCode {
        match self {
            ContextError::BudgetExceeded { .. } => StatusCode::InvalidArguments,
            ContextError::InvalidConfig { .. } => StatusCode::InvalidConfig,
            ContextError::BuildError { .. } => StatusCode::Internal,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Result type for context operations.
pub type Result<T> = std::result::Result<T, ContextError>;

#[cfg(test)]
mod tests {
    use super::context_error::*;
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err: ContextError = BudgetExceededSnafu {
            message: "system prompt too large",
        }
        .build();
        assert!(err.to_string().contains("system prompt too large"));

        let err: ContextError = InvalidConfigSnafu {
            message: "negative token count",
        }
        .build();
        assert!(err.to_string().contains("negative token count"));

        let err: ContextError = BuildSnafu {
            message: "missing environment",
        }
        .build();
        assert!(err.to_string().contains("missing environment"));
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            BudgetExceededSnafu { message: "test" }
                .build()
                .status_code(),
            StatusCode::InvalidArguments
        );
        assert_eq!(
            InvalidConfigSnafu { message: "test" }.build().status_code(),
            StatusCode::InvalidConfig
        );
        assert_eq!(
            BuildSnafu { message: "test" }.build().status_code(),
            StatusCode::Internal
        );
    }
}
