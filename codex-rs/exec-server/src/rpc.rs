use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;

type PendingRequest = oneshot::Sender<Result<Value, JSONRPCErrorError>>;
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type RequestRoute<S> =
    Box<dyn Fn(Arc<S>, JSONRPCRequest) -> BoxFuture<RpcServerOutboundMessage> + Send + Sync>;
type NotificationRoute<S> =
    Box<dyn Fn(Arc<S>, JSONRPCNotification) -> BoxFuture<Result<(), String>> + Send + Sync>;

#[derive(Debug)]
pub(crate) enum RpcClientEvent {
    Notification(JSONRPCNotification),
    Disconnected { reason: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RpcServerOutboundMessage {
    Response {
        request_id: RequestId,
        result: Value,
    },
    Error {
        request_id: RequestId,
        error: JSONRPCErrorError,
    },
    #[allow(dead_code)]
    Notification(JSONRPCNotification),
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct RpcNotificationSender {
    outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>,
}

impl RpcNotificationSender {
    pub(crate) fn new(outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>) -> Self {
        Self { outgoing_tx }
    }

    #[allow(dead_code)]
    pub(crate) async fn notify<P: Serialize>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<(), JSONRPCErrorError> {
        let params = serde_json::to_value(params).map_err(|err| internal_error(err.to_string()))?;
        self.outgoing_tx
            .send(RpcServerOutboundMessage::Notification(
                JSONRPCNotification {
                    method: method.to_string(),
                    params: Some(params),
                },
            ))
            .await
            .map_err(|_| internal_error("RPC connection closed while sending notification".into()))
    }
}

pub(crate) struct RpcRouter<S> {
    request_routes: HashMap<&'static str, RequestRoute<S>>,
    notification_routes: HashMap<&'static str, NotificationRoute<S>>,
}

impl<S> Default for RpcRouter<S> {
    fn default() -> Self {
        Self {
            request_routes: HashMap::new(),
            notification_routes: HashMap::new(),
        }
    }
}

impl<S> RpcRouter<S>
where
    S: Send + Sync + 'static,
{
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn request<P, R, F, Fut>(&mut self, method: &'static str, handler: F)
    where
        P: DeserializeOwned + Send + 'static,
        R: Serialize + Send + 'static,
        F: Fn(Arc<S>, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R, JSONRPCErrorError>> + Send + 'static,
    {
        self.request_routes.insert(
            method,
            Box::new(move |state, request| {
                let request_id = request.id;
                let params = request.params;
                let response =
                    decode_request_params::<P>(params).map(|params| handler(state, params));
                Box::pin(async move {
                    let response = match response {
                        Ok(response) => response.await,
                        Err(error) => {
                            return RpcServerOutboundMessage::Error { request_id, error };
                        }
                    };
                    match response {
                        Ok(result) => match serde_json::to_value(result) {
                            Ok(result) => RpcServerOutboundMessage::Response { request_id, result },
                            Err(err) => RpcServerOutboundMessage::Error {
                                request_id,
                                error: internal_error(err.to_string()),
                            },
                        },
                        Err(error) => RpcServerOutboundMessage::Error { request_id, error },
                    }
                })
            }),
        );
    }

    pub(crate) fn notification<P, F, Fut>(&mut self, method: &'static str, handler: F)
    where
        P: DeserializeOwned + Send + 'static,
        F: Fn(Arc<S>, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.notification_routes.insert(
            method,
            Box::new(move |state, notification| {
                let params = decode_notification_params::<P>(notification.params)
                    .map(|params| handler(state, params));
                Box::pin(async move {
                    let handler = match params {
                        Ok(handler) => handler,
                        Err(err) => return Err(err),
                    };
                    handler.await
                })
            }),
        );
    }

    pub(crate) fn request_route(&self, method: &str) -> Option<&RequestRoute<S>> {
        self.request_routes.get(method)
    }

    pub(crate) fn notification_route(&self, method: &str) -> Option<&NotificationRoute<S>> {
        self.notification_routes.get(method)
    }
}

pub(crate) struct RpcClient {
    write_tx: mpsc::Sender<JSONRPCMessage>,
    pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
    next_request_id: AtomicI64,
    transport_tasks: Vec<JoinHandle<()>>,
    reader_task: JoinHandle<()>,
}

impl RpcClient {
    pub(crate) fn new(connection: JsonRpcConnection) -> (Self, mpsc::Receiver<RpcClientEvent>) {
        let (write_tx, mut incoming_rx, _disconnected_rx, transport_tasks) =
            connection.into_parts();
        let pending = Arc::new(Mutex::new(HashMap::<RequestId, PendingRequest>::new()));
        let (event_tx, event_rx) = mpsc::channel(128);

        let pending_for_reader = Arc::clone(&pending);
        let reader_task = tokio::spawn(async move {
            while let Some(event) = incoming_rx.recv().await {
                match event {
                    JsonRpcConnectionEvent::Message(message) => {
                        if let Err(err) =
                            handle_server_message(&pending_for_reader, &event_tx, message).await
                        {
                            let _ = err;
                            break;
                        }
                    }
                    JsonRpcConnectionEvent::MalformedMessage { reason } => {
                        let _ = reason;
                        break;
                    }
                    JsonRpcConnectionEvent::Disconnected { reason } => {
                        let _ = event_tx.send(RpcClientEvent::Disconnected { reason }).await;
                        drain_pending(&pending_for_reader).await;
                        return;
                    }
                }
            }

            let _ = event_tx
                .send(RpcClientEvent::Disconnected { reason: None })
                .await;
            drain_pending(&pending_for_reader).await;
        });

        (
            Self {
                write_tx,
                pending,
                next_request_id: AtomicI64::new(1),
                transport_tasks,
                reader_task,
            },
            event_rx,
        )
    }

    pub(crate) async fn notify<P: Serialize>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<(), serde_json::Error> {
        let params = serde_json::to_value(params)?;
        self.write_tx
            .send(JSONRPCMessage::Notification(JSONRPCNotification {
                method: method.to_string(),
                params: Some(params),
            }))
            .await
            .map_err(|_| {
                serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "JSON-RPC transport closed",
                ))
            })
    }

    pub(crate) async fn call<P, T>(&self, method: &str, params: &P) -> Result<T, RpcCallError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let request_id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::SeqCst));
        let (response_tx, response_rx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(request_id.clone(), response_tx);

        let params = match serde_json::to_value(params) {
            Ok(params) => params,
            Err(err) => {
                self.pending.lock().await.remove(&request_id);
                return Err(RpcCallError::Json(err));
            }
        };
        if self
            .write_tx
            .send(JSONRPCMessage::Request(JSONRPCRequest {
                id: request_id.clone(),
                method: method.to_string(),
                params: Some(params),
                trace: None,
            }))
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&request_id);
            return Err(RpcCallError::Closed);
        }

        let result = response_rx.await.map_err(|_| RpcCallError::Closed)?;
        let response = match result {
            Ok(response) => response,
            Err(error) => return Err(RpcCallError::Server(error)),
        };
        serde_json::from_value(response).map_err(RpcCallError::Json)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) async fn pending_request_count(&self) -> usize {
        self.pending.lock().await.len()
    }
}

