#![allow(clippy::expect_used)]

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_utils_path_uri::PathUri;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::client_api::ExecServerTransportParams;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FsReadFileResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeResponse;
use crate::protocol::InitializeWireParams;
use crate::protocol::InitializeWireResponse;

#[derive(Clone, Copy)]
enum ServerPathFormat {
    PathUri,
    LegacyNative,
}

#[tokio::test]
async fn remote_file_system_sends_path_uris_without_native_conversion() {
    let (websocket_url, captured_paths, server) =
        record_read_file_paths(ServerPathFormat::PathUri, /*expected_requests*/ 2).await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(websocket_url),
    ));
    let paths = vec![
        PathUri::parse("file:///C:/Users/Alice/src/main.rs").expect("valid drive URI"),
        PathUri::parse("file://server/share/src/main.rs").expect("valid UNC URI"),
    ];

    for path in &paths {
        assert_eq!(
            file_system
                .read_file(path, /*sandbox*/ None)
                .await
                .expect("remote read should succeed"),
            Vec::<u8>::new()
        );
    }

    assert_eq!(
        captured_paths.await.expect("captured paths"),
        paths.iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    server.await.expect("recording server should succeed");
}

#[tokio::test]
async fn remote_file_system_uses_native_paths_with_legacy_servers() {
    let (websocket_url, captured_paths, server) =
        record_read_file_paths(ServerPathFormat::LegacyNative, /*expected_requests*/ 1).await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(websocket_url),
    ));
    let path = PathUri::from_path(std::env::temp_dir().join("legacy-server.txt"))
        .expect("native path URI");

    assert_eq!(
        file_system
            .read_file(&path, /*sandbox*/ None)
            .await
            .expect("remote read should succeed"),
        Vec::<u8>::new()
    );

    assert_eq!(
        captured_paths.await.expect("captured paths"),
        vec![
            path.to_abs_path()
                .expect("native path")
                .to_string_lossy()
                .into_owned()
        ]
    );
    server.await.expect("recording server should succeed");
}

async fn record_read_file_paths(
    server_path_format: ServerPathFormat,
    expected_requests: usize,
) -> (
    String,
    oneshot::Receiver<Vec<String>>,
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
        complete_websocket_initialize(&mut websocket, server_path_format).await;

        let mut captured_paths = Vec::with_capacity(expected_requests);
        for _ in 0..expected_requests {
            let request = match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Request(request) if request.method == FS_READ_FILE_METHOD => {
                    request
                }
                other => panic!("expected fs/readFile request, got {other:?}"),
            };
            let params = request.params.expect("fs/readFile params should exist");
            captured_paths.push(
                params
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .expect("fs/readFile path should be a string")
                    .to_string(),
            );
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

async fn complete_websocket_initialize(
    websocket: &mut WebSocketStream<TcpStream>,
    server_path_format: ServerPathFormat,
) {
    let request = match read_jsonrpc_websocket(websocket).await {
        JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
        other => panic!("expected initialize request, got {other:?}"),
    };
    let params: InitializeWireParams =
        serde_json::from_value(request.params.expect("initialize params should exist"))
            .expect("initialize params should deserialize");
    assert!(params.filesystem_path_uris);
    let response = InitializeResponse {
        session_id: "session-1".to_string(),
    };
    let result = match server_path_format {
        ServerPathFormat::PathUri => serde_json::to_value(InitializeWireResponse {
            response,
            filesystem_path_uris: true,
        }),
        ServerPathFormat::LegacyNative => serde_json::to_value(response),
    }
    .expect("initialize response should serialize");
    write_jsonrpc_websocket(
        websocket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: request.id,
            result,
        }),
    )
    .await;

    match read_jsonrpc_websocket(websocket).await {
        JSONRPCMessage::Notification(notification) if notification.method == INITIALIZED_METHOD => {
        }
        other => panic!("expected initialized notification, got {other:?}"),
    }
}

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
