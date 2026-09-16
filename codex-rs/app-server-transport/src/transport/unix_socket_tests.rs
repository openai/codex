use super::AppServerTransport;
use super::CHANNEL_CAPACITY;
use super::DaemonShutdownAccess;
use super::TransportEvent;
use super::acquire_app_server_startup_lock;
use super::app_server_control_socket_path;
use super::start_control_socket_acceptor;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_core::config::find_codex_home;
use codex_uds::UnixStream;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::io::Result as IoResult;
use std::path::Path;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Bytes;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
use tokio_util::sync::CancellationToken;

#[test]
fn listen_unix_socket_parses_as_unix_socket_transport() {
    assert_eq!(
        AppServerTransport::from_listen_url("unix://"),
        Ok(AppServerTransport::UnixSocket {
            socket_path: default_control_socket_path()
        })
    );
}

#[test]
fn listen_unix_socket_accepts_absolute_custom_path() {
    assert_eq!(
        AppServerTransport::from_listen_url("unix:///tmp/codex.sock"),
        Ok(AppServerTransport::UnixSocket {
            socket_path: absolute_path("/tmp/codex.sock")
        })
    );
}

#[test]
fn listen_unix_socket_accepts_relative_custom_path() {
    assert_eq!(
        AppServerTransport::from_listen_url("unix://codex.sock"),
        Ok(AppServerTransport::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")
                .expect("relative path should resolve")
        })
    );
}

#[tokio::test]
async fn control_socket_acceptor_upgrades_and_forwards_websocket_text_messages_and_pings() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let accept_handle = start_control_socket_acceptor(
        socket_path.clone(),
        transport_event_tx,
        shutdown_token.clone(),
        DaemonShutdownAccess::Disabled,
    )
    .await
    .expect("control socket acceptor should start");

    let stream = connect_to_socket(socket_path.as_path())
        .await
        .expect("client should connect");
    let (mut websocket, response) = client_async("ws://localhost/rpc", stream)
        .await
        .expect("websocket upgrade should complete");
    assert_eq!(response.status().as_u16(), 101);

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

#[tokio::test]
async fn shutdown_is_only_accepted_on_managed_local_socket_for_its_own_pid() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
    let shutdown = CancellationToken::new();
    let acceptor = start_control_socket_acceptor(
        socket_path.clone(),
        tx,
        shutdown.clone(),
        DaemonShutdownAccess::Disabled,
    )
    .await
    .expect("acceptor");

    let stream = connect_to_socket(socket_path.as_path())
        .await
        .expect("connect");
    assert!(
        client_async("ws://localhost/daemon/shutdown", stream)
            .await
            .is_err()
    );
    assert!(rx.try_recv().is_err());
    shutdown.cancel();
    acceptor.await.expect("acceptor shutdown");

    let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
    let shutdown = CancellationToken::new();
    let acceptor = start_control_socket_acceptor(
        socket_path.clone(),
        tx,
        shutdown.clone(),
        DaemonShutdownAccess::Managed,
    )
    .await
    .expect("managed acceptor");
    let stream = connect_to_socket(socket_path.as_path())
        .await
        .expect("connect");
    let (mut websocket, _) = client_async("ws://localhost/daemon/shutdown", stream)
        .await
        .expect("upgrade");
    websocket
        .send(WebSocketMessage::Text("0".into()))
        .await
        .expect("wrong pid");
    assert!(!matches!(
        websocket.next().await,
        Some(Ok(WebSocketMessage::Text(_)))
    ));
    assert!(rx.try_recv().is_err());

    let stream = connect_to_socket(socket_path.as_path())
        .await
        .expect("connect");
    let (mut websocket, _) = client_async("ws://localhost/daemon/shutdown", stream)
        .await
        .expect("upgrade");
    let pid = std::process::id().to_string();
    websocket
        .send(WebSocketMessage::Text(pid.clone().into()))
        .await
        .expect("request");
    assert_eq!(
        websocket.next().await.expect("ack").expect("ack frame"),
        WebSocketMessage::Text(pid.into())
    );
    assert!(
        rx.try_recv().is_err(),
        "server must wait until the ack is received"
    );
    websocket.close(None).await.expect("confirm receipt");
    assert!(matches!(
        timeout(Duration::from_secs(2), rx.recv()).await,
        Ok(Some(TransportEvent::DaemonShutdown))
    ));
    shutdown.cancel();
    acceptor.await.expect("acceptor shutdown");
}

