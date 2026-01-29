//! Error types for the sandbox crate.

use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
use snafu::Snafu;

/// Errors that can occur during sandbox operations.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum SandboxError {
    /// A path is not allowed by the sandbox configuration.
    #[snafu(display("Path denied: {path}"))]
    PathDenied {
        path: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Write access is not allowed in the current sandbox mode.
    #[snafu(display("Write access denied: {message}"))]
    WriteDenied {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Network access is not allowed.
    #[snafu(display("Network access denied"))]
    NetworkDenied {
        #[snafu(implicit)]
        location: Location,
    },

    /// The sandbox platform is not available on this OS.
    #[snafu(display("Platform not available: {message}"))]
    PlatformNotAvailable {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// An error occurred while applying the sandbox configuration.
    #[snafu(display("Sandbox apply error: {message}"))]
    ApplyError {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for SandboxError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::PathDenied { .. } => StatusCode::PermissionDenied,
            Self::WriteDenied { .. } => StatusCode::PermissionDenied,
            Self::NetworkDenied { .. } => StatusCode::PermissionDenied,
            Self::PlatformNotAvailable { .. } => StatusCode::Unsupported,
            Self::ApplyError { .. } => StatusCode::Internal,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub type Result<T> = std::result::Result<T, SandboxError>;
