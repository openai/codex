use crate::RESPONSES_PATH;
use crate::auth::SharedAuthProvider;
use crate::common::ResponseEvent;
use crate::common::ResponseInterrupt;
use crate::common::ResponseStream;
use crate::common::ResponsesWsRequest;
use crate::common::SafetyBufferingTreatment;
use crate::common::WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY;
use crate::common::with_tool_results;
use crate::endpoint::responses_websocket_probe::ResponsesWebsocketClose;
use crate::endpoint::responses_websocket_probe::ResponsesWebsocketProbe;
use crate::endpoint::responses_websocket_probe::ResponsesWebsocketProbeError;
use crate::endpoint::responses_websocket_probe::ResponsesWebsocketProbeFailureStage;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::rate_limits::parse_rate_limit_event;
use crate::responses_headers::json_headers_to_http_headers;
use crate::safety_buffering::treatment_from_headers;
use crate::sse::ResponsesStreamEvent;
use crate::sse::process_responses_event;
use crate::telemetry::WebsocketTelemetry;
use bytes::Bytes;
use codex_client::TransportError;
use codex_http_client::HttpClientFactory;
use codex_http_client::RetryAfter;
use codex_websocket_client::WebSocketConnection;
use codex_websocket_client::WebSocketConnector;
use futures::FutureExt;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use http::HeaderMap;
use http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use serde_json::map::Map as JsonMap;
use std::future::Future;
use std::future::poll_fn;
use std::sync::Arc;
use std::sync::OnceLock;
use std::task::Context;
use std::task::Poll;
use std::task::ready;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::Utf8Bytes;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::Fragment;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::frame::coding::Data;
use tracing::Instrument;
use tracing::Span;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;
use tracing::instrument::WithSubscriber;
use tungstenite::extensions::ExtensionsConfig;
use tungstenite::extensions::compression::deflate::DeflateConfig;
use tungstenite::protocol::WebSocketConfig;
use url::Url;

struct WsStream {
    max_message_bytes: Option<usize>,
    routing_affinity: Option<codex_http_client::ChatGptRoutingAffinity>,
    handshake_models_etag: Option<String>,
    tx_command: mpsc::Sender<WsCommand>,
    rx_message: mpsc::UnboundedReceiver<Result<Message, WsError>>,
    pump_task: tokio::task::JoinHandle<()>,
}

struct WsRequestText {
    text: Utf8Bytes,
    private: bool,
}

struct WsCommand {
    deadline: Instant,
    request: WsRequestText,
    tx_result: oneshot::Sender<WsSendOutcome>,
}

enum WsSendOutcome {
    Flushed,
    NotStarted(ApiError),
    Interrupted(ApiError),
}

impl WsSendOutcome {
    async fn receive(result: &mut oneshot::Receiver<Self>) -> Result<Self, ApiError> {
        let outcome = result
            .await
            .unwrap_or_else(|_| Self::NotStarted(map_ws_stream_error(WsError::ConnectionClosed)));
        match outcome {
            Self::NotStarted(error) | Self::Interrupted(error)
                if matches!(error, ApiError::Transport(TransportError::Policy(_))) =>
            {
                Err(error)
            }
            outcome => Ok(outcome),
        }
    }

    fn into_result(self) -> Result<(), ApiError> {
        match self {
            Self::Flushed => Ok(()),
            Self::NotStarted(error) | Self::Interrupted(error) => Err(error),
        }
    }
}

const FRAGMENT_SIZE: usize = 1024 * 1024;
const RECEIVE_BATCH_SIZE: usize = 32;
const UPLOAD_READ_AHEAD_BYTES: usize = 1024 * 1024;

fn request_frames(text: Utf8Bytes) -> impl Iterator<Item = Message> {
    let (message, mut data) = if text.len() <= FRAGMENT_SIZE {
        (Some(Message::Text(text)), Bytes::new())
    } else {
        (None, Bytes::from(text))
    };
    let mut opcode = Data::Text;
    message.into_iter().chain(std::iter::from_fn(move || {
        if data.is_empty() {
            return None;
        }
        let payload = data.split_to(data.len().min(FRAGMENT_SIZE));
        let fragment = if data.is_empty() {
            Fragment::Final
        } else {
            Fragment::More
        };
        let frame = Frame::message_fragment(payload, opcode, fragment);
        opcode = Data::Continue;
        Some(Message::Frame(frame))
    }))
}

/// Drives the socket while waiting for commands and while flushing their frames.
struct WsPump<S> {
    socket: S,
    messages: mpsc::UnboundedSender<Result<Message, WsError>>,
    read_budget: Option<usize>,
    queued_pongs: usize,
}

