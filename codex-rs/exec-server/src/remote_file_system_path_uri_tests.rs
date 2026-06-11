#![allow(clippy::expect_used)]

#[cfg(windows)]
use codex_app_server_protocol::JSONRPCMessage;
#[cfg(windows)]
use codex_app_server_protocol::JSONRPCResponse;
#[cfg(windows)]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
#[cfg(windows)]
use futures::SinkExt;
#[cfg(windows)]
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::io;
#[cfg(windows)]
use tokio::net::TcpListener;
#[cfg(windows)]
use tokio::net::TcpStream;
#[cfg(windows)]
use tokio::sync::oneshot;
#[cfg(windows)]
use tokio::time::Duration;
#[cfg(windows)]
use tokio::time::timeout;
#[cfg(windows)]
use tokio_tungstenite::WebSocketStream;
#[cfg(windows)]
use tokio_tungstenite::accept_async;
#[cfg(windows)]
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::client_api::ExecServerTransportParams;
#[cfg(windows)]
use crate::protocol::FS_READ_FILE_METHOD;
#[cfg(windows)]
use crate::protocol::FsReadFileParams;
#[cfg(windows)]
use crate::protocol::FsReadFileResponse;
#[cfg(windows)]
use crate::protocol::INITIALIZE_METHOD;
#[cfg(windows)]
use crate::protocol::INITIALIZED_METHOD;
#[cfg(windows)]
use crate::protocol::InitializeResponse;

#[tokio::test]
async fn non_native_uri_is_rejected_before_connecting() {
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url("not a websocket URL".to_string()),
    ));

    let error = file_system
        .read_file(&non_native_uri(), /*sandbox*/ None)
        .await
        .expect_err("non-native URI should be rejected");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[cfg(windows)]
#[tokio::test]
async fn remote_file_system_sends_explicit_windows_native_paths() {
    let (websocket_url, captured_paths, server) = record_read_file_paths(2).await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(websocket_url),
    ));
    let paths = vec![
        (
            PathUri::parse("file:///C:/Users/Alice/src/main.rs").expect("valid drive URI"),
            absolute_windows_path(r"C:\Users\Alice\src\main.rs"),
        ),
        (
            PathUri::parse("file://server/share/src/main.rs").expect("valid UNC URI"),
            absolute_windows_path(r"\\server\share\src\main.rs"),
        ),
    ];
    let expected_paths = paths
        .iter()
        .map(|(_, expected_path)| expected_path.clone())
        .collect::<Vec<_>>();

    for (path, _) in paths {
        assert_eq!(
            file_system
                .read_file(&path, /*sandbox*/ None)
                .await
                .expect("remote read should succeed"),
            Vec::<u8>::new()
        );
    }

    assert_eq!(
        captured_paths.await.expect("captured paths"),
        expected_paths
    );
    server.await.expect("recording server should succeed");
}

fn non_native_uri() -> PathUri {
    #[cfg(unix)]
    let uri = "file://server/share/file.txt";
    #[cfg(windows)]
    let uri = "file:///usr/local/file.txt";

    PathUri::parse(uri).expect("valid non-native URI")
}

#[cfg(windows)]
fn absolute_windows_path(path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute Windows path")
}

#[cfg(windows)]
async fn record_read_file_paths(
    expected_requests: usize,
) -> (
    String,
    oneshot::Receiver<Vec<AbsolutePathBuf>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!("ws://{}", listener.local_addr().expect("listener address"));
    let (captured_paths_tx, captured_paths_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("listener should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("websocket handshake should succeed");
        complete_websocket_initialize(&mut websocket).await;

        let mut captured_paths = Vec::with_capacity(expected_requests);
        for _ in 0..expected_requests {
            let request = match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Request(request) if request.method == FS_READ_FILE_METHOD => {
                    request
                }
                other => panic!("expected fs/readFile request, got {other:?}"),
            };
            let params: FsReadFileParams =
                serde_json::from_value(request.params.expect("fs/readFile params should exist"))
                    .expect("fs/readFile params should deserialize");
            captured_paths.push(params.path);
            write_jsonrpc_websocket(
                &mut websocket,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(FsReadFileResponse {
                        data_base64: String::new(),
                    })
                    .expect("fs/readFile response should serialize"),
                }),
            )
            .await;
        }
        captured_paths_tx
            .send(captured_paths)
            .expect("captured paths receiver should stay open");
    });

    (websocket_url, captured_paths_rx, server)
}

#[cfg(windows)]
async fn complete_websocket_initialize(websocket: &mut WebSocketStream<TcpStream>) {
    let request = match read_jsonrpc_websocket(websocket).await {
        JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
        other => panic!("expected initialize request, got {other:?}"),
    };
    write_jsonrpc_websocket(
        websocket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: request.id,
            result: serde_json::to_value(InitializeResponse {
                session_id: "session-1".to_string(),
            })
            .expect("initialize response should serialize"),
        }),
    )
    .await;

    match read_jsonrpc_websocket(websocket).await {
        JSONRPCMessage::Notification(notification) if notification.method == INITIALIZED_METHOD => {
        }
        other => panic!("expected initialized notification, got {other:?}"),
    }
}

#[cfg(windows)]
async fn read_jsonrpc_websocket(websocket: &mut WebSocketStream<TcpStream>) -> JSONRPCMessage {
    loop {
        match timeout(Duration::from_secs(1), websocket.next())
            .await
            .expect("json-rpc websocket read should not time out")
            .expect("websocket should stay open")
            .expect("websocket frame should read")
        {
            Message::Text(text) => {
                return serde_json::from_str(text.as_ref())
                    .expect("json-rpc text frame should parse");
            }
            Message::Binary(bytes) => {
                return serde_json::from_slice(bytes.as_ref())
                    .expect("json-rpc binary frame should parse");
            }
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected json-rpc websocket frame, got {other:?}"),
        }
    }
}

#[cfg(windows)]
async fn write_jsonrpc_websocket(
    websocket: &mut WebSocketStream<TcpStream>,
    message: JSONRPCMessage,
) {
    let encoded = serde_json::to_string(&message).expect("json-rpc should serialize");
    websocket
        .send(Message::Text(encoded.into()))
        .await
        .expect("json-rpc websocket frame should write");
}
