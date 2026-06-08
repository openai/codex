use codex_app_server_protocol::AttestationGenerateParams;
use codex_app_server_protocol::AttestationGenerateResponse;
use codex_app_server_protocol::ClientNotification as ProtocolClientNotification;
use codex_app_server_protocol::ClientRequest as ProtocolClientRequest;
use codex_app_server_protocol::ClientResponse as ProtocolClientResponse;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::MemoryResetResponse;
use codex_app_server_protocol::RequestId as ProtocolRequestId;
use codex_app_server_protocol::RpcError as ProtocolRpcError;
use codex_app_server_protocol::ServerNotification as ProtocolServerNotification;
use codex_app_server_protocol::ServerRequest as ProtocolServerRequest;
use codex_app_server_protocol::ServerResponse as ProtocolServerResponse;
use pretty_assertions::assert_eq;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;

use super::*;
use crate::outgoing_message::OutgoingError;
use crate::outgoing_message::OutgoingResponse;
use proto::ClientError;
use proto::ClientNotification;
use proto::ClientRequest;
use proto::RequestId;
use proto::RpcError;
use proto::ServerResponse;
use proto::TraceContext;
use proto::client_message;
use proto::client_notification;
use proto::client_request;
use proto::client_response;
use proto::codex_app_server_client::CodexAppServerClient;
use proto::request_id;
use proto::server_message;
use proto::server_notification;
use proto::server_request;
use proto::server_response;

