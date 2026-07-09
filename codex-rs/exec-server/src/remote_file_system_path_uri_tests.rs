#![allow(clippy::expect_used)]

use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_file_system::FindUpErrorPolicy;
use codex_file_system::FindUpMatchKind;
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
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::ExecServerTransportParams;
use crate::protocol::FS_FIND_UP_BATCH_METHOD;
use crate::protocol::FS_FIND_UP_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_READ_TEXT_PREFIXES_BATCH_METHOD;
use crate::protocol::FsFindUpBatchParams;
use crate::protocol::FsFindUpBatchResponse;
use crate::protocol::FsFindUpParams;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsReadTextPrefixesBatchParams;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeResponse;

#[tokio::test]
async fn remote_file_system_sends_path_and_sandbox_cwd_uris_without_native_conversion() {
    let (websocket_url, captured_params, server) =
        record_read_file_params(/*expected_requests*/ 2).await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
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
    let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        sandbox_cwd,
    );

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

#[tokio::test]
async fn find_up_batch_method_not_found_falls_back_concurrently_and_preserves_order() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!("ws://{}", listener.local_addr().expect("listener address"));
    let starts = vec![
        PathUri::parse("file:///C:/Users/Alice/src").expect("valid drive URI"),
        PathUri::parse("file://server/share/src").expect("valid UNC URI"),
    ];
    let expected_starts = starts.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("listener should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("websocket handshake should succeed");
        complete_websocket_initialize(&mut websocket).await;

        let batch_request = match read_jsonrpc_websocket(&mut websocket).await {
            JSONRPCMessage::Request(request) if request.method == FS_FIND_UP_BATCH_METHOD => {
                request
            }
            other => panic!("expected fs/findUpBatch request, got {other:?}"),
        };
        let batch_params: FsFindUpBatchParams = serde_json::from_value(
            batch_request
                .params
                .expect("fs/findUpBatch params should exist"),
        )
        .expect("fs/findUpBatch params should deserialize");
        assert_eq!(
            batch_params
                .requests
                .iter()
                .map(|request| request.start.clone())
                .collect::<Vec<_>>(),
            expected_starts
        );
        write_jsonrpc_websocket(
            &mut websocket,
            JSONRPCMessage::Error(JSONRPCError {
                error: JSONRPCErrorError {
                    code: -32601,
                    data: None,
                    message: "method not found".to_string(),
                },
                id: batch_request.id,
            }),
        )
        .await;

        // Read both fallbacks before responding. A serial fallback would time out here.
        let mut singles = Vec::new();
        for _ in 0..2 {
            let request = match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Request(request) if request.method == FS_FIND_UP_METHOD => request,
                other => panic!("expected fs/findUp request, got {other:?}"),
            };
            let params: FsFindUpParams = serde_json::from_value(
                request
                    .params
                    .clone()
                    .expect("fs/findUp params should exist"),
            )
            .expect("fs/findUp params should deserialize");
            singles.push((request, params));
        }
        for (request, params) in singles.into_iter().rev() {
            let visited_ancestor_count = if params.start == expected_starts[0] {
                1
            } else {
                2
            };
            write_jsonrpc_websocket(
                &mut websocket,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(FindUpOutcome {
                        matched: None,
                        visited_ancestor_count,
                        metadata_probe_count: visited_ancestor_count,
                        ignored_error_count: 0,
                        ignored_errors: Vec::new(),
                        ignored_errors_truncated: false,
                    })
                    .expect("find-up response should serialize"),
                }),
            )
            .await;
        }
    });
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
    ));
    let options = FindUpOptions {
        candidate_relative_paths: vec!["marker".to_string()],
        match_kind: FindUpMatchKind::Any,
        non_not_found_error_policy: FindUpErrorPolicy::Propagate,
    };
    let requests = starts
        .into_iter()
        .map(|start| FindUpRequest {
            start,
            options: options.clone(),
        })
        .collect::<Vec<_>>();

    let outcomes = file_system
        .find_up_batch(&requests, /*sandbox*/ None)
        .await
        .expect("method-not-found should use single-search fallback")
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("fallback searches should succeed");

    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.visited_ancestor_count)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    server.await.expect("recording server should succeed");
}