impl<S> WsPump<S>
where
    S: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
{
    fn new(socket: S, messages: mpsc::UnboundedSender<Result<Message, WsError>>) -> Self {
        Self {
            socket,
            messages,
            read_budget: None,
            queued_pongs: 0,
        }
    }

    fn poll_incoming(
        &mut self,
        cx: &mut Context<'_>,
        recovering: bool,
    ) -> Poll<Result<(), WsError>> {
        if self.queued_pongs != 0 && !recovering && self.socket.poll_flush_unpin(cx)?.is_ready() {
            self.queued_pongs = 0;
        }
        for _ in 0..RECEIVE_BATCH_SIZE {
            if self.read_budget == Some(0) {
                return Poll::Ready(Ok(()));
            }
            if !recovering && self.queued_pongs >= RECEIVE_BATCH_SIZE {
                return Poll::Pending;
            }
            let next = self.socket.poll_next_unpin(cx);
            if recovering && next.is_pending() {
                return Poll::Ready(Ok(()));
            }
            let Some(message) = ready!(next).transpose()? else {
                return Poll::Ready(Err(WsError::ConnectionClosed));
            };
            if let Some(budget) = &mut self.read_budget
                && (recovering || !matches!(message, Message::Ping(_) | Message::Pong(_)))
            {
                *budget = budget.saturating_sub(message.len() + std::mem::size_of::<Message>());
            }
            match message {
                Message::Ping(_) => {
                    // Queue the automatic Pong before starting the next data frame.
                    match self.socket.poll_flush_unpin(cx) {
                        Poll::Ready(Ok(())) => self.queued_pongs = 0,
                        Poll::Ready(Err(error))
                            if !recovering
                                || codex_websocket_client::network_policy_denial(&error)
                                    .is_some() =>
                        {
                            return Poll::Ready(Err(error));
                        }
                        Poll::Pending | Poll::Ready(Err(_)) => self.queued_pongs += 1,
                    }
                }
                Message::Pong(_) => {}
                Message::Text(_) | Message::Binary(_) | Message::Close(_) | Message::Frame(_) => {
                    let closed = matches!(message, Message::Close(_));
                    if self.messages.send(Ok(message)).is_err() || closed {
                        return Poll::Ready(Err(WsError::ConnectionClosed));
                    }
                }
            }
        }
        // The last read was ready, so arrange another poll after the batch.
        cx.waker().wake_by_ref();
        Poll::Pending
    }

    async fn drive<T>(
        &mut self,
        mut operation: impl FnMut(&mut S, &mut Context<'_>) -> Poll<Result<T, WsError>>,
    ) -> Result<T, WsError> {
        let mut first = self.read_budget.is_some();
        poll_fn(|cx| {
            // Read before starting a frame. Once blocked, give the write the first
            // use of the task's cooperative budget on each wake.
            let read_first = std::mem::take(&mut first);
            if read_first {
                let _ = self.poll_incoming(cx, /*recovering*/ false)?;
            }
            let result = operation(&mut self.socket, cx);
            if !read_first && result.is_pending() {
                let _ = self.poll_incoming(cx, /*recovering*/ false)?;
            }
            result
        })
        .await
    }

    async fn recover(&mut self, error: WsError, deadline: Instant) -> WsError {
        if matches!(error, WsError::ConnectionClosed)
            || codex_websocket_client::network_policy_denial(&error).is_some()
        {
            return error;
        }
        // Drain immediately readable replies until the socket would block.
        let replies = poll_fn(|cx| self.poll_incoming(cx, /*recovering*/ true));
        if let Ok(Err(read_error)) = tokio::time::timeout_at(deadline, replies).await
            && codex_websocket_client::network_policy_denial(&read_error).is_some()
        {
            return read_error;
        }
        error
    }

    async fn send_frames(&mut self, text: Utf8Bytes, deadline: Instant) -> WsSendOutcome {
        // Limit how far we read ahead of the response consumer during a send.
        self.read_budget = Some(UPLOAD_READ_AHEAD_BYTES);
        let mut started = false;
        for frame in request_frames(text) {
            let mut frame = Some(frame);
            let write = self.drive(|socket, cx| {
                if frame.is_some() {
                    ready!(socket.poll_ready_unpin(cx))?;
                }
                if let Some(frame) = frame.take() {
                    // Even an I/O failure can follow a partially written frame header.
                    let result = socket.start_send_unpin(frame);
                    started |= matches!(result, Ok(()) | Err(WsError::Io(_)));
                    result?;
                }
                socket.poll_flush_unpin(cx)
            });
            let error = match tokio::time::timeout_at(deadline, write).await {
                Ok(Ok(())) => continue,
                Ok(Err(error)) if started => {
                    map_ws_stream_error(self.recover(error, deadline).await)
                }
                Ok(Err(error)) => map_ws_stream_error(error),
                Err(_) => ApiError::Stream("idle timeout sending websocket request".into()),
            };
            return if started {
                WsSendOutcome::Interrupted(error)
            } else {
                WsSendOutcome::NotStarted(error)
            };
        }
        self.read_budget = None;
        WsSendOutcome::Flushed
    }

    async fn run(mut self, mut commands: mpsc::Receiver<WsCommand>) {
        loop {
            let command = self.drive(|_, cx| commands.poll_recv(cx).map(Ok)).await;
            let WsCommand {
                request,
                deadline,
                tx_result,
            } = match command {
                Ok(Some(command)) => command,
                Ok(None) | Err(WsError::ConnectionClosed) => break,
                Err(error) => {
                    let _ = self.messages.send(Err(error));
                    break;
                }
            };
            let send = self.send_frames(request.text, deadline);
            // Tungstenite traces frame payloads from inside this future.
            let outcome = if request.private {
                send.with_subscriber(tracing::Dispatch::none()).await
            } else {
                send.await
            };
            let flushed = matches!(outcome, WsSendOutcome::Flushed);
            let _ = tx_result.send(outcome);
            if !flushed {
                break;
            }
        }
    }
}

impl WsStream {
    fn new(
        inner: WebSocketConnection,
        max_message_bytes: Option<usize>,
        handshake_models_etag: Option<String>,
    ) -> Self {
        let routing_affinity = inner.routing_affinity().cloned();
        let mut stream = Self::with_socket(inner, max_message_bytes, routing_affinity);
        stream.handshake_models_etag = handshake_models_etag;
        stream
    }

    fn with_socket(
        socket: impl Stream<Item = Result<Message, WsError>>
        + Sink<Message, Error = WsError>
        + Send
        + Unpin
        + 'static,
        max_message_bytes: Option<usize>,
        routing_affinity: Option<codex_http_client::ChatGptRoutingAffinity>,
    ) -> Self {
        let (tx_command, rx_command) = mpsc::channel::<WsCommand>(32);
        let (tx_message, rx_message) = mpsc::unbounded_channel();

        let pump_task = tokio::spawn(WsPump::new(socket, tx_message).run(rx_command));

        Self {
            max_message_bytes,
            routing_affinity,
            handshake_models_etag: None,
            tx_command,
            rx_message,
            pump_task,
        }
    }

    /// Queue a write with a deadline.
    async fn queue_request(
        &mut self,
        request: WsRequestText,
        idle_timeout: Duration,
    ) -> Result<oneshot::Receiver<WsSendOutcome>, ApiError> {
        if self
            .max_message_bytes
            .is_some_and(|limit| request.text.len() > limit)
        {
            return Err(ApiError::WebsocketMessageTooLarge);
        }
        let deadline = tokio::time::sleep(idle_timeout).deadline();
        let (tx_result, rx_result) = oneshot::channel();
        let _ = self
            .tx_command
            .send(WsCommand {
                request,
                deadline,
                tx_result,
            })
            .await;
        Ok(rx_result)
    }

    async fn send_create(
        &mut self,
        request: WsRequestText,
        idle_timeout: Duration,
        telemetry: Option<&dyn WebsocketTelemetry>,
        connection_reused: bool,
    ) -> Result<WsSendOutcome, ApiError> {
        let start = Instant::now();
        let mut send = self.queue_request(request, idle_timeout).await?;
        let result = match WsSendOutcome::receive(&mut send).await {
            Ok(WsSendOutcome::NotStarted(mut error)) => {
                // Replies preceding the create are stale. Recover any idle transport error.
                while let Some(message) = self.next().await {
                    if let Err(idle_error) = message {
                        error = map_ws_stream_error(idle_error);
                        break;
                    }
                }
                Err(error)
            }
            outcome => outcome,
        };
        if let Some(t) = telemetry {
            let send_error = match &result {
                Ok(WsSendOutcome::Flushed) => None,
                Ok(WsSendOutcome::NotStarted(error) | WsSendOutcome::Interrupted(error))
                | Err(error) => Some(error),
            };
            t.on_ws_request(start.elapsed(), send_error, connection_reused);
        }
        result
    }

    async fn next(&mut self) -> Option<Result<Message, WsError>> {
        self.rx_message.recv().await
    }
}

impl Drop for WsStream {
    fn drop(&mut self) {
        self.pump_task.abort();
    }
}

const X_CODEX_TURN_STATE_HEADER: &str = "x-codex-turn-state";
const X_CODEX_WEBSOCKET_MAX_MESSAGE_BYTES_HEADER: &str = "x-codex-websocket-max-message-bytes";
const X_MODELS_ETAG_HEADER: &str = "x-models-etag";
const X_REASONING_INCLUDED_HEADER: &str = "x-reasoning-included";
const OPENAI_MODEL_HEADER: &str = "openai-model";
const WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE: &str = "websocket_connection_limit_reached";
const WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE: &str = "Responses websocket connection limit reached (60 minutes). Create a new websocket connection to continue.";
const PREVIOUS_RESPONSE_NOT_FOUND_CODE: &str = "previous_response_not_found";
const PREVIOUS_RESPONSE_NOT_FOUND_MESSAGE: &str =
    "Previous response was not found. Retrying the full request.";
const RESPONSES_WEBSOCKET_TIMING_KIND: &str = "responsesapi.websocket_timing";
const RESPONSES_WEBSOCKET_TIMING_EVENT_TARGET: &str = "codex_api::responses_websocket_timing";
const SESSION_ID_CLIENT_METADATA_KEY: &str = "session_id";
const THREAD_ID_CLIENT_METADATA_KEY: &str = "thread_id";
const TURN_ID_CLIENT_METADATA_KEY: &str = "turn_id";
const WS_STREAM_REQUEST_START_MS_CLIENT_METADATA_KEY: &str = "x-codex-ws-stream-request-start-ms";

#[cfg_attr(test, derive(Default))]
struct ResponsesWebsocketTimingLogContext {
    model: String,
    session_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    traceparent: Option<String>,
    previous_response_id: Option<String>,
    request_start_ms: Option<String>,
    warmup: bool,
    connection_reused: bool,
}

pub struct ResponsesWebsocketConnection {
    routing_affinity: Option<codex_http_client::ChatGptRoutingAffinity>,
    stream: Arc<Mutex<Option<WsStream>>>,
    // TODO (pakrym): is this the right place for timeout?
    idle_timeout: Duration,
    server_reasoning_included: bool,
    server_model: Option<String>,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
}

impl std::fmt::Debug for ResponsesWebsocketConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponsesWebsocketConnection")
            .field("stream", &"<ws-stream>")
            .field("idle_timeout", &self.idle_timeout)
            .field("server_reasoning_included", &self.server_reasoning_included)
            .field("server_model", &self.server_model)
            .field("telemetry", &self.telemetry.as_ref().map(|_| "<telemetry>"))
            .finish()
    }
}

