//! Error types for context management.

use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
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

/// Create a Location from the caller's position.
#[track_caller]
fn caller_location() -> Location {
    let loc = std::panic::Location::caller();
    Location::new(loc.file(), loc.line(), loc.column())
}

impl ContextError {
    /// Create a budget exceeded error.
    #[track_caller]
    pub fn budget_exceeded(message: impl Into<String>) -> Self {
        Self::BudgetExceeded {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create an invalid config error.
    #[track_caller]
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a build error.
    #[track_caller]
    pub fn build_error(message: impl Into<String>) -> Self {
        Self::BuildError {
            message: message.into(),
            location: caller_location(),
        }
    }
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
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err = ContextError::budget_exceeded("system prompt too large");
        assert!(err.to_string().contains("system prompt too large"));

        let err = ContextError::invalid_config("negative token count");
        assert!(err.to_string().contains("negative token count"));

        let err = ContextError::build_error("missing environment");
        assert!(err.to_string().contains("missing environment"));
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            ContextError::budget_exceeded("test").status_code(),
            StatusCode::InvalidArguments
        );
        assert_eq!(
            ContextError::invalid_config("test").status_code(),
            StatusCode::InvalidConfig
        );
        assert_eq!(
            ContextError::build_error("test").status_code(),
            StatusCode::Internal
        );
    }
}
