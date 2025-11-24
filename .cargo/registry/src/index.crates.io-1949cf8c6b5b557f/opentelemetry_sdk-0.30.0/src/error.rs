//! Wrapper for error from trace, logs and metrics part of open telemetry.

use std::{result::Result, time::Duration};

use thiserror::Error;

/// Trait for errors returned by exporters
pub trait ExportError: std::error::Error + Send + Sync + 'static {
    /// The name of exporter that returned this error
    fn exporter_name(&self) -> &'static str;
}

#[derive(Error, Debug)]
/// Errors that can occur during SDK operations export(), force_flush() and shutdown().
pub enum OTelSdkError {
    /// Shutdown has already been invoked.
    ///
    /// While shutdown is idempotent and calling it multiple times has no
    /// impact, this error suggests that another part of the application is
    /// invoking `shutdown` earlier than intended. Users should review their
    /// code to identify unintended or duplicate shutdown calls and ensure it is
    /// only triggered once at the correct place.
    #[error("Shutdown already invoked")]
    AlreadyShutdown,

    /// Operation timed out before completing.
    ///
    /// This does not necessarily indicate a failure—operation may still be
    /// complete. If this occurs frequently, consider increasing the timeout
    /// duration to allow more time for completion.
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    /// Operation failed due to an internal error.
    ///
    /// The error message is intended for logging purposes only and should not
    /// be used to make programmatic decisions. It is implementation-specific
    /// and subject to change without notice. Consumers of this error should not
    /// rely on its content beyond logging.
    #[error("Operation failed: {0}")]
    InternalFailure(String),
}

/// A specialized `Result` type for Shutdown operations.
pub type OTelSdkResult = Result<(), OTelSdkError>;
