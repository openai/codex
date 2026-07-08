use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;

pub(crate) const SESSION_ALREADY_ATTACHED_ERROR_CODE: i64 = -32010;
const MAX_IN_FLIGHT_REGULAR_CALLS: usize = 1024;
const RESERVED_CLEANUP_CALLS: usize = 1;

#[derive(Debug)]
pub(crate) enum RpcCallError {
    /// The underlying JSON-RPC transport closed before this call completed.
    Closed,
    /// The response bytes were valid JSON-RPC but not the expected result type.
    Json(serde_json::Error),
    /// The executor returned a JSON-RPC error response for this call.
    Server(JSONRPCErrorError),
    /// The executor did not return a response before the caller's deadline.
    TimedOut { method: String, timeout: Duration },
    /// The client already has the maximum number of regular RPC calls in flight.
    PendingRequestLimitExceeded { limit: usize },
}

type PendingRequest = oneshot::Sender<Result<Value, RpcCallError>>;
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type RequestRoute<S> = Box<
    dyn Fn(Arc<S>, JSONRPCRequest) -> BoxFuture<Option<RpcServerOutboundMessage>> + Send + Sync,
>;
type NotificationRoute<S> =
    Box<dyn Fn(Arc<S>, JSONRPCNotification) -> BoxFuture<Result<(), String>> + Send + Sync>;

enum RpcCallTimeout {
    None,
    After(Duration),
}

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
    Notification(JSONRPCNotification),
}

#[derive(Clone)]
pub(crate) struct RpcNotificationSender {
    outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>,
}

impl RpcNotificationSender {
    pub(crate) fn new(outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>) -> Self {
        Self { outgoing_tx }
    }

