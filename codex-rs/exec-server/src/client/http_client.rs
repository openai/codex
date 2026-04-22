use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCErrorError;
use futures::StreamExt;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use serde_json::Value;
use serde_json::from_value;
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tracing::debug;

use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpHeader;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

/// Maximum queued body frames per streamed executor HTTP response.
const HTTP_BODY_DELTA_CHANNEL_CAPACITY: usize = 256;

#[derive(Default)]
struct ExecutorHttpBodyStreamRegistry {
    reserved_ids: HashMap<String, Option<JoinHandle<()>>>,
}

pub(crate) struct ExecutorPendingHttpBodyStream {
    pub(crate) request_id: String,
    response: reqwest::Response,
}

pub(crate) struct ExecutorHttpClient {
    notifications: RpcNotificationSender,
    body_streams: Mutex<ExecutorHttpBodyStreamRegistry>,
}

/// Request-scoped stream of body chunks for an executor HTTP response.
///
/// The initial `http/request` call returns status and headers. This stream then
/// receives the ordered `http/request/bodyDelta` notifications for that request
/// id until EOF or a terminal error.
pub struct HttpResponseBodyStream {
    inner: Arc<Inner>,
    request_id: String,
    next_seq: u64,
    rx: mpsc::Receiver<HttpRequestBodyDeltaNotification>,
    // Terminal frames can carry a final chunk; return that once, then EOF.
    pending_eof: bool,
    closed: bool,
}

impl ExecutorHttpClient {
    pub(crate) fn new(notifications: RpcNotificationSender) -> Self {
        Self {
            notifications,
            body_streams: Mutex::new(ExecutorHttpBodyStreamRegistry::default()),
        }
    }

    pub(crate) async fn shutdown(&self) {
        let tasks = {
            let mut body_streams = self.body_streams.lock().await;
            body_streams
                .reserved_ids
                .drain()
                .filter_map(|(_, task)| task)
                .collect::<Vec<_>>()
        };
        for task in tasks {
            task.abort();
        }
    }

    pub(crate) async fn run_http_request(
        &self,
        params: HttpRequestParams,
    ) -> Result<(HttpRequestResponse, Option<ExecutorPendingHttpBodyStream>), JSONRPCErrorError>
    {
        let stream_response = params.stream_response;
        let request_id = params.request_id.clone();
        if stream_response {
            self.reserve_http_body_stream(&request_id).await?;
        }

        let result = run_executor_http_request(params).await;
        if result.is_err() && stream_response {
            self.release_http_body_stream(&request_id).await;
        }
        result
    }

    pub(crate) async fn start_http_body_stream(
        self: &Arc<Self>,
        pending_stream: ExecutorPendingHttpBodyStream,
    ) {
        let request_id = pending_stream.request_id.clone();
        let finished_request_id = request_id.clone();
        let http_client = Arc::clone(self);
        let notifications = self.notifications.clone();
        let task = tokio::spawn(async move {
            stream_executor_http_body(pending_stream, notifications).await;
            http_client
                .release_http_body_stream(&finished_request_id)
                .await;
        });
        let mut body_streams = self.body_streams.lock().await;
        if let Some(entry) = body_streams.reserved_ids.get_mut(&request_id) {
            *entry = Some(task);
        } else {
            task.abort();
        }
    }

    pub(crate) async fn release_http_body_stream(&self, request_id: &str) {
        let mut body_streams = self.body_streams.lock().await;
        body_streams.reserved_ids.remove(request_id);
    }

    async fn reserve_http_body_stream(&self, request_id: &str) -> Result<(), JSONRPCErrorError> {
        let mut body_streams = self.body_streams.lock().await;
        if body_streams.reserved_ids.contains_key(request_id) {
            return Err(invalid_params(format!(
                "http/request streamResponse requestId `{request_id}` is already active"
            )));
        }
        body_streams
            .reserved_ids
            .insert(request_id.to_string(), None);
        Ok(())
    }
}

