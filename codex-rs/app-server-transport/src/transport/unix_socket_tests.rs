use super::AppServerListenerStartup;
use super::AppServerTransport;
use super::CHANNEL_CAPACITY;
use super::EnsuredAppServerListener;
use super::TransportEvent;
use super::app_server_control_socket_path;
use super::ensure_control_socket_listener;
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
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
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
async fn ensure_control_socket_listener_reuses_a_live_listener() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let lock_path = test_startup_lock_path(temp_dir.path());
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
    let start_attempts = Arc::new(AtomicUsize::new(0));

    let listener = ensure_control_socket_listener(socket_path, lock_path, {
        let start_attempts = Arc::clone(&start_attempts);
        move || async move {
            start_attempts.fetch_add(1, Ordering::SeqCst);
            Ok(AppServerListenerStartup::Untracked)
        }
    })
    .await
    .expect("existing listener should be reused");

    assert_eq!(listener, EnsuredAppServerListener::ReusedExisting);
    assert_eq!(start_attempts.load(Ordering::SeqCst), 0);

    shutdown_token.cancel();
    accept_handle.await.expect("acceptor should join");
}

#[tokio::test]
async fn ensure_control_socket_listener_serializes_concurrent_creators() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let lock_path = test_startup_lock_path(temp_dir.path());
    let start_attempts = Arc::new(AtomicUsize::new(0));

    let ensure_one = ensure_control_socket_listener(socket_path.clone(), lock_path.clone(), {
        let socket_path = socket_path.clone();
        let start_attempts = Arc::clone(&start_attempts);
        move || start_test_listener(socket_path, start_attempts)
    });
    let ensure_two = ensure_control_socket_listener(socket_path.clone(), lock_path.clone(), {
        let socket_path = socket_path.clone();
        let start_attempts = Arc::clone(&start_attempts);
        move || start_test_listener(socket_path, start_attempts)
    });
    let ensure_three = ensure_control_socket_listener(socket_path.clone(), lock_path, {
        let start_attempts = Arc::clone(&start_attempts);
        move || start_test_listener(socket_path, start_attempts)
    });

    let (one, two, three) = tokio::join!(ensure_one, ensure_two, ensure_three);
    let listeners = [
        one.expect("first ensure should succeed"),
        two.expect("second ensure should succeed"),
        three.expect("third ensure should succeed"),
    ];

    assert_eq!(start_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        listeners
            .iter()
            .filter(|listener| **listener == EnsuredAppServerListener::StartedNew)
            .count(),
        1
    );
}

#[tokio::test]
async fn ensure_control_socket_listener_waits_for_slow_startup_without_spawning_again() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let lock_path = test_startup_lock_path(temp_dir.path());
    let start_attempts = Arc::new(AtomicUsize::new(0));
    let slow_start_entered = Arc::new(Notify::new());
    let allow_slow_start_to_finish = Arc::new(Notify::new());

    let ensure_one = tokio::spawn(ensure_control_socket_listener(
        socket_path.clone(),
        lock_path.clone(),
        {
            let socket_path = socket_path.clone();
            let start_attempts = Arc::clone(&start_attempts);
            let slow_start_entered = Arc::clone(&slow_start_entered);
            let allow_slow_start_to_finish = Arc::clone(&allow_slow_start_to_finish);
            move || async move {
                slow_start_entered.notify_one();
                allow_slow_start_to_finish.notified().await;
                start_test_listener(socket_path, start_attempts).await
            }
        },
    ));
    slow_start_entered.notified().await;

    let ensure_two = ensure_control_socket_listener(socket_path.clone(), lock_path.clone(), {
        let socket_path = socket_path.clone();
        let start_attempts = Arc::clone(&start_attempts);
        move || start_test_listener(socket_path, start_attempts)
    });
    let ensure_three = ensure_control_socket_listener(socket_path, lock_path, {
        move || async move {
            Err(std::io::Error::other(
                "third ensure must not start another listener",
            ))
        }
    });
    allow_slow_start_to_finish.notify_one();

    let (one, two, three) = tokio::join!(ensure_one, ensure_two, ensure_three);
    let listeners = [
        one.expect("first ensure task should join")
            .expect("first ensure should succeed"),
        two.expect("second ensure should succeed"),
        three.expect("third ensure should succeed"),
    ];

    assert_eq!(start_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        listeners
            .iter()
            .filter(|listener| **listener == EnsuredAppServerListener::StartedNew)
            .count(),
        1
    );
}

#[cfg(unix)]
#[tokio::test]
async fn ensure_control_socket_listener_reports_child_exit_before_bind() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let socket_path = test_socket_path(temp_dir.path());
    let lock_path = test_startup_lock_path(temp_dir.path());

    let err = ensure_control_socket_listener(socket_path, lock_path, move || async move {
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg("exit 17")
            .spawn()?;
        Ok(AppServerListenerStartup::Child(child))
    })
    .await
    .expect_err("child exit before socket bind should surface as an error");

    assert!(
        err.to_string()
            .contains("detached app-server listener exited before opening")
    );
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
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

    shutdown_token.cancel();
    accept_handle.await.expect("acceptor should join");
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

async fn start_test_listener(
    socket_path: AbsolutePathBuf,
    start_attempts: Arc<AtomicUsize>,
) -> IoResult<AppServerListenerStartup> {
    start_attempts.fetch_add(1, Ordering::SeqCst);
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let _accept_handle =
        start_control_socket_acceptor(socket_path, transport_event_tx, CancellationToken::new())
            .await?;
    Ok(AppServerListenerStartup::Untracked)
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