impl Drop for RpcClient {
    fn drop(&mut self) {
        for task in &self.transport_tasks {
            task.abort();
        }
        self.reader_task.abort();
    }
}

#[derive(Debug)]
pub(crate) enum RpcCallError {
    Closed,
    Json(serde_json::Error),
    Server(JSONRPCErrorError),
}

pub(crate) fn encode_server_message(
    message: RpcServerOutboundMessage,
) -> Result<JSONRPCMessage, serde_json::Error> {
    match message {
        RpcServerOutboundMessage::Response { request_id, result } => {
            Ok(JSONRPCMessage::Response(JSONRPCResponse {
                id: request_id,
                result,
            }))
        }
        RpcServerOutboundMessage::Error { request_id, error } => {
            Ok(JSONRPCMessage::Error(JSONRPCError {
                id: request_id,
                error,
            }))
        }
        RpcServerOutboundMessage::Notification(notification) => {
            Ok(JSONRPCMessage::Notification(notification))
        }
    }
}

pub(crate) fn invalid_request(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32600,
        data: None,
        message,
    }
}

pub(crate) fn method_not_found(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32601,
        data: None,
        message,
    }
}

pub(crate) fn invalid_params(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32602,
        data: None,
        message,
    }
}

pub(crate) fn not_found(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32004,
        data: None,
        message,
    }
}

pub(crate) fn internal_error(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32603,
        data: None,
        message,
    }
}

fn decode_request_params<P>(params: Option<Value>) -> Result<P, JSONRPCErrorError>
where
    P: DeserializeOwned,
{
    decode_params(params).map_err(|err| invalid_params(err.to_string()))
}

fn decode_notification_params<P>(params: Option<Value>) -> Result<P, String>
where
    P: DeserializeOwned,
{
    decode_params(params).map_err(|err| err.to_string())
}

fn decode_params<P>(params: Option<Value>) -> Result<P, serde_json::Error>
where
    P: DeserializeOwned,
{
    let params = params.unwrap_or(Value::Null);
    match serde_json::from_value(params.clone()) {
        Ok(params) => Ok(params),
        Err(err) => {
            if matches!(params, Value::Object(ref map) if map.is_empty()) {
                serde_json::from_value(Value::Null).map_err(|_| err)
            } else {
                Err(err)
            }
        }
    }
}

