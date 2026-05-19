use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::ExecServerConnection;
use super::RemoteExecServerClient;
use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::HttpHeader;
use crate::ProcessId;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::client_api::ExecServerTransportParams;
use crate::client_api::HttpClient;
use crate::process::ExecBackend;
use crate::process::ExecProcessEvent;
use crate::protocol::ByteChunk;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsReadDirectoryEntry;
use crate::protocol::FsReadDirectoryResponse;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsRemoveResponse;
use crate::protocol::FsWriteFileResponse;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteResponse;
use crate::protocol::WriteStatus;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteApi {
    Start,
    Read,
    Write,
    Terminate,
    FsReadFile,
    FsWriteFile,
    FsCreateDirectory,
    FsGetMetadata,
    FsReadDirectory,
    FsRemove,
    FsCopy,
    HttpRequest,
    HttpRequestStream,
}

impl RemoteApi {
    const ALL: [Self; 13] = [
        Self::Start,
        Self::Read,
        Self::Write,
        Self::Terminate,
        Self::FsReadFile,
        Self::FsWriteFile,
        Self::FsCreateDirectory,
        Self::FsGetMetadata,
        Self::FsReadDirectory,
        Self::FsRemove,
        Self::FsCopy,
        Self::HttpRequest,
        Self::HttpRequestStream,
    ];

    const NON_REPLAYABLE: [Self; 12] = [
        Self::Start,
        Self::Write,
        Self::Terminate,
        Self::FsReadFile,
        Self::FsWriteFile,
        Self::FsCreateDirectory,
        Self::FsGetMetadata,
        Self::FsReadDirectory,
        Self::FsRemove,
        Self::FsCopy,
        Self::HttpRequest,
        Self::HttpRequestStream,
    ];

    fn method(self) -> &'static str {
        match self {
            Self::Start => EXEC_METHOD,
            Self::Read => EXEC_READ_METHOD,
            Self::Write => EXEC_WRITE_METHOD,
            Self::Terminate => EXEC_TERMINATE_METHOD,
            Self::FsReadFile => FS_READ_FILE_METHOD,
            Self::FsWriteFile => FS_WRITE_FILE_METHOD,
            Self::FsCreateDirectory => FS_CREATE_DIRECTORY_METHOD,
            Self::FsGetMetadata => FS_GET_METADATA_METHOD,
            Self::FsReadDirectory => FS_READ_DIRECTORY_METHOD,
            Self::FsRemove => FS_REMOVE_METHOD,
            Self::FsCopy => FS_COPY_METHOD,
            Self::HttpRequest | Self::HttpRequestStream => HTTP_REQUEST_METHOD,
        }
    }

    async fn respond_success(self, peer: &mut WebSocketJsonRpcPeer, request: JSONRPCRequest) {
        match self {
            Self::Start => {
                peer.write_response(
                    request.id,
                    ExecResponse {
                        process_id: ProcessId::from("proc"),
                    },
                )
                .await;
            }
            Self::Read => {
                peer.write_response(request.id, successful_read_response())
                    .await;
            }
            Self::Write => {
                peer.write_response(
                    request.id,
                    WriteResponse {
                        status: WriteStatus::Accepted,
                    },
                )
                .await;
            }
            Self::Terminate => {
                peer.write_response(request.id, TerminateResponse { running: false })
                    .await;
            }
            Self::FsReadFile => {
                peer.write_response(
                    request.id,
                    FsReadFileResponse {
                        data_base64: base64::engine::general_purpose::STANDARD
                            .encode(b"remote file"),
                    },
                )
                .await;
            }
            Self::FsWriteFile => {
                peer.write_response(request.id, FsWriteFileResponse {})
                    .await;
            }
            Self::FsCreateDirectory => {
                peer.write_response(request.id, FsCreateDirectoryResponse {})
                    .await;
            }
            Self::FsGetMetadata => {
                peer.write_response(
                    request.id,
                    FsGetMetadataResponse {
                        is_directory: false,
                        is_file: true,
                        is_symlink: false,
                        created_at_ms: 11,
                        modified_at_ms: 22,
                    },
                )
                .await;
            }
            Self::FsReadDirectory => {
                peer.write_response(
                    request.id,
                    FsReadDirectoryResponse {
                        entries: vec![FsReadDirectoryEntry {
                            file_name: "entry.txt".to_string(),
                            is_directory: false,
                            is_file: true,
                        }],
                    },
                )
                .await;
            }
            Self::FsRemove => {
                peer.write_response(request.id, FsRemoveResponse {}).await;
            }
            Self::FsCopy => {
                peer.write_response(request.id, FsCopyResponse {}).await;
            }
            Self::HttpRequest => {
                peer.write_response(request.id, successful_http_response())
                    .await;
            }
            Self::HttpRequestStream => {
                let params: HttpRequestParams = decode_request_params(&request);
                assert!(params.stream_response);
                peer.write_response(request.id, successful_http_stream_response())
                    .await;
                peer.write_notification(
                    HTTP_REQUEST_BODY_DELTA_METHOD,
                    HttpRequestBodyDeltaNotification {
                        request_id: params.request_id,
                        seq: 1,
                        delta: ByteChunk::from(Vec::new()),
                        done: true,
                        error: None,
                    },
                )
                .await;
            }
        }
    }
}