impl ResponsesWebsocketConnection {
    fn new(
        mut stream: WsStream,
        idle_timeout: Duration,
        server_reasoning_included: bool,
        server_model: Option<String>,
        telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    ) -> Self {
        let routing_affinity = stream.routing_affinity.take();
        Self {
            stream: Arc::new(Mutex::new(Some(stream))),
            idle_timeout,
            server_reasoning_included,
            server_model,
            telemetry,
            routing_affinity,
        }
    }

    pub async fn is_closed(&self) -> bool {
        self.stream
            .lock()
            .await
            .as_ref()
            .is_none_or(|stream| stream.pump_task.is_finished())
    }

    pub async fn stream_request(
        &self,
        request: ResponsesWsRequest<'_>,
        connection_reused: bool,
        turn_state: Option<Arc<OnceLock<String>>>,
    ) -> Result<ResponseStream, ApiError> {
        self.stream_request_with_observer(request, connection_reused, turn_state, |_| {})
            .await
    }

    /// Gives the observer the serialized request before private tool results are attached.
    #[instrument(
        name = "responses_websocket.stream_request",
        level = "info",
        skip_all,
        fields(transport = "responses_websocket", api.path = "/responses")
    )]
    pub async fn stream_request_with_observer(
        &self,
        request: ResponsesWsRequest<'_>,
        connection_reused: bool,
        turn_state: Option<Arc<OnceLock<String>>>,
        on_encoded: impl FnOnce(&bytes::Bytes),
    ) -> Result<ResponseStream, ApiError> {
        let (tx_event, rx_event) =
            mpsc::channel::<std::result::Result<ResponseEvent, ApiError>>(1600);
        let (tx_interrupt, rx_interrupt) = oneshot::channel();
        let stream = Arc::clone(&self.stream);
        let idle_timeout = self.idle_timeout;
        let server_reasoning_included = self.server_reasoning_included;
        let server_model = self.server_model.clone();
        let telemetry = self.telemetry.clone();
        let ResponsesWsRequest::ResponseCreate(ws_request) = &request;
        let client_metadata = ws_request.client_metadata.as_ref();
        let timing_log_context = ResponsesWebsocketTimingLogContext {
            model: ws_request.model.to_string(),
            session_id: client_metadata
                .and_then(|metadata| metadata.get(SESSION_ID_CLIENT_METADATA_KEY))
                .cloned(),
            thread_id: client_metadata
                .and_then(|metadata| metadata.get(THREAD_ID_CLIENT_METADATA_KEY))
                .cloned(),
            turn_id: client_metadata
                .and_then(|metadata| metadata.get(TURN_ID_CLIENT_METADATA_KEY))
                .cloned(),
            traceparent: client_metadata
                .and_then(|metadata| {
                    metadata.get(WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY)
                })
                .cloned(),
            previous_response_id: ws_request.previous_response_id.clone(),
            request_start_ms: client_metadata
                .and_then(|metadata| metadata.get(WS_STREAM_REQUEST_START_MS_CLIENT_METADATA_KEY))
                .cloned(),
            warmup: ws_request.generate == Some(false),
            connection_reused,
        };
        let request_text = Utf8Bytes::from(serialize_websocket_request(&request)?);
        on_encoded(request_text.as_ref());
        let enriched = with_tool_results(request_text.as_str(), &ws_request.tool_results);
        let private = enriched.is_some();
        let request_text = WsRequestText {
            private,
            text: enriched.map(Utf8Bytes::from).unwrap_or(request_text),
        };

        let current_span = Span::current();
        tokio::spawn(
            #[expect(
                clippy::await_holding_invalid_type,
                reason = "the guard serializes exclusive use of the websocket stream for the lifetime of the response stream"
            )]
            async move {
                if let Some(model) = server_model {
                    let _ = tx_event.send(Ok(ResponseEvent::ServerModel(model))).await;
                }
                if server_reasoning_included {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::ServerReasoningIncluded(true)))
                        .await;
                }
                // Abandoned work has a cleanup deadline; ordinary steering preserves the response.
                let interrupt = async move {
                    match rx_interrupt.await {
                        Ok(interrupt) => interrupt,
                        Err(_) => std::future::pending().await,
                    }
                }
                .shared();
                let interrupt_timeout = async {
                    match interrupt.clone().await {
                        ResponseInterrupt::Drain => std::future::pending().await,
                        ResponseInterrupt::Cancel { deadline } => {
                            tokio::time::sleep_until(deadline).await;
                        }
                    }
                };
                tokio::pin!(interrupt_timeout);
                let mut guard = tokio::select! {
                    biased;
                    _ = tx_event.closed() => return,
                    () = &mut interrupt_timeout => {
                        let _ = tx_event.send(Err(ApiError::Stream(
                            "timed out draining interrupted response".into(),
                        ))).await;
                        return;
                    }
                    guard = stream.lock() => guard,
                };
                if tx_event.is_closed() {
                    return;
                }
                let Some(mut ws_stream) = guard.take() else {
                    let _ = tx_event
                        .send(Err(ApiError::Stream(
                            "websocket connection is closed".to_string(),
                        )))
                        .await;
                    return;
                };
                // Warmup drains events without updating the catalog. Preserve the handshake
                // ETag until the first generation, and never replay it on a reused connection:
                // later response metadata may advertise a newer catalog.
                if !timing_log_context.warmup
                    && let Some(etag) = ws_stream.handshake_models_etag.take()
                {
                    let _ = tx_event.send(Ok(ResponseEvent::ModelsEtag(etag))).await;
                }
                let mut completion = None;
                let result = {
                    let response = run_websocket_response_stream(
                        &mut ws_stream,
                        tx_event.clone(),
                        request_text,
                        idle_timeout,
                        telemetry,
                        turn_state.as_deref(),
                        &timing_log_context,
                        interrupt.clone(),
                        &mut completion,
                    );
                    tokio::pin!(response);

                    tokio::select! {
                        biased;
                        result = &mut response => result,
                        () = &mut interrupt_timeout => Err(ApiError::Stream(
                            "timed out draining interrupted response".into(),
                        )),
                        () = tx_event.closed() => {
                            // Release the lock so later requests can open a new connection
                            // while this task finishes cleanup on the detached socket.
                            drop(guard);
                            if matches!(
                                interrupt.clone().now_or_never(),
                                Some(ResponseInterrupt::Cancel { .. })
                            ) {
                                tokio::select! {
                                    _ = &mut response => {}
                                    () = &mut interrupt_timeout => {}
                                }
                            }
                            return;
                        }
                    }
                };

                if let Err(err) = result {
                    // A terminal stream error should reach the caller immediately. Waiting for a
                    // graceful close handshake here can stall indefinitely and mask the error.
                    drop(guard);
                    drop(ws_stream);
                    // Failed cancellation retires the socket while preserving any received
                    // completion and its usage for the abandoned classifier's cleanup.
                    let event = completion.map_or_else(|| Err(err), Ok);
                    let _ = tx_event.send(event).await;
                } else {
                    *guard = Some(ws_stream);
                    if let Some(completed) = completion {
                        // The handshake is finished. Consumer backpressure must not let the
                        // interruption deadline discard completion or its usage accounting.
                        drop(guard);
                        let _ = tx_event.send(Ok(completed)).await;
                    }
                }
            }
            .instrument(current_span),
        );

        Ok(ResponseStream {
            rx_event,
            upstream_request_id: None,
            routing_affinity: self.routing_affinity.clone(),
            interrupt: Some(tx_interrupt),
        })
    }
}

