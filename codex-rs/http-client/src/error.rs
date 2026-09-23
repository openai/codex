//! Errors returned by the shared Codex HTTP transport.

use crate::client::HttpError;
use crate::retry_after::RetryAfter;
use http::HeaderMap;
use http::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    Policy(#[from] crate::NetworkPolicyDenied),
    #[error("http {status}: {body:?}")]
    Http {
        status: StatusCode,
        url: Option<String>,
        headers: Option<HeaderMap>,
        body: Option<String>,
        retry_after: Option<RetryAfter>,
    },
    #[error("retry limit reached")]
    RetryLimit,
    #[error("timeout")]
    Timeout,
    #[error("connection failed: {0}")]
    Connection(#[source] HttpError),
    #[error("network error: {0}")]
    Network(String),
    #[error("request build error: {0}")]
    Build(String),
    #[error("response body exceeds the {max_bytes} byte limit")]
    ResponseTooLarge { max_bytes: usize },
}

impl TransportError {
    /// Returns retry advice captured when this error response arrived.
    pub fn retry_after(&self) -> Option<RetryAfter> {
        match self {
            Self::Http { retry_after, .. } => *retry_after,
            Self::Policy(_)
            | Self::RetryLimit
            | Self::Timeout
            | Self::Connection(_)
            | Self::Network(_)
            | Self::Build(_)
            | Self::ResponseTooLarge { .. } => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("stream failed: {0}")]
    Stream(String),
    #[error("timeout")]
    Timeout,
}
