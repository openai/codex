use std::time::Duration;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    UnsupportedOperation(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("{source}")]
    ResponseStreamFailed {
        #[source]
        source: reqwest::Error,
        request_id: Option<String>,
    },
    #[error("{0}")]
    Stream(String, Option<Duration>),
    #[error("unexpected status {status}: {body}")]
    UnexpectedStatus {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("retry limit reached (status {status}, request id: {request_id:?})")]
    RetryLimit {
        status: reqwest::StatusCode,
        request_id: Option<String>,
    },
    #[error("missing environment variable {var}")]
    MissingEnvVar {
        var: String,
        instructions: Option<String>,
    },
    #[error("{0}")]
    Auth(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}
