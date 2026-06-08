use super::CHANNEL_CAPACITY;
use super::ConnectionOrigin;
use super::NativeIncomingMessage;
use super::TransportEvent;
use super::enqueue_native_incoming_message;
use super::grpc_api_conversions::decode_client_request;
use super::grpc_api_conversions::decode_client_response;
use super::grpc_api_conversions::decode_error;
use super::grpc_api_conversions::decode_error_response;
use super::grpc_api_conversions::decode_server_notification;
use super::grpc_api_conversions::decode_server_request;
use super::grpc_api_conversions::decode_server_response;
use super::grpc_api_conversions::encode_client_error;
use super::grpc_api_conversions::encode_client_request;
use super::grpc_api_conversions::encode_client_response;
use super::grpc_api_conversions::encode_error;
use super::grpc_api_conversions::encode_server_notification;
use super::grpc_api_conversions::encode_server_request;
use super::grpc_api_conversions::encode_server_response;
use super::next_connection_id;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::QueuedOutgoingMessage;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RpcError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_protocol::protocol::W3cTraceContext;
use futures::Stream;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tonic::Streaming;
use tonic::transport::Server;
use tracing::error;
use tracing::info;
use tracing::warn;

#[path = "proto/codex.app_server.v2.rs"]
pub mod proto;

use proto::ClientMessage;
use proto::HealthRequest;
use proto::HealthResponse;
use proto::SchemaRequest;
use proto::SchemaResponse;
use proto::ServerMessage;
use proto::codex_app_server_server::CodexAppServer;
use proto::codex_app_server_server::CodexAppServerServer;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum NativeServerMessage {
    Response(ClientResponse),
    Error {
        request_id: RequestId,
        error: RpcError,
    },
    Notification(ServerNotification),
    Request(ServerRequest),
}

pub fn encode_grpc_client_request(
    request: ClientRequest,
    trace: Option<W3cTraceContext>,
) -> Result<ClientMessage, Status> {
    Ok(ClientMessage {
        payload: Some(proto::client_message::Payload::Request(
            encode_client_request(request, trace)?,
        )),
    })
}

pub fn encode_grpc_client_notification(
    notification: ClientNotification,
) -> Result<ClientMessage, Status> {
    let method = match notification {
        ClientNotification::Initialized => {
            proto::client_notification::Method::Initialized(proto::Empty {})
        }
    };
    Ok(ClientMessage {
        payload: Some(proto::client_message::Payload::Notification(
            proto::ClientNotification {
                method: Some(method),
            },
        )),
    })
}

pub fn encode_grpc_server_response(response: ServerResponse) -> Result<ClientMessage, Status> {
    Ok(ClientMessage {
        payload: Some(proto::client_message::Payload::Response(
            encode_server_response(response)?,
        )),
    })
}

pub fn encode_grpc_client_error(
    request_id: RequestId,
    error: RpcError,
) -> Result<ClientMessage, Status> {
    Ok(ClientMessage {
        payload: Some(proto::client_message::Payload::Error(encode_client_error(
            request_id, error,
        )?)),
    })
}

pub fn decode_grpc_server_message(message: ServerMessage) -> Result<NativeServerMessage, Status> {
    match message
        .payload
        .ok_or_else(|| Status::invalid_argument("missing server message payload"))?
    {
        proto::server_message::Payload::Response(response) => Ok(NativeServerMessage::Response(
            decode_client_response(response)?,
        )),
        proto::server_message::Payload::Error(error) => {
            let (request_id, error) = decode_error_response(error)?;
            Ok(NativeServerMessage::Error { request_id, error })
        }
        proto::server_message::Payload::Notification(notification) => Ok(
            NativeServerMessage::Notification(decode_server_notification(notification)?),
        ),
        proto::server_message::Payload::Request(request) => Ok(NativeServerMessage::Request(
            decode_server_request(request)?,
        )),
    }
}

const GRPC_OUTBOUND_CHANNEL_CAPACITY: usize = 32 * 1024;
const GRPC_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
const GRPC_MAX_CONCURRENT_REQUESTS_PER_CONNECTION: usize = 8;
const GRPC_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const GRPC_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);
const GRPC_PROTO_SOURCE: &str = include_str!("proto/codex.app_server.v2.proto");
const _: () = assert!(GRPC_OUTBOUND_CHANNEL_CAPACITY > CHANNEL_CAPACITY);