impl HttpResponseBodyStream {
    /// Receives the next response-body chunk.
    ///
    /// Returns `Ok(None)` at EOF and converts sequence gaps or executor-side
    /// stream errors into protocol errors.
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>, ExecServerError> {
        if self.pending_eof {
            self.pending_eof = false;
            self.finish().await;
            return Ok(None);
        }

        let Some(delta) = self.rx.recv().await else {
            self.finish().await;
            if let Some(error) = self
                .inner
                .take_http_body_stream_failure(&self.request_id)
                .await
            {
                return Err(ExecServerError::Protocol(format!(
                    "http response stream `{}` failed: {error}",
                    self.request_id
                )));
            }
            return Ok(None);
        };
        if delta.seq != self.next_seq {
            self.finish().await;
            return Err(ExecServerError::Protocol(format!(
                "http response stream `{}` received seq {}, expected {}",
                self.request_id, delta.seq, self.next_seq
            )));
        }
        self.next_seq += 1;
        let chunk = delta.delta.into_inner();

        if let Some(error) = delta.error {
            self.finish().await;
            return Err(ExecServerError::Protocol(format!(
                "http response stream `{}` failed: {error}",
                self.request_id
            )));
        }
        if delta.done {
            self.finish().await;
            if chunk.is_empty() {
                return Ok(None);
            }
            self.pending_eof = true;
        }
        Ok(Some(chunk))
    }

    /// Removes this stream from the connection routing table once it reaches EOF.
    async fn finish(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.inner.remove_http_body_stream(&self.request_id).await;
    }
}

impl Drop for HttpResponseBodyStream {
    /// Schedules stream-route removal if the consumer drops before EOF.
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        spawn_remove_http_body_stream(Arc::clone(&self.inner), self.request_id.clone());
    }
}

/// Active route registration owned while `http_request_stream` awaits headers.
struct HttpBodyStreamRegistration {
    inner: Arc<Inner>,
    request_id: String,
    active: bool,
}

impl Drop for HttpBodyStreamRegistration {
    /// Removes the route if the stream request future is cancelled before headers return.
    fn drop(&mut self) {
        if self.active {
            spawn_remove_http_body_stream(Arc::clone(&self.inner), self.request_id.clone());
        }
    }
}

impl ExecServerClient {
    /// Performs an executor-side HTTP request and buffers the response body.
    pub async fn http_request(
        &self,
        mut params: HttpRequestParams,
    ) -> Result<HttpRequestResponse, ExecServerError> {
        params.stream_response = false;
        self.call(HTTP_REQUEST_METHOD, &params).await
    }

    /// Performs an executor-side HTTP request and returns a body stream.
    ///
    /// The method sets `stream_response` and replaces any caller-supplied
    /// `request_id` with a connection-local id, so late deltas from abandoned
    /// streams cannot be confused with later requests.
    pub async fn http_request_stream(
        &self,
        mut params: HttpRequestParams,
    ) -> Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError> {
        params.stream_response = true;
        let request_id = self.inner.next_http_body_stream_request_id();
        params.request_id = request_id.clone();
        let (tx, rx) = mpsc::channel(HTTP_BODY_DELTA_CHANNEL_CAPACITY);
        self.inner
            .insert_http_body_stream(request_id.clone(), tx)
            .await?;
        let mut registration = HttpBodyStreamRegistration {
            inner: Arc::clone(&self.inner),
            request_id: request_id.clone(),
            active: true,
        };
        let response = match self.call(HTTP_REQUEST_METHOD, &params).await {
            Ok(response) => response,
            Err(error) => {
                self.inner.remove_http_body_stream(&request_id).await;
                registration.active = false;
                return Err(error);
            }
        };
        registration.active = false;
        Ok((
            response,
            HttpResponseBodyStream {
                inner: Arc::clone(&self.inner),
                request_id,
                next_seq: 1,
                rx,
                pending_eof: false,
                closed: false,
            },
        ))
    }
}

