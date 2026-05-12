use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use app_test_support::DISABLE_PLUGIN_STARTUP_TASKS_ARG;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_uds::UnixStream;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;

// macOS and Windows CI can spend tens of seconds starting the app-server test
// binary under Bazel before it accepts JSON-RPC over the control socket.
#[cfg(any(target_os = "macos", windows))]
pub(super) const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
pub(super) const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) type WsClient = WebSocketStream<UnixStream>;

#[tokio::test]
async fn unix_socket_transport_routes_per_connection_handshake_and_responses() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, socket_path, _socket_dir) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws1 = connect_websocket(&socket_path).await?;
    let mut ws2 = connect_websocket(&socket_path).await?;

    send_initialize_request(&mut ws1, /*id*/ 1, "ws_client_one").await?;
    let first_init = read_response_for_id(&mut ws1, /*id*/ 1).await?;
    assert_eq!(first_init.id, RequestId::Integer(1));

    // Initialize responses are request-scoped and must not leak to other
    // connections.
    assert_no_message(&mut ws2, Duration::from_millis(250)).await?;

    send_config_read_request(&mut ws2, /*id*/ 2).await?;
    let not_initialized = read_error_for_id(&mut ws2, /*id*/ 2).await?;
    assert_eq!(not_initialized.error.message, "Not initialized");

    send_initialize_request(&mut ws2, /*id*/ 3, "ws_client_two").await?;
    let second_init = read_response_for_id(&mut ws2, /*id*/ 3).await?;
    assert_eq!(second_init.id, RequestId::Integer(3));

    // Same request-id on different connections must route independently.
    send_config_read_request(&mut ws1, /*id*/ 77).await?;
    send_config_read_request(&mut ws2, /*id*/ 77).await?;
    let ws1_config = read_response_for_id(&mut ws1, /*id*/ 77).await?;
    let ws2_config = read_response_for_id(&mut ws2, /*id*/ 77).await?;

    assert_eq!(ws1_config.id, RequestId::Integer(77));
    assert_eq!(ws2_config.id, RequestId::Integer(77));
    assert!(ws1_config.result.get("config").is_some());
    assert!(ws2_config.result.get("config").is_some());

    process
        .kill()
        .await
        .context("failed to stop app-server process")?;
    Ok(())
}

#[tokio::test]
async fn unix_socket_disconnect_keeps_last_subscribed_thread_loaded_until_idle_timeout()
-> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, socket_path, _socket_dir) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws1 = connect_websocket(&socket_path).await?;
    send_initialize_request(&mut ws1, /*id*/ 1, "ws_thread_owner").await?;
    read_response_for_id(&mut ws1, /*id*/ 1).await?;

    let thread_id = start_thread(&mut ws1, /*id*/ 2).await?;
    assert_loaded_threads(&mut ws1, /*id*/ 3, &[thread_id.as_str()]).await?;

    ws1.close(None).await.context("failed to close websocket")?;
    drop(ws1);

    let mut ws2 = connect_websocket(&socket_path).await?;
    send_initialize_request(&mut ws2, /*id*/ 4, "ws_reconnect_client").await?;
    read_response_for_id(&mut ws2, /*id*/ 4).await?;

    wait_for_loaded_threads(&mut ws2, /*first_id*/ 5, &[thread_id.as_str()]).await?;

    process
        .kill()
        .await
        .context("failed to stop app-server process")?;
    Ok(())
}

