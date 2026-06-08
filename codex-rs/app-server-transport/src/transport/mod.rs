use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingError;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::QueuedOutgoingMessage;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RpcError;
use codex_app_server_protocol::ServerResponse;
use codex_protocol::protocol::W3cTraceContext;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Size of the bounded channels used to communicate between tasks.
pub const CHANNEL_CAPACITY: usize = 128;

mod grpc;
mod grpc_api_conversions;
mod grpc_native_types;

pub use grpc::NativeServerMessage;
pub use grpc::decode_grpc_server_message;
pub use grpc::encode_grpc_client_error;
pub use grpc::encode_grpc_client_notification;
pub use grpc::encode_grpc_client_request;
pub use grpc::encode_grpc_server_response;
pub use grpc::proto as grpc_proto;
pub use grpc::start_grpc_acceptor;

const OVERLOADED_ERROR_CODE: i64 = -32001;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppServerTransport {
    Grpc { bind_address: SocketAddr },
    Off,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppServerTransportParseError {
    UnsupportedListenUrl(String),
    InvalidGrpcListenUrl(String),
}

impl std::fmt::Display for AppServerTransportParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppServerTransportParseError::UnsupportedListenUrl(listen_url) => write!(
                f,
                "unsupported --listen URL `{listen_url}`; expected `grpc://IP:PORT` or `off`"
            ),
            AppServerTransportParseError::InvalidGrpcListenUrl(listen_url) => write!(
                f,
                "invalid gRPC --listen URL `{listen_url}`; expected `grpc://IP:PORT`"
            ),
        }
    }
}

impl std::error::Error for AppServerTransportParseError {}

impl AppServerTransport {
    pub const DEFAULT_LISTEN_URL: &'static str = "grpc://127.0.0.1:0";

    pub fn from_listen_url(listen_url: &str) -> Result<Self, AppServerTransportParseError> {
        if listen_url == "off" {
            return Ok(Self::Off);
        }
        if let Some(socket_addr) = listen_url.strip_prefix("grpc://") {
            let bind_address = socket_addr.parse::<SocketAddr>().map_err(|_| {
                AppServerTransportParseError::InvalidGrpcListenUrl(listen_url.to_string())
            })?;
            return Ok(Self::Grpc { bind_address });
        }
        Err(AppServerTransportParseError::UnsupportedListenUrl(
            listen_url.to_string(),
        ))
    }
}

impl FromStr for AppServerTransport {
    type Err = AppServerTransportParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_listen_url(value)
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum TransportEvent {
    ConnectionOpened {
        connection_id: ConnectionId,
        origin: ConnectionOrigin,
        writer: mpsc::Sender<QueuedOutgoingMessage>,
        disconnect_sender: Option<CancellationToken>,
    },
    ConnectionClosed {
        connection_id: ConnectionId,
    },
    IncomingNativeMessage {
        connection_id: ConnectionId,
        message: NativeIncomingMessage,
    },
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum NativeIncomingMessage {
    Request {
        request: ClientRequest,
        trace: Option<W3cTraceContext>,
    },
    Notification(ClientNotification),
    Response(ServerResponse),
    Error {
        request_id: RequestId,
        error: RpcError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionOrigin {
    InProcess,
    Grpc,
}

static CONNECTION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_connection_id() -> ConnectionId {
    ConnectionId(CONNECTION_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
}

async fn enqueue_native_incoming_message(
    transport_event_tx: &mpsc::Sender<TransportEvent>,
    writer: &mpsc::Sender<QueuedOutgoingMessage>,
    connection_id: ConnectionId,
    message: NativeIncomingMessage,
) -> bool {
    let event = TransportEvent::IncomingNativeMessage {
        connection_id,
        message,
    };
    match transport_event_tx.try_send(event) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Closed(_)) => false,
        Err(mpsc::error::TrySendError::Full(TransportEvent::IncomingNativeMessage {
            connection_id,
            message: NativeIncomingMessage::Request { request, .. },
        })) => {
            let overload_error = OutgoingMessage::Error(OutgoingError {
                id: request.id().clone(),
                error: RpcError {
                    code: OVERLOADED_ERROR_CODE,
                    message: "Server overloaded; retry later.".to_string(),
                    data: None,
                },
            });
            match writer.try_send(QueuedOutgoingMessage::new(overload_error)) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(
                        "dropping overload response for connection {:?}: outbound queue is full",
                        connection_id
                    );
                    true
                }
            }
        }
        Err(mpsc::error::TrySendError::Full(event)) => transport_event_tx.send(event).await.is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_grpc_and_off_transports() {
        assert_eq!(
            AppServerTransport::from_listen_url("grpc://127.0.0.1:50051"),
            Ok(AppServerTransport::Grpc {
                bind_address: "127.0.0.1:50051".parse().expect("valid address"),
            })
        );
        assert_eq!(
            AppServerTransport::from_listen_url("off"),
            Ok(AppServerTransport::Off)
        );
    }
}
