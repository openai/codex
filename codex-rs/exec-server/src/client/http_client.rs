use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use bytes::Bytes;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_client::build_reqwest_client_with_custom_ca;
use futures::FutureExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use serde_json::Value;
use serde_json::from_value;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::debug;

use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use crate::HttpClient;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpHeader;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

/// Maximum queued body frames per streamed HTTP response.
const HTTP_BODY_DELTA_CHANNEL_CAPACITY: usize = 256;

pub(crate) struct PendingHttpBodyStream {
    pub(crate) request_id: String,
    response: reqwest::Response,
}

pub(crate) struct HttpRequestRunner {
    client: reqwest::Client,
}

#[derive(Clone, Default)]
pub(crate) struct LocalHttpClient;

enum HttpResponseBodyStreamInner {
    Local {
        body: Pin<Box<dyn futures::Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    },
    Remote {
        inner: Arc<Inner>,
        request_id: String,
        next_seq: u64,
        rx: mpsc::Receiver<HttpRequestBodyDeltaNotification>,
        pending_eof: bool,
        closed: bool,
    },
}

/// Request-scoped stream of body chunks for an HTTP response.
///
/// The initial `http/request` call returns status and headers. This stream then
/// receives the ordered `http/request/bodyDelta` notifications for that request
/// id until EOF or a terminal error.
pub struct HttpResponseBodyStream {
    inner: HttpResponseBodyStreamInner,
}

impl ExecServerClient {
    /// Performs an HTTP request and buffers the response body.
    pub async fn http_request(
        &self,
        mut params: HttpRequestParams,
    ) -> Result<HttpRequestResponse, ExecServerError> {
        params.stream_response = false;
        self.call(HTTP_REQUEST_METHOD, &params).await
    }

    /// Performs an HTTP request and returns a body stream.
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
                inner: HttpResponseBodyStreamInner::Remote {
                    inner: Arc::clone(&self.inner),
                    request_id,
                    next_seq: 1,
                    rx,
                    pending_eof: false,
                    closed: false,
                },
            },
        ))
    }
}

async fn finish_remote_stream(inner: &Arc<Inner>, request_id: &str, closed: &mut bool) {
    if *closed {
        return;
    }
    *closed = true;
    inner.remove_http_body_stream(request_id).await;
}

impl HttpResponseBodyStream {
    /// Receives the next response-body chunk.
    ///
    /// Returns `Ok(None)` at EOF and converts sequence gaps or stream-side
    /// stream errors into protocol errors.
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>, ExecServerError> {
        match &mut self.inner {
            HttpResponseBodyStreamInner::Local { body } => match body.next().await {
                Some(chunk) => match chunk {
                    Ok(bytes) => Ok(Some(bytes.to_vec())),
                    Err(error) => Err(ExecServerError::HttpRequest(error.to_string())),
                },
                None => Ok(None),
            },
            HttpResponseBodyStreamInner::Remote {
                inner,
                request_id,
                next_seq,
                rx,
                pending_eof,
                closed,
            } => {
                if *pending_eof {
                    *pending_eof = false;
                    finish_remote_stream(inner, request_id, closed).await;
                    return Ok(None);
                }

                let Some(delta) = rx.recv().await else {
                    finish_remote_stream(inner, request_id, closed).await;
                    if let Some(error) = inner.take_http_body_stream_failure(request_id).await {
                        return Err(ExecServerError::Protocol(format!(
                            "http response stream `{request_id}` failed: {error}",
                        )));
                    }
                    return Ok(None);
                };
                if delta.seq != *next_seq {
                    finish_remote_stream(inner, request_id, closed).await;
                    return Err(ExecServerError::Protocol(format!(
                        "http response stream `{request_id}` received seq {}, expected {}",
                        delta.seq, *next_seq
                    )));
                }
                *next_seq += 1;
                let chunk = delta.delta.into_inner();

                if let Some(error) = delta.error {
                    finish_remote_stream(inner, request_id, closed).await;
                    return Err(ExecServerError::Protocol(format!(
                        "http response stream `{request_id}` failed: {error}",
                    )));
                }
                if delta.done {
                    finish_remote_stream(inner, request_id, closed).await;
                    if chunk.is_empty() {
                        return Ok(None);
                    }
                    *pending_eof = true;
                }
                Ok(Some(chunk))
            }
        }
    }
}