struct WebSocketJsonRpcPeer {
    websocket: WebSocketStream<TcpStream>,
}

impl WebSocketJsonRpcPeer {
    async fn accept(listener: &TcpListener) -> Self {
        let (stream, _) = timeout(Duration::from_secs(1), listener.accept())
            .await
            .expect("websocket accept should not time out")
            .expect("websocket accept should succeed");
        let websocket = accept_async(stream)
            .await
            .expect("websocket handshake should succeed");
        Self { websocket }
    }

    async fn complete_initialize(
        &mut self,
        expected_resume_session_id: Option<&str>,
        session_id: &str,
    ) {
        let request = self.read_request(INITIALIZE_METHOD).await;
        assert_eq!(
            decode_request_params::<InitializeParams>(&request),
            InitializeParams {
                client_name: crate::client_transport::ENVIRONMENT_CLIENT_NAME.to_string(),
                resume_session_id: expected_resume_session_id.map(ToOwned::to_owned),
            }
        );
        self.write_response(
            request.id,
            InitializeResponse {
                session_id: session_id.to_string(),
            },
        )
        .await;
        self.read_notification(INITIALIZED_METHOD).await;
    }

    async fn read_request(&mut self, expected_method: &str) -> JSONRPCRequest {
        let message = self.read_message().await;
        let JSONRPCMessage::Request(request) = message else {
            panic!("expected JSON-RPC request `{expected_method}`, got {message:?}");
        };
        assert_eq!(request.method, expected_method);
        request
    }

    async fn read_notification(&mut self, expected_method: &str) -> JSONRPCNotification {
        let message = self.read_message().await;
        let JSONRPCMessage::Notification(notification) = message else {
            panic!("expected JSON-RPC notification `{expected_method}`, got {message:?}");
        };
        assert_eq!(notification.method, expected_method);
        notification
    }

    async fn write_response<T>(&mut self, id: codex_app_server_protocol::RequestId, result: T)
    where
        T: serde::Serialize,
    {
        self.write_message(JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::to_value(result).expect("json-rpc response should serialize"),
        }))
        .await;
    }

    async fn write_error(
        &mut self,
        id: codex_app_server_protocol::RequestId,
        code: i64,
        message: impl Into<String>,
    ) {
        self.write_message(JSONRPCMessage::Error(JSONRPCError {
            id,
            error: JSONRPCErrorError {
                code,
                message: message.into(),
                data: None,
            },
        }))
        .await;
    }

    async fn write_notification<T>(&mut self, method: &str, params: T)
    where
        T: serde::Serialize,
    {
        self.write_message(JSONRPCMessage::Notification(JSONRPCNotification {
            method: method.to_string(),
            params: Some(
                serde_json::to_value(params).expect("json-rpc notification should serialize"),
            ),
        }))
        .await;
    }

    async fn read_message(&mut self) -> JSONRPCMessage {
        loop {
            let message = timeout(Duration::from_secs(1), self.websocket.next())
                .await
                .expect("websocket read should not time out")
                .expect("websocket should stay open")
                .expect("websocket read should succeed");
            match message {
                Message::Text(text) => {
                    return serde_json::from_str(text.as_ref())
                        .expect("websocket text should contain json-rpc");
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(&bytes)
                        .expect("websocket binary should contain json-rpc");
                }
                Message::Ping(payload) => {
                    self.websocket
                        .send(Message::Pong(payload))
                        .await
                        .expect("websocket pong should send");
                }
                Message::Pong(_) => {}
                Message::Close(frame) => panic!("websocket closed unexpectedly: {frame:?}"),
                Message::Frame(_) => panic!("unexpected raw websocket frame"),
            }
        }
    }

    async fn write_message(&mut self, message: JSONRPCMessage) {
        let encoded = serde_json::to_string(&message).expect("json-rpc message should serialize");
        self.websocket
            .send(Message::Text(encoded.into()))
            .await
            .expect("websocket message should send");
    }
}

