//! Signal-driven shutdown tests for Unix websocket servers.

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::WsClient;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_error_for_id;
use super::connection_handling_websocket::read_jsonrpc_message;
use super::connection_handling_websocket::read_notification_for_method;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
#[cfg(unix)]
use std::process::Command as StdCommand;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
use wiremock::Mock;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_ctrl_c_waits_for_running_turn_before_exit() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
        thread_id,
        turn_id,
    } = start_ctrl_c_restart_fixture(Duration::from_secs(6)).await?;

    send_sigint(&process)?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    send_turn_start_request(&mut ws, /*id*/ 4, &thread_id).await?;
    let rejected = read_error_for_id(&mut ws, /*id*/ 4).await?;
    assert_eq!(rejected.error.code, -32600);
    assert_eq!(rejected.error.data, None);

    send_request(
        &mut ws,
        "thread/read",
        /*id*/ 5,
        Some(json!({"threadId": thread_id, "includeTurns": false})),
    )
    .await?;
    read_response_for_id(&mut ws, /*id*/ 5).await?;

    for (method, params) in [
        (
            "thread/shellCommand",
            json!({"threadId": thread_id, "command": "echo blocked"}),
        ),
        (
            "turn/steer",
            json!({"threadId": thread_id, "expectedTurnId": turn_id, "input": []}),
        ),
        ("thread/queue/start", json!({"threadId": thread_id})),
        ("thread/start", json!({})),
        ("thread/fork", json!({"threadId": thread_id})),
        ("thread/resume", json!({"threadId": thread_id})),
        ("thread/delete", json!({"threadId": thread_id})),
        (
            "thread/settings/update",
            json!({"threadId": thread_id, "model": "other"}),
        ),
        (
            "turn/settings/update",
            json!({"threadId": thread_id, "turnId": "unused", "model": "other"}),
        ),
        (
            "thread/revert",
            json!({"threadId": thread_id, "beforeTurnId": "unused"}),
        ),
        (
            "review/start",
            json!({"threadId": thread_id, "target": {"type": "uncommittedChanges"}}),
        ),
    ] {
        send_request(&mut ws, method, /*id*/ 9, Some(params)).await?;
        assert_eq!(
            read_error_for_id(&mut ws, /*id*/ 9).await?.error,
            rejected.error
        );
    }

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
        ..
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

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_sigterm_waits_for_running_turn_before_exit() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
        ..
    } = start_ctrl_c_restart_fixture(Duration::from_secs(3)).await?;

    send_sigterm(&process, _codex_home.path())?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    let status = wait_for_process_exit_within(
        &mut process,
        Duration::from_secs(10),
        "timed out waiting for graceful SIGTERM restart shutdown",
    )
    .await?;
    assert!(status.success(), "expected graceful exit, got {status}");

    expect_websocket_disconnect(&mut ws).await?;

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_second_sigterm_forces_exit_while_turn_running() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
        ..
    } = start_ctrl_c_restart_fixture(Duration::from_secs(3)).await?;

    send_sigterm(&process, _codex_home.path())?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    send_sigterm(&process, _codex_home.path())?;
    let status = wait_for_process_exit_within(
        &mut process,
        Duration::from_secs(2),
        "timed out waiting for forced SIGTERM restart shutdown",
    )
    .await?;
    assert!(status.success(), "expected graceful exit, got {status}");

    expect_websocket_disconnect(&mut ws).await?;

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_repeated_sighup_keeps_waiting_for_running_turn() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
        ..
    } = start_ctrl_c_restart_fixture(Duration::from_secs(3)).await?;

    send_sighup(&process)?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    send_sighup(&process)?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;

    let status = wait_for_process_exit_within(
        &mut process,
        Duration::from_secs(10),
        "timed out waiting for graceful repeated SIGHUP restart shutdown",
    )
    .await?;
    assert!(status.success(), "expected graceful exit, got {status}");

    expect_websocket_disconnect(&mut ws).await?;

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn websocket_transport_allows_turn_interrupt_during_drain() -> Result<()> {
    let GracefulCtrlCFixture {
        _codex_home,
        _server,
        mut process,
        mut ws,
        thread_id,
        turn_id,
    } = start_ctrl_c_restart_fixture(Duration::from_secs(3)).await?;

    send_sigint(&process)?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;
    send_turn_start_request(&mut ws, /*id*/ 5, &thread_id).await?;
    assert_eq!(
        read_error_for_id(&mut ws, /*id*/ 5).await?.error.code,
        -32600
    );
    send_request(
        &mut ws,
        "turn/interrupt",
        /*id*/ 4,
        Some(json!({"threadId": thread_id, "turnId": turn_id})),
    )
    .await?;
    read_response_for_id(&mut ws, /*id*/ 4).await?;
    let status = wait_for_process_exit_within(
        &mut process,
        Duration::from_secs(10),
        "timed out waiting for shutdown after turn interruption",
    )
    .await?;
    assert!(status.success());
    Ok(())
}

