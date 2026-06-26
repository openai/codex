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
use futures::SinkExt;
use futures::StreamExt;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type PendingCalls = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(15);
const OUTBOUND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const MAX_EVENT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CdpEvent {
    Message(Value),
    Disconnected(String),
}

#[derive(Clone)]
pub(crate) struct CdpClient {
    outbound: mpsc::Sender<Message>,
    pending: PendingCalls,
    events: broadcast::Sender<CdpEvent>,
    next_id: Arc<AtomicU64>,
    call_timeout: Duration,
}

impl CdpClient {
    pub(crate) async fn connect(websocket_url: &str) -> Result<Self> {
        Self::connect_with_call_timeout(websocket_url, CALL_TIMEOUT).await
    }

    async fn connect_with_call_timeout(
        websocket_url: &str,
        call_timeout: Duration,
    ) -> Result<Self> {
        let (socket, _) = timeout(CONNECT_TIMEOUT, connect_async(websocket_url))
            .await
            .context("timed out connecting to Carbonyl DevTools")??;
        let (outbound, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let client = Self {
            outbound,
            pending: Arc::new(Mutex::new(HashMap::new())),
            events,
            next_id: Arc::new(AtomicU64::new(/*v*/ 1)),
            call_timeout,
        };
        tokio::spawn(run_pump(
            socket,
            outbound_rx,
            client.pending.clone(),
            client.events.clone(),
        ));
        client.call("Page.enable", json!({})).await?;
        client.call("Runtime.enable", json!({})).await?;
        client.call("DOM.enable", json!({})).await?;
        client.call("Accessibility.enable", json!({})).await?;
        client
            .call("Page.setLifecycleEventsEnabled", json!({ "enabled": true }))
            .await?;
        Ok(client)
    }

    pub(crate) fn subscribe_events(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(/*val*/ 1, Ordering::Relaxed);
        let request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
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
            if self
                .outbound
                .send(Message::Text(request.to_string().into()))
                .await
                .is_err()
            {
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

async fn run_pump(
    mut socket: Socket,
    mut outbound: mpsc::Receiver<Message>,
    pending: PendingCalls,
    events: broadcast::Sender<CdpEvent>,
) {
    let disconnect_reason = loop {
        tokio::select! {
            outgoing = outbound.recv() => match outgoing {
                Some(message) => {
                    if let Err(error) = socket.send(message).await {
                        break format!("failed to send DevTools message: {error}");
                    }
                }
                None => {
                    let _ = socket.close(None).await;
                    break "DevTools client closed".to_string();
                }
            },
            incoming = socket.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    let response = match serde_json::from_str::<Value>(text.as_str()) {
                        Ok(response) => response,
                        Err(error) => break format!("failed to decode DevTools message: {error}"),
                    };
                    dispatch_message(response, &pending, &events);
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

    fail_pending(&pending, &disconnect_reason);
    let _ = events.send(CdpEvent::Disconnected(disconnect_reason));
}

fn dispatch_message(response: Value, pending: &PendingCalls, events: &broadcast::Sender<CdpEvent>) {
    let response_tx = response.get("id").and_then(Value::as_u64).and_then(|id| {
        pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
    });
    if let Some(response_tx) = response_tx {
        let _ = response_tx.send(Ok(response));
    } else if should_broadcast_event(&response) {
        let _ = events.send(CdpEvent::Message(response));
    }
}

fn should_broadcast_event(message: &Value) -> bool {
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

fn fail_pending(pending: &PendingCalls, reason: &str) {
    let pending = std::mem::take(&mut *pending.lock().unwrap_or_else(PoisonError::into_inner));
    for response_tx in pending.into_values() {
        let _ = response_tx.send(Err(reason.to_string()));
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
