#![allow(clippy::expect_used)]

use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
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
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
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
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
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
        missing_path_behavior: None,
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
async fn concurrent_remote_metadata_requests_share_only_in_flight_results() {
    let (abandoned_request_tx, abandoned_request_rx) = oneshot::channel();
    let (websocket_url, captured_params, server) = record_metadata_params(vec![
        MetadataResponse::Immediate(Ok(metadata_response(/*size*/ 42))),
        MetadataResponse::Abandoned(abandoned_request_tx),
        MetadataResponse::Immediate(Ok(metadata_response(/*size*/ 43))),
    ])
    .await;
    let file_system = std::sync::Arc::new(RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )));
    let path = PathUri::parse("file:///workspace/project/AGENTS.md").expect("valid path URI");

    let (first, second) = tokio::join!(
        file_system.get_metadata(&path, /*sandbox*/ None),
        file_system.get_metadata(&path, /*sandbox*/ None),
    );
    let expected = FileMetadata {
        is_directory: false,
        is_file: true,
        is_symlink: false,
        size: 42,
        created_at_ms: 10,
        modified_at_ms: 20,
    };
    assert_eq!(first.expect("first metadata request"), expected);
    assert_eq!(second.expect("second metadata request"), expected);
    assert!(file_system.metadata_requests.lock().await.is_empty());

    let initializing_file_system = std::sync::Arc::clone(&file_system);
    let initializing_path = path.clone();
    let initializer = tokio::spawn(async move {
        initializing_file_system
            .get_metadata(&initializing_path, /*sandbox*/ None)
            .await
    });
    abandoned_request_rx
        .await
        .expect("server should receive the abandoned metadata request");

    let follower = file_system.get_metadata(&path, /*sandbox*/ None);
    tokio::pin!(follower);
    assert!(futures::poll!(follower.as_mut()).is_pending());
    assert_eq!(
        file_system
            .metadata_requests
            .lock()
            .await
            .get(&path)
            .map(std::sync::Arc::strong_count),
        Some(3)
    );

    initializer.abort();
    assert!(
        initializer
            .await
            .expect_err("metadata initializer should be aborted")
            .is_cancelled()
    );

    let mut refreshed = expected;
    refreshed.size = 43;
    assert_eq!(
        follower
            .await
            .expect("waiting metadata request should retry the abandoned initializer"),
        refreshed
    );
    assert!(file_system.metadata_requests.lock().await.is_empty());
    assert_eq!(
        captured_params.await.expect("captured metadata requests"),
        vec![
            FsGetMetadataParams {
                path: path.clone(),
                sandbox: None,
            };
            3
        ]
    );
    server.await.expect("metadata server should succeed");
}

#[tokio::test]
async fn concurrent_remote_metadata_errors_are_shared_but_retried() {
    let (websocket_url, captured_params, server) = record_metadata_params(vec![
        MetadataResponse::Immediate(Err(JSONRPCErrorError {
            code: NOT_FOUND_ERROR_CODE,
            data: None,
            message: "metadata not found".to_string(),
        })),
        MetadataResponse::Immediate(Ok(metadata_response(/*size*/ 42))),
    ])
    .await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ));
    let path = PathUri::parse("file:///workspace/project/AGENTS.md").expect("valid path URI");

    let (first, second) = tokio::join!(
        file_system.get_metadata(&path, /*sandbox*/ None),
        file_system.get_metadata(&path, /*sandbox*/ None),
    );
    assert_eq!(
        first.expect_err("first metadata error").kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        second.expect_err("shared metadata error").kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        file_system
            .get_metadata(&path, /*sandbox*/ None)
            .await
            .expect("failed metadata request should be retried")
            .size,
        42
    );
    assert_eq!(
        captured_params.await.expect("captured metadata requests"),
        vec![
            FsGetMetadataParams {
                path: path.clone(),
                sandbox: None,
            },
            FsGetMetadataParams {
                path,
                sandbox: None,
            },
        ]
    );
    server.await.expect("metadata server should succeed");
}