fn decode_request_params<T>(request: &JSONRPCRequest) -> T
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(
        request
            .params
            .clone()
            .expect("json-rpc request should include params"),
    )
    .expect("json-rpc request params should deserialize")
}

fn test_remote_client(websocket_url: String) -> RemoteExecServerClient {
    RemoteExecServerClient::new(ExecServerTransportParams::websocket_url(websocket_url))
}

fn test_exec_params(process_id: &ProcessId) -> ExecParams {
    ExecParams {
        process_id: process_id.clone(),
        argv: vec!["/bin/echo".to_string(), "hello".to_string()],
        cwd: std::env::current_dir().expect("current directory should be available"),
        env_policy: /*env_policy*/ None,
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
    }
}

fn successful_read_response() -> ReadResponse {
    ReadResponse {
        chunks: Vec::new(),
        next_seq: 9,
        exited: false,
        exit_code: None,
        closed: false,
        failure: None,
    }
}

fn successful_http_response() -> HttpRequestResponse {
    HttpRequestResponse {
        status: 200,
        headers: vec![HttpHeader {
            name: "content-type".to_string(),
            value: "text/plain".to_string(),
        }],
        body: ByteChunk::from(b"ok".to_vec()),
    }
}

fn successful_http_stream_response() -> HttpRequestResponse {
    HttpRequestResponse {
        body: ByteChunk::from(Vec::new()),
        ..successful_http_response()
    }
}

fn test_http_request_params() -> HttpRequestParams {
    HttpRequestParams {
        method: "GET".to_string(),
        url: "https://example.com/test".to_string(),
        headers: Vec::new(),
        body: None,
        timeout_ms: Some(123),
        request_id: "caller-request".to_string(),
        stream_response: false,
    }
}

fn absolute_test_path(name: &str) -> AbsolutePathBuf {
    let path = std::env::temp_dir().join(name);
    AbsolutePathBuf::from_absolute_path(&path).expect("absolute path")
}

