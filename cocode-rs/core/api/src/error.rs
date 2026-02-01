//! Error types for the API layer.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
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

impl ApiError {
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
        use api_error::*;
        use hyper_sdk::HyperError;

        match err {
            HyperError::NetworkError(msg) => NetworkSnafu { message: msg }.build(),
            HyperError::AuthenticationFailed(msg) => AuthenticationSnafu { message: msg }.build(),
            HyperError::RateLimitExceeded(msg) => RateLimitedSnafu {
                message: msg,
                retry_after_ms: 1000i64,
            }
            .build(),
            HyperError::Retryable { message, delay } => {
                let ms = delay.map(|d| d.as_millis() as i64).unwrap_or(1000);
                RateLimitedSnafu {
                    message,
                    retry_after_ms: ms,
                }
                .build()
            }
            HyperError::ContextWindowExceeded(msg) => ContextOverflowSnafu { message: msg }.build(),
            HyperError::StreamError(msg) => StreamSnafu { message: msg }.build(),
            HyperError::StreamIdleTimeout(timeout) => StreamIdleTimeoutSnafu {
                timeout_secs: timeout.as_secs() as i64,
            }
            .build(),
            HyperError::InvalidRequest(msg) => InvalidRequestSnafu { message: msg }.build(),
            HyperError::ProviderError { message, .. } => ProviderSnafu { message }.build(),
            other => SdkSnafu {
                message: other.to_string(),
            }
            .build(),
        }
    }
}

/// Result type for API operations.
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::api_error::*;
    use super::*;

    #[test]
    fn test_error_retryable() {
        assert!(NetworkSnafu { message: "test" }.build().is_retryable());
        assert!(
            RateLimitedSnafu {
                message: "test",
                retry_after_ms: 1000i64
            }
            .build()
            .is_retryable()
        );
        assert!(OverloadedSnafu { message: "test" }.build().is_retryable());
        assert!(
            !AuthenticationSnafu { message: "test" }
                .build()
                .is_retryable()
        );
        assert!(
            !InvalidRequestSnafu { message: "test" }
                .build()
                .is_retryable()
        );
    }

    #[test]
    fn test_retry_delay() {
        let err: ApiError = RateLimitedSnafu {
            message: "test",
            retry_after_ms: 5000i64,
        }
        .build();
        assert_eq!(err.retry_delay(), Some(Duration::from_millis(5000)));

        let err: ApiError = NetworkSnafu { message: "test" }.build();
        assert_eq!(err.retry_delay(), None);
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            NetworkSnafu { message: "test" }.build().status_code(),
            StatusCode::NetworkError
        );
        assert_eq!(
            AuthenticationSnafu { message: "test" }
                .build()
                .status_code(),
            StatusCode::AuthenticationFailed
        );
        assert_eq!(
            RateLimitedSnafu {
                message: "test",
                retry_after_ms: 1000i64
            }
            .build()
            .status_code(),
            StatusCode::RateLimited
        );
    }

    #[test]
    fn test_context_overflow() {
        let err: ApiError = ContextOverflowSnafu {
            message: "max context exceeded",
        }
        .build();
        assert!(err.is_context_overflow());
        assert!(!err.is_retryable());
        assert_eq!(err.status_code(), StatusCode::InvalidArguments);
    }

    #[test]
    fn test_is_stream_error() {
        let stream_err: ApiError = StreamSnafu {
            message: "stream failed",
        }
        .build();
        assert!(stream_err.is_stream_error());

        let timeout_err: ApiError = StreamIdleTimeoutSnafu {
            timeout_secs: 30i64,
        }
        .build();
        assert!(timeout_err.is_stream_error());

        let network_err: ApiError = NetworkSnafu {
            message: "network error",
        }
        .build();
        assert!(!network_err.is_stream_error());

        let rate_err: ApiError = RateLimitedSnafu {
            message: "rate limited",
            retry_after_ms: 1000i64,
        }
        .build();
        assert!(!rate_err.is_stream_error());

        let overflow_err: ApiError = ContextOverflowSnafu {
            message: "overflow",
        }
        .build();
        assert!(!overflow_err.is_stream_error());
    }

    #[test]
    fn test_from_hyper_error_context_overflow() {
        let hyper_err =
            hyper_sdk::HyperError::ContextWindowExceeded("Context too long".to_string());
        let api_err: ApiError = hyper_err.into();
        assert!(api_err.is_context_overflow());
    }
}
