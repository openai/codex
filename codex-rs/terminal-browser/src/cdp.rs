use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
#[cfg(test)]
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[cfg(test)]
use futures::SinkExt;
#[cfg(test)]
use futures::StreamExt;
#[cfg(test)]
use tokio_tungstenite::MaybeTlsStream;
#[cfg(test)]
use tokio_tungstenite::WebSocketStream;
#[cfg(test)]
use tokio_tungstenite::connect_async;
#[cfg(test)]
use tokio_tungstenite::tungstenite::Message;

#[cfg(test)]
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type PendingCalls = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;
type PageSessionId = Arc<Mutex<Option<String>>>;

#[cfg(test)]
const CONNECT_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);
const CALL_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 15);
const OUTBOUND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const MAX_EVENT_BYTES: usize = 16 * 1024;
const MAX_CDP_FRAME_BYTES: usize = 16 * 1024 * 1024;
const PIPE_READ_BUFFER_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CdpEvent {
    Message(Value),
    Disconnected(String),
}

pub(crate) struct ConnectedPage {
    pub(crate) client: CdpClient,
    pub(crate) title: String,
}

#[derive(Clone)]
pub(crate) struct CdpClient {
    outbound: mpsc::Sender<String>,
    pending: PendingCalls,
    events: broadcast::Sender<CdpEvent>,
    next_id: Arc<AtomicU64>,
    page_session_id: PageSessionId,
    call_timeout: Duration,
}

impl CdpClient {
    #[cfg(test)]
    pub(crate) async fn connect(websocket_url: &str) -> Result<Self> {
        Self::connect_with_call_timeout(websocket_url, CALL_TIMEOUT).await
    }

    #[cfg(test)]
    async fn connect_with_call_timeout(
        websocket_url: &str,
        call_timeout: Duration,
    ) -> Result<Self> {
        let (socket, _) = timeout(CONNECT_TIMEOUT, connect_async(websocket_url))
            .await
            .context("timed out connecting to Carbonyl DevTools")??;
        let (client, outbound_rx) = Self::new(call_timeout);
        tokio::spawn(run_websocket_pump(
            socket,
            outbound_rx,
            client.pending.clone(),
            client.events.clone(),
            client.page_session_id.clone(),
        ));
        client.initialize_page().await?;
        Ok(client)
    }

    #[cfg(unix)]
    pub(crate) async fn connect_pipe(
        reader: std::os::unix::net::UnixStream,
        writer: std::os::unix::net::UnixStream,
    ) -> Result<ConnectedPage> {
        Self::connect_pipe_io(
            tokio::net::UnixStream::from_std(reader)
                .context("adopt Carbonyl DevTools output pipe")?,
            tokio::net::UnixStream::from_std(writer)
                .context("adopt Carbonyl DevTools input pipe")?,
            CALL_TIMEOUT,
        )
        .await
    }

    async fn connect_pipe_io<R, W>(
        reader: R,
        writer: W,
        call_timeout: Duration,
    ) -> Result<ConnectedPage>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (client, outbound_rx) = Self::new(call_timeout);
        tokio::spawn(run_pipe_pump(
            reader,
            writer,
            outbound_rx,
            client.pending.clone(),
            client.events.clone(),
            client.page_session_id.clone(),
        ));