struct GrpcResponse {
    result: Result<ServerMessage, Status>,
    write_complete_tx: Option<oneshot::Sender<()>>,
}

struct GrpcResponseStream {
    receiver: mpsc::Receiver<GrpcResponse>,
}

impl Stream for GrpcResponseStream {
    type Item = Result<ServerMessage, Status>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.receiver.poll_recv(cx) {
            Poll::Ready(Some(response)) => {
                if let Some(write_complete_tx) = response.write_complete_tx {
                    let _ = write_complete_tx.send(());
                }
                Poll::Ready(Some(response.result))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Clone)]
struct GrpcService {
    transport_event_tx: mpsc::Sender<TransportEvent>,
}

#[tonic::async_trait]
impl CodexAppServer for GrpcService {
    type SessionStream = GrpcResponseStream;

    async fn session(
        &self,
        request: Request<Streaming<ClientMessage>>,
    ) -> Result<Response<Self::SessionStream>, Status> {
        let peer_addr = request.remote_addr();
        let connection_id = next_connection_id();
        let (writer_tx, writer_rx) =
            mpsc::channel::<QueuedOutgoingMessage>(GRPC_OUTBOUND_CHANNEL_CAPACITY);
        let writer_tx_for_reader = writer_tx.clone();
        let disconnect_token = CancellationToken::new();
        self.transport_event_tx
            .send(TransportEvent::ConnectionOpened {
                connection_id,
                origin: ConnectionOrigin::Grpc,
                writer: writer_tx,
                disconnect_sender: Some(disconnect_token.clone()),
            })
            .await
            .map_err(|_| Status::unavailable("app-server processor unavailable"))?;

        info!(%connection_id, ?peer_addr, "gRPC client connected");
        let (response_tx, response_rx) = mpsc::channel(CHANNEL_CAPACITY);
        tokio::spawn(run_grpc_connection(
            request.into_inner(),
            writer_rx,
            writer_tx_for_reader,
            response_tx,
            self.transport_event_tx.clone(),
            connection_id,
            disconnect_token,
        ));

        Ok(Response::new(GrpcResponseStream {
            receiver: response_rx,
        }))
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse {
            status: proto::health_response::ServingStatus::Serving.into(),
        }))
    }

    async fn schema(
        &self,
        _request: Request<SchemaRequest>,
    ) -> Result<Response<SchemaResponse>, Status> {
        Ok(Response::new(SchemaResponse {
            proto_source: GRPC_PROTO_SOURCE.to_string(),
        }))
    }
}

#[allow(clippy::print_stderr)]
fn print_grpc_startup_banner(addr: SocketAddr) {
    eprintln!("codex app-server (experimental gRPC)");
    eprintln!("  listening on: grpc://{addr}");
    eprintln!("  rpc: codex.app_server.v2.CodexAppServer/Session (bidirectional streaming)");
}

pub async fn start_grpc_acceptor(
    bind_address: SocketAddr,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
) -> IoResult<JoinHandle<()>> {
    if !bind_address.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "refusing to start experimental gRPC listener {bind_address}; only loopback listeners are currently supported"
            ),
        ));
    }

    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    print_grpc_startup_banner(local_addr);
    info!("app-server gRPC listening on grpc://{local_addr}");

    Ok(start_grpc_acceptor_on_listener(
        listener,
        transport_event_tx,
        shutdown_token,
    ))
}

fn start_grpc_acceptor_on_listener(
    listener: TcpListener,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let service = CodexAppServerServer::new(GrpcService { transport_event_tx })
            .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
            .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE);
        let result = Server::builder()
            .concurrency_limit_per_connection(GRPC_MAX_CONCURRENT_REQUESTS_PER_CONNECTION)
            .http2_keepalive_interval(Some(GRPC_KEEPALIVE_INTERVAL))
            .http2_keepalive_timeout(Some(GRPC_KEEPALIVE_TIMEOUT))
            .tcp_keepalive(Some(GRPC_KEEPALIVE_INTERVAL))
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                shutdown_token.cancelled().await;
            })
            .await;
        if let Err(err) = result {
            error!("gRPC acceptor failed: {err}");
        }
        info!("gRPC acceptor shutting down");
    })
}

