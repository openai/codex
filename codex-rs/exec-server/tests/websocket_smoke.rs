#![cfg(unix)]

use std::process::Stdio;
use std::time::Duration;

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
use tokio::process::Command;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_initialize_over_websocket() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let websocket_url = reserve_websocket_url()?;
    let mut child = Command::new(binary);
    child.args(["--listen", &websocket_url]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::inherit());
    let mut child = child.spawn()?;

    let (mut websocket, _) = connect_websocket_when_ready(&websocket_url).await?;
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
async fn exec_server_reports_malformed_websocket_json_and_keeps_running() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let websocket_url = reserve_websocket_url()?;
    let mut child = Command::new(binary);
    child.args(["--listen", &websocket_url]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::inherit());
    let mut child = child.spawn()?;

    let (mut websocket, _) = connect_websocket_when_ready(&websocket_url).await?;
    futures::SinkExt::send(&mut websocket, Message::Text("not-json".to_string().into())).await?;

    let Some(Ok(Message::Text(response_text))) = futures::StreamExt::next(&mut websocket).await
    else {
        panic!("expected malformed-message error response");
    };
    let response: JSONRPCMessage = serde_json::from_str(response_text.as_ref())?;
    let JSONRPCMessage::Error(JSONRPCError { id, error }) = response else {
        panic!("expected malformed-message error response");
    };
    assert_eq!(id, RequestId::Integer(-1));
    assert_eq!(error.code, -32600);
    assert!(
        error
            .message
            .starts_with("failed to parse websocket JSON-RPC message from exec-server websocket"),
        "unexpected malformed-message error: {}",
        error.message
    );

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
        panic!("expected initialize response after malformed input");
    };
    let response: JSONRPCMessage = serde_json::from_str(response_text.as_ref())?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response after malformed input");
    };
    assert_eq!(id, RequestId::Integer(1));
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    assert_eq!(initialize_response, InitializeResponse {});

    child.start_kill()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_stubs_process_start_over_websocket() -> anyhow::Result<()> {
    let binary = cargo_bin("codex-exec-server")?;
    let websocket_url = reserve_websocket_url()?;
    let mut child = Command::new(binary);
    child.args(["--listen", &websocket_url]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::inherit());
    let mut child = child.spawn()?;

    let (mut websocket, _) = connect_websocket_when_ready(&websocket_url).await?;
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

fn reserve_websocket_url() -> anyhow::Result<String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(format!("ws://{addr}"))
}

async fn connect_websocket_when_ready(
    websocket_url: &str,
) -> anyhow::Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::handshake::client::Response,
)> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match connect_async(websocket_url).await {
            Ok(websocket) => return Ok(websocket),
            Err(err)
                if tokio::time::Instant::now() < deadline
                    && matches!(
                        err,
                        tokio_tungstenite::tungstenite::Error::Io(ref io_err)
                            if io_err.kind() == std::io::ErrorKind::ConnectionRefused
                    ) =>
            {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(err) => return Err(err.into()),
        }
    }
}
