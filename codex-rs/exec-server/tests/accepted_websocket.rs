use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use axum::Router;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::any;
use codex_exec_server::EnvironmentInfo;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::EnvironmentObservedStatus;
use codex_exec_server::EnvironmentStatus;
use codex_exec_server::EnvironmentStatusKind;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecResponse;
use codex_exec_server::ExecServerClientConnectOptions;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadParams;
use codex_exec_server::ReadResponse;
use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_utils_path_uri::PathUri;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

type AcceptedSocket = axum::extract::ws::WebSocket;
const SESSION_ALREADY_ATTACHED_ERROR_CODE: i64 = -32010;

#[tokio::test]
async fn accepted_websocket_rejects_initial_resume_session_id() -> Result<()> {
    let (websocket_url, mut accepted_sockets, server_task) = start_acceptor().await?;
    let (_socket, _) = connect_async(&websocket_url).await?;
    let accepted_websocket = timeout(TEST_TIMEOUT, accepted_sockets.recv())
        .await?
        .context("accepted websocket channel should remain open")?;
    let mut options = accepted_options();
    options.resume_session_id = Some("session-1".to_string());

    let error = EnvironmentManager::from_accepted_websocket(
        "environment-1".to_string(),
        accepted_websocket,
        options,
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
    .expect_err("initial accepted websocket should reject a resume session ID");

    assert!(
        error
            .to_string()
            .contains("initial connection cannot resume a session"),
        "unexpected error: {error}"
    );

    server_task.abort();
    let _ = server_task.await;
    Ok(())
}

#[tokio::test]
async fn accepted_websocket_environment_info_uses_initialization_metadata() -> Result<()> {
    let (websocket_url, mut accepted_sockets, server_task) = start_acceptor().await?;
    let (_socket, manager) =
        connect_executor(&websocket_url, &mut accepted_sockets, "session-1").await?;
    let environment = manager
        .default_environment()
        .context("accepted environment")?;

    assert_eq!(
        timeout(TEST_TIMEOUT, environment.info()).await??,
        EnvironmentInfo::local()
    );

    server_task.abort();
    let _ = server_task.await;
    Ok(())
}

#[tokio::test]
async fn accepted_websocket_environment_is_ready_immediately() -> Result<()> {
    let (websocket_url, mut accepted_sockets, server_task) = start_acceptor().await?;
    let (mut socket, manager) =
        connect_executor(&websocket_url, &mut accepted_sockets, "session-1").await?;

    let status_task =
        tokio::spawn(async move { manager.get_environment_status("environment-1").await });
    let request = receive_jsonrpc(&mut socket).await?;
    let JSONRPCMessage::Request(JSONRPCRequest { id, method, .. }) = request else {
        anyhow::bail!("expected environment status request, got {request:?}");
    };
    assert_eq!(method, "environment/status");
    send_jsonrpc(
        &mut socket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::to_value(EnvironmentStatus {
                status: EnvironmentStatusKind::Ready,
            })?,
        }),
    )
    .await?;

    assert_eq!(
        timeout(TEST_TIMEOUT, status_task).await??,
        Some(EnvironmentObservedStatus::Ready)
    );

    server_task.abort();
    let _ = server_task.await;
    Ok(())
}

#[tokio::test]
async fn accepted_websocket_replacement_retires_old_socket_and_retries() -> Result<()> {
    let (websocket_url, mut accepted_sockets, server_task) = start_acceptor().await?;
    let (mut first_socket, manager) =
        connect_executor(&websocket_url, &mut accepted_sockets, "session-1").await?;
    let (mut rejected_socket, rejected_websocket) =
        connect_replacement_executor(&websocket_url, &mut accepted_sockets).await?;
    manager
        .replace_accepted_websocket("environment-1", rejected_websocket)
        .await?;
    let previous_socket_event = timeout(TEST_TIMEOUT, first_socket.next())
        .await
        .context("the previous accepted websocket should be retired before replacement")?;
    assert!(
        matches!(
            previous_socket_event,
            None | Some(Ok(Message::Close(_))) | Some(Err(_))
        ),
        "the previous accepted websocket should close before replacement: {previous_socket_event:?}"
    );
    let initialize = receive_jsonrpc(&mut rejected_socket).await?;
    let JSONRPCMessage::Request(JSONRPCRequest { id, method, .. }) = initialize else {
        anyhow::bail!("expected replacement initialize request, got {initialize:?}");
    };
    assert_eq!(method, "initialize");

    let (_overlapping_socket, overlapping_websocket) =
        connect_replacement_executor(&websocket_url, &mut accepted_sockets).await?;
    manager
        .replace_accepted_websocket("environment-1", overlapping_websocket)
        .await
        .expect_err("an overlapping replacement should be rejected");

    send_jsonrpc(
        &mut rejected_socket,
        JSONRPCMessage::Error(JSONRPCError {
            id,
            error: JSONRPCErrorError {
                code: SESSION_ALREADY_ATTACHED_ERROR_CODE,
                message: "session session-1 is already attached to another connection".to_string(),
                data: None,
            },
        }),
    )
    .await?;
    let rejected_socket_event = timeout(TEST_TIMEOUT, rejected_socket.next())
        .await
        .context("rejected replacement websocket should close")?;
    assert!(
        matches!(
            rejected_socket_event,
            None | Some(Ok(Message::Close(_))) | Some(Err(_))
        ),
        "rejected replacement websocket should close: {rejected_socket_event:?}"
    );

    let (mut replacement_socket, replacement_websocket) =
        connect_replacement_executor(&websocket_url, &mut accepted_sockets).await?;
    manager
        .replace_accepted_websocket("environment-1", replacement_websocket)
        .await?;
    complete_initialize(&mut replacement_socket, "session-1", Some("session-1")).await?;

    server_task.abort();
    let _ = server_task.await;
    Ok(())
}