async fn wait_for_disconnect(connection: &ExecServerConnection) {
    timeout(Duration::from_secs(1), async {
        loop {
            if connection.disconnected_error().is_some() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("connection should notice disconnect");
}

async fn invoke_remote_api(client: &RemoteExecServerClient, api: RemoteApi) -> Result<()> {
    let process_id = ProcessId::from("proc");
    match api {
        RemoteApi::Start => {
            let process = RemoteProcess::new(client.clone());
            let started = process.start(test_exec_params(&process_id)).await?;
            assert_eq!(started.process.process_id(), &process_id);
        }
        RemoteApi::Read => {
            assert_eq!(
                client
                    .read(ReadParams {
                        process_id,
                        after_seq: Some(7),
                        max_bytes: Some(1024),
                        wait_ms: Some(25),
                    })
                    .await?,
                successful_read_response()
            );
        }
        RemoteApi::Write => {
            assert_eq!(
                client.write(&process_id, b"stdin".to_vec()).await?,
                WriteResponse {
                    status: WriteStatus::Accepted,
                }
            );
        }
        RemoteApi::Terminate => {
            assert_eq!(
                client.terminate(&process_id).await?,
                TerminateResponse { running: false }
            );
        }
        RemoteApi::FsReadFile => {
            let file_system = RemoteFileSystem::new(client.clone());
            assert_eq!(
                file_system
                    .read_file(&absolute_test_path("remote-read"), /*sandbox*/ None)
                    .await?,
                b"remote file".to_vec()
            );
        }
        RemoteApi::FsWriteFile => {
            let file_system = RemoteFileSystem::new(client.clone());
            file_system
                .write_file(
                    &absolute_test_path("remote-write"),
                    b"contents".to_vec(),
                    /*sandbox*/ None,
                )
                .await?;
        }
        RemoteApi::FsCreateDirectory => {
            let file_system = RemoteFileSystem::new(client.clone());
            file_system
                .create_directory(
                    &absolute_test_path("remote-dir"),
                    CreateDirectoryOptions { recursive: true },
                    /*sandbox*/ None,
                )
                .await?;
        }
        RemoteApi::FsGetMetadata => {
            let file_system = RemoteFileSystem::new(client.clone());
            assert_eq!(
                file_system
                    .get_metadata(&absolute_test_path("remote-meta"), /*sandbox*/ None)
                    .await?,
                crate::FileMetadata {
                    is_directory: false,
                    is_file: true,
                    is_symlink: false,
                    created_at_ms: 11,
                    modified_at_ms: 22,
                }
            );
        }
        RemoteApi::FsReadDirectory => {
            let file_system = RemoteFileSystem::new(client.clone());
            assert_eq!(
                file_system
                    .read_directory(&absolute_test_path("remote-list"), /*sandbox*/ None)
                    .await?,
                vec![ReadDirectoryEntry {
                    file_name: "entry.txt".to_string(),
                    is_directory: false,
                    is_file: true,
                }]
            );
        }
        RemoteApi::FsRemove => {
            let file_system = RemoteFileSystem::new(client.clone());
            file_system
                .remove(
                    &absolute_test_path("remote-remove"),
                    RemoveOptions {
                        recursive: true,
                        force: true,
                    },
                    /*sandbox*/ None,
                )
                .await?;
        }
        RemoteApi::FsCopy => {
            let file_system = RemoteFileSystem::new(client.clone());
            file_system
                .copy(
                    &absolute_test_path("remote-copy-source"),
                    &absolute_test_path("remote-copy-destination"),
                    CopyOptions { recursive: true },
                    /*sandbox*/ None,
                )
                .await?;
        }
        RemoteApi::HttpRequest => {
            assert_eq!(
                client.http_request(test_http_request_params()).await?,
                successful_http_response()
            );
        }
        RemoteApi::HttpRequestStream => {
            let (response, mut body) = client
                .http_request_stream(test_http_request_params())
                .await?;
            assert_eq!(response, successful_http_stream_response());
            assert_eq!(body.recv().await?, None);
        }
    }
    Ok(())
}

async fn assert_remote_api_reconnects_before_dispatch(api: RemoteApi) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let mut first = WebSocketJsonRpcPeer::accept(&listener).await;
        first
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        second
            .complete_initialize(Some("session-1"), "session-1")
            .await;
        let request = second.read_request(api.method()).await;
        api.respond_success(&mut second, request).await;
    });

    let client = test_remote_client(websocket_url);
    let first_connection = client.connection().await?;
    wait_for_disconnect(&first_connection).await;
    invoke_remote_api(&client, api).await?;

    server.await.expect("test websocket server should finish");
    Ok(())
}

