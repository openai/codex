//! Error extension traits.
//!
//! This module provides the [`ErrorExt`] trait for unified error handling with:
//! - Status code classification
//! - Retry semantics
//! - User-friendly error messages
//!
//! # Integration with snafu-virtstack
//!
//! For errors using `#[stack_trace_debug]`, the virtual stack trace is
//! automatically available via the `VirtualStackTrace` trait.
//!
//! # Example
//!
//! ```ignore
//! use common_error::{ErrorExt, StatusCode, stack_trace_debug};
//! use snafu::Snafu;
//!
//! #[stack_trace_debug]
//! #[derive(Snafu)]
//! pub enum MyError {
//!     #[snafu(display("Network error"))]
//!     Network { source: reqwest::Error },
//!
//!     #[snafu(display("Rate limited"))]
//!     RateLimited { retry_after: std::time::Duration },
//! }
//!
//! impl ErrorExt for MyError {
//!     fn status_code(&self) -> StatusCode {
//!         match self {
//!             Self::Network { .. } => StatusCode::NetworkError,
//!             Self::RateLimited { .. } => StatusCode::RateLimited,
//!         }
//!     }
//!
//!     fn as_any(&self) -> &dyn std::any::Any { self }
//! }
//! ```

use crate::StatusCode;
use std::any::Any;
use std::time::Duration;

/// Extension trait for errors with status code and retryability.
///
/// All error types in cocode-rs should implement this trait to provide:
/// - Unified status code classification
/// - Retry semantics (is_retryable, retry_after)
/// - User-friendly output messages
///
/// # Implementing for Nested Errors
///
/// When your error wraps another error that implements `ErrorExt`,
/// delegate to the source's `status_code()`:
///
/// ```ignore
/// fn status_code(&self) -> StatusCode {
///     match self {
///         Self::Upstream { source, .. } => source.status_code(),
///         Self::Local { .. } => StatusCode::Internal,
///     }
/// }
/// ```
pub trait ErrorExt: std::error::Error {
    /// Returns the status code for this error.
    ///
    /// Override this to provide appropriate classification.
    /// Default returns `StatusCode::Unknown`.
    fn status_code(&self) -> StatusCode {
        StatusCode::Unknown
    }

    /// Returns true if this error is retryable.
    ///
    /// By default, delegates to `status_code().is_retryable()`.
    /// Override for custom retry logic.
    fn is_retryable(&self) -> bool {
        self.status_code().is_retryable()
    }

    /// Returns the retry delay if applicable.
    ///
    /// For rate-limited errors, return the suggested wait duration.
    fn retry_after(&self) -> Option<Duration> {
        None
    }

    /// Returns a user-friendly error message.
    ///
    /// For internal/unknown errors, hides implementation details.
    /// For other errors, returns the Display string.
    fn output_msg(&self) -> String {
        match self.status_code() {
            StatusCode::Internal | StatusCode::Unknown => {
                format!("Internal error: {}", self.status_code() as i32)
            }
            _ => self.to_string(),
        }
    }

    /// Returns self as Any for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// A boxed error that implements `ErrorExt`.
///
/// Use this to wrap external errors or for type erasure.
pub type BoxedError = Box<dyn ErrorExt + Send + Sync>;

/// Wraps any `std::error::Error` into a `BoxedError` with the given status code.
pub fn boxed<E>(error: E, status_code: StatusCode) -> BoxedError
where
    E: std::error::Error + Send + Sync + 'static,
{
    Box::new(PlainError {
        message: error.to_string(),
        status_code,
        source: Some(Box::new(error)),
    })
}

/// A simple error type for wrapping external errors.
#[derive(Debug)]
pub struct PlainError {
    message: String,
    status_code: StatusCode,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl PlainError {
    /// Creates a new PlainError with the given message and status code.
    pub fn new(message: impl Into<String>, status_code: StatusCode) -> Self {
        Self {
            message: message.into(),
            status_code,
            source: None,
        }
    }
}

impl std::fmt::Display for PlainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PlainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

impl ErrorExt for PlainError {
    fn status_code(&self) -> StatusCode {
        self.status_code
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_error() {
        let err = PlainError::new("test error", StatusCode::InvalidArguments);
        assert_eq!(err.status_code(), StatusCode::InvalidArguments);
        assert_eq!(err.to_string(), "test error");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_plain_error_retryable() {
        let err = PlainError::new("network error", StatusCode::NetworkError);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_output_msg_hides_internal() {
        let err = PlainError::new("sensitive details", StatusCode::Internal);
        assert_eq!(err.output_msg(), "Internal error: 1001");
    }

    #[test]
    fn test_output_msg_shows_user_errors() {
        let err = PlainError::new("Invalid parameter: foo", StatusCode::InvalidArguments);
        assert_eq!(err.output_msg(), "Invalid parameter: foo");
    }

    #[test]
    fn test_boxed_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let boxed = boxed(io_err, StatusCode::FileNotFound);

        assert_eq!(boxed.status_code(), StatusCode::FileNotFound);
        assert!(boxed.source().is_some());
    }
}
