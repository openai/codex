use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
#[cfg(unix)]
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
#[cfg(unix)]
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
#[cfg(unix)]
use codex_app_server_protocol::ThreadStartParams;
#[cfg(unix)]
use codex_app_server_protocol::ThreadStartResponse;
#[cfg(unix)]
use codex_app_server_protocol::TurnStartParams;
#[cfg(unix)]
use codex_app_server_protocol::UserInput as V2UserInput;
#[cfg(unix)]
use core_test_support::responses;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::json;
use std::net::SocketAddr;
use std::path::Path;
#[cfg(unix)]
use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
#[cfg(unix)]
use wiremock::Mock;
#[cfg(unix)]
use wiremock::matchers::method;
#[cfg(unix)]
use wiremock::matchers::path_regex;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);

type WsClient = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[tokio::test]
async fn websocket_transport_routes_per_connection_handshake_and_responses() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let bind_addr = reserve_local_addr()?;
    let mut process = spawn_websocket_server(codex_home.path(), bind_addr).await?;

    let mut ws1 = connect_websocket(bind_addr).await?;
    let mut ws2 = connect_websocket(bind_addr).await?;

    send_initialize_request(&mut ws1, 1, "ws_client_one").await?;
    let first_init = read_response_for_id(&mut ws1, 1).await?;
    assert_eq!(first_init.id, RequestId::Integer(1));

    // Initialize responses are request-scoped and must not leak to other
    // connections.
    assert_no_message(&mut ws2, Duration::from_millis(250)).await?;

    send_config_read_request(&mut ws2, 2).await?;
    let not_initialized = read_error_for_id(&mut ws2, 2).await?;
    assert_eq!(not_initialized.error.message, "Not initialized");

    send_initialize_request(&mut ws2, 3, "ws_client_two").await?;
    let second_init = read_response_for_id(&mut ws2, 3).await?;
    assert_eq!(second_init.id, RequestId::Integer(3));

    // Same request-id on different connections must route independently.
    send_config_read_request(&mut ws1, 77).await?;
    send_config_read_request(&mut ws2, 77).await?;
    let ws1_config = read_response_for_id(&mut ws1, 77).await?;
    let ws2_config = read_response_for_id(&mut ws2, 77).await?;

    assert_eq!(ws1_config.id, RequestId::Integer(77));
    assert_eq!(ws2_config.id, RequestId::Integer(77));
    assert!(ws1_config.result.get("config").is_some());
    assert!(ws2_config.result.get("config").is_some());

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_ctrl_c_waits_for_running_turn_before_exit() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
    } = start_ctrl_c_restart_fixture(Duration::from_secs(3)).await?;

    send_sigint(&process)?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    let status = wait_for_process_exit_within(
        &mut process,
        Duration::from_secs(10),
        "timed out waiting for graceful Ctrl-C restart shutdown",
    )
    .await?;
    assert!(status.success(), "expected graceful exit, got {status}");

    expect_websocket_disconnect(&mut ws).await?;

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_second_ctrl_c_forces_exit_while_turn_running() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
    } = start_ctrl_c_restart_fixture(Duration::from_secs(3)).await?;

    send_sigint(&process)?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    send_sigint(&process)?;
    let status = wait_for_process_exit_within(
        &mut process,
        Duration::from_secs(2),
        "timed out waiting for forced Ctrl-C restart shutdown",
    )
    .await?;
    assert!(status.success(), "expected graceful exit, got {status}");

    expect_websocket_disconnect(&mut ws).await?;

    Ok(())
}

async fn spawn_websocket_server(codex_home: &Path, bind_addr: SocketAddr) -> Result<Child> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
        .context("should find app-server binary")?;
    let mut cmd = Command::new(program);
    cmd.arg("--listen")
        .arg(format!("ws://{bind_addr}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("CODEX_HOME", codex_home)
        .env("RUST_LOG", "debug");
    let mut process = cmd
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn websocket app-server process")?;

    if let Some(stderr) = process.stderr.take() {
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                eprintln!("[websocket app-server stderr] {line}");
            }
        });
    }

    Ok(process)
}

fn reserve_local_addr() -> Result<SocketAddr> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(addr)
}

