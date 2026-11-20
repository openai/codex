//! Shared app-server wire client, event types and WebSocket codec, without a runtime dependency.
//! Embedded clients and server transports re-export these types to preserve their existing APIs.

mod remote;
mod remote_websocket;
mod websocket_compression;

pub use remote::RemoteAppServerClient;
pub use remote::RemoteAppServerConnectArgs;
pub use remote::RemoteAppServerEndpoint;
pub use remote::RemoteAppServerRequestHandle;
pub use remote::websocket_url_supports_auth_token;
pub use websocket_compression::CompressionConnection;
pub use websocket_compression::WEBSOCKET_COMPRESSION_HEADER;
pub use websocket_compression::WebSocketCompression;
pub use websocket_compression::ZSTD_STREAM_PROTOCOL;
pub use websocket_compression::ZstdDecoder;
pub use websocket_compression::ZstdEncoder;
pub use websocket_compression::zstd_codec_pair;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fmt;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::time::Duration;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Stable identifier for a transport connection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConnectionId(pub u64);

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Event emitted from the app-server to the in-process client.
///
/// [`Lagged`](Self::Lagged) is a transport health marker, not an application
/// event — it signals that the consumer fell behind and some events were dropped.
#[derive(Debug, Clone)]
pub enum InProcessServerEvent {
    /// Server request that requires client response/rejection.
    ServerRequest(Box<ServerRequest>),
    /// App-server notification directed to the embedded client.
    ServerNotification(Box<ServerNotification>),
    /// Indicates one or more events were dropped due to backpressure.
    Lagged { skipped: usize },
}

/// Raw app-server request result for typed in-process requests.
///
/// Even on the in-process path, successful responses still travel back through
/// the same JSON-RPC result envelope used by socket/stdio transports because
/// `MessageProcessor` continues to produce that shape internally.
pub type RequestResult = std::result::Result<JsonRpcResult, JSONRPCErrorError>;

#[derive(Debug, Clone)]
pub enum AppServerEvent {
    Lagged {
        skipped: usize,
    },
    ServerNotification(Box<ServerNotification>),
    ServerRequest(Box<ServerRequest>),
    /// The signed-in Cloud account may have changed its discoverable thread inventory.
    CloudInventoryChanged(Option<Box<ServerNotification>>),
    /// A native Cloud reply stream ended. The underlying thread can still be running.
    ThreadStreamDisconnected(Box<codex_app_server_protocol::ErrorNotification>),
    /// The Cloud worker is listening again on the same conversation.
    ThreadStreamReconnected(String),
    Disconnected {
        message: String,
    },
}

impl From<InProcessServerEvent> for AppServerEvent {
    fn from(value: InProcessServerEvent) -> Self {
        match value {
            InProcessServerEvent::Lagged { skipped } => Self::Lagged { skipped },
            InProcessServerEvent::ServerNotification(notification) => {
                Self::ServerNotification(notification)
            }
            InProcessServerEvent::ServerRequest(request) => Self::ServerRequest(request),
        }
    }
}

/// Layered error for typed app-server requests.
///
/// This keeps transport failures, server-side JSON-RPC failures, and response
/// decode failures distinct so callers can decide whether to retry, surface a
/// server error, or treat the response as an internal request/response mismatch.
#[derive(Debug)]
pub enum TypedRequestError {
    Transport {
        method: String,
        source: IoError,
    },
    Server {
        method: String,
        source: JSONRPCErrorError,
    },
    Deserialize {
        method: String,
        source: serde_json::Error,
    },
}

impl fmt::Display for TypedRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { method, source } => {
                write!(f, "{method} transport error: {source}")
            }
            Self::Server { method, source } => {
                write!(
                    f,
                    "{method} failed: {} (code {})",
                    source.message, source.code
                )?;
                if let Some(data) = source.data.as_ref() {
                    write!(f, ", data: {data}")?;
                }
                Ok(())
            }
            Self::Deserialize { method, source } => {
                write!(f, "{method} response decode error: {source}")
            }
        }
    }
}

impl Error for TypedRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport { source, .. } => Some(source),
            Self::Server { .. } => None,
            Self::Deserialize { source, .. } => Some(source),
        }
    }
}

/// Decode a completed transport response without holding its request future.
// Await transport in a separate statement before calling this: dropping the
// completed future must precede any caller-provided Deserialize implementation.
pub fn decode_typed_response<T>(
    method: &str,
    response: IoResult<RequestResult>,
) -> Result<T, TypedRequestError>
where
    T: DeserializeOwned,
{
    let response = response.map_err(|source| TypedRequestError::Transport {
        method: method.to_string(),
        source,
    })?;
    let result = response.map_err(|source| TypedRequestError::Server {
        method: method.to_string(),
        source,
    })?;
    serde_json::from_value(result).map_err(|source| TypedRequestError::Deserialize {
        method: method.to_string(),
        source,
    })
}
