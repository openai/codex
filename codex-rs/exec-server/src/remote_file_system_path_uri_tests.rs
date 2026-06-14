#![allow(clippy::expect_used)]

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
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
async fn remote_file_system_sends_path_and_sandbox_cwd_uris_without_native_conversion() {
    let (websocket_url, captured_params, server) =
        record_read_file_params(/*expected_requests*/ 2).await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(websocket_url),
    ));
    let paths = vec![
        PathUri::parse("file:///C:/Users/Alice/src/main.rs").expect("valid drive URI"),
        PathUri::parse("file://server/share/src/main.rs").expect("valid UNC URI"),
    ];
    let sandbox_cwd = non_native_cwd();
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Write,
    }]);
    let sandbox = codex_file_system::AppFileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        sandbox_cwd,
    )
    .into_exec();

    for path in &paths {
        assert_eq!(
            file_system
                .read_file(path, Some(&sandbox))
                .await
                .expect("remote read should succeed"),
            Vec::<u8>::new()
        );
    }

    let expected_params = paths
        .into_iter()
        .map(|path| FsReadFileParams {
            path,
            sandbox: Some(sandbox.clone()),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        captured_params.await.expect("captured params"),
        expected_params
    );
    server.await.expect("recording server should succeed");
}

async fn record_read_file_params(
    expected_requests: usize,
) -> (
    String,
    oneshot::Receiver<Vec<FsReadFileParams>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!("ws://{}", listener.local_addr().expect("listener address"));
    let (captured_params_tx, captured_params_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("listener should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("websocket handshake should succeed");
        complete_websocket_initialize(&mut websocket).await;

        let mut captured_params = Vec::with_capacity(expected_requests);
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
            captured_params.push(params);
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
        captured_params_tx
            .send(captured_params)
            .expect("captured params receiver should stay open");
    });

    (websocket_url, captured_params_rx, server)
}

fn non_native_cwd() -> PathUri {
    #[cfg(unix)]
    let uri = "file://server/share/checkout";
    #[cfg(windows)]
    let uri = "file:///usr/local/checkout";

    PathUri::parse(uri).expect("non-native cwd URI")
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
