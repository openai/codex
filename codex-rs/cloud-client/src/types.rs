//! Caller-facing native gRPC types. Diagnostics never include credentials or server payloads.

use bytes::Bytes;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use std::fmt;

/// Caller-owned ChatGPT bearer token and selected account, validated as HTTP headers.
#[derive(Clone, Debug)]
pub struct Credentials {
    pub(crate) headers: HeaderMap,
}

impl Credentials {
    pub fn new(bearer_token: &str, account_id: &str) -> Result<Self, Error> {
        if bearer_token.is_empty() || account_id.is_empty() {
            return Err(Error::InvalidCredentials);
        }
        let mut headers = HeaderMap::new();
        for (name, value) in [
            (AUTHORIZATION, format!("Bearer {bearer_token}")),
            (
                http::HeaderName::from_static("chatgpt-account-id"),
                account_id.to_owned(),
            ),
        ] {
            let mut value = HeaderValue::from_str(&value).map_err(|_| Error::InvalidCredentials)?;
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Ok(Self { headers })
    }
}

/// Resume an existing thread; `wait` distinguishes admission from readiness.
#[derive(Clone, prost::Message)]
#[prost(skip_debug)]
pub struct ResumeRequest {
    #[prost(string, tag = "1")]
    pub thread_id: String,
    #[prost(bool, tag = "2")]
    pub wait: bool,
    #[prost(bool, tag = "3")]
    pub shared_access: bool,
}

/// An Attach event containing the complete nested protobuf message bytes.
/// Payloads are native Notification/ServerRequest messages, not JSON-RPC or ProtoJSON.
/// Keeping them opaque preserves unknown fields until full payload typing is added.
pub enum Event {
    Notification(Bytes),
    ServerRequest(Bytes),
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Notification(_) => "Notification([redacted])",
            Self::ServerRequest(_) => "ServerRequest([redacted])",
        })
    }
}

/// Native gRPC status, including binary error details. Payloads are redacted in diagnostics.
pub struct RpcStatus {
    pub code: i32,
    pub message: String,
    pub details: Bytes,
}

impl fmt::Debug for RpcStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RpcStatus")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

impl From<tonic::Status> for Error {
    fn from(status: tonic::Status) -> Self {
        Self::Rpc(RpcStatus {
            code: status.code() as i32,
            message: status.message().to_owned(),
            details: Bytes::copy_from_slice(status.details()),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid gRPC origin (HTTPS required except on unrestricted unmanaged loopback)")]
    InvalidEndpoint,
    #[error("invalid bearer token or account header")]
    InvalidCredentials,
    #[error("could not construct the HTTP client")]
    ClientBuild,
    #[error("gRPC request timed out; request outcome may be unknown")]
    Timeout,
    #[error("gRPC failed; request outcome may be unknown: {0:?}")]
    Rpc(RpcStatus),
    #[error("invalid gRPC event envelope")]
    InvalidResponse,
}
