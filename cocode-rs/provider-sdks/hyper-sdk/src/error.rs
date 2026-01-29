//! Error types for hyper-sdk.
//!
//! # Error Chain Design
//!
//! hyper-sdk converts provider-specific errors into a unified `HyperError` type.
//! Errors like `NetworkError` and `ParseError` store stringified messages rather
//! than wrapping source errors directly. This is intentional for several reasons:
//!
//! 1. **Provider Independence**: Each provider SDK has different error types
//!    (reqwest::Error, serde_json::Error, etc.). Storing strings allows uniform
//!    handling without leaking provider-specific types.
//!
//! 2. **API Stability**: Wrapping source errors would expose internal dependencies,
//!    making semver-compatible changes harder.
//!
//! 3. **Serialization**: String errors serialize cleanly for logging and debugging.
//!
//! The `From` implementations preserve error context by including the source error's
//! Display output, which typically contains the full error chain information.

use std::time::Duration;
use thiserror::Error;

/// Result type alias using HyperError.
pub type Result<T> = std::result::Result<T, HyperError>;

/// Errors that can occur when using hyper-sdk.
#[derive(Debug, Error)]
pub enum HyperError {
    /// Provider not found in registry.
    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    /// Model not found or not supported by provider.
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// Requested capability is not supported by the model.
    #[error("unsupported capability: {0}")]
    UnsupportedCapability(String),

    /// Authentication failed (invalid or missing API key).
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Context window exceeded.
    #[error("context window exceeded: {0}")]
    ContextWindowExceeded(String),

    /// Invalid request parameters.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Network or HTTP error.
    ///
    /// The string contains the source error's display output, preserving error chain info.
    /// See module-level documentation for why we use strings instead of wrapping sources.
    #[error("network error: {0}")]
    NetworkError(String),

    /// Provider returned an error response.
    #[error("provider error: {code}: {message}")]
    ProviderError {
        /// Error code from the provider.
        code: String,
        /// Error message from the provider.
        message: String,
    },

    /// Failed to parse response from provider.
    ///
    /// The string contains the source error's display output, preserving error chain info.
    /// See module-level documentation for why we use strings instead of wrapping sources.
    #[error("parse error: {0}")]
    ParseError(String),

    /// Streaming error.
    #[error("stream error: {0}")]
    StreamError(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Internal SDK error.
    #[error("internal error: {0}")]
    Internal(String),

    /// Retryable error with optional suggested delay.
    #[error("retryable error: {message}")]
    Retryable {
        /// Error message.
        message: String,
        /// Suggested delay before retry (parsed from error message).
        delay: Option<Duration>,
    },

    /// Previous response not found (session continuity).
    #[error("previous response not found: {0}")]
    PreviousResponseNotFound(String),

    /// Quota exceeded (different from rate limit, requires billing change).
    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    /// Stream idle timeout (no events received within timeout period).
    #[error("stream idle timeout after {0:?}")]
    StreamIdleTimeout(Duration),
}

impl HyperError {
    /// Check if this error is retryable.
    ///
    /// Returns `true` for transient errors that may succeed on retry:
    /// - `Retryable` variant (explicitly marked as retryable)
    /// - `RateLimitExceeded` (temporary rate limiting)
    /// - `NetworkError` (connection issues, timeouts)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            HyperError::Retryable { .. }
                | HyperError::RateLimitExceeded(_)
                | HyperError::NetworkError(_)
        )
    }

    /// Get the suggested retry delay, if available.
    ///
    /// Only returns a value for `Retryable` errors that include a parsed delay.
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            HyperError::Retryable { delay, .. } => *delay,
            _ => None,
        }
    }
}