pub(super) async fn spawn_websocket_server(codex_home: &Path) -> Result<(Child, PathBuf, TempDir)> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
        .context("should find app-server binary")?;
    #[cfg(unix)]
    let socket_dir = tempfile::Builder::new()
        .prefix("cxs-")
        .tempdir_in("/tmp")
        .context("failed to create short app-server socket temp dir")?;
    #[cfg(not(unix))]
    let socket_dir = tempfile::Builder::new()
        .prefix("cxs-")
        .tempdir()
        .context("failed to create app-server socket temp dir")?;
    let socket_path = socket_dir.path().join("c.sock");
    let listen_url = format!("unix://{}", socket_path.display());
    let mut cmd = Command::new(program);
    cmd.arg("--listen")
        .arg(&listen_url)
        .arg(DISABLE_PLUGIN_STARTUP_TASKS_ARG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("CODEX_HOME", codex_home)
        .env("RUST_LOG", "warn");
    let mut process = cmd
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn app-server process")?;

    let stderr = process
        .stderr
        .take()
        .context("failed to capture app-server stderr")?;
    let mut stderr_reader = BufReader::new(stderr).lines();
    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            eprintln!("[app-server stderr] {line}");
        }
    });

    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    loop {
        if socket_path.exists() {
            return Ok((process, socket_path, socket_dir));
        }
        if let Some(status) = process.try_wait()? {
            bail!("app-server exited before creating control socket: {status}");
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for app-server control socket at {}",
                socket_path.display()
            );
        }
        sleep(Duration::from_millis(50)).await;
    }
}

pub(super) async fn connect_websocket(socket_path: &Path) -> Result<WsClient> {
    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    loop {
        match UnixStream::connect(socket_path).await {
            Ok(stream) => match client_async("ws://localhost/rpc", stream).await {
                Ok((websocket, _response)) => return Ok(websocket),
                Err(err) => {
                    if Instant::now() >= deadline {
                        bail!(
                            "failed to upgrade websocket over {}: {err}",
                            socket_path.display()
                        );
                    }
                }
            },
            Err(err) => {
                if Instant::now() >= deadline {
                    bail!("failed to connect to {}: {err}", socket_path.display());
                }
            }
        }
        sleep(Duration::from_millis(50)).await;
    }
}

pub(super) async fn send_initialize_request(
    stream: &mut WsClient,
    id: i64,
    client_name: &str,
) -> Result<()> {
    let params = InitializeParams {
        client_info: ClientInfo {
            name: client_name.to_string(),
            title: Some("WebSocket Test Client".to_string()),
            version: "0.1.0".to_string(),
        },
        capabilities: None,
    };
    send_request(
        stream,
        "initialize",
        id,
        Some(serde_json::to_value(params)?),
    )
    .await
}

async fn start_thread(stream: &mut WsClient, id: i64) -> Result<String> {
    send_request(
        stream,
        "thread/start",
        id,
        Some(serde_json::to_value(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })?),
    )
    .await?;
    let response = read_response_for_id(stream, id).await?;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(response)?;
    Ok(thread.id)
}

async fn assert_loaded_threads(stream: &mut WsClient, id: i64, expected: &[&str]) -> Result<()> {
    let response = request_loaded_threads(stream, id).await?;
    let mut actual = response.data;
    actual.sort();
    let mut expected = expected
        .iter()
        .map(|thread_id| (*thread_id).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(response.next_cursor, None);
    Ok(())
}

async fn wait_for_loaded_threads(
    stream: &mut WsClient,
    first_id: i64,
    expected: &[&str],
) -> Result<()> {
    let mut next_id = first_id;
    let expected = expected
        .iter()
        .map(|thread_id| (*thread_id).to_string())
        .collect::<Vec<_>>();
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let response = request_loaded_threads(stream, next_id).await?;
            next_id += 1;
            let mut actual = response.data;
            actual.sort();
            if actual == expected {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("timed out waiting for loaded thread list")??;
    Ok(())
}

async fn request_loaded_threads(
    stream: &mut WsClient,
    id: i64,
) -> Result<ThreadLoadedListResponse> {
    send_request(
        stream,
        "thread/loaded/list",
        id,
        Some(serde_json::to_value(ThreadLoadedListParams::default())?),
    )
    .await?;
    let response = read_response_for_id(stream, id).await?;
    to_response::<ThreadLoadedListResponse>(response)
}

async fn send_config_read_request(stream: &mut WsClient, id: i64) -> Result<()> {
    send_request(
        stream,
        "config/read",
        id,
        Some(json!({ "includeLayers": false })),
    )
    .await
}

pub(super) async fn send_request(
    stream: &mut WsClient,
    method: &str,
    id: i64,
    params: Option<serde_json::Value>,
) -> Result<()> {
    let message = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(id),
        method: method.to_string(),
        params,
        trace: None,
    });
    send_jsonrpc(stream, message).await
}