    pub(crate) async fn response(
        &self,
        request_id: RequestId,
        result: Value,
    ) -> Result<(), JSONRPCErrorError> {
        self.outgoing_tx
            .send(RpcServerOutboundMessage::Response { request_id, result })
            .await
            .map_err(|_| internal_error("RPC connection closed while sending response".into()))
    }

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
                            return Some(RpcServerOutboundMessage::Error { request_id, error });
                        }
                    };
                    Some(match response {
                        Ok(result) => match serde_json::to_value(result) {
                            Ok(result) => RpcServerOutboundMessage::Response { request_id, result },
                            Err(err) => RpcServerOutboundMessage::Error {
                                request_id,
                                error: internal_error(err.to_string()),
                            },
                        },
                        Err(error) => RpcServerOutboundMessage::Error { request_id, error },
                    })
                })
            }),
        );
    }

    pub(crate) fn request_with_id<P, F, Fut>(&mut self, method: &'static str, handler: F)
    where
        P: DeserializeOwned + Send + 'static,
        F: Fn(Arc<S>, RequestId, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), JSONRPCErrorError>> + Send + 'static,
    {
        self.request_routes.insert(
            method,
            Box::new(move |state, request| {
                let request_id = request.id;
                let params = decode_request_params::<P>(request.params)
                    .map(|params| handler(state, request_id.clone(), params));
                Box::pin(async move {
                    let response = match params {
                        Ok(response) => response.await,
                        Err(error) => {
                            return Some(RpcServerOutboundMessage::Error { request_id, error });
                        }
                    };
                    match response {
                        Ok(()) => None,
                        Err(error) => Some(RpcServerOutboundMessage::Error { request_id, error }),
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

    pub(crate) fn request_route(&self, method: &str) -> Option<(&'static str, &RequestRoute<S>)> {
        self.request_routes
            .get_key_value(method)
            .map(|(&method, route)| (method, route))
    }

    pub(crate) fn notification_route(&self, method: &str) -> Option<&NotificationRoute<S>> {
        self.notification_routes.get(method)
    }
}

pub(crate) struct RpcClient {
    write_tx: mpsc::Sender<JSONRPCMessage>,
    pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
    // Shared transport state from `JsonRpcConnection`. Calls use this to fail
    // immediately when the socket closes, even if no JSON-RPC error response
    // can be delivered for their request id.
    disconnected_rx: watch::Receiver<bool>,
    closed: Arc<AtomicBool>,
    shared_call_slots: Semaphore,
    cleanup_call_slots: Semaphore,
    next_request_id: AtomicI64,
    transport_tasks: Vec<JoinHandle<()>>,
    transport: JsonRpcTransport,
    reader_task: JoinHandle<()>,
}

impl RpcClient {
    pub(crate) fn new(connection: JsonRpcConnection) -> (Self, mpsc::Receiver<RpcClientEvent>) {
        let JsonRpcConnection {
            outgoing_tx: write_tx,
            mut incoming_rx,
            disconnected_rx,
            task_handles: transport_tasks,
            transport,
        } = connection;
        let pending = Arc::new(Mutex::new(HashMap::<RequestId, PendingRequest>::new()));
        let closed = Arc::new(AtomicBool::new(false));
        let (event_tx, event_rx) = mpsc::channel(128);

        let pending_for_reader = Arc::clone(&pending);
        let closed_for_reader = Arc::clone(&closed);
        let transport_for_reader = transport.clone();
        let reader_task = tokio::spawn(async move {
            let disconnect_reason = loop {
                let Some(event) = incoming_rx.recv().await else {
                    break None;
                };
                match event {
                    JsonRpcConnectionEvent::Message(message) => {
                        if let Err(err) =
                            handle_server_message(&pending_for_reader, &event_tx, message).await
                        {
                            let _ = err;
                            break None;
                        }
                    }
                    JsonRpcConnectionEvent::MalformedMessage { reason } => {
                        let _ = reason;
                        break None;
                    }
                    JsonRpcConnectionEvent::Disconnected { reason } => {
                        break reason;
                    }
                }
            };

            closed_for_reader.store(true, Ordering::Release);
            drain_pending(&pending_for_reader).await;
            let _ = event_tx
                .send(RpcClientEvent::Disconnected {
                    reason: disconnect_reason,
                })
                .await;
            transport_for_reader.terminate();
        });

        (
            Self {
                write_tx,
                pending,
                disconnected_rx,
                closed,
                shared_call_slots: Semaphore::new(MAX_IN_FLIGHT_REGULAR_CALLS),
                cleanup_call_slots: Semaphore::new(RESERVED_CLEANUP_CALLS),
                next_request_id: AtomicI64::new(1),
                transport_tasks,
                transport,
                reader_task,
            },
            event_rx,
        )
    }

    pub(crate) async fn notify<P: Serialize>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<(), RpcCallError> {
        let params = serde_json::to_value(params).map_err(RpcCallError::Json)?;
        if self.closed.load(Ordering::Acquire) || *self.disconnected_rx.borrow() {
            return Err(RpcCallError::Closed);
        }
        self.write_tx
            .send(JSONRPCMessage::Notification(JSONRPCNotification {
                method: method.to_string(),
                params: Some(params),
            }))
            .await
            .map_err(|_| RpcCallError::Closed)
    }

    pub(crate) fn is_disconnected(&self) -> bool {
        self.closed.load(Ordering::Acquire) || *self.disconnected_rx.borrow()
    }

    pub(crate) async fn close_transport(&self) {
        self.closed.store(true, Ordering::Release);
        self.transport.terminate();
        for task in &self.transport_tasks {
            task.abort();
        }
        drain_pending(&self.pending).await;
    }

    // Callers keep this permit until `call_inner` returns, so an executor
    // cannot free admission early by guessing a request id and replying before
    // the request leaves the outbound queue.
    fn acquire_regular_call_slot(&self) -> Result<SemaphorePermit<'_>, RpcCallError> {
        self.shared_call_slots.try_acquire().map_err(|_| {
            RpcCallError::PendingRequestLimitExceeded {
                limit: MAX_IN_FLIGHT_REGULAR_CALLS,
            }
        })
    }

    #[tracing::instrument(
        name = "codex.exec_server.request",
        level = "info",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = method,
            method,
        )
    )]
    pub(crate) async fn call<P, T>(&self, method: &str, params: &P) -> Result<T, RpcCallError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let _call_slot = self.acquire_regular_call_slot()?;
        self.call_inner(method, params, RpcCallTimeout::None).await
    }

    pub(crate) async fn call_with_timeout<P, T>(
        &self,
        method: &str,
        params: &P,
        call_timeout: Duration,
    ) -> Result<T, RpcCallError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let _call_slot = self.acquire_regular_call_slot()?;
        self.call_inner(method, params, RpcCallTimeout::After(call_timeout))
            .await
    }

    #[tracing::instrument(
        name = "codex.exec_server.request",
        level = "info",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = method,
            method,
        )
    )]
    pub(crate) async fn call_for_cleanup<P, T>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<T, RpcCallError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let _call_slot = match self.shared_call_slots.try_acquire() {
            Ok(call_slot) => call_slot,
            Err(_) => match self.cleanup_call_slots.try_acquire() {
                Ok(call_slot) => call_slot,
                Err(_) => {
                    self.close_transport().await;
                    return Err(RpcCallError::Closed);
                }
            },
        };
        self.call_inner(method, params, RpcCallTimeout::None).await
    }

    async fn call_inner<P, T>(
        &self,
        method: &str,
        params: &P,
        call_timeout: RpcCallTimeout,
    ) -> Result<T, RpcCallError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let request_id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::SeqCst));
        let (response_tx, response_rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            // Registering the pending request and checking disconnect must be
            // atomic with the reader's drain_pending path. Otherwise a call
            // can sneak in after the drain and wait forever.
            if self.closed.load(Ordering::Acquire) || *self.disconnected_rx.borrow() {
                return Err(RpcCallError::Closed);
            }
            pending.retain(|_, response_tx| !response_tx.is_closed());
            pending.insert(request_id.clone(), response_tx);
        }

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
                trace: codex_otel::current_span_w3c_trace_context(),
            }))
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&request_id);
            return Err(RpcCallError::Closed);
        }

        // Do not race in-flight requests directly against the transport-close
        // watch value. The connection reader receives JSON-RPC messages and
        // the terminal disconnect event on one ordered queue, then drains any
        // still-pending requests. Awaiting this receiver preserves that order:
        // responses already read before EOF still win, and truly pending calls
        // are failed once the reader observes the disconnect.
        let response = match call_timeout {
            RpcCallTimeout::None => response_rx.await,
            RpcCallTimeout::After(call_timeout) => match timeout(call_timeout, response_rx).await {
                Ok(response) => response,
                Err(_) => {
                    self.pending.lock().await.remove(&request_id);
                    return Err(RpcCallError::TimedOut {
                        method: method.to_string(),
                        timeout: call_timeout,
                    });
                }
            },
        };
        let result: Result<Value, RpcCallError> = response.map_err(|_| RpcCallError::Closed)?;
        let response = match result {
            Ok(response) => response,
            Err(error) => return Err(error),
        };
        serde_json::from_value(response).map_err(RpcCallError::Json)
    }

    #[cfg(test)]
    pub(crate) async fn pending_request_count(&self) -> usize {
        self.pending.lock().await.len()
    }
}

