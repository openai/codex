use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::WsClient;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::reserve_local_addr;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use futures::SinkExt;
use futures::StreamExt;
use std::process::Command as StdCommand;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
use wiremock::Mock;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

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

struct GracefulCtrlCFixture {
    _codex_home: TempDir,
    _server: wiremock::MockServer,
    process: Child,
    ws: WsClient,
}

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

async fn assert_process_does_not_exit_within(process: &mut Child, window: Duration) -> Result<()> {
    match timeout(window, process.wait()).await {
        Err(_) => Ok(()),
        Ok(Ok(status)) => bail!("process exited too early during graceful drain: {status}"),
        Ok(Err(err)) => Err(err).context("failed waiting for process"),
    }
}

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
