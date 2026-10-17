//! Exercise negotiated and legacy peers through the TCP and local socket listeners.

use super::*;
use crate::OutgoingError;
use crate::OutgoingMessage;
use crate::transport::DaemonShutdownAccess;
use crate::transport::start_control_socket_acceptor;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_uds::UnixStream;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::error::Error;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::client_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[tokio::test]
async fn tcp_compression_negotiates_and_retains_history() -> Result<(), Box<dyn Error>> {
    for compression in [WebSocketCompression::Plain, WebSocketCompression::Zstd] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (transport_event_tx, mut events) = mpsc::channel(CHANNEL_CAPACITY);
        let router = Router::new()
            .fallback(any(websocket_upgrade_handler))
            .with_state(WebSocketListenerState {
                transport_event_tx,
                auth_policy: Arc::new(WebsocketAuthPolicy::default()),
                compression,
            });
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
        });
        for offer in [None, Some(ZSTD_STREAM_PROTOCOL), Some("future-protocol")] {
            let mut request = format!("ws://{address}").into_client_request()?;
            if let Some(offer) = offer {
                request.headers_mut().insert(
                    WEBSOCKET_COMPRESSION_HEADER,
                    HeaderValue::from_static(offer),
                );
            }
            let (websocket, response) = connect_async(request).await?;
            let expected = offer.filter(|offer| {
                compression == WebSocketCompression::Zstd && *offer == ZSTD_STREAM_PROTOCOL
            });
            assert_eq!(
                response
                    .headers()
                    .get(WEBSOCKET_COMPRESSION_HEADER)
                    .map(HeaderValue::to_str)
                    .transpose()?,
                expected
            );
            timeout(
                Duration::from_secs(/*secs*/ 5),
                roundtrip(
                    websocket,
                    &mut events,
                    WebSocketCompression::from_response_header(expected)?,
                ),
            )
            .await??;
        }
        server.abort();
    }
    Ok(())
}

#[tokio::test]
async fn local_socket_compression_negotiates_and_retains_history() -> Result<(), Box<dyn Error>> {
    for compression in [WebSocketCompression::Plain, WebSocketCompression::Zstd] {
        let temp_dir = tempfile::TempDir::new()?;
        let socket_path = AbsolutePathBuf::from_absolute_path(
            temp_dir.path().join("app-server-control").join("rpc.sock"),
        )?;
        let (transport_event_tx, mut events) = mpsc::channel(CHANNEL_CAPACITY);
        let shutdown = CancellationToken::new();
        let server = start_control_socket_acceptor(
            socket_path.clone(),
            transport_event_tx,
            shutdown.clone(),
            DaemonShutdownAccess::Disabled,
            compression,
        )
        .await?;
        for offer in [None, Some(ZSTD_STREAM_PROTOCOL), Some("future-protocol")] {
            let stream = UnixStream::connect(socket_path.as_path()).await?;
            let mut request = "ws://localhost/rpc".into_client_request()?;
            if let Some(offer) = offer {
                request.headers_mut().insert(
                    WEBSOCKET_COMPRESSION_HEADER,
                    HeaderValue::from_static(offer),
                );
            }
            let (websocket, response) = client_async(request, stream).await?;
            let expected = offer.filter(|offer| {
                compression == WebSocketCompression::Zstd && *offer == ZSTD_STREAM_PROTOCOL
            });
            assert_eq!(
                response
                    .headers()
                    .get(WEBSOCKET_COMPRESSION_HEADER)
                    .map(HeaderValue::to_str)
                    .transpose()?,
                expected
            );
            timeout(
                Duration::from_secs(/*secs*/ 5),
                roundtrip(
                    websocket,
                    &mut events,
                    WebSocketCompression::from_response_header(expected)?,
                ),
            )
            .await??;
        }
        shutdown.cancel();
        server.await?;
    }
    Ok(())
}

async fn roundtrip<S: AsyncRead + AsyncWrite + Unpin>(
    mut websocket: WebSocketStream<S>,
    events: &mut mpsc::Receiver<TransportEvent>,
    compression: WebSocketCompression,
) -> Result<(), Box<dyn Error>> {
    let Some(TransportEvent::ConnectionOpened { writer, .. }) = events.recv().await else {
        return Err("expected connection opened".into());
    };
    let mut encoder = ZstdEncoder::new()?;
    let mut decoder = ZstdDecoder::new(/*max_message_size*/ 1024)?;
    for index in 0..2 {
        let request = format!(r#"{{"method":"example","id":{index}}}"#);
        let message = match compression {
            WebSocketCompression::Plain => {
                TungsteniteWebSocketMessage::Text(request.clone().into())
            }
            WebSocketCompression::Zstd => {
                TungsteniteWebSocketMessage::Binary(encoder.encode(&request)?.into())
            }
        };
        websocket.send(message).await?;
        let Some(TransportEvent::IncomingMessage { message, .. }) = events.recv().await else {
            return Err("expected incoming RPC".into());
        };
        assert_eq!(message, serde_json::from_str::<JSONRPCMessage>(&request)?);
        let response = OutgoingMessage::Error(OutgoingError {
            id: RequestId::Integer(index),
            error: JSONRPCErrorError {
                code: -32601,
                message: "unknown method".to_string(),
                data: None,
            },
        });
        let expected = serde_json::to_string(&response)?;
        writer.send(QueuedOutgoingMessage::new(response)).await?;
        let text = loop {
            let response = websocket.next().await.ok_or("expected response")??;
            match (compression, response) {
                (WebSocketCompression::Plain, TungsteniteWebSocketMessage::Text(text)) => {
                    break text.to_string();
                }
                (WebSocketCompression::Zstd, TungsteniteWebSocketMessage::Binary(bytes)) => {
                    break decoder.decode(&bytes)?;
                }
                (
                    _,
                    TungsteniteWebSocketMessage::Ping(_) | TungsteniteWebSocketMessage::Pong(_),
                ) => {}
                _ => return Err("response has incorrect WebSocket message type".into()),
            }
        };
        assert_eq!(text, expected);
        websocket
            .send(TungsteniteWebSocketMessage::Ping(Bytes::from_static(
                b"check",
            )))
            .await?;
        assert_eq!(
            websocket.next().await.ok_or("expected pong")??,
            TungsteniteWebSocketMessage::Pong(Bytes::from_static(b"check"))
        );
    }
    websocket.close(/*msg*/ None).await?;
    assert!(matches!(
        events.recv().await,
        Some(TransportEvent::ConnectionClosed { .. })
    ));
    Ok(())
}