impl Drop for RpcClient {
    fn drop(&mut self) {
        self.transport.terminate();
        for task in &self.transport_tasks {
            task.abort();
        }
        self.reader_task.abort();
    }
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

pub(crate) fn session_already_attached(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: SESSION_ALREADY_ATTACHED_ERROR_CODE,
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
                let _ = pending.send(Err(RpcCallError::Server(error)));
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
        let _ = pending.send(Err(RpcCallError::Closed));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use codex_exec_server_protocol::JSONRPCMessage;
    use codex_exec_server_protocol::JSONRPCNotification;
    use codex_exec_server_protocol::JSONRPCResponse;
    use codex_exec_server_protocol::RequestId;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::task::JoinSet;
    use tokio::time::timeout;
    use tracing::Instrument;
    use tracing_subscriber::filter::filter_fn;
    use tracing_subscriber::prelude::*;

    use super::MAX_IN_FLIGHT_REGULAR_CALLS;
    use super::RpcCallError;
    use super::RpcClient;
    use crate::connection::JsonRpcConnection;
    use crate::connection::JsonRpcConnectionEvent;
    use crate::connection::JsonRpcTransport;

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
        let connection =
            JsonRpcConnection::from_stdio(client_stdout, client_stdin, "test-rpc".to_string());
        let (client, _events_rx) = RpcClient::new(connection);

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

    #[tokio::test(start_paused = true)]
    async fn rpc_client_call_has_no_implicit_deadline() {
        let (client_stdin, server_reader) = tokio::io::duplex(4096);
        let (mut server_writer, client_stdout) = tokio::io::duplex(4096);
        let connection =
            JsonRpcConnection::from_stdio(client_stdout, client_stdin, "test-rpc".to_string());
        let (client, _events_rx) = RpcClient::new(connection);
        let mut lines = BufReader::new(server_reader).lines();

        let params = serde_json::json!({});
        let call = client.call::<_, serde_json::Value>("slow", &params);
        tokio::pin!(call);
        assert!(futures::poll!(call.as_mut()).is_pending());
        let request = match read_jsonrpc_line(&mut lines).await {
            JSONRPCMessage::Request(request) => request,
            other => panic!("expected JSON-RPC request, got {other:?}"),
        };

        tokio::time::advance(Duration::from_secs(61)).await;
        assert!(futures::poll!(call.as_mut()).is_pending());

        let expected = serde_json::json!({ "value": "done" });
        write_jsonrpc_line(
            &mut server_writer,
            JSONRPCMessage::Response(JSONRPCResponse {
                id: request.id,
                result: expected.clone(),
            }),
        )
        .await;
        assert_eq!(call.await.expect("RPC response"), expected);
    }

    #[tokio::test]
    async fn rpc_client_timeout_removes_pending_request() {
        let (client_stdin, server_reader) = tokio::io::duplex(4096);
        let (server_writer, client_stdout) = tokio::io::duplex(4096);
        let (release_server_tx, release_server_rx) = tokio::sync::oneshot::channel();
        let connection =
            JsonRpcConnection::from_stdio(client_stdout, client_stdin, "test-rpc".to_string());
        let (client, _events_rx) = RpcClient::new(connection);

        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let request = read_jsonrpc_line(&mut lines).await;
            assert!(matches!(request, JSONRPCMessage::Request(_)));
            let _server_writer = server_writer;
            let _ = release_server_rx.await;
        });