impl From<reqwest::Error> for HyperError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            HyperError::NetworkError(format!("request timeout: {err}"))
        } else if err.is_connect() {
            HyperError::NetworkError(format!("connection failed: {err}"))
        } else {
            HyperError::NetworkError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for HyperError {
    fn from(err: serde_json::Error) -> Self {
        HyperError::ParseError(err.to_string())
    }
}

/// Parse retry-after delay from an error message.
///
/// Looks for patterns like "try again in Xs" or "try again in Xms" in the message.
/// This is commonly used by rate-limited APIs to suggest when to retry.
///
/// # Examples
///
/// ```
/// use hyper_sdk::error::parse_retry_after;
/// use std::time::Duration;
///
/// assert_eq!(parse_retry_after("try again in 5s"), Some(Duration::from_secs(5)));
/// assert_eq!(parse_retry_after("try again in 500ms"), Some(Duration::from_millis(500)));
/// assert_eq!(parse_retry_after("try again in 2.5 seconds"), Some(Duration::from_secs_f64(2.5)));
/// assert_eq!(parse_retry_after("some error"), None);
/// ```
pub fn parse_retry_after(message: &str) -> Option<Duration> {
    let re = retry_after_regex();
    let captures = re.captures(message)?;

    let value = captures.get(1)?;
    let unit = captures.get(2)?;

    let value: f64 = value.as_str().parse().ok()?;
    let unit = unit.as_str().to_ascii_lowercase();

    if unit == "s" || unit.starts_with("second") {
        Some(Duration::from_secs_f64(value))
    } else if unit == "ms" {
        Some(Duration::from_millis(value as u64))
    } else {
        None
    }
}

fn retry_after_regex() -> &'static regex_lite::Regex {
    static RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex_lite::Regex::new(r"(?i)try again in\s*(\d+(?:\.\d+)?)\s*(s|ms|seconds?)")
            .expect("invalid regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = HyperError::ProviderNotFound("openai".to_string());
        assert_eq!(err.to_string(), "provider not found: openai");

        let err = HyperError::UnsupportedCapability("Vision".to_string());
        assert_eq!(err.to_string(), "unsupported capability: Vision");

        let err = HyperError::ProviderError {
            code: "invalid_api_key".to_string(),
            message: "The API key is invalid".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "provider error: invalid_api_key: The API key is invalid"
        );
    }

    #[test]
    fn test_is_retryable() {
        assert!(
            HyperError::Retryable {
                message: "rate limited".to_string(),
                delay: None,
            }
            .is_retryable()
        );
        assert!(HyperError::RateLimitExceeded("limit reached".to_string()).is_retryable());
        assert!(HyperError::NetworkError("connection refused".to_string()).is_retryable());

        assert!(!HyperError::AuthenticationFailed("invalid key".to_string()).is_retryable());
        assert!(!HyperError::QuotaExceeded("quota exceeded".to_string()).is_retryable());
    }

    #[test]
    fn test_retry_delay() {
        let err = HyperError::Retryable {
            message: "try again".to_string(),
            delay: Some(Duration::from_secs(5)),
        };
        assert_eq!(err.retry_delay(), Some(Duration::from_secs(5)));

        let err_no_delay = HyperError::Retryable {
            message: "try again".to_string(),
            delay: None,
        };
        assert_eq!(err_no_delay.retry_delay(), None);

        let other_err = HyperError::NetworkError("timeout".to_string());
        assert_eq!(other_err.retry_delay(), None);
    }

    #[test]
    fn test_parse_retry_after() {
        // Seconds
        assert_eq!(
            parse_retry_after("try again in 5s"),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            parse_retry_after("Try Again In 10s"),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            parse_retry_after("try again in 2.5 seconds"),
            Some(Duration::from_secs_f64(2.5))
        );
        assert_eq!(
            parse_retry_after("try again in 1 second"),
            Some(Duration::from_secs(1))
        );

        // Milliseconds
        assert_eq!(
            parse_retry_after("try again in 500ms"),
            Some(Duration::from_millis(500))
        );

        // No match
        assert_eq!(parse_retry_after("some error message"), None);
        assert_eq!(parse_retry_after("rate limit exceeded"), None);
        assert_eq!(parse_retry_after(""), None);
    }

    #[test]
    fn test_new_error_display() {
        let err = HyperError::StreamIdleTimeout(Duration::from_secs(60));
        assert!(err.to_string().contains("60"));

        let err = HyperError::QuotaExceeded("monthly limit".to_string());
        assert_eq!(err.to_string(), "quota exceeded: monthly limit");

        let err = HyperError::PreviousResponseNotFound("resp_123".to_string());
        assert_eq!(err.to_string(), "previous response not found: resp_123");

        let err = HyperError::Retryable {
            message: "rate limited".to_string(),
            delay: Some(Duration::from_secs(5)),
        };
        assert_eq!(err.to_string(), "retryable error: rate limited");
    }

    // =========================================================================
    // Comprehensive error scenario tests
    // =========================================================================

    #[test]
    fn test_all_error_variants_display() {
        // Test that all error variants produce valid display strings
        let errors: Vec<HyperError> = vec![
            HyperError::ProviderNotFound("openai".into()),
            HyperError::ModelNotFound("gpt-5".into()),
            HyperError::UnsupportedCapability("Vision".to_string()),
            HyperError::AuthenticationFailed("invalid key".into()),
            HyperError::RateLimitExceeded("429".into()),
            HyperError::ContextWindowExceeded("too long".into()),
            HyperError::InvalidRequest("bad params".into()),
            HyperError::NetworkError("timeout".into()),
            HyperError::ProviderError {
                code: "500".into(),
                message: "internal error".into(),
            },
            HyperError::ParseError("invalid json".into()),
            HyperError::StreamError("stream closed".into()),
            HyperError::ConfigError("missing field".into()),
            HyperError::Internal("bug".into()),
            HyperError::Retryable {
                message: "retry".into(),
                delay: Some(Duration::from_secs(1)),
            },
            HyperError::PreviousResponseNotFound("resp_123".into()),
            HyperError::QuotaExceeded("monthly".into()),
            HyperError::StreamIdleTimeout(Duration::from_secs(60)),
        ];

        for err in errors {
            let display = err.to_string();
            assert!(!display.is_empty(), "Error should have display: {:?}", err);
        }
    }

    #[test]
    fn test_retryable_classification_exhaustive() {
        // Retryable errors
        let retryable = [
            HyperError::Retryable {
                message: "temp".into(),
                delay: None,
            },
            HyperError::Retryable {
                message: "temp".into(),
                delay: Some(Duration::from_secs(1)),
            },
            HyperError::RateLimitExceeded("rate".into()),
            HyperError::NetworkError("net".into()),
        ];
        for err in retryable {
            assert!(err.is_retryable(), "Should be retryable: {:?}", err);
        }

        // Non-retryable errors
        let non_retryable = [
            HyperError::ProviderNotFound("openai".into()),
            HyperError::ModelNotFound("gpt-5".into()),
            HyperError::UnsupportedCapability("Vision".to_string()),
            HyperError::AuthenticationFailed("auth".into()),
            HyperError::ContextWindowExceeded("ctx".into()),
            HyperError::InvalidRequest("req".into()),
            HyperError::ProviderError {
                code: "err".into(),
                message: "msg".into(),
            },
            HyperError::ParseError("parse".into()),
            HyperError::StreamError("stream".into()),
            HyperError::ConfigError("cfg".into()),
            HyperError::Internal("int".into()),
            HyperError::PreviousResponseNotFound("resp".into()),
            HyperError::QuotaExceeded("quota".into()),
            HyperError::StreamIdleTimeout(Duration::from_secs(60)),
        ];
        for err in non_retryable {
            assert!(!err.is_retryable(), "Should NOT be retryable: {:?}", err);
        }
    }

    #[test]
    fn test_retry_delay_only_from_retryable() {
        // Only Retryable variant with delay should return delay
        let with_delay = HyperError::Retryable {
            message: "retry".into(),
            delay: Some(Duration::from_millis(500)),
        };
        assert_eq!(with_delay.retry_delay(), Some(Duration::from_millis(500)));

        let without_delay = HyperError::Retryable {
            message: "retry".into(),
            delay: None,
        };
        assert_eq!(without_delay.retry_delay(), None);

        // All other errors should return None
        let other_errors: Vec<HyperError> = vec![
            HyperError::RateLimitExceeded("rate".into()),
            HyperError::NetworkError("net".into()),
            HyperError::QuotaExceeded("quota".into()),
        ];
        for err in other_errors {
            assert_eq!(
                err.retry_delay(),
                None,
                "Non-Retryable should return None: {:?}",
                err
            );
        }
    }

    #[test]
    fn test_parse_retry_after_edge_cases() {
        // Valid formats
        assert_eq!(
            parse_retry_after("try again in 0s"),
            Some(Duration::from_secs(0))
        );
        assert_eq!(
            parse_retry_after("try again in 0.5s"),
            Some(Duration::from_secs_f64(0.5))
        );
        assert_eq!(
            parse_retry_after("TRY AGAIN IN 5S"),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            parse_retry_after("  try again in 5s  "),
            Some(Duration::from_secs(5))
        );

        // Invalid formats
        assert_eq!(parse_retry_after("try again in -5s"), None); // Negative
        assert_eq!(parse_retry_after("try again in 5h"), None); // Hours not supported
        assert_eq!(parse_retry_after("try again in 5m"), None); // Minutes not supported
        assert_eq!(parse_retry_after("retry in 5s"), None); // Different prefix
        assert_eq!(parse_retry_after("try again in s"), None); // No number
    }

    #[test]
    fn test_error_from_reqwest() {
        // We can't easily create reqwest errors, but we can test the From implementation exists
        // by checking the error types are compatible
        fn assert_from<T: From<reqwest::Error>>() {}
        assert_from::<HyperError>();
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let hyper_err: HyperError = json_err.into();
        assert!(matches!(hyper_err, HyperError::ParseError(_)));
        assert!(hyper_err.to_string().contains("parse error"));
    }

    #[test]
    fn test_quota_vs_rate_limit_distinction() {
        // Quota exceeded is NOT retryable (requires billing change)
        let quota = HyperError::QuotaExceeded("monthly quota".into());
        assert!(!quota.is_retryable());

        // Rate limit IS retryable (temporary)
        let rate = HyperError::RateLimitExceeded("too many requests".into());
        assert!(rate.is_retryable());
    }

    #[test]
    fn test_context_window_exceeded_not_retryable() {
        // Context window errors are not retryable - need to reduce input
        let err = HyperError::ContextWindowExceeded("max 128k tokens".into());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_stream_idle_timeout_not_retryable() {
        // Idle timeout is a local timeout, not a transient server error
        let err = HyperError::StreamIdleTimeout(Duration::from_secs(60));
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("60"));
    }

    #[test]
    fn test_previous_response_not_found() {
        let err = HyperError::PreviousResponseNotFound("resp_abc123".into());
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("resp_abc123"));
    }

    #[test]
    fn test_provider_error_with_special_characters() {
        let err = HyperError::ProviderError {
            code: "error_code_123".into(),
            message: "Message with \"quotes\" and 'apostrophes' and\nnewlines".into(),
        };
        let display = err.to_string();
        assert!(display.contains("error_code_123"));
        assert!(display.contains("quotes"));
    }
}
