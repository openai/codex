use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use codex_protocol::protocol::W3cTraceContext;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::Span;
use tracing::field;

use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;

pub(crate) const SESSION_ALREADY_ATTACHED_ERROR_CODE: i64 = -32010;

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
}

struct PendingRequest {
    response_tx: oneshot::Sender<Result<Value, RpcCallError>>,
    method: String,
    trace: Option<W3cTraceContext>,
}
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

fn rpc_call_span(method: &str) -> Span {
    tracing::info_span!(
        "exec_server.rpc.call",
        method,
        request_id = field::Empty,
        pending_registered_ms = field::Empty,
        request_serialized_ms = field::Empty,
        request_enqueued_ms = field::Empty,
        response_received_ms = field::Empty,
        response_decoded_ms = field::Empty,
    )
}

#[derive(Debug)]
pub(crate) enum RpcClientEvent {
    Notification {
        notification: JSONRPCNotification,
        queued_at: Instant,
        trace: Option<W3cTraceContext>,
    },
    Disconnected {
        reason: Option<String>,
    },
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

/// Carries executor-local timing and trace state across the server outbound queue.
///
/// The metadata is deliberately not serialized onto the JSON-RPC wire. Noise
/// transport correlation uses the originating request ID or process ID instead.
pub(crate) struct RpcServerOutboundEnvelope {
    pub(crate) message: RpcServerOutboundMessage,
    pub(crate) queued_at: Instant,
    pub(crate) trace: Option<W3cTraceContext>,
}

impl RpcServerOutboundEnvelope {
    pub(crate) fn from_current_span(message: RpcServerOutboundMessage) -> Self {
        Self::with_trace(message, codex_otel::current_span_w3c_trace_context())
    }

