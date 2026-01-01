use std::time::Duration;

pub type Result<T> = std::result::Result<T, AnthropicError>;

#[derive(Debug, thiserror::Error)]
pub enum AnthropicError {
    #[error("configuration error: {0}")]
    Configuration(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error [{status}]: {message}")]
    Api {
        status: u16,
        message: String,
        request_id: Option<String>,
    },

    #[error("authentication failed: {0}")]
    Authentication(String),

    #[error("rate limit exceeded")]
    RateLimited { retry_after: Option<Duration> },

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("internal server error")]
    InternalServerError,
}

impl AnthropicError {
    /// Returns true if the error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::RateLimited { .. } => true,
            Self::InternalServerError => true,
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