        let target_list: TargetList =
            serde_json::from_value(client.call_root("Target.getTargets", json!({})).await?)
                .context("decode Carbonyl page targets")?;
        let target = target_list
            .target_infos
            .into_iter()
            .find(|target| target.kind == "page")
            .context("Carbonyl DevTools did not expose a page target")?;
        let attached: AttachedTarget = serde_json::from_value(
            client
                .call_root(
                    "Target.attachToTarget",
                    json!({
                        "targetId": target.target_id,
                        "flatten": true,
                    }),
                )
                .await?,
        )
        .context("decode Carbonyl page session")?;
        *client
            .page_session_id
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(attached.session_id);
        client.initialize_page().await?;
        Ok(ConnectedPage {
            client,
            title: target.title,
        })
    }

    fn new(call_timeout: Duration) -> (Self, mpsc::Receiver<String>) {
        let (outbound, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        (
            Self {
                outbound,
                pending: Arc::new(Mutex::new(/*t*/ HashMap::new())),
                events,
                next_id: Arc::new(AtomicU64::new(/*v*/ 1)),
                page_session_id: Arc::new(Mutex::new(/*t*/ None)),
                call_timeout,
            },
            outbound_rx,
        )
    }

    async fn initialize_page(&self) -> Result<()> {
        self.call("Page.enable", json!({})).await?;
        self.call("Runtime.enable", json!({})).await?;
        self.call("DOM.enable", json!({})).await?;
        self.call("Accessibility.enable", json!({})).await?;
        self.call("Page.setLifecycleEventsEnabled", json!({ "enabled": true }))
            .await?;
        Ok(())
    }

    pub(crate) fn subscribe_events(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let page_session_id = self
            .page_session_id
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        self.call_with_session(method, params, page_session_id.as_deref())
            .await
    }

    pub(crate) async fn call_browser(&self, method: &str, params: Value) -> Result<Value> {
        self.call_root(method, params).await
    }

    async fn call_root(&self, method: &str, params: Value) -> Result<Value> {
        self.call_with_session(method, params, /*session_id*/ None)
            .await
    }

    async fn call_with_session(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value> {
        let id = self.next_id.fetch_add(/*val*/ 1, Ordering::Relaxed);
        let mut request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(session_id) = session_id {
            request["sessionId"] = Value::String(session_id.to_string());
        }
        let encoded = serde_json::to_string(&request).context("encode CDP request")?;
        anyhow::ensure!(
            encoded.len() <= MAX_CDP_FRAME_BYTES,
            "CDP {method} request exceeded the {MAX_CDP_FRAME_BYTES}-byte frame limit"
        );

        let (response_tx, response_rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, response_tx);
        let _pending_call = PendingCallGuard {
            pending: self.pending.clone(),
            id,
        };

        let result = timeout(self.call_timeout, async {
            if self.outbound.send(encoded).await.is_err() {
                bail!("Carbonyl closed the DevTools connection");
            }
            response_rx
                .await
                .context("Carbonyl closed the DevTools connection")?
                .map_err(anyhow::Error::msg)
        })
        .await;

        let response = match result {
            Ok(response) => response?,
            Err(_) => {
                bail!("timed out waiting for CDP {method}");
            }
        };
        if let Some(error) = response.get("error") {
            bail!("CDP {method} failed: {error}");
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    pub(crate) async fn evaluate(&self, expression: &str) -> Result<Value> {
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetList {
    target_infos: Vec<TargetInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetInfo {
    target_id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachedTarget {
    session_id: String,
}

struct PendingCallGuard {
    pending: PendingCalls,
    id: u64,
}

impl Drop for PendingCallGuard {
    fn drop(&mut self) {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.id);
    }
}

struct PipeFrameDecoder {
    frame: Vec<u8>,
    max_frame_bytes: usize,
}

impl PipeFrameDecoder {
    fn new(max_frame_bytes: usize) -> Self {
        Self {
            frame: Vec::with_capacity(PIPE_READ_BUFFER_BYTES.min(max_frame_bytes)),
            max_frame_bytes,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<Value>> {
        let mut messages = Vec::new();
        for &byte in chunk {
            if byte == 0 {
                let message = serde_json::from_slice(&self.frame)
                    .context("failed to decode DevTools pipe message")?;
                messages.push(message);
                self.frame.clear();
                continue;
            }
            anyhow::ensure!(
                self.frame.len() < self.max_frame_bytes,
                "DevTools pipe message exceeded the {}-byte frame limit",
                self.max_frame_bytes
            );
            self.frame.push(byte);
        }
        Ok(messages)
    }
}

async fn run_pipe_pump<R, W>(
    mut reader: R,
    writer: W,
    outbound: mpsc::Receiver<String>,
    pending: PendingCalls,
    events: broadcast::Sender<CdpEvent>,
    page_session_id: PageSessionId,
) where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let mut writer_task = tokio::spawn(run_pipe_writer(writer, outbound));
    let mut decoder = PipeFrameDecoder::new(MAX_CDP_FRAME_BYTES);
    let mut read_buffer = [0_u8; PIPE_READ_BUFFER_BYTES];
    let disconnect_reason = loop {
        tokio::select! {
            incoming = reader.read(&mut read_buffer) => match incoming {
                Ok(0) => break "Carbonyl closed the DevTools connection".to_string(),
                Ok(count) => match decoder.push(&read_buffer[..count]) {
                    Ok(messages) => {
                        for message in messages {
                            dispatch_message(message, &pending, &events, &page_session_id);
                        }
                    }
                    Err(error) => break error.to_string(),
                },
                Err(error) => break format!("DevTools pipe connection failed: {error}"),
            },
            writer_result = &mut writer_task => break match writer_result {
                Ok(reason) => reason,
                Err(error) => format!("DevTools pipe writer stopped: {error}"),
            },
        }
    };

    writer_task.abort();
    finish_disconnect(&pending, &events, disconnect_reason);
}

async fn run_pipe_writer<W>(mut writer: W, mut outbound: mpsc::Receiver<String>) -> String
where
    W: AsyncWrite + Send + Unpin + 'static,
{
    while let Some(message) = outbound.recv().await {
        if let Err(error) = writer.write_all(message.as_bytes()).await {
            return format!("failed to send DevTools pipe message: {error}");
        }
        if let Err(error) = writer.write_all(&[0]).await {
            return format!("failed to delimit DevTools pipe message: {error}");
        }
    }
    "DevTools client closed".to_string()
}

#[cfg(test)]
async fn run_websocket_pump(
    mut socket: Socket,
    mut outbound: mpsc::Receiver<String>,
    pending: PendingCalls,
    events: broadcast::Sender<CdpEvent>,
    page_session_id: PageSessionId,
) {
    let disconnect_reason = loop {
        tokio::select! {
            outgoing = outbound.recv() => match outgoing {
                Some(message) => {
                    if let Err(error) = socket.send(Message::Text(message.into())).await {
                        break format!("failed to send DevTools message: {error}");
                    }
                }
                None => {
                    let _ = socket.close(/*msg*/ None).await;
                    break "DevTools client closed".to_string();
                }
            },
            incoming = socket.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    let response = match serde_json::from_str::<Value>(text.as_str()) {
                        Ok(response) => response,
                        Err(error) => break format!("failed to decode DevTools message: {error}"),
                    };
                    dispatch_message(response, &pending, &events, &page_session_id);
                }
                Some(Ok(Message::Ping(bytes))) => {
                    if let Err(error) = socket.send(Message::Pong(bytes)).await {
                        break format!("failed to reply to DevTools ping: {error}");
                    }
                }
                Some(Ok(Message::Close(frame))) => {
                    break format!("Carbonyl closed the DevTools connection: {frame:?}");
                }
                Some(Ok(Message::Binary(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Err(error)) => break format!("DevTools connection failed: {error}"),
                None => break "Carbonyl closed the DevTools connection".to_string(),
            }
        }
    };

    finish_disconnect(&pending, &events, disconnect_reason);
}

fn dispatch_message(
    response: Value,
    pending: &PendingCalls,
    events: &broadcast::Sender<CdpEvent>,
    page_session_id: &PageSessionId,
) {
    let response_tx = response.get("id").and_then(Value::as_u64).and_then(|id| {
        pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
    });
    if let Some(response_tx) = response_tx {
        let _ = response_tx.send(Ok(response));
        return;
    }
    let page_session_id = page_session_id
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if should_broadcast_event(&response, page_session_id.as_deref()) {
        let _ = events.send(CdpEvent::Message(response));
    }
}

fn should_broadcast_event(message: &Value, page_session_id: Option<&str>) -> bool {
    if let Some(page_session_id) = page_session_id
        && message.get("sessionId").and_then(Value::as_str) != Some(page_session_id)
    {
        return false;
    }
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return false;
    };
    matches!(
        method,
        "Page.domContentEventFired"
            | "Page.frameNavigated"
            | "Page.lifecycleEvent"
            | "Page.loadEventFired"
            | "Page.navigatedWithinDocument"
    ) && serde_json::to_vec(message).is_ok_and(|encoded| encoded.len() <= MAX_EVENT_BYTES)
}

fn finish_disconnect(pending: &PendingCalls, events: &broadcast::Sender<CdpEvent>, reason: String) {
    fail_pending(pending, &reason);
    let _ = events.send(CdpEvent::Disconnected(reason));
}

fn fail_pending(pending: &PendingCalls, reason: &str) {
    let pending = std::mem::take(&mut *pending.lock().unwrap_or_else(PoisonError::into_inner));
    for response_tx in pending.into_values() {
        let _ = response_tx.send(Err(reason.to_string()));
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
