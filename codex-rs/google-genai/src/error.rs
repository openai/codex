//! Error types for Google Generative AI client.

use thiserror::Error;

/// Result type alias for GenAI operations.
pub type Result<T> = std::result::Result<T, GenAiError>;

/// Errors that can occur when using the Google Generative AI client.
#[derive(Debug, Error)]
pub enum GenAiError {
    /// Configuration error (missing API key, invalid settings, etc.)
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Network error (connection failed, timeout, etc.)
    #[error("Network error: {0}")]
    Network(String),

    /// API error returned by the server.
    #[error("API error [{code}] {status}: {message}")]
    Api {
        code: i32,
        message: String,
        status: String,
    },

    /// Parse error (failed to deserialize response).
    #[error("Parse error: {0}")]
    Parse(String),

    /// Validation error (invalid request parameters).
    #[error("Validation error: {0}")]
    Validation(String),

    /// Context length exceeded.
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),

    /// Quota exceeded.
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    /// Content blocked by safety filters.
    #[error("Content blocked: {0}")]
    ContentBlocked(String),
}

impl GenAiError {
    /// Check if this is a retryable error.
    pub fn is_retryable(&self) -> bool {
        match self {
            GenAiError::Network(_) => true,
            GenAiError::Api { code, .. } => {
                // 429 = rate limit, 500+ = server errors
                *code == 429 || *code >= 500
            }
            _ => false,
        }
    }

    /// Check if this is a quota exceeded error.
    pub fn is_quota_exceeded(&self) -> bool {
        matches!(self, GenAiError::QuotaExceeded(_))
            || matches!(self, GenAiError::Api { code: 429, .. })
    }

    /// Check if this is a context length exceeded error.
    pub fn is_context_length_exceeded(&self) -> bool {
        matches!(self, GenAiError::ContextLengthExceeded(_))
    }
}

impl From<reqwest::Error> for GenAiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            GenAiError::Network(format!("Request timeout: {err}"))
        } else if err.is_connect() {
            GenAiError::Network(format!("Connection failed: {err}"))
        } else {
            GenAiError::Network(err.to_string())
        }
    }
}

impl From<serde_json::Error> for GenAiError {
    fn from(err: serde_json::Error) -> Self {
        GenAiError::Parse(err.to_string())
    }
}