impl Inner {
    /// Routes one streamed HTTP body notification into its request-local receiver.
    pub(super) async fn handle_http_body_delta_notification(
        &self,
        params: Option<Value>,
    ) -> Result<(), ExecServerError> {
        let params: HttpRequestBodyDeltaNotification = from_value(params.unwrap_or(Value::Null))?;
        // Unknown request ids are ignored intentionally: a stream may have already
        // reached EOF and released its route.
        if let Some(tx) = self
            .http_body_streams
            .load()
            .get(&params.request_id)
            .cloned()
        {
            let request_id = params.request_id.clone();
            let terminal_delta = params.done || params.error.is_some();
            match tx.try_send(params) {
                Ok(()) => {
                    if terminal_delta {
                        self.remove_http_body_stream(&request_id).await;
                    }
                }
                Err(TrySendError::Closed(_)) => {
                    self.remove_http_body_stream(&request_id).await;
                    debug!("http response stream receiver dropped before body delta delivery");
                }
                Err(TrySendError::Full(_)) => {
                    self.record_http_body_stream_failure(
                        &request_id,
                        "body delta channel filled before delivery".to_string(),
                    )
                    .await;
                    self.remove_http_body_stream(&request_id).await;
                    debug!(
                        "closing http response stream `{request_id}` after body delta backpressure"
                    );
                }
            }
        }
        Ok(())
    }

    /// Fails active streamed HTTP bodies so callers do not wait forever after a
    /// transport disconnect or notification handling failure.
    pub(super) async fn fail_all_http_body_streams(&self, message: String) {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        let streams = streams.as_ref().clone();
        self.http_body_streams.store(Arc::new(HashMap::new()));
        for (request_id, tx) in streams {
            if tx
                .try_send(HttpRequestBodyDeltaNotification {
                    request_id: request_id.clone(),
                    seq: 1,
                    delta: Vec::new().into(),
                    done: true,
                    error: Some(message.clone()),
                })
                .is_err()
            {
                let mut next_failures = self.http_body_stream_failures.load().as_ref().clone();
                next_failures.insert(request_id, message.clone());
                self.http_body_stream_failures
                    .store(Arc::new(next_failures));
            }
        }
    }

    /// Allocates a connection-local streamed HTTP response id.
    fn next_http_body_stream_request_id(&self) -> String {
        let id = self
            .http_body_stream_next_id
            .fetch_add(1, Ordering::Relaxed);
        format!("http-{id}")
    }

    /// Registers a request id before issuing an executor streaming HTTP call.
    async fn insert_http_body_stream(
        &self,
        request_id: String,
        tx: mpsc::Sender<HttpRequestBodyDeltaNotification>,
    ) -> Result<(), ExecServerError> {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        if streams.contains_key(&request_id) {
            return Err(ExecServerError::Protocol(format!(
                "http response stream already registered for request {request_id}"
            )));
        }
        let mut next_streams = streams.as_ref().clone();
        next_streams.insert(request_id.clone(), tx);
        self.http_body_streams.store(Arc::new(next_streams));
        let failures = self.http_body_stream_failures.load();
        if failures.contains_key(&request_id) {
            let mut next_failures = failures.as_ref().clone();
            next_failures.remove(&request_id);
            self.http_body_stream_failures
                .store(Arc::new(next_failures));
        }
        Ok(())
    }

    /// Removes a request id after EOF, terminal error, or request failure.
    async fn remove_http_body_stream(
        &self,
        request_id: &str,
    ) -> Option<mpsc::Sender<HttpRequestBodyDeltaNotification>> {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        let stream = streams.get(request_id).cloned();
        stream.as_ref()?;
        let mut next_streams = streams.as_ref().clone();
        next_streams.remove(request_id);
        self.http_body_streams.store(Arc::new(next_streams));
        stream
    }

    async fn record_http_body_stream_failure(&self, request_id: &str, message: String) {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let failures = self.http_body_stream_failures.load();
        let mut next_failures = failures.as_ref().clone();
        next_failures.insert(request_id.to_string(), message);
        self.http_body_stream_failures
            .store(Arc::new(next_failures));
    }

    async fn take_http_body_stream_failure(&self, request_id: &str) -> Option<String> {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let failures = self.http_body_stream_failures.load();
        let error = failures.get(request_id).cloned();
        error.as_ref()?;
        let mut next_failures = failures.as_ref().clone();
        next_failures.remove(request_id);
        self.http_body_stream_failures
            .store(Arc::new(next_failures));
        error
    }
}