mod connector;

/// Client for connecting to the Responses WebSocket endpoint for one provider.
pub struct ResponsesWebsocketClient {
    provider: Provider,
    auth: SharedAuthProvider,
}

impl ResponsesWebsocketClient {
    /// Creates a Responses WebSocket client for an already-resolved provider and auth source.
    pub fn new(provider: Provider, auth: SharedAuthProvider) -> Self {
        Self { provider, auth }
    }

    pub async fn connect(
        &self,
        http_client_factory: &HttpClientFactory,
        extra_headers: HeaderMap,
        default_headers: HeaderMap,
        turn_state: Option<Arc<OnceLock<String>>>,
        telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    ) -> Result<ResponsesWebsocketConnection, ApiError> {
        let connector = WebSocketConnector::new(http_client_factory)
            .map_err(|err| ApiError::Stream(format!("failed to configure websocket TLS: {err}")))?;
        self.connect_with_connector(
            &connector,
            extra_headers,
            default_headers,
            turn_state,
            telemetry,
        )
        .await
    }

    /// Opens a WebSocket connection long enough to validate the upgrade response.
    ///
    /// The probe uses the same URL construction, headers, authentication, TLS,
    /// and custom-CA path as a real Responses WebSocket connection, but it does
    /// not send a request frame. After the HTTP 101 upgrade succeeds, it waits
    /// briefly for an immediate server close frame so diagnostics can distinguish
    /// a usable connection from a policy rejection that closes right away.
    pub async fn probe_handshake(
        &self,
        http_client_factory: &HttpClientFactory,
        extra_headers: HeaderMap,
        default_headers: HeaderMap,
        immediate_close_timeout: Duration,
    ) -> Result<ResponsesWebsocketProbe, ResponsesWebsocketProbeError> {
        let ws_url = self
            .provider
            .websocket_url_for_path(RESPONSES_PATH)
            .map_err(|err| {
                ResponsesWebsocketProbeError::before_upgrade(
                    ApiError::Stream(format!("failed to build websocket URL: {err}")),
                    ResponsesWebsocketProbeFailureStage::Unknown,
                )
            })?;

        let headers = merge_request_headers(&self.provider.headers, extra_headers, default_headers);
        let headers = self.auth.apply_auth_headers(headers).await.map_err(|err| {
            ResponsesWebsocketProbeError::before_upgrade(
                err.into(),
                ResponsesWebsocketProbeFailureStage::Unknown,
            )
        })?;

        let connector = WebSocketConnector::new(http_client_factory).map_err(|err| {
            ResponsesWebsocketProbeError::before_upgrade(
                ApiError::Stream(format!("failed to configure websocket TLS: {err}")),
                ResponsesWebsocketProbeFailureStage::Tls,
            )
        })?;
        let (mut stream, response, reasoning_included, server_model) = connect_websocket(
            ws_url.clone(),
            headers,
            &connector,
            /*turn_state*/ None,
        )
        .await?;
        let status = response.status();
        let mut headers = HeaderMap::new();
        for name in [
            "x-request-id",
            "x-oai-request-id",
            "cf-ray",
            "x-ms-request-id",
        ] {
            for value in response.headers().get_all(name) {
                headers.append(name, value.clone());
            }
        }
        let immediate_close = tokio::time::timeout(immediate_close_timeout, stream.next())
            .await
            .ok()
            .flatten()
            .transpose()
            .map_err(|error| ResponsesWebsocketProbeError {
                error: Box::new(map_ws_stream_error(error)),
                stage: ResponsesWebsocketProbeFailureStage::Unknown,
                status: Some(status),
                headers: Some(headers.clone()),
            })?
            .and_then(immediate_close_from_message);

        Ok(ResponsesWebsocketProbe {
            url: ws_url.to_string(),
            status,
            headers,
            reasoning_included,
            server_model_present: server_model.is_some(),
            immediate_close,
        })
    }
}

fn immediate_close_from_message(message: Message) -> Option<ResponsesWebsocketClose> {
    let Message::Close(frame) = message else {
        return None;
    };
    Some(match frame {
        Some(frame) => close_frame_to_probe(frame),
        None => ResponsesWebsocketClose {
            code: "1005".to_string(),
            reason: String::new(),
        },
    })
}

fn close_frame_to_probe(frame: CloseFrame) -> ResponsesWebsocketClose {
    ResponsesWebsocketClose {
        code: frame.code.to_string(),
        reason: frame.reason.to_string(),
    }
}

fn merge_request_headers(
    provider_headers: &HeaderMap,
    extra_headers: HeaderMap,
    default_headers: HeaderMap,
) -> HeaderMap {
    let mut headers = provider_headers.clone();
    headers.extend(extra_headers);
    for (name, value) in &default_headers {
        if let http::header::Entry::Vacant(entry) = headers.entry(name) {
            entry.insert(value.clone());
        }
    }
    headers
}

