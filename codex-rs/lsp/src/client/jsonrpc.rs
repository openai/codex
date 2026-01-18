use crate::client::transport::Transport;
use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(data) = &self.data {
            write!(f, "jsonrpc error {}: {} ({data})", self.code, self.message)
        } else {
            write!(f, "jsonrpc error {}: {}", self.code, self.message)
        }
    }
}

impl std::error::Error for JsonRpcError {}

#[derive(Debug)]
pub enum IncomingMessage {
    Notification {
        method: String,
        params: Option<Value>,
    },
    Request {
        id: Value,
        method: String,
        params: Option<Value>,
    },
}

#[derive(Clone)]
pub struct JsonRpcClient {
    inner: Arc<Inner>,
}

pub struct JsonRpcIncoming {
    pub rx: mpsc::Receiver<IncomingMessage>,
}

struct Inner {
    transport: Arc<Transport>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, JsonRpcError>>>>,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    next_id: AtomicU64,
    closed: Notify,
}

impl JsonRpcClient {
    pub fn new(transport: Transport) -> (Self, JsonRpcIncoming) {
        let transport = Arc::new(transport);
        let (incoming_tx, incoming_rx) = mpsc::channel(64);
        let inner = Arc::new(Inner {
            transport: Arc::clone(&transport),
            pending: Mutex::new(HashMap::new()),
            incoming_tx,
            next_id: AtomicU64::new(1),
            closed: Notify::new(),
        });
        let client = Self {
            inner: Arc::clone(&inner),
        };
        tokio::spawn(read_loop(Arc::clone(&inner)));
        (client, JsonRpcIncoming { rx: incoming_rx })
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<()> {
        let mut payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(params) = params {
            payload["params"] = params;
        }
        let message = serde_json::to_string(&payload).context("serialize jsonrpc notification")?;
        self.inner.transport.write_message(&message).await
    }

    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, JsonRpcError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let mut payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if let Some(params) = params {
            payload["params"] = params;
        }
        let message = serde_json::to_string(&payload).map_err(|err| JsonRpcError {
            code: -32603,
            message: format!("serialize request failed: {err}"),
            data: None,
        })?;
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.inner.pending.lock().await;
            pending.insert(id, tx);
        }
        if let Err(err) = self.inner.transport.write_message(&message).await {
            let mut pending = self.inner.pending.lock().await;
            pending.remove(&id);
            return Err(JsonRpcError {
                code: -32603,
                message: format!("send request failed: {err}"),
                data: None,
            });
        }
        rx.await.unwrap_or_else(|_err| {
            Err(JsonRpcError {
                code: -32603,
                message: "jsonrpc response channel closed".to_string(),
                data: None,
            })
        })
    }

    pub async fn respond(
        &self,
        id: Value,
        result: Option<Value>,
        error: Option<JsonRpcError>,
    ) -> Result<()> {
        let payload = if let Some(error) = error {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": error.code,
                    "message": error.message,
                    "data": error.data,
                }
            })
        } else {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result.unwrap_or(Value::Null),
            })
        };
        let message = serde_json::to_string(&payload).context("serialize response")?;
        self.inner.transport.write_message(&message).await
    }

    pub async fn wait_closed(&self) {
        self.inner.closed.notified().await;
    }
}