#[tokio::test]
async fn remote_metadata_starts_fresh_after_intervening_filesystem_mutation() {
    let path = PathUri::parse("file:///workspace/project/AGENTS.md").expect("valid path URI");
    let source_path =
        PathUri::parse("file:///workspace/project/source.md").expect("valid source path URI");

    for (mutation, mutation_response) in [
        (MetadataMutation::Write, Ok(())),
        (MetadataMutation::CreateDirectory, Ok(())),
        (MetadataMutation::Remove, Ok(())),
        (MetadataMutation::Copy, Ok(())),
        (
            MetadataMutation::Copy,
            Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                data: None,
                message: "mutation partially failed".to_string(),
            }),
        ),
    ] {
        let mutation_fails = mutation_response.is_err();
        let (websocket_url, captured_params, server) = record_metadata_params(vec![
            MetadataResponse::Immediate(Ok(metadata_response(/*size*/ 42))),
            MetadataResponse::Mutation(mutation_response),
            MetadataResponse::Immediate(Ok(metadata_response(/*size*/ 43))),
        ])
        .await;
        let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
            ExecServerTransportParams::websocket_url(
                websocket_url,
                DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
            ),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ));
        file_system
            .client
            .get()
            .await
            .expect("remote filesystem client should connect");

        let stale_request = file_system.get_metadata(&path, /*sandbox*/ None);
        tokio::pin!(stale_request);
        assert!(futures::poll!(stale_request.as_mut()).is_pending());

        let result = match mutation {
            MetadataMutation::Write => {
                file_system
                    .write_file(&path, b"updated".to_vec(), /*sandbox*/ None)
                    .await
            }
            MetadataMutation::CreateDirectory => {
                file_system
                    .create_directory(
                        &path,
                        CreateDirectoryOptions { recursive: true },
                        /*sandbox*/ None,
                    )
                    .await
            }
            MetadataMutation::Remove => {
                file_system
                    .remove(
                        &path,
                        RemoveOptions {
                            recursive: true,
                            force: true,
                        },
                        /*sandbox*/ None,
                    )
                    .await
            }
            MetadataMutation::Copy => {
                file_system
                    .copy(
                        &source_path,
                        &path,
                        CopyOptions { recursive: true },
                        /*sandbox*/ None,
                    )
                    .await
            }
        };
        if mutation_fails {
            assert_eq!(
                result
                    .expect_err("remote filesystem mutation should fail")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        } else {
            result.expect("remote filesystem mutation should succeed");
        }

        let (refreshed, stale) = tokio::join!(
            file_system.get_metadata(&path, /*sandbox*/ None),
            stale_request.as_mut(),
        );
        assert_eq!(stale.expect("original metadata request").size, 42);
        assert_eq!(refreshed.expect("post-mutation metadata request").size, 43);
        assert_eq!(
            captured_params.await.expect("captured metadata requests"),
            vec![
                FsGetMetadataParams {
                    path: path.clone(),
                    sandbox: None,
                };
                2
            ]
        );
        server.await.expect("metadata server should succeed");
    }
}

