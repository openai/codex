#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_utils_cargo_bin::cargo_bin;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_initialize_over_websocket() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let mut child = Command::new(binary);
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::piped());
    let mut child = child.spawn()?;
    let stderr = child.stderr.take().expect("stderr");
    let mut stderr_lines = BufReader::new(stderr).lines();
    let websocket_url = read_websocket_url(&mut stderr_lines).await?;

    let (mut websocket, _) = connect_async(&websocket_url).await?;
    let initialize = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "initialize".to_string(),
        params: Some(serde_json::to_value(InitializeParams {
            client_name: "exec-server-test".to_string(),
        })?),
        trace: None,
    });
    futures::SinkExt::send(
        &mut websocket,
        Message::Text(serde_json::to_string(&initialize)?.into()),
    )
    .await?;

    let Some(Ok(Message::Text(response_text))) = futures::StreamExt::next(&mut websocket).await
    else {
        panic!("expected initialize response");
    };
    let response: JSONRPCMessage = serde_json::from_str(response_text.as_ref())?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response");
    };
    assert_eq!(id, RequestId::Integer(1));
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    assert_eq!(initialize_response, InitializeResponse {});

    let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: Some(serde_json::json!({})),
    });
    futures::SinkExt::send(
        &mut websocket,
        Message::Text(serde_json::to_string(&initialized)?.into()),
    )
    .await?;

    child.start_kill()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_stubs_process_start_over_websocket() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let mut child = Command::new(binary);
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::piped());
    let mut child = child.spawn()?;
    let stderr = child.stderr.take().expect("stderr");
    let mut stderr_lines = BufReader::new(stderr).lines();
    let websocket_url = read_websocket_url(&mut stderr_lines).await?;

    let (mut websocket, _) = connect_async(&websocket_url).await?;
    let initialize = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "initialize".to_string(),
        params: Some(serde_json::to_value(InitializeParams {
            client_name: "exec-server-test".to_string(),
        })?),
        trace: None,
    });
    futures::SinkExt::send(
        &mut websocket,
        Message::Text(serde_json::to_string(&initialize)?.into()),
    )
    .await?;
    let _ = futures::StreamExt::next(&mut websocket).await;

    let exec = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(2),
        method: "process/start".to_string(),
        params: Some(serde_json::json!({
            "processId": "proc-1",
            "argv": ["true"],
            "cwd": std::env::current_dir()?,
            "env": {},
            "tty": false,
            "arg0": null
        })),
        trace: None,
    });
    futures::SinkExt::send(
        &mut websocket,
        Message::Text(serde_json::to_string(&exec)?.into()),
    )
    .await?;

    let Some(Ok(Message::Text(response_text))) = futures::StreamExt::next(&mut websocket).await
    else {
        panic!("expected process/start error");
    };
    let response: JSONRPCMessage = serde_json::from_str(response_text.as_ref())?;
    let JSONRPCMessage::Error(JSONRPCError { id, error }) = response else {
        panic!("expected process/start stub error");
    };
    assert_eq!(id, RequestId::Integer(2));
    assert_eq!(error.code, -32601);
    assert_eq!(
        error.message,
        "exec-server stub does not implement `process/start` yet"
    );

    child.start_kill()?;
    Ok(())
}

async fn read_websocket_url<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> anyhow::Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let line = timeout(Duration::from_secs(5), lines.next_line()).await??;
    let line = line.context("missing websocket startup banner")?;
    let websocket_url = line
        .split_whitespace()
        .find(|part| part.starts_with("ws://"))
        .context("missing websocket URL in startup banner")?;
    Ok(websocket_url.to_string())
}