async fn connect_websocket(
    url: Url,
    headers: HeaderMap,
    connector: &WebSocketConnector,
    turn_state: Option<Arc<OnceLock<String>>>,
) -> Result<(WsStream, Response, bool, Option<String>), ResponsesWebsocketProbeError> {
    info!("connecting to Responses WebSocket");

    let mut request = url.as_str().into_client_request().map_err(|err| {
        ResponsesWebsocketProbeError::before_upgrade(
            ApiError::Stream(format!("failed to build websocket request: {err}")),
            ResponsesWebsocketProbeFailureStage::Unknown,
        )
    })?;
    request.headers_mut().extend(headers);

    let response = connector.connect(request, websocket_config()).await;

    let (stream, response) = match response {
        Ok((stream, response)) => {
            info!(
                status = response.status().as_u16(),
                "successfully connected to Responses WebSocket"
            );
            (stream, response)
        }
        Err(err) => {
            // Tokio-rustls wraps certificate failures in I/O errors.
            let is_tls = match &err {
                WsError::Tls(_) => true,
                WsError::Io(error) => {
                    matches!(error.get_ref(), Some(source) if source.is::<rustls::Error>())
                }
                _ => false,
            };
            let stage = if is_tls {
                ResponsesWebsocketProbeFailureStage::Tls
            } else {
                ResponsesWebsocketProbeFailureStage::Unknown
            };
            error!(?stage, "failed to connect to Responses WebSocket");
            return Err(ResponsesWebsocketProbeError::before_upgrade(
                map_ws_error(err, &url),
                stage,
            ));
        }
    };

    let max_message_bytes = response
        .headers()
        .get(X_CODEX_WEBSOCKET_MAX_MESSAGE_BYTES_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    let reasoning_included = response.headers().contains_key(X_REASONING_INCLUDED_HEADER);
    let models_etag = response
        .headers()
        .get(X_MODELS_ETAG_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let server_model = response
        .headers()
        .get(OPENAI_MODEL_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    if let Some(turn_state) = turn_state
        && let Some(header_value) = response
            .headers()
            .get(X_CODEX_TURN_STATE_HEADER)
            .and_then(|value| value.to_str().ok())
    {
        let _ = turn_state.set(header_value.to_string());
    }
    Ok((
        WsStream::new(stream, max_message_bytes, models_etag),
        response,
        reasoning_included,
        server_model,
    ))
}

fn websocket_config() -> WebSocketConfig {
    let mut extensions = ExtensionsConfig::default();
    extensions.permessage_deflate = Some(DeflateConfig::default());

    let mut config = WebSocketConfig::default();
    config.extensions = extensions;
    config
}

fn map_ws_stream_error(error: WsError) -> ApiError {
    match codex_websocket_client::network_policy_denial(&error) {
        Some(denied) => ApiError::Transport(TransportError::Policy(denied)),
        None => ApiError::Stream(error.to_string()),
    }
}

fn map_ws_error(err: WsError, url: &Url) -> ApiError {
    if let Some(denied) = codex_websocket_client::network_policy_denial(&err) {
        return ApiError::Transport(TransportError::Policy(denied));
    }
    match err {
        WsError::Http(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let retry_after = RetryAfter::from_headers(&headers);
            let body = response
                .body()
                .as_ref()
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok());
            ApiError::Transport(TransportError::Http {
                status,
                url: Some(url.to_string()),
                headers: Some(headers),
                body,
                retry_after,
            })
        }
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            ApiError::Stream("websocket closed".to_string())
        }
        WsError::Io(err) => ApiError::Transport(TransportError::Network(err.to_string())),
        other => ApiError::Transport(TransportError::Network(other.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct WrappedWebsocketError {
    code: Option<String>,
    message: Option<String>,
    #[serde(default)]
    headers: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct WrappedWebsocketErrorEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(alias = "status_code")]
    status: Option<u16>,
    #[serde(default)]
    error: Option<WrappedWebsocketError>,
    #[serde(default)]
    headers: Option<JsonMap<String, Value>>,
}

fn parse_wrapped_websocket_error_event(payload: &str) -> Option<WrappedWebsocketErrorEvent> {
    let event: WrappedWebsocketErrorEvent = serde_json::from_str(payload).ok()?;
    if event.kind != "error" {
        return None;
    }
    Some(event)
}

fn map_wrapped_websocket_error_event(
    event: WrappedWebsocketErrorEvent,
    original_payload: String,
) -> Option<ApiError> {
    let WrappedWebsocketErrorEvent {
        status,
        error,
        headers,
        ..
    } = event;
    let retry_after = [
        error
            .as_ref()
            .and_then(|error| error.headers.as_ref())
            .and_then(Value::as_object),
        headers.as_ref(),
    ]
    .into_iter()
    .flatten()
    .find_map(|headers| RetryAfter::from_headers(&json_headers_to_http_headers(headers)));

    if let Some(error) = error.as_ref()
        && let Some(code) = error.code.as_deref()
        && let Some(fallback_message) = match code {
            WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE => {
                Some(WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE)
            }
            PREVIOUS_RESPONSE_NOT_FOUND_CODE => Some(PREVIOUS_RESPONSE_NOT_FOUND_MESSAGE),
            _ => None,
        }
    {
        return Some(ApiError::Retryable {
            message: error
                .message
                .clone()
                .unwrap_or_else(|| fallback_message.to_string()),
            retry_after,
        });
    }

    let status = StatusCode::from_u16(status?).ok()?;
    if status.is_success() {
        return None;
    }

    Some(ApiError::Transport(TransportError::Http {
        status,
        url: None,
        headers: headers.as_ref().map(json_headers_to_http_headers),
        body: Some(original_payload),
        retry_after,
    }))
}

#[derive(Deserialize)]
struct InterruptAcknowledgement {
    response_id: String,
}

fn is_websocket_receive_failure(
    response: &Result<Option<Result<Message, WsError>>, ApiError>,
) -> bool {
    match response {
        Ok(Some(Ok(message))) => match message {
            Message::Text(_) | Message::Ping(_) | Message::Pong(_) => false,
            Message::Binary(_) | Message::Close(_) | Message::Frame(_) => true,
        },
        Ok(Some(Err(_))) | Ok(None) | Err(_) => true,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_websocket_response_stream(
    ws_stream: &mut WsStream,
    tx_event: mpsc::Sender<std::result::Result<ResponseEvent, ApiError>>,
    request_text: WsRequestText,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    turn_state: Option<&OnceLock<String>>,
    timing_log_context: &ResponsesWebsocketTimingLogContext,
    interrupt: impl Future<Output = ResponseInterrupt> + Clone + Unpin,
    completion: &mut Option<ResponseEvent>,
) -> Result<(), ApiError> {
    let mut last_server_model: Option<String> = None;
    let mut safety_buffering_treatment = SafetyBufferingTreatment::default();
    if matches!(
        interrupt.clone().now_or_never(),
        Some(ResponseInterrupt::Cancel { .. })
    ) {
        // No request reached the backend, so this socket is still reusable.
        return Ok(());
    }
    let mut send_outcome = ws_stream
        .send_create(
            request_text,
            idle_timeout,
            telemetry.as_deref(),
            timing_log_context.connection_reused,
        )
        .await?;

    // A response owns its interrupt, and create must be sent before interrupt.
    let mut interrupt_request = interrupt.clone().fuse();
    let mut interruption = None;
    let mut response_id = None;
    let mut received_text = false;
    let mut awaiting_interrupt_ack = false;
    loop {
        let poll_start = Instant::now();
        let response = tokio::select! {
            response = tokio::time::timeout(idle_timeout, ws_stream.next()) => {
                response.map_err(|_| ApiError::Stream("idle timeout waiting for websocket".into()))
            }
            request = &mut interrupt_request, if response_id.is_some() && matches!(send_outcome, WsSendOutcome::Flushed) => {
                let command = WsRequestText {
                    text: serde_json::json!({
                        "type": "response.interrupt",
                        "response_id": response_id,
                        "mode": "discard_partial_items",
                    }).to_string().into(),
                    private: false,
                };
                let mut send = ws_stream.queue_request(command, idle_timeout).await?;
                send_outcome = WsSendOutcome::receive(&mut send).await?;
                if !matches!(send_outcome, WsSendOutcome::NotStarted(_)) {
                    interruption = Some(request);
                    awaiting_interrupt_ack = matches!(request, ResponseInterrupt::Cancel { .. });
                }
                continue;
            }
        };
        // A failed send retires the pump. Process replies until its queue runs dry.
        if matches!(response, Ok(None))
            && let WsSendOutcome::NotStarted(error) | WsSendOutcome::Interrupted(error) =
                send_outcome
        {
            return Err(error);
        }
        let poll_duration = poll_start.elapsed();
        if let Some(telemetry) = &telemetry
            && is_websocket_receive_failure(&response)
        {
            telemetry.on_ws_receive_failure(poll_duration);
        }
        let message = match response {
            Ok(Some(Ok(msg))) => msg,
            Ok(Some(Err(err))) => {
                return Err(map_ws_stream_error(err));
            }
            Ok(None) => {
                return Err(ApiError::Stream(
                    "stream closed before response.completed".into(),
                ));
            }
            Err(err) => {
                return Err(err);
            }
        };

        match message {
            Message::Text(text) => {
                received_text = true;
                let event = serde_json::from_str::<ResponsesStreamEvent>(&text);
                let kind = event.as_ref().ok().map(ResponsesStreamEvent::kind);
                if let Some(t) = telemetry.as_ref() {
                    t.on_ws_text_event(&text, kind, poll_duration);
                }
                // A wrapped error can still be valid when an unrelated stream field is malformed.
                if matches!(kind, Some("error") | None)
                    && let Some(wrapped_error) = parse_wrapped_websocket_error_event(&text)
                    && let Some(error) =
                        map_wrapped_websocket_error_event(wrapped_error, text.to_string())
                {
                    return Err(error);
                }

                let event = match event {
                    Ok(event) => event,
                    Err(err) => {
                        debug!("failed to parse websocket event: {err}, data: {text}");
                        continue;
                    }
                };
                if matches!(interruption, Some(ResponseInterrupt::Cancel { .. }))
                    && matches!(
                        event.kind(),
                        "response.interrupt.accepted" | "response.interrupt.failed"
                    )
                {
                    let acknowledgement: InterruptAcknowledgement = serde_json::from_str(&text)
                        .map_err(|_| {
                            ApiError::Stream("invalid interruption acknowledgement".into())
                        })?;
                    if !awaiting_interrupt_ack
                        || response_id.as_ref() != Some(&acknowledgement.response_id)
                    {
                        return Err(ApiError::Stream(
                            "unexpected interruption acknowledgement".into(),
                        ));
                    }
                    if event.kind() == "response.interrupt.failed" {
                        return Err(ApiError::Stream(
                            "response interruption was rejected".into(),
                        ));
                    }
                    awaiting_interrupt_ack = false;
                    if completion.is_some() {
                        break;
                    }
                    continue;
                }
                emit_responses_websocket_timing_event(
                    event.kind(),
                    text.as_str(),
                    timing_log_context,
                );
                if event.kind() == "codex.response.metadata"
                    && let Some(etag) =
                        event
                            .headers
                            .as_ref()
                            .and_then(Value::as_object)
                            .and_then(|headers| {
                                json_headers_to_http_headers(headers)
                                    .get(X_MODELS_ETAG_HEADER)
                                    .and_then(|value| value.to_str().ok())
                                    .map(str::to_string)
                            })
                {
                    let _ = tx_event.send(Ok(ResponseEvent::ModelsEtag(etag))).await;
                }
                if let Some(response_turn_state) = event.turn_state()
                    && let Some(turn_state) = turn_state
                {
                    let _ = turn_state.set(response_turn_state);
                }
                let safety_buffering =
                    safety_buffering_for_event(&event, &mut safety_buffering_treatment);
                let metadata_events = event.metadata_events(safety_buffering);
                if event.kind() == "codex.rate_limits" {
                    if let Some(snapshot) = parse_rate_limit_event(&text) {
                        let _ = tx_event.send(Ok(ResponseEvent::RateLimits(snapshot))).await;
                    }
                    continue;
                }
                let model = event.response_model();
                // Preserve terminal usage before any accompanying notification can block.
                let event = match process_responses_event(event) {
                    Ok(Some(event @ ResponseEvent::Completed { .. })) => {
                        *completion = Some(event);
                        Ok(None)
                    }
                    event => event,
                };
                if let Some(model) = model
                    && last_server_model.as_deref() != Some(model.as_str())
                {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::ServerModel(model.clone())))
                        .await;
                    last_server_model = Some(model);
                }
                for metadata_event in metadata_events {
                    let _ = tx_event.send(Ok(metadata_event)).await;
                }
                match event {
                    Ok(Some(event)) => {
                        if let ResponseEvent::Created { response_id: id } = &event {
                            response_id.clone_from(id);
                        }
                        let _ = tx_event.send(Ok(event)).await;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        return Err(error.into_api_error());
                    }
                }
                if completion.is_some() && !awaiting_interrupt_ack {
                    break;
                }
            }
            Message::Binary(_) => {
                return Err(ApiError::Stream("unexpected binary websocket event".into()));
            }
            Message::Close(Some(frame)) if frame.code == CloseCode::Size && !received_text => {
                return Err(ApiError::WebsocketMessageTooLarge);
            }
            Message::Close(_) => {
                return Err(ApiError::Stream(
                    "websocket closed by server before response.completed".into(),
                ));
            }
            Message::Frame(_) => {}
            Message::Ping(_) | Message::Pong(_) => {}
        }
    }

    send_outcome.into_result()?;
    if matches!(
        interrupt.clone().now_or_never(),
        Some(ResponseInterrupt::Cancel { .. })
    ) {
        return Ok(());
    }
    // Ordinary responses deliver completion before releasing the socket.
    // Preserve it if cancellation wins while delivery is backpressured.
    let permit = tx_event.reserve().await;
    if let (Ok(permit), Some(event)) = (permit, completion.take()) {
        permit.send(Ok(event));
    }
    Ok(())
}

fn emit_responses_websocket_timing_event(
    kind: &str,
    payload: &str,
    context: &ResponsesWebsocketTimingLogContext,
) {
    if kind != RESPONSES_WEBSOCKET_TIMING_KIND {
        return;
    }

    // This full payload is excluded from always-on sinks. Opt in with
    // `RUST_LOG='codex_api::responses_websocket_timing=trace'`.
    tracing::event!(
        name: RESPONSES_WEBSOCKET_TIMING_KIND,
        target: RESPONSES_WEBSOCKET_TIMING_EVENT_TARGET,
        tracing::Level::TRACE,
        model = context.model.as_str(),
        session_id = context.session_id.as_deref().unwrap_or_default(),
        thread_id = context.thread_id.as_deref().unwrap_or_default(),
        turn_id = context.turn_id.as_deref().unwrap_or_default(),
        traceparent = context.traceparent.as_deref().unwrap_or_default(),
        previous_response_id = context.previous_response_id.as_deref().unwrap_or_default(),
        request_start_ms = context.request_start_ms.as_deref().unwrap_or_default(),
        warmup = context.warmup,
        connection_reused = context.connection_reused,
        payload,
        "responses websocket timing"
    );
}

fn safety_buffering_for_event(
    event: &ResponsesStreamEvent,
    treatment: &mut SafetyBufferingTreatment,
) -> Option<crate::common::SafetyBuffering> {
    if let Some(headers) = event.headers.as_ref().and_then(Value::as_object)
        && let Some(updated_treatment) =
            treatment_from_headers(&json_headers_to_http_headers(headers))
    {
        *treatment = updated_treatment;
    }
    event.safety_buffering(treatment)
}

fn serialize_websocket_request(request: &ResponsesWsRequest<'_>) -> Result<String, ApiError> {
    serde_json::to_string(request)
        .map_err(|err| ApiError::Stream(format!("failed to encode websocket request: {err}")))
}

#[cfg(test)]
#[path = "responses_websocket_upload_tests.rs"]
mod upload_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ResponseCreateWsRequest;
    use crate::common::ResponsesApiRequest;
    use codex_protocol::ResponseItemId;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use serde_json::value::RawValue;
    use serde_json::value::to_raw_value;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn direct_serialization_preserves_websocket_request_payload() {
        for (country_code, expected_country) in [
            (None, None),
            (Some(" de "), Some(json!("DE"))),
            (Some("xX"), Some(Value::Null)),
        ] {
            assert_serialized_request(country_code, expected_country);
        }
    }

    fn assert_serialized_request(country_code: Option<&str>, expected_country: Option<Value>) {
        let api_request = ResponsesApiRequest {
            request_country: country_code
                .map(codex_protocol::RequestCountry::from_iso_code)
                .transpose()
                .expect("country is valid")
                .unwrap_or_default(),
            model: "gpt-test".to_string(),
            input: vec![ResponseItem::Message {
                status: None,
                encrypted_content: None,
                id: Some(ResponseItemId::with_suffix("msg", "1")),
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "hello".to_string(),
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            }],
            tools: Some(
                Arc::<RawValue>::from(
                    to_raw_value(&vec![json!({
                        "type": "function",
                        "name": "lookup",
                        "parameters": {"type": "object"}
                    })])
                    .expect("serialize tools"),
                )
                .into(),
            ),
            tool_choice: "auto".to_string(),
            parallel_tool_calls: true,
            reasoning: None,
            store: false,
            stream: true,
            stream_options: None,
            include: vec!["reasoning.encrypted_content".to_string()],
            service_tier: Some("priority".to_string()),
            prompt_cache_key: Some("cache-key".to_string()),
            prompt_cache_options: Some(crate::PromptCacheOptions {
                ttl: codex_protocol::turn_input::PromptCacheTtl::TwelveHours,
            }),
            text: None,
            access_programs: Some(
                codex_protocol::turn_input::CyberAccessProgram::DaybreakBlue.into(),
            ),
            client_metadata: Some(HashMap::from([(
                "traceparent".to_string(),
                "00-0123456789abcdef0123456789abcdef-0123456789abcdef-01".to_string(),
            )])),
        };
        let request = ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest {
            previous_response_id: Some("resp-1".to_string()),
            generate: Some(false),
            ..ResponseCreateWsRequest::from(&api_request)
        });

        let mut expected_payload =
            serde_json::to_value(&api_request).expect("serialize responses API request");
        assert_eq!(
            expected_payload.get("request_country"),
            expected_country.as_ref()
        );
        expected_payload["type"] = json!("response.create");
        expected_payload["previous_response_id"] = json!("resp-1");
        expected_payload["generate"] = json!(false);
        let request_text =
            serialize_websocket_request(&request).expect("serialize websocket request");
        assert!(request_text.starts_with(
            r#"{"type":"response.create","model":"gpt-test","stream":true,"service_tier":"priority","previous_response_id":"resp-1","input":"#
        ));
        let wire_payload =
            serde_json::from_str::<Value>(&request_text).expect("parse websocket request");

        assert_eq!(
            wire_payload.get("request_country"),
            expected_country.as_ref()
        );
        assert_eq!(wire_payload, expected_payload);
    }

    #[test]
    fn websocket_config_enables_permessage_deflate() {
        let config = websocket_config();
        assert!(config.extensions.permessage_deflate.is_some());
    }

    #[test]
    fn parse_wrapped_websocket_error_event_maps_to_transport_http() {
        let payload = json!({
            "type": "error",
            "status": 429,
            "error": {
                "type": "usage_limit_reached",
                "message": "The usage limit has been reached",
                "plan_type": "pro",
                "resets_at": 1738888888
            },
            "headers": {
                "x-codex-primary-used-percent": "100.0",
                "x-codex-primary-window-minutes": 15
            }
        })
        .to_string();

        let wrapped_error = parse_wrapped_websocket_error_event(&payload)
            .expect("expected websocket error payload to be parsed");
        let api_error = map_wrapped_websocket_error_event(wrapped_error, payload)
            .expect("expected websocket error payload to map to ApiError");

        let ApiError::Transport(TransportError::Http {
            status,
            headers,
            body,
            ..
        }) = api_error
        else {
            panic!("expected ApiError::Transport(Http)");
        };

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let headers = headers.expect("expected headers");
        assert_eq!(
            headers
                .get("x-codex-primary-used-percent")
                .and_then(|value| value.to_str().ok()),
            Some("100.0")
        );
        assert_eq!(
            headers
                .get("x-codex-primary-window-minutes")
                .and_then(|value| value.to_str().ok()),
            Some("15")
        );
        let body = body.expect("expected body");
        assert!(body.contains("usage_limit_reached"));
        assert!(body.contains("The usage limit has been reached"));
    }

    /// Websocket error advice uses the shared HTTP value validation and duplicate handling.
    #[tokio::test(start_paused = true)]
    async fn wrapped_websocket_retry_after_uses_http_header_validation() {
        for (error_headers, expected_seconds) in [
            (json!("invalid"), 12),
            (json!({"retry-after": "\n5\n"}), 12),
            (json!({"retry-after": "\t5\t"}), 5),
            (json!({"Retry-After": "5", "retry-after": "30"}), 30),
        ] {
            let payload = json!({
                "type": "error",
                "status": 429,
                "error": {"code": "rate_limit_exceeded", "headers": error_headers},
                "headers": {"retry-after": "12"}
            })
            .to_string();
            let wrapped = parse_wrapped_websocket_error_event(&payload).unwrap();
            let Some(ApiError::Transport(TransportError::Http { retry_after, .. })) =
                map_wrapped_websocket_error_event(wrapped, payload)
            else {
                panic!("expected a websocket HTTP error");
            };
            assert_eq!(
                retry_after,
                RetryAfter::from_delay(Duration::from_secs(expected_seconds))
            );
        }
    }

    #[test]
    fn wrapped_websocket_usage_limit_preserves_optional_window() {
        for (window, expected) in [(Some(10080), Some(10080)), (None, None)] {
            let mut payload = json!({
                "type": "error",
                "status": 429,
                "error": {
                    "type": "usage_limit_reached",
                    "plan_type": "pro"
                }
            });
            if let Some(window) = window {
                payload["error"]["limit_window_minutes"] = json!(window);
            }
            let payload = payload.to_string();
            let wrapped = parse_wrapped_websocket_error_event(&payload).expect("websocket error");
            let api_error = map_wrapped_websocket_error_event(wrapped, payload).expect("API error");
            let err = crate::api_bridge::map_api_error(api_error);
            let codex_protocol::error::CodexErrorDetails::UsageLimitReached(usage_limit) =
                err.details()
            else {
                panic!("expected usage-limit error, got {err:?}");
            };
            assert_eq!(usage_limit.limit_window_minutes, expected);
        }
    }

    #[test]
    fn parse_wrapped_websocket_error_event_ignores_non_error_payloads() {
        let payload = json!({
            "type": "response.created",
            "response": {
                "id": "resp-1"
            }
        })
        .to_string();

        let wrapped_error = parse_wrapped_websocket_error_event(&payload);
        assert!(wrapped_error.is_none());
    }

    #[test]
    fn parse_wrapped_websocket_error_event_with_status_maps_invalid_request() {
        let payload = json!({
            "type": "error",
            "status": 400,
            "error": {
                "type": "invalid_request_error",
                "message": "Model does not support image inputs"
            }
        })
        .to_string();

        let wrapped_error = parse_wrapped_websocket_error_event(&payload)
            .expect("expected websocket error payload to be parsed");
        let api_error = map_wrapped_websocket_error_event(wrapped_error, payload)
            .expect("expected websocket error payload to map to ApiError");
        let ApiError::Transport(TransportError::Http { status, body, .. }) = api_error else {
            panic!("expected ApiError::Transport(Http)");
        };
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let body = body.expect("expected body");
        assert!(body.contains("invalid_request_error"));
        assert!(body.contains("Model does not support image inputs"));
    }

    #[test]
    fn wrapped_context_length_error_respects_http_status() {
        for status in [400, 503] {
            let payload = json!({
                "type": "error",
                "status": status,
                "error": {"code": "context_length_exceeded"}
            })
            .to_string();
            let event = parse_wrapped_websocket_error_event(&payload).unwrap();
            let error = map_wrapped_websocket_error_event(event, payload).unwrap();
            let error = crate::api_bridge::map_api_error(error);
            assert_eq!(
                matches!(
                    error.details(),
                    codex_protocol::error::CodexErrorDetails::ContextWindowExceeded
                ),
                status == 400
            );
            assert_eq!(
                error.retry_delay(/*retry_count*/ 1).is_some(),
                status == 503
            );
        }
    }

    #[test]
    fn parse_wrapped_websocket_error_event_with_connection_limit_maps_retryable() {
        let payload = json!({
            "type": "error",
            "status": 400,
            "error": {
                "type": "invalid_request_error",
                "code": "websocket_connection_limit_reached",
                "message": "Responses websocket connection limit reached (60 minutes). Create a new websocket connection to continue."
            }
        })
        .to_string();

        let wrapped_error = parse_wrapped_websocket_error_event(&payload)
            .expect("expected websocket error payload to be parsed");
        let api_error = map_wrapped_websocket_error_event(wrapped_error, payload)
            .expect("expected websocket error payload to map to ApiError");
        let ApiError::Retryable {
            message,
            retry_after,
        } = api_error
        else {
            panic!("expected ApiError::Retryable");
        };
        assert_eq!(message, WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE);
        assert_eq!(retry_after, None);
    }

    #[test]
    fn parse_wrapped_websocket_error_event_without_status_is_not_mapped() {
        let payload = json!({
            "type": "error",
            "error": {
                "type": "usage_limit_reached",
                "message": "The usage limit has been reached"
            },
            "headers": {
                "x-codex-primary-used-percent": "100.0",
                "x-codex-primary-window-minutes": 15
            }
        })
        .to_string();

        let wrapped_error = parse_wrapped_websocket_error_event(&payload)
            .expect("expected websocket error payload to be parsed");
        let api_error = map_wrapped_websocket_error_event(wrapped_error, payload);
        assert!(api_error.is_none());
    }

    #[test]
    fn merge_request_headers_matches_http_precedence() {
        let mut provider_headers = HeaderMap::new();
        provider_headers.insert(
            "originator",
            HeaderValue::from_static("provider-originator"),
        );
        provider_headers.insert("x-priority", HeaderValue::from_static("provider"));

        let mut extra_headers = HeaderMap::new();
        extra_headers.insert("x-priority", HeaderValue::from_static("extra"));

        let mut default_headers = HeaderMap::new();
        default_headers.insert("originator", HeaderValue::from_static("default-originator"));
        default_headers.insert("x-priority", HeaderValue::from_static("default"));
        default_headers.insert("x-default-only", HeaderValue::from_static("default-only"));

        let merged = merge_request_headers(&provider_headers, extra_headers, default_headers);

        assert_eq!(
            merged.get("originator"),
            Some(&HeaderValue::from_static("provider-originator"))
        );
        assert_eq!(
            merged.get("x-priority"),
            Some(&HeaderValue::from_static("extra"))
        );
        assert_eq!(
            merged.get("x-default-only"),
            Some(&HeaderValue::from_static("default-only"))
        );
    }

    #[test]
    fn websocket_safety_buffering_uses_event_before_header_fallback() {
        let metadata: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "codex.response.metadata",
            "headers": {
                "x-codex-safety-buffering-enabled": "true",
                "x-codex-safety-buffering-faster-model": "gpt-fast-header"
            }
        }))
        .expect("deserialize treatment metadata");
        let event: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "response.output_text.delta",
            "safety_buffering": {
                "use_cases": ["cyber"],
                "reasons": ["user_risk"],
                "retry_model": "gpt-fast-wire"
            }
        }))
        .expect("deserialize safety buffering event");
        let mut treatment = SafetyBufferingTreatment::default();

        assert!(safety_buffering_for_event(&metadata, &mut treatment).is_none());
        let buffering = safety_buffering_for_event(&event, &mut treatment)
            .expect("expected safety buffering payload");

        assert_eq!(
            buffering,
            crate::common::SafetyBuffering {
                use_cases: vec!["cyber".to_string()],
                reasons: vec!["user_risk".to_string()],
                show_buffering_ui: true,
                faster_model: Some("gpt-fast-wire".to_string()),
            }
        );
    }

    #[test]
    fn websocket_safety_buffering_event_controls_visibility_when_header_disables_it() {
        let metadata: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "codex.response.metadata",
            "headers": {
                "x-codex-safety-buffering-enabled": "false",
                "x-codex-safety-buffering-faster-model": "gpt-fast-header"
            }
        }))
        .expect("deserialize treatment metadata");
        let event: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "response.output_text.delta",
            "safety_buffering": {
                "use_cases": ["cyber"],
                "reasons": ["user_risk"]
            }
        }))
        .expect("deserialize safety buffering event");
        let mut treatment = SafetyBufferingTreatment::default();

        assert!(safety_buffering_for_event(&metadata, &mut treatment).is_none());
        let buffering = safety_buffering_for_event(&event, &mut treatment)
            .expect("expected safety buffering payload");

        assert_eq!(
            buffering,
            crate::common::SafetyBuffering {
                use_cases: vec!["cyber".to_string()],
                reasons: vec!["user_risk".to_string()],
                show_buffering_ui: true,
                faster_model: Some("gpt-fast-header".to_string()),
            }
        );
    }
}