async fn assert_remote_api_does_not_replay_after_disconnect(api: RemoteApi) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let request_count = Arc::new(AtomicUsize::new(0));
    let server_request_count = Arc::clone(&request_count);
    let server = tokio::spawn(async move {
        let mut peer = WebSocketJsonRpcPeer::accept(&listener).await;
        peer.complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        let _request = peer.read_request(api.method()).await;
        server_request_count.fetch_add(1, Ordering::SeqCst);
        drop(peer);

        let reconnect = timeout(Duration::from_millis(200), listener.accept()).await;
        assert!(
            reconnect.is_err(),
            "{api:?} should not reconnect during ambiguous replay window"
        );
    });

    let client = test_remote_client(websocket_url);
    let error = invoke_remote_api(&client, api)
        .await
        .expect_err("ambiguous disconnect should fail");
    assert!(
        error.to_string().contains("exec-server transport"),
        "unexpected {api:?} error: {error:#}"
    );

    server.await.expect("test websocket server should finish");
    assert_eq!(request_count.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn remote_client_reuses_one_reconnect_attempt_for_concurrent_callers() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let accepted_connections = Arc::new(AtomicUsize::new(0));
    let server_connections = Arc::clone(&accepted_connections);
    let server = tokio::spawn(async move {
        let mut first = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        first
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        second
            .complete_initialize(Some("session-1"), "session-1")
            .await;

        let extra_connection = timeout(Duration::from_millis(200), listener.accept()).await;
        assert!(
            extra_connection.is_err(),
            "concurrent callers should share the resumed websocket"
        );
    });

    let client = test_remote_client(websocket_url);
    let first_connection = client.connection().await?;
    wait_for_disconnect(&first_connection).await;

    let (first_reconnected, second_reconnected) =
        tokio::join!(client.connection(), client.connection());
    assert_eq!(
        first_reconnected?.session_id(),
        second_reconnected?.session_id()
    );

    server.await.expect("test websocket server should finish");
    assert_eq!(accepted_connections.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn remote_process_disconnect_notifies_resync_before_cursor_recovery() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let (disconnect_tx, disconnect_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut first = WebSocketJsonRpcPeer::accept(&listener).await;
        first
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        let start_request = first.read_request(EXEC_METHOD).await;
        first
            .write_response(
                start_request.id,
                ExecResponse {
                    process_id: ProcessId::from("proc"),
                },
            )
            .await;
        disconnect_rx
            .await
            .expect("test should trigger idle disconnect");
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        second
            .complete_initialize(Some("session-1"), "session-1")
            .await;
        let read_request = second.read_request(EXEC_READ_METHOD).await;
        assert_eq!(
            decode_request_params::<ReadParams>(&read_request),
            ReadParams {
                process_id: ProcessId::from("proc"),
                after_seq: None,
                max_bytes: None,
                wait_ms: Some(0),
            }
        );
        second
            .write_response(read_request.id, successful_read_response())
            .await;
    });

    let client = test_remote_client(websocket_url);
    let process = RemoteProcess::new(client);
    let started = process
        .start(test_exec_params(&ProcessId::from("proc")))
        .await?;
    let mut wake = started.process.subscribe_wake();
    let mut events = started.process.subscribe_events();
    disconnect_tx
        .send(())
        .expect("idle disconnect should signal");

    timeout(Duration::from_secs(1), wake.changed())
        .await
        .context("idle process wake should surface transport resync")??;
    assert_eq!(
        timeout(Duration::from_secs(1), events.recv())
            .await
            .context("idle process event should surface transport resync")??,
        ExecProcessEvent::ResyncRequired
    );
    assert_eq!(
        started
            .process
            .read(
                /*after_seq*/ None,
                /*max_bytes*/ None,
                /*wait_ms*/ Some(0),
            )
            .await?,
        successful_read_response()
    );

    server.await.expect("test websocket server should finish");
    Ok(())
}

#[tokio::test]
async fn remote_client_retries_transient_resume_conflict() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let accepted_connections = Arc::new(AtomicUsize::new(0));
    let server_connections = Arc::clone(&accepted_connections);
    let server = tokio::spawn(async move {
        let mut first = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        first
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        let request = second.read_request(INITIALIZE_METHOD).await;
        assert_eq!(
            decode_request_params::<InitializeParams>(&request),
            InitializeParams {
                client_name: crate::client_transport::ENVIRONMENT_CLIENT_NAME.to_string(),
                resume_session_id: Some("session-1".to_string()),
            }
        );
        second
            .write_error(
                request.id,
                /*code*/ -32600,
                "session session-1 is already attached to another connection",
            )
            .await;

        let mut third = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        third
            .complete_initialize(Some("session-1"), "session-1")
            .await;
    });

    let client = test_remote_client(websocket_url);
    let first_connection = client.connection().await?;
    wait_for_disconnect(&first_connection).await;
    assert_eq!(
        client.connection().await?.session_id(),
        Some("session-1".to_string())
    );

    server.await.expect("test websocket server should finish");
    assert_eq!(accepted_connections.load(Ordering::SeqCst), 3);
    Ok(())
}

#[tokio::test]
async fn remote_client_caches_unknown_session_resume_failure() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let accepted_connections = Arc::new(AtomicUsize::new(0));
    let server_connections = Arc::clone(&accepted_connections);
    let server = tokio::spawn(async move {
        let mut first = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        first
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        server_connections.fetch_add(1, Ordering::SeqCst);
        let request = second.read_request(INITIALIZE_METHOD).await;
        second
            .write_error(
                request.id,
                /*code*/ -32600,
                "unknown session id session-1",
            )
            .await;

        let extra_connection = timeout(Duration::from_millis(200), listener.accept()).await;
        assert!(
            extra_connection.is_err(),
            "terminal resume failure should not open another websocket"
        );
    });

    let client = test_remote_client(websocket_url);
    let first_connection = client.connection().await?;
    wait_for_disconnect(&first_connection).await;
    let first_error = match client.connection().await {
        Ok(_) => panic!("unknown session should fail reconnect"),
        Err(err) => err,
    };
    assert_eq!(
        first_error.to_string(),
        "exec-server rejected request (-32600): unknown session id session-1"
    );
    let second_error = match client.connection().await {
        Ok(_) => panic!("unknown session should stay terminal"),
        Err(err) => err,
    };
    assert_eq!(
        second_error.to_string(),
        "exec-server rejected request (-32600): unknown session id session-1"
    );

    server.await.expect("test websocket server should finish");
    assert_eq!(accepted_connections.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn remote_process_start_releases_session_after_initial_connect_failure() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let first = WebSocketJsonRpcPeer::accept(&listener).await;
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        second
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        let request = second.read_request(EXEC_METHOD).await;
        second
            .write_response(
                request.id,
                ExecResponse {
                    process_id: ProcessId::from("proc"),
                },
            )
            .await;
    });

    let client = test_remote_client(websocket_url);
    let process = RemoteProcess::new(client);
    let params = test_exec_params(&ProcessId::from("proc"));
    assert!(
        process.start(params.clone()).await.is_err(),
        "initial connect should fail before dispatch"
    );
    let started = process.start(params).await?;
    assert_eq!(started.process.process_id(), &ProcessId::from("proc"));

    server.await.expect("test websocket server should finish");
    Ok(())
}

