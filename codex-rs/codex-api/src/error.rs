use crate::rate_limits::RateLimitError;
use codex_client::TransportError;
use codex_http_client::RetryAfter;
use codex_protocol::error::ResponseIncompleteError;
use codex_protocol::protocol::MisalignmentErrorDetails;
use http::StatusCode;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("api error {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("stream error: {0}")]
    Stream(String),
    #[error("stream error: Incomplete response returned, reason: content_filter")]
    ContentFilter,
    #[error(transparent)]
    ResponseIncomplete(Box<ResponseIncompleteError>),
    #[error("context window exceeded")]
    ContextWindowExceeded,
    #[error("quota exceeded")]
    QuotaExceeded,
    #[error("usage not included")]
    UsageNotIncluded,
    #[error("retryable error: {message}")]
    Retryable {
        message: String,
        retry_after: Option<RetryAfter>,
    },
    #[error("rate limit exceeded: {message}")]
    RateLimitExceeded {
        message: String,
        retry_after: Option<RetryAfter>,
    },
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("{message}")]
    ModelNotFound {
        message: String,
        status: Option<StatusCode>,
    },
    #[error("invalid prompt: {message}")]
    InvalidPrompt { message: String },
    #[error("cyber policy: {message}")]
    CyberPolicy { message: String },
    #[error("bio policy: {message}")]
    BioPolicy { message: String },
    #[error("misalignment policy violation: {message}")]
    MisalignmentPolicyViolation {
        message: String,
        misalignment: Option<MisalignmentErrorDetails>,
    },
    #[error("Flex capacity unavailable.")]
    FlexUnavailable,
    #[error("server overloaded")]
    ServerOverloaded { retry_after: Option<RetryAfter> },
}

impl From<RateLimitError> for ApiError {
    fn from(err: RateLimitError) -> Self {
        Self::RateLimit(err.to_string())
    }
}

pub(crate) fn parse_terminal_provider_error(
    error: Option<&Value>,
    status: Option<StatusCode>,
) -> Option<ApiError> {
    let error = error?;
    let code = error.get("code")?.as_str()?;
    match (code, status) {
        ("flex_unavailable", None | Some(StatusCode::TOO_MANY_REQUESTS)) => {
            Some(ApiError::FlexUnavailable)
        }
        (
            "model_not_found",
            None | Some(StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        ) => Some(ApiError::ModelNotFound {
            message: error
                .get("message")
                .and_then(Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or(code)
                .to_owned(),
            status,
        }),
        _ => None,
    }
}

impl From<crate::AuthError> for ApiError {
    fn from(error: crate::AuthError) -> Self {
        Self::Transport(error.into())
    }
}
