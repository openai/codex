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
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeResponse;

#[tokio::test]
async fn remote_file_system_sends_path_uris_without_native_conversion() {
    let (websocket_url, captured_paths, server) =
        record_read_file_paths(/*expected_requests*/ 2).await;
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

    assert_eq!(captured_paths.await.expect("captured paths"), paths);
    server.await.expect("recording server should succeed");
}

async fn record_read_file_paths(
    expected_requests: usize,
) -> (
    String,
    oneshot::Receiver<Vec<PathUri>>,
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
