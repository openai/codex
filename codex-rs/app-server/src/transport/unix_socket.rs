use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::path::Path;

use super::TransportEvent;
use crate::transport::websocket::run_websocket_connection;
use codex_uds::UnixListener;
use codex_uds::UnixStream;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

#[cfg(unix)]
const CONTROL_SOCKET_MODE: u32 = 0o600;

pub(crate) async fn start_control_socket_acceptor(
    socket_path: AbsolutePathBuf,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
) -> IoResult<JoinHandle<()>> {
    prepare_control_socket_path(socket_path.as_path()).await?;
    let listener = UnixListener::bind(socket_path.as_path()).await?;
    let socket_guard = ControlSocketFileGuard { socket_path };
    set_control_socket_permissions(socket_guard.socket_path.as_path()).await?;
    info!(
        socket_path = %socket_guard.socket_path.display(),
        "app-server control socket listening"
    );

    Ok(tokio::spawn(run_control_socket_acceptor(
        listener,
        transport_event_tx,
        shutdown_token,
        socket_guard,
    )))
}

async fn run_control_socket_acceptor(
    mut listener: UnixListener,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    socket_guard: ControlSocketFileGuard,
) {
    let _socket_guard = socket_guard;
    loop {
        let stream = tokio::select! {
            _ = shutdown_token.cancelled() => {
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok(stream) => stream,
                    Err(err) => {
                        if matches!(
                            err.kind(),
                            ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset | ErrorKind::Interrupted
                        ) {
                            warn!("recoverable control socket accept error: {err}");
                            continue;
                        }
                        error!("control socket accept error: {err}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                }
            }
        };

        let transport_event_tx = transport_event_tx.clone();
        tokio::spawn(async move {
            let websocket_stream =
                WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
            let (websocket_writer, websocket_reader) = websocket_stream.split();
            run_websocket_connection(websocket_writer, websocket_reader, transport_event_tx).await;
        });
    }
    info!("control socket acceptor shutting down");
}

async fn prepare_control_socket_path(socket_path: &Path) -> IoResult<()> {
    if let Some(parent) = socket_path.parent() {
        codex_uds::prepare_private_socket_directory(parent).await?;
    }

    match UnixStream::connect(socket_path).await {
        Ok(_stream) => {
            return Err(std::io::Error::new(
                ErrorKind::AddrInUse,
                format!(
                    "app-server control socket is already in use at {}",
                    socket_path.display()
                ),
            ));
        }
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) if err.kind() == ErrorKind::ConnectionRefused => {}
        Err(err) => {
            if !socket_path.exists() {
                return Ok(());
            }
            return Err(err);
        }
    }

    if !socket_path.try_exists()? {
        return Ok(());
    }

    if !codex_uds::is_stale_socket_path(socket_path).await? {
        return Err(std::io::Error::new(
            ErrorKind::AlreadyExists,
            format!(
                "app-server control socket path exists and is not a socket: {}",
                socket_path.display()
            ),
        ));
    }
    tokio::fs::remove_file(socket_path).await
}

#[cfg(unix)]
async fn set_control_socket_permissions(socket_path: &Path) -> IoResult<()> {
    use std::os::unix::fs::PermissionsExt;

    tokio::fs::set_permissions(
        socket_path,
        std::fs::Permissions::from_mode(CONTROL_SOCKET_MODE),
    )
    .await
}

#[cfg(not(unix))]
async fn set_control_socket_permissions(_socket_path: &Path) -> IoResult<()> {
    Ok(())
}

struct ControlSocketFileGuard {
    socket_path: AbsolutePathBuf,
}

impl Drop for ControlSocketFileGuard {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(self.socket_path.as_path())
            && err.kind() != ErrorKind::NotFound
        {
            warn!(
                socket_path = %self.socket_path.display(),
                %err,
                "failed to remove app-server control socket file"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::CHANNEL_CAPACITY;
    use codex_app_server_protocol::JSONRPCMessage;
    use codex_app_server_protocol::JSONRPCNotification;
    use futures::SinkExt;
    use futures::StreamExt;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::tungstenite::Bytes;
    use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
    use tokio_tungstenite::tungstenite::protocol::Role;

    #[tokio::test]
    async fn control_socket_acceptor_forwards_websocket_text_messages_and_pings() {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let socket_path = test_socket_path(temp_dir.path());
        let (transport_event_tx, mut transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let accept_handle = start_control_socket_acceptor(
            socket_path.clone(),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("control socket acceptor should start");

        let stream = connect_to_socket(socket_path.as_path())
            .await
            .expect("client should connect");
        let mut websocket = WebSocketStream::from_raw_socket(stream, Role::Client, None).await;

        let opened = timeout(Duration::from_secs(1), transport_event_rx.recv())
            .await
            .expect("connection opened event should arrive")
            .expect("connection opened event");
        let connection_id = match opened {
            TransportEvent::ConnectionOpened { connection_id, .. } => connection_id,
            _ => panic!("expected connection opened event"),
        };

        let notification = JSONRPCMessage::Notification(JSONRPCNotification {
            method: "initialized".to_string(),
            params: None,
        });
        websocket
            .send(WebSocketMessage::Text(
                serde_json::to_string(&notification)
                    .expect("notification should serialize")
                    .into(),
            ))
            .await
            .expect("notification should send");

        let incoming = timeout(Duration::from_secs(1), transport_event_rx.recv())
            .await
            .expect("incoming message event should arrive")
            .expect("incoming message event");
        assert_eq!(
            match incoming {
                TransportEvent::IncomingMessage {
                    connection_id: incoming_connection_id,
                    message,
                } => (incoming_connection_id, message),
                _ => panic!("expected incoming message event"),
            },
            (connection_id, notification)
        );

        websocket
            .send(WebSocketMessage::Ping(Bytes::from_static(b"check")))
            .await
            .expect("ping should send");
        let pong = timeout(Duration::from_secs(1), websocket.next())
            .await
            .expect("pong should arrive")
            .expect("pong frame")
            .expect("pong should be valid");
        assert_eq!(pong, WebSocketMessage::Pong(Bytes::from_static(b"check")));

        websocket.close(None).await.expect("close should send");
        let closed = timeout(Duration::from_secs(1), transport_event_rx.recv())
            .await
            .expect("connection closed event should arrive")
            .expect("connection closed event");
        assert!(matches!(
            closed,
            TransportEvent::ConnectionClosed {
                connection_id: closed_connection_id,
            } if closed_connection_id == connection_id
        ));

        shutdown_token.cancel();
        accept_handle.await.expect("acceptor should join");
        assert_socket_path_removed(socket_path.as_path());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn control_socket_file_is_private_after_bind() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let socket_path = test_socket_path(temp_dir.path());
        let (transport_event_tx, _transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let accept_handle = start_control_socket_acceptor(
            socket_path.clone(),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("control socket acceptor should start");

        let metadata = tokio::fs::metadata(socket_path.as_path())
            .await
            .expect("socket metadata should exist");
        assert_eq!(metadata.permissions().mode() & 0o777, CONTROL_SOCKET_MODE);

        shutdown_token.cancel();
        accept_handle.await.expect("acceptor should join");
    }

    fn test_socket_path(temp_dir: &Path) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(
            temp_dir
                .join("app-server-control")
                .join("app-server-control.sock"),
        )
        .expect("socket path should resolve")
    }

    async fn connect_to_socket(socket_path: &Path) -> IoResult<UnixStream> {
        UnixStream::connect(socket_path).await
    }

    #[cfg(unix)]
    fn assert_socket_path_removed(socket_path: &Path) {
        assert!(!socket_path.exists());
    }

    #[cfg(windows)]
    fn assert_socket_path_removed(_socket_path: &Path) {
        // uds_windows uses a regular filesystem path as its rendezvous point,
        // but there is no Unix socket filesystem node to assert on.
    }
}
