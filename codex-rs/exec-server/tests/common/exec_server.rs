#![allow(dead_code)]

use std::process::Stdio;
use std::time::Duration;

use anyhow::anyhow;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_utils_cargo_bin::cargo_bin;
use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(25);
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct ExecServerHarness {
    child: Child,
    websocket_url: String,
    websocket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_request_id: i64,
}

impl Drop for ExecServerHarness {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

pub(crate) async fn exec_server() -> anyhow::Result<ExecServerHarness> {
    let binary = cargo_bin("codex-exec-server")?;
    let mut child = Command::new(binary);
    child.args(["--listen", "ws://127.0.0.1:0"]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::inherit());
    let mut child = child.spawn()?;

    let websocket_url = read_listen_url_from_stdout(&mut child).await?;
    let (websocket, _) = connect_websocket_when_ready(&websocket_url).await?;
    Ok(ExecServerHarness {
        child,
        websocket_url,
        websocket,
        next_request_id: 1,
    })
}

impl ExecServerHarness {
    pub(crate) fn websocket_url(&self) -> &str {
        &self.websocket_url
    }

    pub(crate) async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<RequestId> {
        let id = RequestId::Integer(self.next_request_id);
        self.next_request_id += 1;
        self.send_message(JSONRPCMessage::Request(JSONRPCRequest {
            id: id.clone(),
            method: method.to_string(),
            params: Some(params),
            trace: None,
        }))
        .await?;
        Ok(id)
    }

    pub(crate) async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.send_message(JSONRPCMessage::Notification(JSONRPCNotification {
            method: method.to_string(),
            params: Some(params),
        }))
        .await
    }

    pub(crate) async fn send_raw_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.websocket
            .send(Message::Text(text.to_string().into()))
            .await?;
        Ok(())
    }

    pub(crate) async fn next_event(&mut self) -> anyhow::Result<JSONRPCMessage> {
        self.next_event_with_timeout(EVENT_TIMEOUT).await
    }

    pub(crate) async fn wait_for_event<F>(
        &mut self,
        mut predicate: F,
    ) -> anyhow::Result<JSONRPCMessage>
    where
        F: FnMut(&JSONRPCMessage) -> bool,
    {
        let deadline = Instant::now() + EVENT_TIMEOUT;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(anyhow!(
                    "timed out waiting for matching exec-server event after {EVENT_TIMEOUT:?}"
                ));
            }
            let remaining = deadline.duration_since(now);
            let event = self.next_event_with_timeout(remaining).await?;
            if predicate(&event) {
                return Ok(event);
            }
        }
    }

    pub(crate) async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.child.start_kill()?;
        Ok(())
    }

    async fn send_message(&mut self, message: JSONRPCMessage) -> anyhow::Result<()> {
        let encoded = serde_json::to_string(&message)?;
        self.websocket.send(Message::Text(encoded.into())).await?;
        Ok(())
    }

    async fn next_event_with_timeout(
        &mut self,
        timeout_duration: Duration,
    ) -> anyhow::Result<JSONRPCMessage> {
        loop {
            let frame = timeout(timeout_duration, self.websocket.next())
                .await
                .map_err(|_| anyhow!("timed out waiting for exec-server websocket event"))?
                .ok_or_else(|| anyhow!("exec-server websocket closed"))??;

            match frame {
                Message::Text(text) => {
                    return Ok(serde_json::from_str(text.as_ref())?);
                }
                Message::Binary(bytes) => {
                    return Ok(serde_json::from_slice(bytes.as_ref())?);
                }
                Message::Close(_) => return Err(anyhow!("exec-server websocket closed")),
                Message::Ping(_) | Message::Pong(_) => {}
                _ => {}
            }
        }
    }
}

async fn connect_websocket_when_ready(
    websocket_url: &str,
) -> anyhow::Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::handshake::client::Response,
)> {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        match connect_async(websocket_url).await {
            Ok(websocket) => return Ok(websocket),
            Err(err)
                if Instant::now() < deadline
                    && matches!(
                        err,
                        tokio_tungstenite::tungstenite::Error::Io(ref io_err)
                            if io_err.kind() == std::io::ErrorKind::ConnectionRefused
                    ) =>
            {
                sleep(CONNECT_RETRY_INTERVAL).await;
            }
            Err(err) => return Err(err.into()),
        }
    }
}

async fn read_listen_url_from_stdout(child: &mut Child) -> anyhow::Result<String> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture exec-server stdout"))?;
    let mut lines = BufReader::new(stdout).lines();
    let deadline = Instant::now() + CONNECT_TIMEOUT;

    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(anyhow!(
                "timed out waiting for exec-server listen URL on stdout after {CONNECT_TIMEOUT:?}"
            ));
        }
        let remaining = deadline.duration_since(now);
        let line = timeout(remaining, lines.next_line())
            .await
            .map_err(|_| anyhow!("timed out waiting for exec-server stdout"))??
            .ok_or_else(|| anyhow!("exec-server stdout closed before emitting listen URL"))?;
        let listen_url = line.trim();
        if listen_url.starts_with("ws://") {
            return Ok(listen_url.to_string());
        }
    }
}