async fn connect_websocket(bind_addr: SocketAddr) -> Result<WsClient> {
    let url = format!("ws://{bind_addr}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match connect_async(&url).await {
            Ok((stream, _response)) => return Ok(stream),
            Err(err) => {
                if Instant::now() >= deadline {
                    bail!("failed to connect websocket to {url}: {err}");
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

async fn send_initialize_request(stream: &mut WsClient, id: i64, client_name: &str) -> Result<()> {
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

async fn send_config_read_request(stream: &mut WsClient, id: i64) -> Result<()> {
    send_request(
        stream,
        "config/read",
        id,
        Some(json!({ "includeLayers": false })),
    )
    .await
}

#[cfg(unix)]
async fn send_thread_start_request(stream: &mut WsClient, id: i64) -> Result<()> {
    send_request(
        stream,
        "thread/start",
        id,
        Some(serde_json::to_value(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })?),
    )
    .await
}

#[cfg(unix)]
async fn send_turn_start_request(stream: &mut WsClient, id: i64, thread_id: &str) -> Result<()> {
    send_request(
        stream,
        "turn/start",
        id,
        Some(serde_json::to_value(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })?),
    )
    .await
}

async fn send_request(
    stream: &mut WsClient,
    method: &str,
    id: i64,
    params: Option<serde_json::Value>,
) -> Result<()> {
    let message = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(id),
        method: method.to_string(),
        params,
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

async fn read_response_for_id(stream: &mut WsClient, id: i64) -> Result<JSONRPCResponse> {
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

async fn read_error_for_id(stream: &mut WsClient, id: i64) -> Result<JSONRPCError> {
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

async fn read_jsonrpc_message(stream: &mut WsClient) -> Result<JSONRPCMessage> {
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

async fn assert_no_message(stream: &mut WsClient, wait_for: Duration) -> Result<()> {
    match timeout(wait_for, stream.next()).await {
        Ok(Some(Ok(frame))) => bail!("unexpected frame while waiting for silence: {frame:?}"),
        Ok(Some(Err(err))) => bail!("unexpected websocket read error: {err}"),
        Ok(None) => bail!("websocket closed unexpectedly while waiting for silence"),
        Err(_) => Ok(()),
    }
}

#[cfg(unix)]
struct GracefulCtrlCFixture {
    _codex_home: TempDir,
    _server: wiremock::MockServer,
    process: Child,
    ws: WsClient,
}

#[cfg(unix)]
async fn start_ctrl_c_restart_fixture(turn_delay: Duration) -> Result<GracefulCtrlCFixture> {
    let server = responses::start_mock_server().await;
    let delayed_turn_response = create_final_assistant_message_sse_response("Done")?;
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responses::sse_response(delayed_turn_response).set_delay(turn_delay))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let bind_addr = reserve_local_addr()?;
    let process = spawn_websocket_server(codex_home.path(), bind_addr).await?;
    let mut ws = connect_websocket(bind_addr).await?;

    send_initialize_request(&mut ws, 1, "ws_graceful_shutdown").await?;
    let init_response = read_response_for_id(&mut ws, 1).await?;
    assert_eq!(init_response.id, RequestId::Integer(1));

    send_thread_start_request(&mut ws, 2).await?;
    let thread_start_response = read_response_for_id(&mut ws, 2).await?;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;

    send_turn_start_request(&mut ws, 3, &thread.id).await?;
    let turn_start_response = read_response_for_id(&mut ws, 3).await?;
    assert_eq!(turn_start_response.id, RequestId::Integer(3));

    wait_for_responses_post(&server, Duration::from_secs(5)).await?;

    Ok(GracefulCtrlCFixture {
        _codex_home: codex_home,
        _server: server,
        process,
        ws,
    })
}

#[cfg(unix)]
async fn wait_for_responses_post(server: &wiremock::MockServer, wait_for: Duration) -> Result<()> {
    let deadline = Instant::now() + wait_for;
    loop {
        let requests = server
            .received_requests()
            .await
            .context("failed to read mock server requests")?;
        if requests
            .iter()
            .any(|request| request.method == "POST" && request.url.path().ends_with("/responses"))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for /responses request");
        }
        sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
fn send_sigint(process: &Child) -> Result<()> {
    let pid = process
        .id()
        .context("websocket app-server process has no pid")?;
    let status = StdCommand::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .context("failed to invoke kill -INT")?;
    if !status.success() {
        bail!("kill -INT exited with {status}");
    }
    Ok(())
}

#[cfg(unix)]
async fn assert_process_does_not_exit_within(process: &mut Child, window: Duration) -> Result<()> {
    match timeout(window, process.wait()).await {
        Err(_) => Ok(()),
        Ok(Ok(status)) => bail!("process exited too early during graceful drain: {status}"),
        Ok(Err(err)) => Err(err).context("failed waiting for process"),
    }
}

#[cfg(unix)]
async fn wait_for_process_exit_within(
    process: &mut Child,
    window: Duration,
    timeout_context: &'static str,
) -> Result<std::process::ExitStatus> {
    timeout(window, process.wait())
        .await
        .context(timeout_context)?
        .context("failed waiting for websocket app-server process exit")
}

#[cfg(unix)]
async fn expect_websocket_disconnect(stream: &mut WsClient) -> Result<()> {
    loop {
        let frame = timeout(DEFAULT_READ_TIMEOUT, stream.next())
            .await
            .context("timed out waiting for websocket disconnect")?;
        match frame {
            None => return Ok(()),
            Some(Ok(WebSocketMessage::Close(_))) => return Ok(()),
            Some(Ok(WebSocketMessage::Ping(payload))) => {
                stream
                    .send(WebSocketMessage::Pong(payload))
                    .await
                    .context("failed to reply to ping while waiting for disconnect")?;
            }
            Some(Ok(WebSocketMessage::Pong(_))) => {}
            Some(Ok(WebSocketMessage::Frame(_))) => {}
            Some(Ok(WebSocketMessage::Text(_))) => {}
            Some(Ok(WebSocketMessage::Binary(_))) => {}
            Some(Err(_)) => return Ok(()),
        }
    }
}

fn create_config_toml(
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
