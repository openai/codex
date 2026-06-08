use std::fmt;

use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RpcError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
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
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum OutgoingMessage {
    Request(ServerRequest),
    /// AppServerNotification is specific to the case where this is run as an
    /// "app server" as opposed to an MCP server.
    AppServerNotification(ServerNotification),
    Response(OutgoingResponse),
    Error(OutgoingError),
}

#[derive(Debug, Clone)]
pub struct OutgoingResponse {
    pub response: ClientResponse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutgoingError {
    pub error: RpcError,
    pub id: RequestId,
}

#[derive(Debug)]
pub struct QueuedOutgoingMessage {
    pub message: OutgoingMessage,
    pub write_complete_tx: Option<oneshot::Sender<()>>,
}

impl QueuedOutgoingMessage {
    pub fn new(message: OutgoingMessage) -> Self {
        Self {
            message,
            write_complete_tx: None,
        }
    }
}
