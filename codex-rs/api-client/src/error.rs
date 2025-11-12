use codex_protocol::protocol::RateLimitSnapshot;
use reqwest::StatusCode;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    UnsupportedOperation(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("response stream failed: {source}")]
    ResponseStreamFailed {
        #[source]
        source: reqwest::Error,
        request_id: Option<String>,
    },
    #[error("stream error: {0}")]
    Stream(String, Option<std::time::Duration>),
    #[error("usage limit reached")]
    UsageLimitReached {
        plan_type: Option<String>,
        resets_at: Option<i64>,
        rate_limits: Option<RateLimitSnapshot>,
    },
    #[error("unexpected status {status}: {body}")]
    UnexpectedStatus { status: StatusCode, body: String },
    #[error("retry limit reached {status:?} request_id={request_id:?}")]
    RetryLimit {
        status: Option<StatusCode>,
        request_id: Option<String>,
    },
    #[error("missing env var {var}: {instructions:?}")]
    MissingEnvVar {
        var: String,
        instructions: Option<String>,
    },
    #[error("auth error: {0}")]
    Auth(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}