#[tokio::test]
async fn accepted_websocket_reconnect_recovers_running_process_and_output() -> Result<()> {
    let (websocket_url, mut accepted_sockets, server_task) = start_acceptor().await?;
    let (mut first_socket, manager) =
        connect_executor(&websocket_url, &mut accepted_sockets, "session-1").await?;
    let environment = manager
        .default_environment()
        .context("default environment should be installed")?;
    let backend = environment.get_exec_backend();
    let process_id = ProcessId::from("process-1");
    let process_task = tokio::spawn({
        let process_id = process_id.clone();
        async move {
            backend
                .start(ExecParams {
                    process_id,
                    argv: vec!["test-command".to_string()],
                    cwd: PathUri::parse("file:///workspace")?,
                    env_policy: None,
                    env: HashMap::new(),
                    tty: false,
                    pipe_stdin: false,
                    arg0: None,
                    sandbox: None,
                    enforce_managed_network: false,
                    managed_network: None,
                    network_proxy: None,
                    shell_snapshot: None,
                })
                .await
                .map_err(anyhow::Error::from)
        }
    });
    let request = receive_jsonrpc(&mut first_socket).await?;
    let JSONRPCMessage::Request(JSONRPCRequest {
        id, method, params, ..
    }) = request
    else {
        anyhow::bail!("expected process start request, got {request:?}");
    };
    assert_eq!(method, "process/start");
    assert_eq!(
        serde_json::from_value::<ExecParams>(params.context("process params should exist")?)?
            .process_id,
        process_id
    );
    send_jsonrpc(
        &mut first_socket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::to_value(ExecResponse {
                process_id: process_id.clone(),
                sandbox_type: None,
            })?,
        }),
    )
    .await?;
    let process = timeout(TEST_TIMEOUT, process_task).await???.process;

    first_socket.close(/*close_frame*/ None).await?;
    let (mut replacement_socket, replacement_websocket) =
        connect_replacement_executor(&websocket_url, &mut accepted_sockets).await?;
    manager
        .replace_accepted_websocket("environment-1", replacement_websocket)
        .await?;
    complete_initialize(&mut replacement_socket, "session-1", Some("session-1")).await?;

    let request = receive_jsonrpc(&mut replacement_socket).await?;
    let JSONRPCMessage::Request(JSONRPCRequest {
        id, method, params, ..
    }) = request
    else {
        anyhow::bail!("expected recovery process read request, got {request:?}");
    };
    assert_eq!(method, "process/read");
    assert_eq!(
        serde_json::from_value::<ReadParams>(params.context("read params should exist")?)?,
        ReadParams {
            process_id: process_id.clone(),
            after_seq: Some(0),
            max_bytes: None,
            wait_ms: Some(0),
        }
    );
    send_jsonrpc(
        &mut replacement_socket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::to_value(ReadResponse {
                chunks: Vec::new(),
                next_seq: 1,
                exited: false,
                exit_code: None,
                closed: false,
                failure: None,
                sandbox_denied: false,
            })?,
        }),
    )
    .await?;

    let read_task = tokio::spawn(async move {
        process.read(Some(0), /*max_bytes*/ None, Some(0)).await
    });
    let request = receive_jsonrpc(&mut replacement_socket).await?;
    let JSONRPCMessage::Request(JSONRPCRequest {
        id, method, params, ..
    }) = request
    else {
        anyhow::bail!("expected existing process read request, got {request:?}");
    };
    assert_eq!(method, "process/read");
    assert_eq!(
        serde_json::from_value::<ReadParams>(params.context("read params should exist")?)?,
        ReadParams {
            process_id,
            after_seq: Some(0),
            max_bytes: None,
            wait_ms: Some(0),
        }
    );
    let response = ReadResponse {
        chunks: Vec::new(),
        next_seq: 1,
        exited: false,
        exit_code: None,
        closed: false,
        failure: None,
        sandbox_denied: false,
    };
    send_jsonrpc(
        &mut replacement_socket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::to_value(&response)?,
        }),
    )
    .await?;
    assert_eq!(timeout(TEST_TIMEOUT, read_task).await???, response);

    server_task.abort();
    let _ = server_task.await;
    Ok(())
}

