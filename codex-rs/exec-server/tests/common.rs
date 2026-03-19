use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_utils_cargo_bin::cargo_bin;
use futures::{SinkExt, StreamExt};
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use std::process::Stdio;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::connect_async;

enum OutgoingMessage {
    Json(JSONRPCMessage),
    RawText(String),
}

pub struct ExecServer {
    child: tokio::process::Child,
    next_request_id: AtomicI64,
    incoming_rx: mpsc::Receiver<JSONRPCMessage>,
    outgoing_tx: mpsc::Sender<OutgoingMessage>,
    reader_task: JoinHandle<()>,
    writer_task: JoinHandle<()>,
}

impl ExecServer {
    pub async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<RequestId> {
        let request_id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::SeqCst));
        let request = JSONRPCRequest {
            id: request_id.clone(),
            method: method.to_string(),
            params: Some(params),
            trace: None,
        };
        self.outgoing_tx
            .send(OutgoingMessage::Json(JSONRPCMessage::Request(request)))
            .await?;
        Ok(request_id)
    }

    pub async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()> {
        let notification = JSONRPCNotification {
            method: method.to_string(),
            params: Some(params),
        };
        self.outgoing_tx
            .send(OutgoingMessage::Json(JSONRPCMessage::Notification(
                notification,
            )))
            .await?;
        Ok(())
    }

    pub async fn send_raw_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.outgoing_tx
            .send(OutgoingMessage::RawText(text.to_string()))
            .await?;
        Ok(())
    }

    pub async fn next_event(&mut self) -> anyhow::Result<JSONRPCMessage> {
        self.incoming_rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("exec-server closed before next event"))
    }

    pub async fn wait_for_event(
        &mut self,
        predicate: impl Fn(&JSONRPCMessage) -> bool,
    ) -> anyhow::Result<JSONRPCMessage> {
        loop {
            let event = self.next_event().await?;
            if predicate(&event) {
                return Ok(event);
            }
        }
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.reader_task.abort();
        self.writer_task.abort();
        self.child.start_kill()?;
        Ok(())
    }
}

pub mod exec_server {
    use super::*;

    pub async fn exec_server() -> anyhow::Result<super::ExecServer> {
        let binary = cargo_bin("codex-exec-server")?;
        let mut child = Command::new(binary);
        child.args(["--listen", "ws://127.0.0.1:0"]);
        child.stdin(Stdio::null());
        child.stdout(Stdio::null());
        child.stderr(Stdio::piped());
        let mut child = child.spawn()?;

        let stderr = child.stderr.take().expect("stderr should be piped");
        let mut stderr_lines = BufReader::new(stderr).lines();
        let websocket_url = read_websocket_url(&mut stderr_lines).await?;

        let (websocket, _) = connect_async(websocket_url).await?;
        let (mut outgoing_ws, mut incoming_ws) = websocket.split();
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<OutgoingMessage>(128);
        let (incoming_tx, incoming_rx) = mpsc::channel::<JSONRPCMessage>(128);

        let reader_task = tokio::spawn(async move {
            while let Some(message) = incoming_ws.next().await {
                let Ok(message) = message else {
                    break;
                };
                let outgoing = match message {
                    Message::Text(text) => serde_json::from_str::<JSONRPCMessage>(&text),
                    Message::Binary(bytes) => serde_json::from_slice::<JSONRPCMessage>(&bytes),
                    _ => continue,
                };
                if let Ok(message) = outgoing && let Err(_err) = incoming_tx.send(message).await {
                    break;
                }
            }
        });

        let writer_task = tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                let outgoing = match message {
                    OutgoingMessage::Json(message) => {
                        match serde_json::to_string(&message) {
                            Ok(json) => Message::Text(json.into()),
                            Err(_) => continue,
                        }
                    }
                    OutgoingMessage::RawText(message) => Message::Text(message.into()),
                };
                if outgoing_ws.send(outgoing).await.is_err() {
                    break;
                }
            }
        });

        Ok(ExecServer {
            child,
            next_request_id: AtomicI64::new(1),
            incoming_rx,
            outgoing_tx,
            reader_task,
            writer_task,
        })
    }

    async fn read_websocket_url<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> anyhow::Result<String>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let line = timeout(std::time::Duration::from_secs(5), lines.next_line())
            .await??
            .ok_or_else(|| anyhow::anyhow!("missing websocket startup banner"))?;

        let websocket_url = line
            .split_whitespace()
            .find(|part| part.starts_with("ws://"))
            .ok_or_else(|| anyhow::anyhow!("missing websocket URL in startup banner: {line}"))?;
        Ok(websocket_url.to_string())
    }
}