#[tokio::test]
async fn text_prefix_batch_method_not_found_falls_back_to_complete_read() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!("ws://{}", listener.local_addr().expect("listener address"));
    let path = PathUri::parse("file:///C:/Users/Alice/skill.txt").expect("valid drive URI");
    let expected_path = path.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("listener should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("websocket handshake should succeed");
        complete_websocket_initialize(&mut websocket).await;
        let batch_request = match read_jsonrpc_websocket(&mut websocket).await {
            JSONRPCMessage::Request(request)
                if request.method == FS_READ_TEXT_PREFIXES_BATCH_METHOD =>
            {
                request
            }
            other => panic!("expected fs/readTextPrefixesBatch request, got {other:?}"),
        };
        let params: FsReadTextPrefixesBatchParams = serde_json::from_value(
            batch_request
                .params
                .expect("text-prefix batch params should exist"),
        )
        .expect("text-prefix batch params should deserialize");
        assert_eq!(params.paths, vec![expected_path.clone()]);
        assert_eq!(params.prefix_byte_limit, 3);
        write_jsonrpc_websocket(
            &mut websocket,
            JSONRPCMessage::Error(JSONRPCError {
                error: JSONRPCErrorError {
                    code: -32601,
                    data: None,
                    message: "method not found".to_string(),
                },
                id: batch_request.id,
            }),
        )
        .await;

        let request = match read_jsonrpc_websocket(&mut websocket).await {
            JSONRPCMessage::Request(request) if request.method == FS_READ_FILE_METHOD => request,
            other => panic!("expected fs/readFile request, got {other:?}"),
        };
        let params: FsReadFileParams = serde_json::from_value(
            request
                .params
                .clone()
                .expect("read-file params should exist"),
        )
        .expect("read-file params should deserialize");
        assert_eq!(params.path, expected_path);
        write_jsonrpc_websocket(
            &mut websocket,
            JSONRPCMessage::Response(JSONRPCResponse {
                id: request.id,
                result: serde_json::to_value(FsReadFileResponse {
                    data_base64: "YWJjZA==".to_string(),
                })
                .expect("read response should serialize"),
            }),
        )
        .await;
    });
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
    ));

    let actual = file_system
        .read_text_prefixes_batch(&[path], 3, /*sandbox*/ None)
        .await
        .expect("legacy fallback should succeed")
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("legacy reads should succeed");

    assert_eq!(
        actual,
        vec![TextFilePrefix {
            text: "abc".to_string(),
            complete: false,
        }]
    );
    server.await.expect("recording server should succeed");
}

#[tokio::test]
async fn find_up_batch_rejects_malformed_response_cardinality_without_legacy_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!("ws://{}", listener.local_addr().expect("listener address"));
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("listener should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("websocket handshake should succeed");
        complete_websocket_initialize(&mut websocket).await;
        let request = match read_jsonrpc_websocket(&mut websocket).await {
            JSONRPCMessage::Request(request) if request.method == FS_FIND_UP_BATCH_METHOD => {
                request
            }
            other => panic!("expected fs/findUpBatch request, got {other:?}"),
        };
        write_jsonrpc_websocket(
            &mut websocket,
            JSONRPCMessage::Response(JSONRPCResponse {
                id: request.id,
                result: serde_json::to_value(FsFindUpBatchResponse {
                    results: Vec::new(),
                })
                .expect("find-up batch response should serialize"),
            }),
        )
        .await;
    });
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
    ));
    let requests = [FindUpRequest {
        start: PathUri::parse("file:///C:/Users/Alice/src").expect("valid drive URI"),
        options: FindUpOptions {
            candidate_relative_paths: vec!["marker".to_string()],
            match_kind: FindUpMatchKind::Any,
            non_not_found_error_policy: FindUpErrorPolicy::Propagate,
        },
    }];

    let error = file_system
        .find_up_batch(&requests, /*sandbox*/ None)
        .await
        .expect_err("malformed cardinality should be surfaced to the consumer");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
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
                environment_info: None,
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
