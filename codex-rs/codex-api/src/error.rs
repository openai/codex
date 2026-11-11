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
    #[error("websocket request exceeds the peer's message size limit")]
    WebsocketMessageTooLarge,
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
    #[error("session budget exceeded")]
    SessionBudgetExceeded,
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
    AccessProgramNotEnabled { message: String },
    #[error("{message}")]
    InvalidAudioInput {
        message: String,
        /// True when the provider rejected an individual clip, independent of request size.
        per_recording: bool,
    },
    #[error("{message}")]
    ModelNotFound {
        message: String,
        status: Option<StatusCode>,
    },
    #[error("invalid prompt: {message}")]
    InvalidPrompt { message: String },
    #[error("{message}")]
    InferenceHookDenied { message: String },
    #[error("{message}")]
    ContentPolicyViolation {
        message: String,
        status: Option<StatusCode>,
    },
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
    /// Server-confirmed capacity failure on a policy-restricted route.
    #[error("policy-restricted capacity unavailable")]
    PolicyCapacityUnavailable { retry_after: Option<RetryAfter> },
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
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty());
    match (code, status) {
        ("inference_hook_denied", None | Some(StatusCode::FORBIDDEN)) => {
            Some(ApiError::InferenceHookDenied {
                message: message
                    .unwrap_or("Your workspace inference hook blocked this request.")
                    .to_owned(),
            })
        }
        ("context_length_exceeded", None | Some(StatusCode::BAD_REQUEST)) => {
            Some(ApiError::ContextWindowExceeded)
        }
        // A non-streaming 400 already follows the ordinary invalid-request path.
        ("invalid_access_program" | "unsupported_access_program", None) => {
            Some(ApiError::InvalidRequest {
                message: message
                    .unwrap_or("The requested access program cannot be used with this model.")
                    .to_owned(),
            })
        }
        ("content_policy_violation", None | Some(StatusCode::BAD_REQUEST)) => {
            Some(ApiError::ContentPolicyViolation {
                message: message
                    .unwrap_or("The input was rejected by the content policy.")
                    .to_owned(),
                status,
            })
        }
        // HTTP 403s retain their request ID and headers in the API bridge.
        ("access_program_not_enabled", None) => Some(ApiError::AccessProgramNotEnabled {
            message: message
                .unwrap_or("The requested access program is not enabled.")
                .to_owned(),
        }),
        ("budget_reached", None | Some(StatusCode::BAD_REQUEST)) => {
            Some(ApiError::SessionBudgetExceeded)
        }
        (
            "audio_too_large"
            | "audio_too_short"
            | "invalid_audio"
            | "invalid_audio_data"
            | "invalid_audio_format"
            | "missing_audio_sample_rate",
            None | Some(StatusCode::BAD_REQUEST),
        ) => Some(ApiError::InvalidAudioInput {
            per_recording: code != "audio_too_large",
            message: message.unwrap_or("Invalid audio input.").to_owned(),
        }),
        ("flex_unavailable", None | Some(StatusCode::TOO_MANY_REQUESTS)) => {
            Some(ApiError::FlexUnavailable)
        }
        (
            "model_not_found",
            None | Some(StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        ) => Some(ApiError::ModelNotFound {
            message: message.unwrap_or(code).to_owned(),
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
