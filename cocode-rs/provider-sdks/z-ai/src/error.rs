//! Error types for the Z.AI SDK.

use std::time::Duration;

/// Result type alias for Z.AI SDK operations.
pub type Result<T> = std::result::Result<T, ZaiError>;

/// Error type for Z.AI SDK operations.
#[derive(Debug, thiserror::Error)]
pub enum ZaiError {
    /// Configuration error (e.g., missing API key).
    #[error("configuration error: {0}")]
    Configuration(String),

    /// Validation error (e.g., invalid parameters).
    #[error("validation error: {0}")]
    Validation(String),

    /// Network error.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// API error with status code and message.
    #[error("API error [{status}]: {message}")]
    Api {
        status: i32,
        message: String,
        request_id: Option<String>,
    },

    /// Authentication error (401).
    #[error("authentication failed: {0}")]
    Authentication(String),

    /// Rate limit exceeded (429).
    #[error("rate limit exceeded")]
    RateLimited { retry_after: Option<Duration> },

    /// Bad request (400).
    #[error("invalid request: {0}")]
    BadRequest(String),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal server error (500).
    #[error("internal server error")]
    InternalServerError,

    /// Server flow exceeded (503).
    #[error("server flow exceeded")]
    ServerFlowExceeded,

    /// JWT generation error.
    #[error("JWT generation error: {0}")]
    JwtError(String),
}

impl ZaiError {
    /// Returns true if the error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::RateLimited { .. } => true,
            Self::InternalServerError => true,
            Self::ServerFlowExceeded => true,
            Self::Api { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }

    /// Returns the retry-after duration if available.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}