        let call_timeout = Duration::from_millis(10);
        let result = client
            .call_with_timeout::<_, serde_json::Value>("slow", &serde_json::json!({}), call_timeout)
            .await;
        assert!(matches!(
            result,
            Err(super::RpcCallError::TimedOut { method, timeout })
                if method == "slow" && timeout == call_timeout
        ));
        assert_eq!(client.pending_request_count().await, 0);

        let _ = release_server_tx.send(());
        if let Err(err) = server.await {
            panic!("server task failed: {err}");
        }
    }

    #[tokio::test]
    async fn rpc_client_bounds_in_flight_calls_and_preserves_cleanup() {
        let (outgoing_tx, outgoing_rx) = tokio::sync::mpsc::channel(/*buffer*/ 1);
        outgoing_tx
            .send(JSONRPCMessage::Notification(JSONRPCNotification {
                method: "blocker".to_string(),
                params: None,
            }))
            .await
            .expect("outbound queue should accept the blocker");
        let (incoming_tx, incoming_rx) = tokio::sync::mpsc::channel(MAX_IN_FLIGHT_REGULAR_CALLS);
        let (_disconnected_tx, disconnected_rx) = tokio::sync::watch::channel(/*init*/ false);
        let connection = JsonRpcConnection {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: Vec::new(),
            transport: JsonRpcTransport::Plain,
        };
        let (client, _events_rx) = RpcClient::new(connection);
        let client = Arc::new(client);
        let mut calls = JoinSet::new();

        for index in 0..MAX_IN_FLIGHT_REGULAR_CALLS {
            let client = Arc::clone(&client);
            calls.spawn(async move {
                client
                    .call::<_, serde_json::Value>("pending", &serde_json::json!({ "index": index }))
                    .await
            });
        }
        timeout(Duration::from_secs(1), async {
            while client.pending_request_count().await < MAX_IN_FLIGHT_REGULAR_CALLS {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("pending requests should reach the regular limit");

        for request_id in 1..=MAX_IN_FLIGHT_REGULAR_CALLS {
            incoming_tx
                .send(JsonRpcConnectionEvent::Message(JSONRPCMessage::Response(
                    JSONRPCResponse {
                        id: RequestId::Integer(
                            i64::try_from(request_id).expect("request id should fit in i64"),
                        ),
                        result: serde_json::json!({}),
                    },
                )))
                .await
                .expect("reader should accept the spoofed response");
        }
        timeout(Duration::from_secs(1), async {
            while client.pending_request_count().await != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("spoofed responses should drain response routing");

        let params = serde_json::json!({});
        let overflow = client.call::<_, serde_json::Value>("overflow", &params);
        tokio::pin!(overflow);
        assert!(matches!(
            futures::poll!(overflow.as_mut()),
            std::task::Poll::Ready(Err(RpcCallError::PendingRequestLimitExceeded { limit }))
                if limit == MAX_IN_FLIGHT_REGULAR_CALLS
        ));

        let cleanup_client = Arc::clone(&client);
        calls.spawn(async move {
            let params = serde_json::json!({});
            cleanup_client
                .call_for_cleanup::<_, serde_json::Value>("cleanup", &params)
                .await
        });
        timeout(Duration::from_secs(1), async {
            while client.pending_request_count().await != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cleanup request should use the reserved capacity");

        let cleanup_params = serde_json::json!({});
        let cleanup_overflow = timeout(
            Duration::from_secs(1),
            client.call_for_cleanup::<_, serde_json::Value>("cleanup-overflow", &cleanup_params),
        )
        .await
        .expect("cleanup circuit breaker should not block");
        assert!(matches!(cleanup_overflow, Err(RpcCallError::Closed)));
        assert!(client.is_disconnected());
        assert_eq!(client.pending_request_count().await, 0);

        drop(outgoing_rx);
        while let Some(call) = calls.join_next().await {
            assert!(matches!(
                call.expect("pending call task should join"),
                Err(RpcCallError::Closed)
            ));
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rpc_client_propagates_current_trace_context() {
        let span_exporter = InMemorySpanExporter::default();
        let tracer_provider = SdkTracerProvider::builder()
            .with_simple_exporter(span_exporter)
            .build();
        let tracer = tracer_provider.tracer("exec-server-test");
        let subscriber = tracing_subscriber::registry().with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(filter_fn(codex_otel::OtelProvider::trace_export_filter)),
        );
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);
        tracing::callsite::rebuild_interest_cache();
        let parent_span = tracing::info_span!("outbound-parent");
        let expected_trace = codex_otel::span_w3c_trace_context(&parent_span)
            .expect("parent span should have trace context");

        let (client_stdin, server_reader) = tokio::io::duplex(4096);
        let (mut server_writer, client_stdout) = tokio::io::duplex(4096);
        let connection =
            JsonRpcConnection::from_stdio(client_stdout, client_stdin, "test-rpc".to_string());
        let (client, _events_rx) = RpcClient::new(connection);

        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let request = match read_jsonrpc_line(&mut lines).await {
                JSONRPCMessage::Request(request) => request,
                other => panic!("expected JSON-RPC request, got {other:?}"),
            };
            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id.clone(),
                    result: serde_json::json!({}),
                }),
            )
            .await;
            request.trace
        });

        let response = client
            .call::<_, serde_json::Value>("traced", &serde_json::json!({}))
            .instrument(parent_span)
            .await
            .expect("RPC response");
        assert_eq!(response, serde_json::json!({}));
        let trace = server.await.expect("server task").expect("trace context");
        let expected_traceparent = expected_trace
            .traceparent
            .as_deref()
            .expect("parent traceparent");
        let traceparent = trace.traceparent.as_deref().expect("request traceparent");
        let expected_parts = expected_traceparent.split('-').collect::<Vec<_>>();
        let parts = traceparent.split('-').collect::<Vec<_>>();
        assert_eq!(parts[1], expected_parts[1]);
        assert_ne!(parts[2], expected_parts[2]);
        assert_eq!(trace.tracestate, expected_trace.tracestate);
    }
}