    pub(crate) fn with_trace(
        message: RpcServerOutboundMessage,
        trace: Option<W3cTraceContext>,
    ) -> Self {
        Self {
            message,
            queued_at: Instant::now(),
            trace,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RpcNotificationSender {
    outgoing_tx: mpsc::Sender<RpcServerOutboundEnvelope>,
}

impl RpcNotificationSender {
    pub(crate) fn new(outgoing_tx: mpsc::Sender<RpcServerOutboundEnvelope>) -> Self {
        Self { outgoing_tx }
    }

    pub(crate) async fn response(
        &self,
        request_id: RequestId,
        result: Value,
    ) -> Result<(), JSONRPCErrorError> {
        let permit = self
            .outgoing_tx
            .reserve()
            .instrument(tracing::info_span!(
                "exec_server.rpc_server.outbound_queue_enqueue",
                message_kind = "response",
            ))
            .await
            .map_err(|_| internal_error("RPC connection closed while sending response".into()))?;
        permit.send(RpcServerOutboundEnvelope::from_current_span(
            RpcServerOutboundMessage::Response { request_id, result },
        ));
        Ok(())
    }

    pub(crate) async fn notify<P: Serialize>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<(), JSONRPCErrorError> {
        let params = tracing::info_span!("exec_server.rpc_server.notification_serialize", method,)
            .in_scope(|| serde_json::to_value(params))
            .map_err(|err| internal_error(err.to_string()))?;
        let permit = self
            .outgoing_tx
            .reserve()
            .instrument(tracing::info_span!(
                "exec_server.rpc_server.outbound_queue_enqueue",
                message_kind = "notification",
                method,
            ))
            .await
            .map_err(|_| {
                internal_error("RPC connection closed while sending notification".into())
            })?;
        permit.send(RpcServerOutboundEnvelope::from_current_span(
            RpcServerOutboundMessage::Notification(JSONRPCNotification {
                method: method.to_string(),
                params: Some(params),
            }),
        ));
        Ok(())
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
                        let reader_span = rpc_reader_message_span(
                            &message, /*trace*/ None, /*queue_wait*/ None,
                        );
                        if let Err(err) = handle_server_message(
                            &pending_for_reader,
                            &event_tx,
                            message,
                            /*trace*/ None,
                        )
                        .instrument(reader_span)
                        .await
                        {
                            let _ = err;
                            break None;
                        }
                    }
                    JsonRpcConnectionEvent::TracedMessage {
                        message,
                        trace,
                        queued_at,
                    } => {
                        let reader_span = rpc_reader_message_span(
                            &message,
                            trace.as_ref(),
                            Some(queued_at.elapsed()),
                        );
                        if let Err(err) =
                            handle_server_message(&pending_for_reader, &event_tx, message, trace)
                                .instrument(reader_span)
                                .await
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

    pub(crate) async fn call<P, T>(&self, method: &str, params: &P) -> Result<T, RpcCallError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        self.call_inner(method, params, RpcCallTimeout::None)
            .instrument(rpc_call_span(method))
            .await
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
        self.call_inner(method, params, RpcCallTimeout::After(call_timeout))
            .instrument(rpc_call_span(method))
            .await
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
        let call_started_at = Instant::now();
        let request_number = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        Span::current().record("request_id", request_number);
        let request_id = RequestId::Integer(request_number);
        let (response_tx, response_rx) = oneshot::channel();
        let trace = codex_otel::current_span_w3c_trace_context();
        {
            let mut pending = self
                .pending
                .lock()
                .instrument(tracing::info_span!("exec_server.rpc.pending_registration"))
                .await;
            // Registering the pending request and checking disconnect must be
            // atomic with the reader's drain_pending path. Otherwise a call
            // can sneak in after the drain and wait forever.
            if self.closed.load(Ordering::Acquire) || *self.disconnected_rx.borrow() {
                return Err(RpcCallError::Closed);
            }
            pending.insert(
                request_id.clone(),
                PendingRequest {
                    response_tx,
                    method: method.to_string(),
                    trace: trace.clone(),
                },
            );
        }
        Span::current().record(
            "pending_registered_ms",
            call_started_at.elapsed().as_secs_f64() * 1_000.0,
        );

        let params = match tracing::info_span!("exec_server.rpc.serialize_params")
            .in_scope(|| serde_json::to_value(params))
        {
            Ok(params) => params,
            Err(err) => {
                self.pending.lock().await.remove(&request_id);
                return Err(RpcCallError::Json(err));
            }
        };
        Span::current().record(
            "request_serialized_ms",
            call_started_at.elapsed().as_secs_f64() * 1_000.0,
        );
        let message = JSONRPCMessage::Request(JSONRPCRequest {
            id: request_id.clone(),
            method: method.to_string(),
            params: Some(params),
            trace,
        });
        if self
            .write_tx
            .send(message)
            .instrument(tracing::info_span!("exec_server.rpc.enqueue_request"))
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&request_id);
            return Err(RpcCallError::Closed);
        }
        Span::current().record(
            "request_enqueued_ms",
            call_started_at.elapsed().as_secs_f64() * 1_000.0,
        );

        // Do not race in-flight requests directly against the transport-close
        // watch value. The connection reader receives JSON-RPC messages and
        // the terminal disconnect event on one ordered queue, then drains any
        // still-pending requests. Awaiting this receiver preserves that order:
        // responses already read before EOF still win, and truly pending calls
        // are failed once the reader observes the disconnect.
        let response_wait =
            response_rx.instrument(tracing::info_span!("exec_server.rpc.await_response"));
        let response = match call_timeout {
            RpcCallTimeout::None => response_wait.await,
            RpcCallTimeout::After(call_timeout) => match timeout(call_timeout, response_wait).await
            {
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
        Span::current().record(
            "response_received_ms",
            call_started_at.elapsed().as_secs_f64() * 1_000.0,
        );
        let result: Result<Value, RpcCallError> = response.map_err(|_| RpcCallError::Closed)?;
        let response = match result {
            Ok(response) => response,
            Err(error) => return Err(error),
        };
        let decoded = tracing::info_span!("exec_server.rpc.deserialize_response")
            .in_scope(|| serde_json::from_value(response))
            .map_err(RpcCallError::Json);
        Span::current().record(
            "response_decoded_ms",
            call_started_at.elapsed().as_secs_f64() * 1_000.0,
        );
        decoded
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
    trace: Option<W3cTraceContext>,
) -> Result<(), String> {
    match message {
        JSONRPCMessage::Response(JSONRPCResponse { id, result }) => {
            dispatch_response(pending, id, Ok(result), "response", trace).await;
        }
        JSONRPCMessage::Error(JSONRPCError { id, error }) => {
            dispatch_response(
                pending,
                id,
                Err(RpcCallError::Server(error)),
                "error",
                trace,
            )
            .await;
        }
        JSONRPCMessage::Notification(notification) => {
            let span = notification_dispatch_span(&notification);
            if let Ok(permit) = event_tx.reserve().instrument(span).await {
                permit.send(RpcClientEvent::Notification {
                    notification,
                    queued_at: Instant::now(),
                    trace,
                });
            }
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

async fn dispatch_response(
    pending: &Mutex<HashMap<RequestId, PendingRequest>>,
    id: RequestId,
    result: Result<Value, RpcCallError>,
    message_kind: &'static str,
    received_trace: Option<W3cTraceContext>,
) {
    let lookup_started_at = Instant::now();
    let pending = pending
        .lock()
        .instrument(tracing::info_span!(
            "exec_server.rpc.response_pending_lookup",
            request_id = %id,
        ))
        .await
        .remove(&id);
    let Some(pending) = pending else {
        return;
    };
    let span = tracing::info_span!(
        "exec_server.rpc.response_dispatch",
        message_kind,
        method = pending.method,
        request_id = %id,
        pending_lookup_ms = lookup_started_at.elapsed().as_secs_f64() * 1_000.0,
    );
    if let Some(trace) = received_trace.as_ref().or(pending.trace.as_ref()) {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span.in_scope(|| {
        let _ = pending.response_tx.send(result);
    });
}

fn rpc_reader_message_span(
    message: &JSONRPCMessage,
    trace: Option<&W3cTraceContext>,
    queue_wait: Option<std::time::Duration>,
) -> Span {
    let (message_kind, method, request_id, process_id, seq) = message_trace_fields(message);
    let span = tracing::info_span!(
        "exec_server.rpc.reader_message_dequeued",
        message_kind,
        method,
        request_id,
        process_id,
        seq,
        queue_wait_ms = queue_wait.map(|wait| wait.as_secs_f64() * 1_000.0),
    );
    if let Some(trace) = trace {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span
}

fn notification_dispatch_span(notification: &JSONRPCNotification) -> Span {
    let process_id = notification
        .params
        .as_ref()
        .and_then(|params| params.get("processId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let seq = notification
        .params
        .as_ref()
        .and_then(|params| params.get("seq"))
        .and_then(Value::as_u64);
    tracing::info_span!(
        "exec_server.rpc.notification_event_enqueue",
        method = notification.method,
        process_id,
        seq,
    )
}

fn message_trace_fields(message: &JSONRPCMessage) -> (&str, &str, String, &str, Option<u64>) {
    let (message_kind, method, request_id, payload) = match message {
        JSONRPCMessage::Request(request) => (
            "request",
            request.method.as_str(),
            request.id.to_string(),
            request.params.as_ref(),
        ),
        JSONRPCMessage::Notification(notification) => (
            "notification",
            notification.method.as_str(),
            String::new(),
            notification.params.as_ref(),
        ),
        JSONRPCMessage::Response(response) => (
            "response",
            "",
            response.id.to_string(),
            Some(&response.result),
        ),
        JSONRPCMessage::Error(error) => {
            ("error", "", error.id.to_string(), error.error.data.as_ref())
        }
    };
    let process_id = payload
        .and_then(|payload| payload.get("processId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let seq = payload
        .and_then(|payload| payload.get("seq"))
        .and_then(Value::as_u64);
    (message_kind, method, request_id, process_id, seq)
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
        let _ = pending.response_tx.send(Err(RpcCallError::Closed));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use codex_exec_server_protocol::JSONRPCMessage;
    use codex_exec_server_protocol::JSONRPCResponse;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::time::timeout;
    use tracing::Instrument;
    use tracing_subscriber::filter::filter_fn;
    use tracing_subscriber::prelude::*;

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
        let traceparent = trace.traceparent.expect("request traceparent");
        let expected_traceparent = expected_trace.traceparent.expect("parent span traceparent");
        let trace_id = traceparent.split('-').nth(1).expect("request trace id");
        let expected_trace_id = expected_traceparent
            .split('-')
            .nth(1)
            .expect("parent span trace id");
        assert_eq!(trace_id, expected_trace_id);
        assert_ne!(traceparent, expected_traceparent);
    }
}