async fn start_acceptor() -> Result<(
    String,
    mpsc::UnboundedReceiver<AcceptedSocket>,
    JoinHandle<()>,
)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let (accepted_tx, accepted_rx) = mpsc::unbounded_channel();
    let app = Router::new()
        .route("/", any(accept_websocket))
        .with_state(accepted_tx);
    let server_task = tokio::spawn(async move {
        let result = axum::serve(listener, app).await;
        assert!(
            result.is_ok(),
            "accepted websocket test server should run: {result:?}"
        );
    });
    Ok((format!("ws://{local_addr}/"), accepted_rx, server_task))
}

async fn accept_websocket(
    websocket: WebSocketUpgrade,
    State(accepted_tx): State<mpsc::UnboundedSender<AcceptedSocket>>,
) -> impl IntoResponse {
    websocket.on_upgrade(move |websocket| async move {
        let _ = accepted_tx.send(websocket);
    })
}

async fn connect_executor(
    websocket_url: &str,
    accepted_sockets: &mut mpsc::UnboundedReceiver<AcceptedSocket>,
    session_id: &str,
) -> Result<(
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    EnvironmentManager,
)> {
    let (mut websocket, _) = connect_async(websocket_url).await?;
    let accepted_websocket = timeout(TEST_TIMEOUT, accepted_sockets.recv())
        .await?
        .context("accepted websocket channel should remain open")?;
    let manager_task = tokio::spawn(EnvironmentManager::from_accepted_websocket(
        "environment-1".to_string(),
        accepted_websocket,
        accepted_options(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    ));
    complete_initialize(&mut websocket, session_id, /*resume_session_id*/ None).await?;
    let manager = timeout(TEST_TIMEOUT, manager_task).await???;
    Ok((websocket, manager))
}

async fn connect_replacement_executor(
    websocket_url: &str,
    accepted_sockets: &mut mpsc::UnboundedReceiver<AcceptedSocket>,
) -> Result<(
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    AcceptedSocket,
)> {
    let (websocket, _) = connect_async(websocket_url).await?;
    let accepted_websocket = timeout(TEST_TIMEOUT, accepted_sockets.recv())
        .await?
        .context("accepted websocket channel should remain open")?;
    Ok((websocket, accepted_websocket))
}

fn accepted_options() -> ExecServerClientConnectOptions {
    ExecServerClientConnectOptions {
        client_name: "host-test".to_string(),
        initialize_timeout: TEST_TIMEOUT,
        resume_session_id: None,
    }
}

async fn complete_initialize<S>(
    websocket: &mut WebSocketStream<S>,
    session_id: &str,
    resume_session_id: Option<&str>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let initialize = receive_jsonrpc(&mut *websocket).await?;
    let JSONRPCMessage::Request(JSONRPCRequest {
        id, method, params, ..
    }) = initialize
    else {
        anyhow::bail!("expected initialize request, got {initialize:?}");
    };
    assert_eq!(method, "initialize");
    assert_eq!(
        serde_json::from_value::<InitializeParams>(
            params.context("initialize request should contain params")?
        )?,
        InitializeParams {
            client_name: "host-test".to_string(),
            resume_session_id: resume_session_id.map(str::to_string),
        }
    );
    send_jsonrpc(
        &mut *websocket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::to_value(InitializeResponse {
                session_id: session_id.to_string(),
                environment_info: Some(EnvironmentInfo::local()),
            })?,
        }),
    )
    .await?;
    let initialized = receive_jsonrpc(&mut *websocket).await?;
    assert_eq!(
        initialized,
        JSONRPCMessage::Notification(JSONRPCNotification {
            method: "initialized".to_string(),
            params: Some(serde_json::json!({})),
        })
    );
    Ok(())
}

async fn send_jsonrpc<S>(websocket: &mut WebSocketStream<S>, message: JSONRPCMessage) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    websocket
        .send(Message::Text(serde_json::to_string(&message)?.into()))
        .await?;
    Ok(())
}

async fn receive_jsonrpc<S>(websocket: &mut WebSocketStream<S>) -> Result<JSONRPCMessage>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let message = websocket
            .next()
            .await
            .context("accepted websocket should remain open")??;
        if let Message::Text(text) = message {
            return Ok(serde_json::from_str(&text)?);
        }
    }
}