#[tokio::test]
async fn app_server_startup_lock_serializes_waiters() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let lock_path = test_startup_lock_path(temp_dir.path());
    let first_lock = acquire_app_server_startup_lock(lock_path.clone())
        .await
        .expect("first startup lock should succeed");
    let mut second_lock = tokio::spawn(acquire_app_server_startup_lock(lock_path));

    assert!(
        timeout(Duration::from_millis(100), &mut second_lock)
            .await
            .is_err()
    );

    drop(first_lock);
    second_lock
        .await
        .expect("second startup lock task should join")
        .expect("second startup lock should succeed");
}

#[cfg(unix)]
#[tokio::test]
async fn control_socket_rejects_writable_parent_without_changing_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let path = AbsolutePathBuf::from_absolute_path(directory.path().join("rpc.sock")).unwrap();
    let (tx, _rx) = mpsc::channel(CHANNEL_CAPACITY);
    let error = start_control_socket_acceptor(
        path.clone(),
        tx,
        CancellationToken::new(),
        DaemonShutdownAccess::Disabled,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        std::fs::metadata(directory.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o777
    );
    assert!(std::fs::symlink_metadata(path).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn control_socket_file_is_private_after_bind() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let parent = socket_path.as_path().parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let accept_handle = start_control_socket_acceptor(
        socket_path.clone(),
        transport_event_tx,
        shutdown_token.clone(),
        DaemonShutdownAccess::Disabled,
    )
    .await
    .expect("control socket acceptor should start");

    let metadata = tokio::fs::metadata(socket_path.as_path())
        .await
        .expect("socket metadata should exist");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(
        std::fs::metadata(parent).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let physical_path = std::fs::read_link(socket_path.as_path()).expect("rendezvous symlink");
    assert_eq!(
        physical_path.parent(),
        Some(
            codex_uds::shared_daemon_socket_directory()
                .unwrap()
                .as_path()
        )
    );

    shutdown_token.cancel();
    accept_handle.await.expect("acceptor should join");
    assert!(!physical_path.exists());
    assert!(std::fs::symlink_metadata(socket_path.as_path()).is_err());

    // Simulate a dangling rendezvous left by an interrupted cleanup.
    std::os::unix::fs::symlink(&physical_path, socket_path.as_path()).unwrap();
    let (sender, _receiver) = mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown = CancellationToken::new();
    let (first, second) = tokio::join!(
        start_control_socket_acceptor(
            socket_path.clone(),
            sender.clone(),
            shutdown.clone(),
            DaemonShutdownAccess::Disabled,
        ),
        start_control_socket_acceptor(
            socket_path.clone(),
            sender,
            shutdown.clone(),
            DaemonShutdownAccess::Disabled,
        ),
    );
    let (acceptor, error) = match (first, second) {
        (Ok(acceptor), Err(error)) | (Err(error), Ok(acceptor)) => (acceptor, error),
        _ => panic!("exactly one concurrent restart should replace the stale symlink"),
    };
    assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    let _client = connect_to_socket(socket_path.as_path()).await.unwrap();

    // Cleanup must not remove a replacement at the advertised path.
    std::fs::remove_file(socket_path.as_path()).unwrap();
    std::fs::write(socket_path.as_path(), b"replacement").unwrap();
    shutdown.cancel();
    acceptor.await.unwrap();
    assert_eq!(
        std::fs::read(socket_path.as_path()).unwrap(),
        b"replacement"
    );
    assert!(!physical_path.exists());
}

#[cfg(windows)]
#[tokio::test]
async fn control_socket_pins_directory_until_shutdown() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let directory = socket_path.as_path().parent().unwrap();
    let moved = temp_dir.path().join("moved");
    let (tx, _rx) = mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown = CancellationToken::new();
    let acceptor = start_control_socket_acceptor(
        socket_path.clone(),
        tx,
        shutdown.clone(),
        DaemonShutdownAccess::Disabled,
    )
    .await
    .expect("acceptor");
    assert!(std::fs::rename(directory, &moved).is_err());
    shutdown.cancel();
    acceptor.await.expect("shutdown");
    std::fs::rename(directory, moved).expect("directory unpinned after cleanup");
}

fn absolute_path(path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(path).expect("absolute path")
}

fn default_control_socket_path() -> AbsolutePathBuf {
    let codex_home = find_codex_home().expect("codex home");
    app_server_control_socket_path(&codex_home).expect("default control socket path")
}

fn test_socket_path(temp_dir: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(
        temp_dir
            .join("app-server-control")
            .join("app-server-control.sock"),
    )
    .expect("socket path should resolve")
}

fn test_startup_lock_path(temp_dir: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(
        temp_dir
            .join("app-server-control")
            .join("app-server-startup.lock"),
    )
    .expect("startup lock path should resolve")
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