#[tokio::test]
async fn grpc_listener_rejects_non_loopback_addresses() {
    let (transport_event_tx, _transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let err = start_grpc_acceptor(
        "0.0.0.0:0".parse().expect("valid socket address"),
        transport_event_tx,
        CancellationToken::new(),
    )
    .await
    .expect_err("non-loopback listener should be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[tokio::test]
async fn grpc_session_forwards_native_messages_in_both_directions() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("test listener address");
    let (transport_event_tx, mut transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let accept_handle =
        start_grpc_acceptor_on_listener(listener, transport_event_tx, shutdown_token.clone());

    let mut client = CodexAppServerClient::connect(format!("http://{addr}"))
        .await
        .expect("connect gRPC client");
    let (client_tx, client_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let mut server_stream = client
        .session(ReceiverStream::new(client_rx))
        .await
        .expect("open bidirectional session")
        .into_inner();

    let opened = receive_event(&mut transport_event_rx, "connection opened").await;
    let (connection_id, writer) = match opened {
        TransportEvent::ConnectionOpened {
            connection_id,
            origin,
            writer,
            ..
        } => {
            assert_eq!(origin, ConnectionOrigin::Grpc);
            (connection_id, writer)
        }
        event => panic!("expected connection opened event, got {event:?}"),
    };

    client_tx
        .send(ClientMessage {
            payload: Some(client_message::Payload::Request(ClientRequest {
                id: Some(integer_request_id(1)),
                trace: Some(TraceContext {
                    traceparent: Some(
                        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
                    ),
                    tracestate: Some("vendor=value".to_string()),
                }),
                method: Some(client_request::Method::MemoryReset(proto::Empty {})),
            })),
        })
        .await
        .expect("send client request");
    match receive_event(&mut transport_event_rx, "native request").await {
        TransportEvent::IncomingNativeMessage {
            connection_id: incoming_connection_id,
            message: NativeIncomingMessage::Request { request, trace },
        } => {
            assert_eq!(incoming_connection_id, connection_id);
            match request {
                ProtocolClientRequest::MemoryReset { request_id, params } => {
                    assert_eq!(request_id, ProtocolRequestId::Integer(1));
                    assert_eq!(params, None);
                }
                request => panic!("expected memory/reset request, got {request:?}"),
            }
            let trace = trace.expect("trace context");
            assert_eq!(
                trace.traceparent.as_deref(),
                Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            );
            assert_eq!(trace.tracestate.as_deref(), Some("vendor=value"));
        }
        event => panic!("expected native request event, got {event:?}"),
    }

    client_tx
        .send(ClientMessage {
            payload: Some(client_message::Payload::Notification(ClientNotification {
                method: Some(client_notification::Method::Initialized(proto::Empty {})),
            })),
        })
        .await
        .expect("send client notification");
    match receive_event(&mut transport_event_rx, "native notification").await {
        TransportEvent::IncomingNativeMessage {
            connection_id: incoming_connection_id,
            message: NativeIncomingMessage::Notification(notification),
        } => {
            assert_eq!(incoming_connection_id, connection_id);
            assert!(matches!(
                notification,
                ProtocolClientNotification::Initialized
            ));
        }
        event => panic!("expected native notification event, got {event:?}"),
    }

    client_tx
        .send(ClientMessage {
            payload: Some(client_message::Payload::Response(ServerResponse {
                id: Some(integer_request_id(2)),
                method: Some(server_response::Method::AttestationGenerate(
                    proto::AttestationGenerateResponse {
                        token: "attestation-token".to_string(),
                    },
                )),
            })),
        })
        .await
        .expect("send client response");
    match receive_event(&mut transport_event_rx, "native response").await {
        TransportEvent::IncomingNativeMessage {
            connection_id: incoming_connection_id,
            message:
                NativeIncomingMessage::Response(ProtocolServerResponse::AttestationGenerate {
                    request_id,
                    response,
                }),
        } => {
            assert_eq!(incoming_connection_id, connection_id);
            assert_eq!(request_id, ProtocolRequestId::Integer(2));
            assert_eq!(
                response,
                AttestationGenerateResponse {
                    token: "attestation-token".to_string(),
                }
            );
        }
        event => panic!("expected native response event, got {event:?}"),
    }

    client_tx
        .send(ClientMessage {
            payload: Some(client_message::Payload::Error(ClientError {
                id: Some(integer_request_id(3)),
                error: Some(RpcError {
                    code: -32602,
                    message: "invalid approval".to_string(),
                    data: None,
                }),
            })),
        })
        .await
        .expect("send client error");
    match receive_event(&mut transport_event_rx, "native error").await {
        TransportEvent::IncomingNativeMessage {
            connection_id: incoming_connection_id,
            message: NativeIncomingMessage::Error { request_id, error },
        } => {
            assert_eq!(incoming_connection_id, connection_id);
            assert_eq!(request_id, ProtocolRequestId::Integer(3));
            assert_eq!(
                error,
                ProtocolRpcError {
                    code: -32602,
                    message: "invalid approval".to_string(),
                    data: None,
                }
            );
        }
        event => panic!("expected native error event, got {event:?}"),
    }

    writer
        .send(QueuedOutgoingMessage::new(OutgoingMessage::Response(
            OutgoingResponse {
                response: ProtocolClientResponse::MemoryReset {
                    request_id: ProtocolRequestId::Integer(7),
                    response: MemoryResetResponse {},
                },
            },
        )))
        .await
        .expect("queue native response");
    let response = receive_server_message(&mut server_stream).await;
    let Some(server_message::Payload::Response(response)) = response.payload else {
        panic!("expected native response");
    };
    assert_eq!(response.id, Some(integer_request_id(7)));
    let Some(client_response::Method::MemoryReset(payload)) = response.method else {
        panic!("expected memory/reset response");
    };
    assert_eq!(payload, proto::Empty {});

    writer
        .send(QueuedOutgoingMessage::new(OutgoingMessage::Error(
            OutgoingError {
                id: ProtocolRequestId::Integer(8),
                error: ProtocolRpcError {
                    code: -32601,
                    message: "method not found".to_string(),
                    data: None,
                },
            },
        )))
        .await
        .expect("queue native error");
    let error = receive_server_message(&mut server_stream).await;
    let Some(server_message::Payload::Error(error)) = error.payload else {
        panic!("expected native error");
    };
    assert_eq!(error.id, Some(integer_request_id(8)));
    assert_eq!(error.error.expect("rpc error").code, -32601);

    writer
        .send(QueuedOutgoingMessage::new(
            OutgoingMessage::AppServerNotification(ProtocolServerNotification::ConfigWarning(
                ConfigWarningNotification {
                    summary: "check config".to_string(),
                    details: None,
                    path: None,
                    range: None,
                },
            )),
        ))
        .await
        .expect("queue native notification");
    let notification = receive_server_message(&mut server_stream).await;
    let Some(server_message::Payload::Notification(notification)) = notification.payload else {
        panic!("expected native notification");
    };
    let Some(server_notification::Method::ConfigWarning(payload)) = notification.method else {
        panic!("expected config warning notification");
    };
    assert_eq!(
        crate::transport::grpc_native_types::decode_native::<ConfigWarningNotification>(payload)
            .expect("decode config warning notification"),
        ConfigWarningNotification {
            summary: "check config".to_string(),
            details: None,
            path: None,
            range: None,
        }
    );

    writer
        .send(QueuedOutgoingMessage::new(OutgoingMessage::Request(
            ProtocolServerRequest::AttestationGenerate {
                request_id: ProtocolRequestId::Integer(9),
                params: AttestationGenerateParams {},
            },
        )))
        .await
        .expect("queue native server request");
    let request = receive_server_message(&mut server_stream).await;
    let Some(server_message::Payload::Request(request)) = request.payload else {
        panic!("expected native server request");
    };
    assert_eq!(request.id, Some(integer_request_id(9)));
    let Some(server_request::Method::AttestationGenerate(payload)) = request.method else {
        panic!("expected attestation/generate request");
    };
    assert_eq!(payload, proto::Empty {});

    drop(client_tx);
    drop(server_stream);
    drop(client);
    let closed = receive_event(&mut transport_event_rx, "connection closed").await;
    assert!(matches!(
        closed,
        TransportEvent::ConnectionClosed {
            connection_id: closed_connection_id,
        } if closed_connection_id == connection_id
    ));

    shutdown_token.cancel();
    accept_handle.await.expect("join gRPC acceptor");
}

fn integer_request_id(id: i64) -> RequestId {
    RequestId {
        value: Some(request_id::Value::IntegerId(id)),
    }
}

async fn receive_event(
    transport_event_rx: &mut mpsc::Receiver<TransportEvent>,
    description: &str,
) -> TransportEvent {
    timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{description} timeout"))
        .unwrap_or_else(|| panic!("{description} channel closed"))
}

async fn receive_server_message(stream: &mut Streaming<ServerMessage>) -> ServerMessage {
    timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("server message timeout")
        .expect("read server message")
        .expect("server stream ended")
}
