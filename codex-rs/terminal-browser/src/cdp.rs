use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(crate) struct CdpClient {
    socket: Socket,
    next_id: u64,
}

impl CdpClient {
    pub(crate) async fn connect(websocket_url: &str) -> Result<Self> {
        let (socket, _) = timeout(
            Duration::from_secs(/*secs*/ 10),
            connect_async(websocket_url),
        )
        .await
        .context("timed out connecting to Carbonyl DevTools")??;
        let mut client = Self { socket, next_id: 1 };
        client.call("Page.enable", json!({})).await?;
        client.call("Runtime.enable", json!({})).await?;
        Ok(client)
    }

    pub(crate) async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.socket
            .send(Message::Text(request.to_string().into()))
            .await
            .with_context(|| format!("send CDP request {method}"))?;

        timeout(Duration::from_secs(/*secs*/ 15), async {
            loop {
                let message = self
                    .socket
                    .next()
                    .await
                    .context("Carbonyl closed the DevTools connection")??;
                match message {
                    Message::Text(text) => {
                        let response: Value =
                            serde_json::from_str(text.as_str()).context("decode CDP response")?;
                        if response.get("id").and_then(Value::as_u64) != Some(id) {
                            continue;
                        }
                        if let Some(error) = response.get("error") {
                            bail!("CDP {method} failed: {error}");
                        }
                        return Ok(response.get("result").cloned().unwrap_or(Value::Null));
                    }
                    Message::Close(frame) => {
                        bail!("Carbonyl closed the DevTools connection: {frame:?}");
                    }
                    Message::Ping(bytes) => {
                        self.socket.send(Message::Pong(bytes)).await?;
                    }
                    Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
                }
            }
        })
        .await
        .with_context(|| format!("timed out waiting for CDP {method}"))?
    }

    pub(crate) async fn evaluate(&mut self, expression: &str) -> Result<Value> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;
        if let Some(details) = result.get("exceptionDetails") {
            bail!("browser script failed: {details}");
        }
        Ok(result
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }
}