async fn read_loop(inner: Arc<Inner>) {
    loop {
        let message = match inner.transport.read_message().await {
            Ok(message) => message,
            Err(err) => {
                tracing::warn!("lsp transport closed: {err}");
                close_pending(
                    &inner,
                    JsonRpcError {
                        code: -32603,
                        message: "lsp transport closed".to_string(),
                        data: None,
                    },
                )
                .await;
                mark_closed(&inner).await;
                break;
            }
        };

        let value: Value = match serde_json::from_str(&message) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!("invalid jsonrpc payload: {err}");
                continue;
            }
        };

        let method = value
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_string);
        let id_value = value.get("id").cloned();
        if let Some(method) = method {
            let params = value.get("params").cloned();
            if let Some(id) = id_value {
                let _ = inner
                    .incoming_tx
                    .send(IncomingMessage::Request { id, method, params })
                    .await;
            } else {
                let _ = inner
                    .incoming_tx
                    .send(IncomingMessage::Notification { method, params })
                    .await;
            }
            continue;
        }

        let Some(id) = id_value else {
            continue;
        };
        let id = match id.as_u64().or_else(|| id.as_i64().map(|v| v as u64)) {
            Some(id) => id,
            None => {
                tracing::warn!("jsonrpc response with non-integer id");
                continue;
            }
        };
        let result = if let Some(error) = value.get("error") {
            let error = JsonRpcError {
                code: error.get("code").and_then(Value::as_i64).unwrap_or(-32603),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_string(),
                data: error.get("data").cloned(),
            };
            Err(error)
        } else {
            Ok(value.get("result").cloned().unwrap_or(Value::Null))
        };

        let sender = {
            let mut pending = inner.pending.lock().await;
            pending.remove(&id)
        };
        if let Some(sender) = sender {
            let _ = sender.send(result);
        }
    }
}

async fn close_pending(inner: &Inner, error: JsonRpcError) {
    let pending = {
        let mut guard = inner.pending.lock().await;
        guard.drain().map(|(_, tx)| tx).collect::<Vec<_>>()
    };
    for tx in pending {
        let _ = tx.send(Err(error.clone()));
    }
}

async fn mark_closed(inner: &Inner) {
    inner.closed.notify_waiters();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::transport::Transport;
    use crate::client::transport::read_framed_message;
    use crate::client::transport::write_framed_message;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use tokio::io::BufReader;
    use tokio::io::duplex;

    #[tokio::test]
    async fn request_roundtrip() {
        let (client_io, server_io) = duplex(2048);
        let (client_read, client_write) = tokio::io::split(client_io);
        let transport = Transport::from_io(
            Box::new(BufReader::new(client_read)),
            Box::new(client_write),
        );
        let (client, _) = JsonRpcClient::new(transport);

        let (server_read, mut server_write) = tokio::io::split(server_io);
        let mut server_reader = BufReader::new(server_read);

        let server_task = tokio::spawn(async move {
            let message = read_framed_message(&mut server_reader).await.unwrap();
            let value: Value = serde_json::from_str(&message).unwrap();
            assert_eq!(value["method"], "test/request");
            let id = value["id"].as_u64().unwrap();
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "ok": true }
            });
            let response = serde_json::to_string(&response).unwrap();
            write_framed_message(&mut server_write, &response)
                .await
                .unwrap();
        });

        let response = client
            .request("test/request", Some(serde_json::json!({ "value": 1 })))
            .await
            .unwrap();
        assert_eq!(response["ok"], true);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn incoming_request_delivered() {
        let (client_io, server_io) = duplex(2048);
        let (client_read, client_write) = tokio::io::split(client_io);
        let transport = Transport::from_io(
            Box::new(BufReader::new(client_read)),
            Box::new(client_write),
        );
        let (client, mut incoming) = JsonRpcClient::new(transport);

        let (server_read, mut server_write) = tokio::io::split(server_io);
        let mut server_reader = BufReader::new(server_read);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "workspace/configuration",
            "params": { "items": [] }
        });
        let request = serde_json::to_string(&request).unwrap();
        write_framed_message(&mut server_write, &request)
            .await
            .unwrap();

        let incoming_message = incoming.rx.recv().await.expect("incoming request");
        let id = match incoming_message {
            IncomingMessage::Request { id, method, .. } => {
                assert_eq!(method, "workspace/configuration");
                id
            }
            other => panic!("unexpected message: {other:?}"),
        };

        client
            .respond(id, Some(Value::Array(Vec::new())), None)
            .await
            .unwrap();

        let response = read_framed_message(&mut server_reader).await.unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["id"], 5);
    }
}