impl Drop for HttpResponseBodyStream {
    /// Schedules stream-route removal if the consumer drops before EOF.
    fn drop(&mut self) {
        if let HttpResponseBodyStreamInner::Remote {
            inner,
            request_id,
            closed,
            ..
        } = &mut self.inner
        {
            if *closed {
                return;
            }
            *closed = true;
            spawn_remove_http_body_stream(Arc::clone(inner), request_id.clone());
        }
    }
}

impl LocalHttpClient {
    fn build_client(timeout_ms: Option<u64>) -> Result<reqwest::Client, ExecServerError> {
        let builder = match timeout_ms {
            None => reqwest::Client::builder(),
            Some(timeout_ms) => {
                reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms))
            }
        };
        build_reqwest_client_with_custom_ca(builder)
            .map_err(|error| ExecServerError::HttpRequest(error.to_string()))
    }
}

impl HttpClient for LocalHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        async move {
            let runner = HttpRequestRunner::new(params.timeout_ms)
                .map_err(|error| ExecServerError::HttpRequest(error.message))?;
            let (response, _) = runner
                .run(HttpRequestParams {
                    stream_response: false,
                    ..params
                })
                .await
                .map_err(|error| ExecServerError::HttpRequest(error.message))?;
            Ok(response)
        }
        .boxed()
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        async move {
            let runner = HttpRequestRunner::new(params.timeout_ms)
                .map_err(|error| ExecServerError::HttpRequest(error.message))?;
            let (response, pending_stream) = runner
                .run(HttpRequestParams {
                    stream_response: true,
                    ..params
                })
                .await
                .map_err(|error| ExecServerError::HttpRequest(error.message))?;
            let pending_stream = pending_stream.ok_or_else(|| {
                ExecServerError::Protocol(
                    "http request stream did not return a response body stream".to_string(),
                )
            })?;
            Ok((
                response,
                HttpResponseBodyStream {
                    inner: HttpResponseBodyStreamInner::Local {
                        body: Box::pin(pending_stream.response.bytes_stream()),
                    },
                },
            ))
        }
        .boxed()
    }
}

impl HttpRequestRunner {
    pub(crate) fn new(timeout_ms: Option<u64>) -> Result<Self, JSONRPCErrorError> {
        let client = LocalHttpClient::build_client(timeout_ms)
            .map_err(|error| internal_error(error.to_string()))?;
        Ok(Self { client })
    }

    pub(crate) async fn run(
        &self,
        params: HttpRequestParams,
    ) -> Result<(HttpRequestResponse, Option<PendingHttpBodyStream>), JSONRPCErrorError> {
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

        let headers = Self::build_headers(params.headers)?;
        let mut request = self.client.request(method, url).headers(headers);
        if let Some(body) = params.body {
            request = request.body(body.into_inner());
        }

        let response = request
            .send()
            .await
            .map_err(|err| internal_error(format!("http/request failed: {err}")))?;
        let status = response.status().as_u16();
        let headers = Self::response_headers(response.headers());

        if params.stream_response {
            return Ok((
                HttpRequestResponse {
                    status,
                    headers,
                    body: Vec::new().into(),
                },
                Some(PendingHttpBodyStream {
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

    fn build_headers(headers: Vec<HttpHeader>) -> Result<HeaderMap, JSONRPCErrorError> {
        let mut header_map = HeaderMap::new();
        for header in headers {
            let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|err| {
                invalid_params(format!("http/request header name is invalid: {err}"))
            })?;
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

    fn response_headers(headers: &HeaderMap) -> Vec<HttpHeader> {
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

    pub(crate) async fn stream_body(
        pending_stream: PendingHttpBodyStream,
        notifications: RpcNotificationSender,
    ) {
        let PendingHttpBodyStream {
            request_id,
            response,
        } = pending_stream;
        let mut seq = 1;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            match chunk {
                Ok(bytes) => {
                    if !send_body_delta(
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
                    let _ = send_body_delta(
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

        let _ = send_body_delta(
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

/// Schedules HTTP body route removal from synchronous drop paths.
fn spawn_remove_http_body_stream(inner: Arc<Inner>, request_id: String) {
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            inner.remove_http_body_stream(&request_id).await;
        });
    }
}

async fn send_body_delta(
    notifications: &RpcNotificationSender,
    delta: HttpRequestBodyDeltaNotification,
) -> bool {
    notifications
        .notify(HTTP_REQUEST_BODY_DELTA_METHOD, &delta)
        .await
        .is_ok()
}