#[tokio::test]
async fn remote_process_drop_releases_session_for_process_id_reuse() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let mut peer = WebSocketJsonRpcPeer::accept(&listener).await;
        peer.complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        for _ in 0..2 {
            let request = peer.read_request(EXEC_METHOD).await;
            peer.write_response(
                request.id,
                ExecResponse {
                    process_id: ProcessId::from("proc"),
                },
            )
            .await;
        }
    });

    let client = test_remote_client(websocket_url);
    let process = RemoteProcess::new(client);
    let params = test_exec_params(&ProcessId::from("proc"));
    let started = process.start(params.clone()).await?;
    drop(started);
    let restarted = timeout(Duration::from_secs(1), async {
        loop {
            match process.start(params.clone()).await {
                Ok(restarted) => return Ok(restarted),
                Err(crate::ExecServerError::Protocol(message))
                    if message == "session already registered for process proc" =>
                {
                    tokio::task::yield_now().await;
                }
                Err(err) => return Err(err),
            }
        }
    })
    .await
    .context("dropped process session should unregister")??;
    assert_eq!(restarted.process.process_id(), &ProcessId::from("proc"));

    server.await.expect("test websocket server should finish");
    Ok(())
}

#[tokio::test]
async fn remote_client_reconnects_before_dispatching_every_remote_api() -> Result<()> {
    for api in RemoteApi::ALL {
        assert_remote_api_reconnects_before_dispatch(api).await?;
    }
    Ok(())
}

#[tokio::test]
async fn remote_client_replays_cursor_read_once_after_disconnect() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("test websocket listener should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let first_request = Arc::new(StdMutex::new(None));
    let replayed_request = Arc::new(StdMutex::new(None));
    let server_first_request = Arc::clone(&first_request);
    let server_replayed_request = Arc::clone(&replayed_request);
    let server = tokio::spawn(async move {
        let mut first = WebSocketJsonRpcPeer::accept(&listener).await;
        first
            .complete_initialize(/*expected_resume_session_id*/ None, "session-1")
            .await;
        let request = first.read_request(EXEC_READ_METHOD).await;
        *server_first_request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(decode_request_params::<ReadParams>(&request));
        drop(first);

        let mut second = WebSocketJsonRpcPeer::accept(&listener).await;
        second
            .complete_initialize(Some("session-1"), "session-1")
            .await;
        let request = second.read_request(EXEC_READ_METHOD).await;
        *server_replayed_request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(decode_request_params::<ReadParams>(&request));
        second
            .write_response(request.id, successful_read_response())
            .await;
    });

    let client = test_remote_client(websocket_url);
    let params = ReadParams {
        process_id: ProcessId::from("proc"),
        after_seq: Some(7),
        max_bytes: Some(1024),
        wait_ms: Some(25),
    };
    assert_eq!(
        client.read(params.clone()).await?,
        successful_read_response()
    );

    server.await.expect("test websocket server should finish");
    assert_eq!(
        first_request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
        Some(params.clone())
    );
    assert_eq!(
        replayed_request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
        Some(params)
    );
    Ok(())
}

#[tokio::test]
async fn remote_client_does_not_replay_non_read_apis_after_disconnect() -> Result<()> {
    for api in RemoteApi::NON_REPLAYABLE {
        assert_remote_api_does_not_replay_after_disconnect(api).await?;
    }
    Ok(())
}