#[tokio::test]
async fn remote_metadata_requests_do_not_cross_path_or_sandbox_boundaries() {
    let metadata = metadata_response(/*size*/ 42);
    let (websocket_url, captured_params, server) = record_metadata_params(vec![
        MetadataResponse::Immediate(Ok(metadata.clone())),
        MetadataResponse::Immediate(Ok(metadata.clone())),
        MetadataResponse::Immediate(Ok(metadata.clone())),
        MetadataResponse::Immediate(Ok(metadata)),
    ])
    .await;
    let file_system = RemoteFileSystem::new(LazyRemoteExecServerClient::new(
        ExecServerTransportParams::websocket_url(
            websocket_url,
            DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        ),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ));
    let first_path = PathUri::parse("file:///workspace/project/AGENTS.md").expect("valid path URI");
    let second_path = PathUri::parse("file:///workspace/project/SKILL.md").expect("valid path URI");
    let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            }]),
            NetworkSandboxPolicy::Restricted,
        ),
        non_native_cwd(),
    );

    let (first, second) = tokio::join!(
        file_system.get_metadata(&first_path, /*sandbox*/ None),
        file_system.get_metadata(&second_path, /*sandbox*/ None),
    );
    first.expect("metadata for first path");
    second.expect("metadata for second path");

    let (first, second) = tokio::join!(
        file_system.get_metadata(&first_path, Some(&sandbox)),
        file_system.get_metadata(&first_path, Some(&sandbox)),
    );
    first.expect("first sandboxed metadata request");
    second.expect("second sandboxed metadata request");

    let captured_params = captured_params.await.expect("captured metadata requests");
    assert_eq!(
        captured_params
            .iter()
            .filter(|params| params.path == first_path && params.sandbox.is_none())
            .count(),
        1
    );
    assert_eq!(
        captured_params
            .iter()
            .filter(|params| params.path == second_path && params.sandbox.is_none())
            .count(),
        1
    );
    assert_eq!(
        captured_params
            .iter()
            .filter(|params| params.path == first_path && params.sandbox.as_ref() == Some(&sandbox))
            .count(),
        2
    );
    server.await.expect("metadata server should succeed");
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

enum MetadataResponse {
    Immediate(Result<FsGetMetadataResponse, JSONRPCErrorError>),
    Abandoned(oneshot::Sender<()>),
    Mutation(Result<(), JSONRPCErrorError>),
}

fn metadata_response(size: u64) -> FsGetMetadataResponse {
    FsGetMetadataResponse {
        is_directory: false,
        is_file: true,
        is_symlink: false,
        size,
        created_at_ms: 10,
        modified_at_ms: 20,
    }
}

#[derive(Clone, Copy)]
enum MetadataMutation {
    Write,
    CreateDirectory,
    Remove,
    Copy,
}

async fn record_metadata_params(
    responses: Vec<MetadataResponse>,
) -> (
    String,
    oneshot::Receiver<Vec<FsGetMetadataParams>>,
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

        let mut captured_params = Vec::with_capacity(responses.len());
        for response in responses {
            let request = match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Request(request) => request,
                other => panic!("expected filesystem request, got {other:?}"),
            };
            if matches!(response, MetadataResponse::Mutation(_)) {
                assert!(matches!(
                    request.method.as_str(),
                    FS_WRITE_FILE_METHOD
                        | FS_CREATE_DIRECTORY_METHOD
                        | FS_REMOVE_METHOD
                        | FS_COPY_METHOD
                ));
            } else {
                assert_eq!(request.method, FS_GET_METADATA_METHOD);
                let params: FsGetMetadataParams = serde_json::from_value(
                    request.params.expect("fs/getMetadata params should exist"),
                )
                .expect("fs/getMetadata params should deserialize");
                captured_params.push(params);
            }
            let response = match response {
                MetadataResponse::Immediate(Ok(metadata)) => {
                    JSONRPCMessage::Response(JSONRPCResponse {
                        id: request.id,
                        result: serde_json::to_value(metadata)
                            .expect("fs/getMetadata response should serialize"),
                    })
                }
                MetadataResponse::Immediate(Err(error))
                | MetadataResponse::Mutation(Err(error)) => JSONRPCMessage::Error(JSONRPCError {
                    error,
                    id: request.id,
                }),
                MetadataResponse::Mutation(Ok(())) => JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::json!({}),
                }),
                MetadataResponse::Abandoned(received_tx) => {
                    received_tx
                        .send(())
                        .expect("abandoned request observer should stay open");
                    continue;
                }
            };
            write_jsonrpc_websocket(&mut websocket, response).await;
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
