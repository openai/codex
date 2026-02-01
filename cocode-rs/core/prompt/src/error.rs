//! Error types for prompt generation.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
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
    use super::prompt_error::*;
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err: PromptError = TemplateSnafu {
            message: "invalid placeholder",
        }
        .build();
        assert!(err.to_string().contains("invalid placeholder"));

        let err: PromptError = MissingContextSnafu { field: "platform" }.build();
        assert!(err.to_string().contains("platform"));
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            TemplateSnafu { message: "test" }.build().status_code(),
            StatusCode::Internal
        );
        assert_eq!(
            MissingContextSnafu { field: "test" }.build().status_code(),
            StatusCode::InvalidArguments
        );
    }
}