async fn send_jsonrpc(stream: &mut WsClient, message: JSONRPCMessage) -> Result<()> {
    let payload = serde_json::to_string(&message)?;
    stream
        .send(WebSocketMessage::Text(payload.into()))
        .await
        .context("failed to send websocket frame")
}

pub(super) async fn read_response_for_id(
    stream: &mut WsClient,
    id: i64,
) -> Result<JSONRPCResponse> {
    let target_id = RequestId::Integer(id);
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Response(response) = message
            && response.id == target_id
        {
            return Ok(response);
        }
    }
}

pub(super) async fn read_notification_for_method(
    stream: &mut WsClient,
    method: &str,
) -> Result<JSONRPCNotification> {
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Notification(notification) = message
            && notification.method == method
        {
            return Ok(notification);
        }
    }
}

pub(super) async fn read_response_and_notification_for_method(
    stream: &mut WsClient,
    id: i64,
    method: &str,
) -> Result<(JSONRPCResponse, JSONRPCNotification)> {
    let target_id = RequestId::Integer(id);
    let mut response = None;
    let mut notification = None;

    while response.is_none() || notification.is_none() {
        let message = read_jsonrpc_message(stream).await?;
        match message {
            JSONRPCMessage::Response(candidate) if candidate.id == target_id => {
                response = Some(candidate);
            }
            JSONRPCMessage::Notification(candidate) if candidate.method == method => {
                if notification.replace(candidate).is_some() {
                    bail!(
                        "received duplicate notification for method `{method}` before completing paired read"
                    );
                }
            }
            _ => {}
        }
    }

    let Some(response) = response else {
        bail!("response must be set before returning");
    };
    let Some(notification) = notification else {
        bail!("notification must be set before returning");
    };

    Ok((response, notification))
}

pub(super) async fn read_error_for_id(stream: &mut WsClient, id: i64) -> Result<JSONRPCError> {
    let target_id = RequestId::Integer(id);
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Error(err) = message
            && err.id == target_id
        {
            return Ok(err);
        }
    }
}

pub(super) async fn read_jsonrpc_message(stream: &mut WsClient) -> Result<JSONRPCMessage> {
    loop {
        let frame = timeout(DEFAULT_READ_TIMEOUT, stream.next())
            .await
            .context("timed out waiting for websocket frame")?
            .context("websocket stream ended unexpectedly")?
            .context("failed to read websocket frame")?;

        match frame {
            WebSocketMessage::Text(text) => return Ok(serde_json::from_str(text.as_ref())?),
            WebSocketMessage::Ping(payload) => {
                stream.send(WebSocketMessage::Pong(payload)).await?;
            }
            WebSocketMessage::Pong(_) => {}
            WebSocketMessage::Close(frame) => {
                bail!("websocket closed unexpectedly: {frame:?}")
            }
            WebSocketMessage::Binary(_) => bail!("unexpected binary websocket frame"),
            WebSocketMessage::Frame(_) => {}
        }
    }
}

pub(super) async fn assert_no_message(stream: &mut WsClient, wait_for: Duration) -> Result<()> {
    match timeout(wait_for, stream.next()).await {
        Ok(Some(Ok(frame))) => bail!("unexpected frame while waiting for silence: {frame:?}"),
        Ok(Some(Err(err))) => bail!("unexpected websocket read error: {err}"),
        Ok(None) => bail!("websocket closed unexpectedly while waiting for silence"),
        Err(_) => Ok(()),
    }
}

pub(super) fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
