use crate::rate_limits::RateLimitError;
use codex_client::TransportError;
use http::StatusCode;
use std::time::Duration;
use thiserror::Error;

pub(crate) const CYBER_SAFETY_BLOCK_ADVICE: &str = "This request has been flagged for potentially high-risk cyber activity. Apply for trusted access: https://chatgpt.com/cyber to avoid future blocks,\n  /fork or edit the last message and try again, or let us know with /feedback if you think this is a safety check false positive.";

pub(crate) fn is_cyber_safety_block(error_code: Option<&str>) -> bool {
    error_code.is_some_and(|code| code.to_ascii_lowercase().contains("cyber"))
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("api error {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("stream error: {0}")]
    Stream(String),
    #[error("context window exceeded")]
    ContextWindowExceeded,
    #[error("quota exceeded")]
    QuotaExceeded,
    #[error("usage not included")]
    UsageNotIncluded,
    #[error("retryable error: {message}")]
    Retryable {
        message: String,
        delay: Option<Duration>,
    },
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("server overloaded")]
    ServerOverloaded,
}

impl From<RateLimitError> for ApiError {
    fn from(err: RateLimitError) -> Self {
        Self::RateLimit(err.to_string())
    }
}
