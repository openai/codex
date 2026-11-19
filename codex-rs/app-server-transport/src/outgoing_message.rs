use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotificationEnvelope;
use codex_app_server_protocol::ServerRequest;
use serde::Serialize;
use tokio::sync::oneshot;

pub use codex_app_server_remote_client::ConnectionId;

/// Outgoing message from the server to the client.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum OutgoingMessage {
    Request(ServerRequest),
    /// AppServerNotification is specific to the case where this is run as an
    /// "app server" as opposed to an MCP server.
    AppServerNotification(ServerNotificationEnvelope),
    Response(OutgoingResponse),
    Error(OutgoingError),
}

#[derive(Debug, Clone, Serialize)]
pub struct OutgoingResponse<T = Box<ClientResponsePayload>> {
    pub id: RequestId,
    pub result: T,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutgoingError {
    pub error: JSONRPCErrorError,
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
