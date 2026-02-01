//! Error types for the plan-mode crate.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
use snafu::Snafu;

/// Result type alias for plan-mode operations.
pub type Result<T> = std::result::Result<T, PlanModeError>;

/// Errors that can occur during plan mode operations.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum PlanModeError {
    /// Failed to create plan directory.
    #[snafu(display("Failed to create plan directory: {message}"))]
    CreateDir {
        message: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    /// Failed to read plan file.
    #[snafu(display("Failed to read plan file: {message}"))]
    ReadFile {
        message: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    /// Failed to write plan file.
    #[snafu(display("Failed to write plan file: {message}"))]
    WriteFile {
        message: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    /// No home directory found.
    #[snafu(display("Could not determine home directory"))]
    NoHomeDir {
        #[snafu(implicit)]
        location: Location,
    },

    /// Slug generation failed after max retries.
    #[snafu(display("Failed to generate unique slug after {max_retries} attempts"))]
    SlugCollision {
        max_retries: i32,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for PlanModeError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::CreateDir { .. } | Self::WriteFile { .. } => StatusCode::IoError,
            Self::ReadFile { .. } => StatusCode::FileNotFound,
            Self::NoHomeDir { .. } => StatusCode::InvalidConfig,
            Self::SlugCollision { .. } => StatusCode::Internal,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = plan_mode_error::NoHomeDirSnafu.build();
        assert!(err.to_string().contains("home directory"));
    }
}
