use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use serde_json::Value;
use serde_json::from_value;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::debug;

use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;

/// Maximum queued body frames per streamed executor HTTP response.
const HTTP_BODY_DELTA_CHANNEL_CAPACITY: usize = 256;

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
    failure: Arc<StdMutex<Option<String>>>,
    // Terminal frames can carry a final chunk; return that once, then EOF.
    pending_eof: bool,
    closed: bool,
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
            if let Some(error) = self
                .failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
            {
                self.finish().await;
                return Err(ExecServerError::Protocol(format!(
                    "http response stream `{}` failed: {error}",
                    self.request_id
                )));
            }
            self.finish().await;
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

#[derive(Clone)]
pub(super) struct HttpBodyStreamRoute {
    tx: mpsc::Sender<HttpRequestBodyDeltaNotification>,
    failure: Arc<StdMutex<Option<String>>>,
}

impl HttpBodyStreamRoute {
    /// Creates an active route that delivers body deltas to the caller.
    fn active(
        tx: mpsc::Sender<HttpRequestBodyDeltaNotification>,
        failure: Arc<StdMutex<Option<String>>>,
    ) -> Self {
        Self { tx, failure }
    }

    /// Sends one body delta to the caller.
    fn try_send(
        &self,
        delta: HttpRequestBodyDeltaNotification,
    ) -> Result<(), TrySendError<HttpRequestBodyDeltaNotification>> {
        self.tx.try_send(delta)
    }

    /// Records a terminal failure that the stream sees after queued chunks.
    fn set_failure(&self, message: String) {
        *self.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
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
        params.request_id = None;
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
        params.request_id = Some(request_id.clone());
        let (tx, rx) = mpsc::channel(HTTP_BODY_DELTA_CHANNEL_CAPACITY);
        let failure = Arc::new(StdMutex::new(None));
        self.inner
            .insert_http_body_stream(
                request_id.clone(),
                HttpBodyStreamRoute::active(tx, Arc::clone(&failure)),
            )
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
                failure,
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
        if let Some(route) = self
            .http_body_streams
            .load()
            .get(&params.request_id)
            .cloned()
        {
            let request_id = params.request_id.clone();
            let terminal_delta = params.done || params.error.is_some();
            match route.try_send(params) {
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
                    route.set_failure("body delta channel filled before delivery".to_string());
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
        let streams = self.take_all_http_body_streams().await;
        for (request_id, route) in streams {
            route.set_failure(message.clone());
            let _ = route.try_send(HttpRequestBodyDeltaNotification {
                request_id,
                seq: 1,
                delta: Vec::new().into(),
                done: true,
                error: Some(message.clone()),
            });
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
        route: HttpBodyStreamRoute,
    ) -> Result<(), ExecServerError> {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        if streams.contains_key(&request_id) {
            return Err(ExecServerError::Protocol(format!(
                "http response stream already registered for request {request_id}"
            )));
        }
        let mut next_streams = streams.as_ref().clone();
        next_streams.insert(request_id, route);
        self.http_body_streams.store(Arc::new(next_streams));
        Ok(())
    }

    /// Removes a request id after EOF, terminal error, or request failure.
    async fn remove_http_body_stream(&self, request_id: &str) -> Option<HttpBodyStreamRoute> {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        let stream = streams.get(request_id).cloned();
        stream.as_ref()?;
        let mut next_streams = streams.as_ref().clone();
        next_streams.remove(request_id);
        self.http_body_streams.store(Arc::new(next_streams));
        stream
    }

    /// Removes every streamed HTTP response route after a transport failure.
    async fn take_all_http_body_streams(&self) -> HashMap<String, HttpBodyStreamRoute> {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        let removed_streams = streams.as_ref().clone();
        self.http_body_streams.store(Arc::new(HashMap::new()));
        removed_streams
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
