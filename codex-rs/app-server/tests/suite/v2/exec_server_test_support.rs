use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

pub(crate) async fn accept_exec_server_environment(
    listener: TcpListener,
    environment_info: Value,
) -> Result<WebSocketStream<TcpStream>> {
    let mut websocket = accept_initialized_exec_server(listener).await?;

    let request = read_exec_server_json(&mut websocket).await?;
    assert_eq!(request["method"], "environment/info");
    websocket
        .send(Message::Text(
            json!({
                "id": request["id"],
                "result": environment_info,
            })
            .to_string()
            .into(),
        ))
        .await?;

    Ok(websocket)
}

pub(crate) async fn accept_initialized_exec_server(
    listener: TcpListener,
) -> Result<WebSocketStream<TcpStream>> {
    let (stream, _) = listener.accept().await?;
    let mut websocket = accept_async(stream).await?;

    let initialize = read_exec_server_json(&mut websocket).await?;
    assert_eq!(initialize["method"], "initialize");
    websocket
        .send(Message::Text(
            json!({
                "id": initialize["id"],
                "result": {"sessionId": "test-session"},
            })
            .to_string()
            .into(),
        ))
        .await?;
    let initialized = read_exec_server_json(&mut websocket).await?;
    assert_eq!(initialized["method"], "initialized");

    Ok(websocket)
}

pub(crate) async fn read_exec_server_json(
    websocket: &mut WebSocketStream<TcpStream>,
) -> Result<Value> {
    loop {
        match websocket
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("exec-server websocket closed"))??
        {
            Message::Text(text) => return Ok(serde_json::from_str(text.as_ref())?),
            Message::Binary(bytes) => return Ok(serde_json::from_slice(bytes.as_ref())?),
            Message::Ping(_) | Message::Pong(_) => {}
            message => anyhow::bail!("expected JSON-RPC message, got {message:?}"),
        }
    }
}
