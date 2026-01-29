//! Error types for prompt generation.

use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
use snafu::Snafu;

/// Prompt generation errors.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum PromptError {
    /// Template rendering error.
    #[snafu(display("Template error: {message}"))]
    Template {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Missing required context field.
    #[snafu(display("Missing context: {field}"))]
    MissingContext {
        field: String,
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

impl PromptError {
    /// Create a template error.
    #[track_caller]
    pub fn template(message: impl Into<String>) -> Self {
        Self::Template {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a missing context error.
    #[track_caller]
    pub fn missing_context(field: impl Into<String>) -> Self {
        Self::MissingContext {
            field: field.into(),
            location: caller_location(),
        }
    }
}

impl ErrorExt for PromptError {
    fn status_code(&self) -> StatusCode {
        match self {
            PromptError::Template { .. } => StatusCode::Internal,
            PromptError::MissingContext { .. } => StatusCode::InvalidArguments,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Result type for prompt operations.
pub type Result<T> = std::result::Result<T, PromptError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err = PromptError::template("invalid placeholder");
        assert!(err.to_string().contains("invalid placeholder"));

        let err = PromptError::missing_context("platform");
        assert!(err.to_string().contains("platform"));
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            PromptError::template("test").status_code(),
            StatusCode::Internal
        );
        assert_eq!(
            PromptError::missing_context("test").status_code(),
            StatusCode::InvalidArguments
        );
    }
}