/// Schedules HTTP body route removal from synchronous drop paths.
fn spawn_remove_http_body_stream(inner: Arc<Inner>, request_id: String) {
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            inner.remove_http_body_stream(&request_id).await;
        });
    }
}

async fn run_executor_http_request(
    params: HttpRequestParams,
) -> Result<(HttpRequestResponse, Option<ExecutorPendingHttpBodyStream>), JSONRPCErrorError> {
    let method = Method::from_bytes(params.method.as_bytes())
        .map_err(|err| invalid_params(format!("http/request method is invalid: {err}")))?;
    let url = Url::parse(&params.url)
        .map_err(|err| invalid_params(format!("http/request url is invalid: {err}")))?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(invalid_params(format!(
                "http/request only supports http and https URLs, got {scheme}"
            )));
        }
    }

    let headers = build_executor_http_headers(params.headers)?;
    let client = match params.timeout_ms {
        None => reqwest::Client::builder(),
        Some(timeout_ms) => reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms)),
    }
    .build()
    .map_err(|err| internal_error(format!("failed to build http/request client: {err}")))?;

    let mut request = client.request(method, url).headers(headers);
    if let Some(body) = params.body {
        request = request.body(body.into_inner());
    }

    let response = request
        .send()
        .await
        .map_err(|err| internal_error(format!("http/request failed: {err}")))?;
    let status = response.status().as_u16();
    let headers = executor_response_headers(response.headers());

    if params.stream_response {
        return Ok((
            HttpRequestResponse {
                status,
                headers,
                body: Vec::new().into(),
            },
            Some(ExecutorPendingHttpBodyStream {
                request_id: params.request_id,
                response,
            }),
        ));
    }

    let body = response.bytes().await.map_err(|err| {
        internal_error(format!("failed to read http/request response body: {err}"))
    })?;

    Ok((
        HttpRequestResponse {
            status,
            headers,
            body: body.to_vec().into(),
        },
        None,
    ))
}

fn build_executor_http_headers(headers: Vec<HttpHeader>) -> Result<HeaderMap, JSONRPCErrorError> {
    let mut header_map = HeaderMap::new();
    for header in headers {
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|err| invalid_params(format!("http/request header name is invalid: {err}")))?;
        let value = HeaderValue::from_str(&header.value).map_err(|err| {
            invalid_params(format!(
                "http/request header value is invalid for {}: {err}",
                header.name
            ))
        })?;
        header_map.append(name, value);
    }
    Ok(header_map)
}

fn executor_response_headers(headers: &HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some(HttpHeader {
                name: name.as_str().to_string(),
                value: value.to_str().ok()?.to_string(),
            })
        })
        .collect()
}

async fn stream_executor_http_body(
    pending_stream: ExecutorPendingHttpBodyStream,
    notifications: RpcNotificationSender,
) {
    let ExecutorPendingHttpBodyStream {
        request_id,
        response,
    } = pending_stream;
    let mut seq = 1;
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(bytes) => {
                if !send_executor_body_delta(
                    &notifications,
                    HttpRequestBodyDeltaNotification {
                        request_id: request_id.clone(),
                        seq,
                        delta: bytes.to_vec().into(),
                        done: false,
                        error: None,
                    },
                )
                .await
                {
                    return;
                }
                seq += 1;
            }
            Err(err) => {
                let _ = send_executor_body_delta(
                    &notifications,
                    HttpRequestBodyDeltaNotification {
                        request_id,
                        seq,
                        delta: Vec::new().into(),
                        done: true,
                        error: Some(err.to_string()),
                    },
                )
                .await;
                return;
            }
        }
    }

    let _ = send_executor_body_delta(
        &notifications,
        HttpRequestBodyDeltaNotification {
            request_id,
            seq,
            delta: Vec::new().into(),
            done: true,
            error: None,
        },
    )
    .await;
}

async fn send_executor_body_delta(
    notifications: &RpcNotificationSender,
    delta: HttpRequestBodyDeltaNotification,
) -> bool {
    notifications
        .notify(HTTP_REQUEST_BODY_DELTA_METHOD, &delta)
        .await
        .is_ok()
}
