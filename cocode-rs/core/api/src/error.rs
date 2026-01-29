//! Error types for the API layer.

use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
use snafu::Snafu;
use std::time::Duration;

/// API layer errors.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum ApiError {
    /// Network error during API call.
    #[snafu(display("Network error: {message}"))]
    Network {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Authentication error.
    #[snafu(display("Authentication failed: {message}"))]
    Authentication {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Rate limit exceeded.
    #[snafu(display("Rate limited: {message}, retry after {retry_after_ms}ms"))]
    RateLimited {
        message: String,
        retry_after_ms: i64,
        #[snafu(implicit)]
        location: Location,
    },

    /// Model overloaded.
    #[snafu(display("Model overloaded: {message}"))]
    Overloaded {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Stream error during streaming response.
    #[snafu(display("Stream error: {message}"))]
    Stream {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Stream idle timeout.
    #[snafu(display("Stream idle timeout after {timeout_secs}s"))]
    StreamIdleTimeout {
        timeout_secs: i64,
        #[snafu(implicit)]
        location: Location,
    },

    /// Invalid request.
    #[snafu(display("Invalid request: {message}"))]
    InvalidRequest {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Provider error.
    #[snafu(display("Provider error: {message}"))]
    Provider {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// All retries exhausted.
    #[snafu(display("Retries exhausted after {attempts} attempts: {message}"))]
    RetriesExhausted {
        attempts: i32,
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Underlying hyper-sdk error.
    #[snafu(display("SDK error: {message}"))]
    Sdk {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Context window exceeded.
    #[snafu(display("Context overflow: {message}"))]
    ContextOverflow {
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

impl ApiError {
    /// Create a network error.
    #[track_caller]
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create an authentication error.
    #[track_caller]
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a rate limited error.
    #[track_caller]
    pub fn rate_limited(message: impl Into<String>, retry_after_ms: i64) -> Self {
        Self::RateLimited {
            message: message.into(),
            retry_after_ms,
            location: caller_location(),
        }
    }

    /// Create an overloaded error.
    #[track_caller]
    pub fn overloaded(message: impl Into<String>) -> Self {
        Self::Overloaded {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a stream error.
    #[track_caller]
    pub fn stream(message: impl Into<String>) -> Self {
        Self::Stream {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a stream idle timeout error.
    #[track_caller]
    pub fn stream_idle_timeout(timeout: Duration) -> Self {
        Self::StreamIdleTimeout {
            timeout_secs: timeout.as_secs() as i64,
            location: caller_location(),
        }
    }

    /// Create an invalid request error.
    #[track_caller]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a provider error.
    #[track_caller]
    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a retries exhausted error.
    #[track_caller]
    pub fn retries_exhausted(attempts: i32, message: impl Into<String>) -> Self {
        Self::RetriesExhausted {
            attempts,
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create an SDK error from hyper-sdk error.
    #[track_caller]
    pub fn sdk(message: impl Into<String>) -> Self {
        Self::Sdk {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a context overflow error.
    #[track_caller]
    pub fn context_overflow(message: impl Into<String>) -> Self {
        Self::ContextOverflow {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Check if this is a context overflow error.
    pub fn is_context_overflow(&self) -> bool {
        matches!(self, ApiError::ContextOverflow { .. })
    }

    /// Check if this is a stream-related error that should trigger fallback.
    ///
    /// Returns true for errors where falling back to non-streaming mode might help.
    pub fn is_stream_error(&self) -> bool {
        matches!(
            self,
            ApiError::Stream { .. } | ApiError::StreamIdleTimeout { .. }
        )
    }

    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ApiError::Network { .. }
                | ApiError::RateLimited { .. }
                | ApiError::Overloaded { .. }
                | ApiError::Stream { .. }
                | ApiError::StreamIdleTimeout { .. }
        )
    }

    /// Get retry delay hint if available.
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            ApiError::RateLimited { retry_after_ms, .. } => {
                Some(Duration::from_millis(*retry_after_ms as u64))
            }
            _ => None,
        }
    }
}

impl ErrorExt for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Network { .. } => StatusCode::NetworkError,
            ApiError::Authentication { .. } => StatusCode::AuthenticationFailed,
            ApiError::RateLimited { .. } => StatusCode::RateLimited,
            ApiError::Overloaded { .. } => StatusCode::ServiceUnavailable,
            ApiError::Stream { .. } => StatusCode::StreamError,
            ApiError::StreamIdleTimeout { .. } => StatusCode::Timeout,
            ApiError::InvalidRequest { .. } => StatusCode::InvalidArguments,
            ApiError::Provider { .. } => StatusCode::ProviderError,
            ApiError::RetriesExhausted { .. } => StatusCode::NetworkError,
            ApiError::Sdk { .. } => StatusCode::Internal,
            ApiError::ContextOverflow { .. } => StatusCode::InvalidArguments,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl From<hyper_sdk::HyperError> for ApiError {
    fn from(err: hyper_sdk::HyperError) -> Self {
        use hyper_sdk::HyperError;

        match err {
            HyperError::NetworkError(msg) => ApiError::network(msg),
            HyperError::AuthenticationFailed(msg) => ApiError::authentication(msg),
            HyperError::RateLimitExceeded(msg) => ApiError::rate_limited(msg, 1000),
            HyperError::Retryable { message, delay } => {
                let ms = delay.map(|d| d.as_millis() as i64).unwrap_or(1000);
                ApiError::rate_limited(message, ms)
            }
            HyperError::ContextWindowExceeded(msg) => ApiError::context_overflow(msg),
            HyperError::StreamError(msg) => ApiError::stream(msg),
            HyperError::StreamIdleTimeout(timeout) => ApiError::stream_idle_timeout(timeout),
            HyperError::InvalidRequest(msg) => ApiError::invalid_request(msg),
            HyperError::ProviderError { message, .. } => ApiError::provider(message),
            other => ApiError::sdk(other.to_string()),
        }
    }
}

/// Result type for API operations.
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        assert!(ApiError::network("test").is_retryable());
        assert!(ApiError::rate_limited("test", 1000).is_retryable());
        assert!(ApiError::overloaded("test").is_retryable());
        assert!(!ApiError::authentication("test").is_retryable());
        assert!(!ApiError::invalid_request("test").is_retryable());
    }

    #[test]
    fn test_retry_delay() {
        let err = ApiError::rate_limited("test", 5000);
        assert_eq!(err.retry_delay(), Some(Duration::from_millis(5000)));

        let err = ApiError::network("test");
        assert_eq!(err.retry_delay(), None);
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            ApiError::network("test").status_code(),
            StatusCode::NetworkError
        );
        assert_eq!(
            ApiError::authentication("test").status_code(),
            StatusCode::AuthenticationFailed
        );
        assert_eq!(
            ApiError::rate_limited("test", 1000).status_code(),
            StatusCode::RateLimited
        );
    }

    #[test]
    fn test_context_overflow() {
        let err = ApiError::context_overflow("max context exceeded");
        assert!(err.is_context_overflow());
        assert!(!err.is_retryable());
        assert_eq!(err.status_code(), StatusCode::InvalidArguments);
    }

    #[test]
    fn test_is_stream_error() {
        assert!(ApiError::stream("stream failed").is_stream_error());
        assert!(ApiError::stream_idle_timeout(Duration::from_secs(30)).is_stream_error());
        assert!(!ApiError::network("network error").is_stream_error());
        assert!(!ApiError::rate_limited("rate limited", 1000).is_stream_error());
        assert!(!ApiError::context_overflow("overflow").is_stream_error());
    }

    #[test]
    fn test_from_hyper_error_context_overflow() {
        let hyper_err =
            hyper_sdk::HyperError::ContextWindowExceeded("Context too long".to_string());
        let api_err: ApiError = hyper_err.into();
        assert!(api_err.is_context_overflow());
    }
}