async fn handle_server_message(
    pending: &Mutex<HashMap<RequestId, PendingRequest>>,
    event_tx: &mpsc::Sender<RpcClientEvent>,
    message: JSONRPCMessage,
) -> Result<(), String> {
    match message {
        JSONRPCMessage::Response(JSONRPCResponse { id, result }) => {
            if let Some(pending) = pending.lock().await.remove(&id) {
                let _ = pending.send(Ok(result));
            }
        }
        JSONRPCMessage::Error(JSONRPCError { id, error }) => {
            if let Some(pending) = pending.lock().await.remove(&id) {
                let _ = pending.send(Err(error));
            }
        }
        JSONRPCMessage::Notification(notification) => {
            let _ = event_tx
                .send(RpcClientEvent::Notification(notification))
                .await;
        }
        JSONRPCMessage::Request(request) => {
            return Err(format!(
                "unexpected JSON-RPC request from remote server: {}",
                request.method
            ));
        }
    }

    Ok(())
}

async fn drain_pending(pending: &Mutex<HashMap<RequestId, PendingRequest>>) {
    let pending = {
        let mut pending = pending.lock().await;
        pending
            .drain()
            .map(|(_, pending)| pending)
            .collect::<Vec<_>>()
    };
    for pending in pending {
        let _ = pending.send(Err(JSONRPCErrorError {
            code: -32000,
            data: None,
            message: "JSON-RPC transport closed".to_string(),
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use codex_app_server_protocol::JSONRPCMessage;
    use codex_app_server_protocol::JSONRPCResponse;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::time::timeout;

    use super::RpcClient;
    use crate::connection::JsonRpcConnection;

    async fn read_jsonrpc_line<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> JSONRPCMessage
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let next_line = timeout(Duration::from_secs(1), lines.next_line()).await;
        let line_result = match next_line {
            Ok(line_result) => line_result,
            Err(err) => panic!("timed out waiting for JSON-RPC line: {err}"),
        };
        let maybe_line = match line_result {
            Ok(maybe_line) => maybe_line,
            Err(err) => panic!("failed to read JSON-RPC line: {err}"),
        };
        let line = match maybe_line {
            Some(line) => line,
            None => panic!("server connection closed before JSON-RPC line arrived"),
        };
        match serde_json::from_str::<JSONRPCMessage>(&line) {
            Ok(message) => message,
            Err(err) => panic!("failed to parse JSON-RPC line: {err}"),
        }
    }

    async fn write_jsonrpc_line<W>(writer: &mut W, message: JSONRPCMessage)
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let encoded = match serde_json::to_string(&message) {
            Ok(encoded) => encoded,
            Err(err) => panic!("failed to encode JSON-RPC message: {err}"),
        };
        if let Err(err) = writer.write_all(format!("{encoded}\n").as_bytes()).await {
            panic!("failed to write JSON-RPC line: {err}");
        }
    }

    #[tokio::test]
    async fn rpc_client_matches_out_of_order_responses_by_request_id() {
        let (client_stdin, server_reader) = tokio::io::duplex(4096);
        let (mut server_writer, client_stdout) = tokio::io::duplex(4096);
        let (client, _events_rx) = RpcClient::new(JsonRpcConnection::from_stdio(
            client_stdout,
            client_stdin,
            "test-rpc".to_string(),
        ));

        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();

            let first = read_jsonrpc_line(&mut lines).await;
            let second = read_jsonrpc_line(&mut lines).await;
            let (slow_request, fast_request) = match (first, second) {
                (
                    JSONRPCMessage::Request(first_request),
                    JSONRPCMessage::Request(second_request),
                ) if first_request.method == "slow" && second_request.method == "fast" => {
                    (first_request, second_request)
                }
                (
                    JSONRPCMessage::Request(first_request),
                    JSONRPCMessage::Request(second_request),
                ) if first_request.method == "fast" && second_request.method == "slow" => {
                    (second_request, first_request)
                }
                _ => panic!("expected slow and fast requests"),
            };

            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: fast_request.id,
                    result: serde_json::json!({ "value": "fast" }),
                }),
            )
            .await;
            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: slow_request.id,
                    result: serde_json::json!({ "value": "slow" }),
                }),
            )
            .await;
        });

        let slow_params = serde_json::json!({ "n": 1 });
        let fast_params = serde_json::json!({ "n": 2 });
        let (slow, fast) = tokio::join!(
            client.call::<_, serde_json::Value>("slow", &slow_params),
            client.call::<_, serde_json::Value>("fast", &fast_params),
        );

        let slow = slow.unwrap_or_else(|err| panic!("slow request failed: {err:?}"));
        let fast = fast.unwrap_or_else(|err| panic!("fast request failed: {err:?}"));
        assert_eq!(slow, serde_json::json!({ "value": "slow" }));
        assert_eq!(fast, serde_json::json!({ "value": "fast" }));

        assert_eq!(client.pending_request_count().await, 0);

        if let Err(err) = server.await {
            panic!("server task failed: {err}");
        }
    }
}