#[cfg(unix)]
#[test_case::test_case("thread/queue/add", json!({
    "clientUserMessageId": "queued-before-drain",
    "input": [{"type": "text", "text": "continue from queue"}],
}); "persisted queue")]
#[test_case::test_case("thread/goal/set", json!({
    "objective": "continue the goal",
}); "goal continuation")]
#[tokio::test]
async fn websocket_transport_drain_stops_automatic_turns(
    rpc_method: &str,
    mut params: serde_json::Value,
) -> Result<()> {
    let (release_target, target_gate) = oneshot::channel();
    let (release_guard, guard_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(target_gate),
            body: create_final_assistant_message_sse_response("Done")?,
        }],
        vec![StreamingSseChunk {
            gate: Some(guard_gate),
            body: create_final_assistant_message_sse_response("Guard done")?,
        }],
    ])
    .await;
    let (_codex_home, mut process, mut ws) = start_ctrl_c_restart_client(server.uri()).await?;
    send_thread_start_request(&mut ws, /*id*/ 2).await?;
    let ThreadStartResponse { thread, .. } =
        to_response(read_response_for_id(&mut ws, /*id*/ 2).await?)?;
    send_turn_start_request(&mut ws, /*id*/ 3, &thread.id).await?;
    read_response_for_id(&mut ws, /*id*/ 3).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request_count(/*count*/ 1),
    )
    .await?;

    params["threadId"] = thread.id.into();
    send_request(&mut ws, rpc_method, /*id*/ 4, Some(params)).await?;
    read_response_for_id(&mut ws, /*id*/ 4).await?;
    // Keep another turn active while the target turn runs its idle hooks.
    send_thread_start_request(&mut ws, /*id*/ 10).await?;
    let ThreadStartResponse { thread, .. } =
        to_response(read_response_for_id(&mut ws, /*id*/ 10).await?)?;
    send_turn_start_request(&mut ws, /*id*/ 11, &thread.id).await?;
    read_response_for_id(&mut ws, /*id*/ 11).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request_count(/*count*/ 2),
    )
    .await?;

    send_sigint(&process)?;
    // Observe admission closing before allowing either response to finish.
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            send_thread_start_request(&mut ws, /*id*/ 12).await?;
            loop {
                match read_jsonrpc_message(&mut ws).await? {
                    JSONRPCMessage::Error(error) if error.id == RequestId::Integer(12) => {
                        assert_eq!(error.error.code, -32600);
                        return Ok::<_, anyhow::Error>(());
                    }
                    JSONRPCMessage::Response(response) if response.id == RequestId::Integer(12) => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    })
    .await??;
    release_target.send(()).expect("target response is waiting");
    read_notification_for_method(&mut ws, "turn/completed").await?;
    assert_process_does_not_exit_within(&mut process, Duration::from_millis(300)).await?;
    release_guard.send(()).expect("guard response is waiting");
    assert!(
        wait_for_process_exit_within(
            &mut process,
            Duration::from_secs(15),
            "automatic continuation kept the daemon alive during drain",
        )
        .await?
        .success()
    );
    assert_eq!(2, server.requests().await.len());
    Ok(())
}

struct GracefulCtrlCFixture {
    _codex_home: TempDir,
    _server: wiremock::MockServer,
    process: Child,
    ws: WsClient,
    thread_id: String,
    turn_id: String,
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

    let (codex_home, process, mut ws) = start_ctrl_c_restart_client(&server.uri()).await?;

    send_thread_start_request(&mut ws, /*id*/ 2).await?;
    let thread_start_response = read_response_for_id(&mut ws, /*id*/ 2).await?;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;

    send_turn_start_request(&mut ws, /*id*/ 3, &thread.id).await?;
    let turn_start_response = read_response_for_id(&mut ws, /*id*/ 3).await?;
    let TurnStartResponse { turn } = to_response(turn_start_response)?;

    wait_for_responses_post(&server, Duration::from_secs(5)).await?;

    Ok(GracefulCtrlCFixture {
        _codex_home: codex_home,
        _server: server,
        process,
        ws,
        thread_id: thread.id,
        turn_id: turn.id,
    })
}

async fn start_ctrl_c_restart_client(server_uri: &str) -> Result<(TempDir, Child, WsClient)> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), server_uri, "never")?;

    let (process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;
    let mut ws = connect_websocket(bind_addr).await?;

    send_request(
        &mut ws,
        "initialize",
        /*id*/ 1,
        Some(serde_json::to_value(InitializeParams {
            client_info: ClientInfo {
                name: "ws_graceful_shutdown".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                ..Default::default()
            }),
        })?),
    )
    .await?;
    let init_response = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(init_response.id, RequestId::Integer(1));

    Ok((codex_home, process, ws))
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
            client_user_message_id: None,
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

#[cfg(unix)]
fn send_sigint(process: &Child) -> Result<()> {
    send_signal(process, "-INT")
}

#[cfg(unix)]
fn send_sigterm(process: &Child, _home: &std::path::Path) -> Result<()> {
    send_signal(process, "-TERM")
}

#[cfg(unix)]
fn send_sighup(process: &Child) -> Result<()> {
    send_signal(process, "-HUP")
}

#[cfg(unix)]
fn send_signal(process: &Child, signal: &str) -> Result<()> {
    let pid = process
        .id()
        .context("websocket app-server process has no pid")?;
    let status = StdCommand::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .status()
        .with_context(|| format!("failed to invoke kill {signal}"))?;
    if !status.success() {
        bail!("kill {signal} exited with {status}");
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