async fn run_grpc_connection(
    mut inbound: Streaming<ClientMessage>,
    mut writer_rx: mpsc::Receiver<QueuedOutgoingMessage>,
    writer_tx_for_reader: mpsc::Sender<QueuedOutgoingMessage>,
    response_tx: mpsc::Sender<GrpcResponse>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    connection_id: ConnectionId,
    disconnect_token: CancellationToken,
) {
    let mut inbound_closed = false;
    loop {
        tokio::select! {
            _ = disconnect_token.cancelled() => break,
            _ = response_tx.closed() => break,
            incoming = inbound.message(), if !inbound_closed => {
                match incoming {
                    Ok(Some(message)) => {
                        let native_message = match decode_client_message(message) {
                            Ok(message) => message,
                            Err(status) => {
                                let _ = response_tx
                                    .send(GrpcResponse {
                                        result: Err(status),
                                        write_complete_tx: None,
                                    })
                                    .await;
                                break;
                            }
                        };
                        if !enqueue_native_incoming_message(
                            &transport_event_tx,
                            &writer_tx_for_reader,
                            connection_id,
                            native_message,
                        )
                        .await
                        {
                            break;
                        }
                    }
                    Ok(None) => inbound_closed = true,
                    Err(err) => {
                        warn!(%connection_id, "gRPC receive error: {err}");
                        break;
                    }
                }
            }
            queued_message = writer_rx.recv() => {
                let Some(queued_message) = queued_message else {
                    break;
                };
                let message = match encode_outgoing_message(queued_message.message) {
                    Ok(message) => message,
                    Err(status) => {
                        let _ = response_tx
                            .send(GrpcResponse {
                                result: Err(status),
                                write_complete_tx: None,
                            })
                            .await;
                        break;
                    }
                };
                if response_tx
                    .send(GrpcResponse {
                        result: Ok(message),
                        write_complete_tx: queued_message.write_complete_tx,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    disconnect_token.cancel();
    let _ = transport_event_tx
        .send(TransportEvent::ConnectionClosed { connection_id })
        .await;
    info!(%connection_id, "gRPC client disconnected");
}

fn decode_client_message(message: ClientMessage) -> Result<NativeIncomingMessage, Status> {
    match message
        .payload
        .ok_or_else(|| Status::invalid_argument("missing client message payload"))?
    {
        proto::client_message::Payload::Request(request) => {
            let (request, trace) = decode_client_request(request)?;
            Ok(NativeIncomingMessage::Request { request, trace })
        }
        proto::client_message::Payload::Notification(notification) => {
            match notification
                .method
                .ok_or_else(|| Status::invalid_argument("missing client notification method"))?
            {
                proto::client_notification::Method::Initialized(_) => {
                    Ok(NativeIncomingMessage::Notification(
                        codex_app_server_protocol::ClientNotification::Initialized,
                    ))
                }
            }
        }
        proto::client_message::Payload::Response(response) => Ok(NativeIncomingMessage::Response(
            decode_server_response(response)?,
        )),
        proto::client_message::Payload::Error(error) => {
            let (request_id, error) = decode_error(error)?;
            Ok(NativeIncomingMessage::Error { request_id, error })
        }
    }
}

fn encode_outgoing_message(message: OutgoingMessage) -> Result<ServerMessage, Status> {
    let payload = match message {
        OutgoingMessage::Request(request) => {
            proto::server_message::Payload::Request(encode_server_request(request)?)
        }
        OutgoingMessage::AppServerNotification(notification) => {
            proto::server_message::Payload::Notification(encode_server_notification(notification)?)
        }
        OutgoingMessage::Response(response) => {
            proto::server_message::Payload::Response(encode_client_response(response.response)?)
        }
        OutgoingMessage::Error(error) => {
            proto::server_message::Payload::Error(encode_error(error.id, error.error)?)
        }
    };
    Ok(ServerMessage {
        payload: Some(payload),
    })
}

#[cfg(test)]
#[path = "grpc_tests.rs"]
mod native_tests;
