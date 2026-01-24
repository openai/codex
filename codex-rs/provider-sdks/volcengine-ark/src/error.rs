//! Error types for the Volcengine Ark SDK.

use std::time::Duration;
use thiserror::Error;

/// Result type alias for Ark SDK operations.
pub type Result<T> = std::result::Result<T, ArkError>;

/// Errors that can occur when using the Volcengine Ark SDK.
#[derive(Debug, Error)]
pub enum ArkError {
    /// Configuration error (e.g., missing API key).
    #[error("configuration error: {0}")]
    Configuration(String),

    /// Validation error (e.g., invalid parameter value).
    #[error("validation error: {0}")]
    Validation(String),

    /// Network error during HTTP request.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// API error with status code and message.
    #[error("API error [{status}]: {message}")]
    Api {
        status: u16,
        message: String,
        request_id: Option<String>,
    },

    /// Authentication failed.
    #[error("authentication failed: {0}")]
    Authentication(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded")]
    RateLimited { retry_after: Option<Duration> },

    /// Invalid request.
    #[error("invalid request: {0}")]
    BadRequest(String),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Parse error with custom message (includes raw body for debugging).
    #[error("parse error: {0}")]
    Parse(String),

    /// Context window exceeded.
    #[error("context window exceeded")]
    ContextWindowExceeded,

    /// Quota exceeded.
    #[error("quota exceeded")]
    QuotaExceeded,

    /// Previous response not found.
    #[error("previous response not found")]
    PreviousResponseNotFound,

    /// Internal server error.
    #[error("internal server error")]
    InternalServerError,
}

impl ArkError {
    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::RateLimited { .. } => true,
            Self::InternalServerError => true,
            Self::Api { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }

    /// Get the retry-after duration if available.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        assert!(ArkError::RateLimited { retry_after: None }.is_retryable());
        assert!(ArkError::InternalServerError.is_retryable());
        assert!(
            ArkError::Api {
                status: 500,
                message: "error".to_string(),
                request_id: None
            }
            .is_retryable()
        );
        assert!(
            ArkError::Api {
                status: 429,
                message: "error".to_string(),
                request_id: None
            }
            .is_retryable()
        );
    }

    #[test]
    fn test_non_retryable_errors() {
        assert!(!ArkError::Configuration("test".to_string()).is_retryable());
        assert!(!ArkError::Validation("test".to_string()).is_retryable());
        assert!(!ArkError::Authentication("test".to_string()).is_retryable());
        assert!(!ArkError::BadRequest("test".to_string()).is_retryable());
        assert!(
            !ArkError::Api {
                status: 400,
                message: "error".to_string(),
                request_id: None
            }
            .is_retryable()
        );
    }
}
