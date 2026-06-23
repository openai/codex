use std::fmt;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use serde::Serialize;
use tokio::sync::oneshot;

/// Stable identifier for a transport connection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConnectionId(pub u64);

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Outgoing message from the server to the client.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum OutgoingMessage {
    Request(ServerRequest),
    /// AppServerNotification is specific to the case where this is run as an
    /// "app server" as opposed to an MCP server.
    AppServerNotification(ServerNotification),
    Response(OutgoingResponse),
    Error(OutgoingError),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutgoingResponse {
    pub id: RequestId,
    pub result: Result,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutgoingError {
    pub error: JSONRPCErrorError,
    pub id: RequestId,
}

/// Details available after an outgoing message has been written by a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutgoingWriteComplete {
    /// Length of the JSON text written by the transport, when serialization occurred.
    pub serialized_bytes: Option<usize>,
}

#[derive(Debug)]
pub struct QueuedOutgoingMessage {
    pub message: OutgoingMessage,
    pub write_complete_tx: Option<oneshot::Sender<OutgoingWriteComplete>>,
}

impl QueuedOutgoingMessage {
    pub fn new(message: OutgoingMessage) -> Self {
        Self {
            message,
            write_complete_tx: None,
        }
    }
}
