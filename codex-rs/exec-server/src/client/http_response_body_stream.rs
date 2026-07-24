//! Shared HTTP response-body stream plumbing for local and remote execution.
//!
//! This module owns the byte-stream type exposed by the `HttpClient`
//! capability plus the remote-side routing table used to turn
//! `http/request/bodyDelta` notifications back into per-request streams.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use bytes::Bytes;
use codex_http_client::HttpError;
use codex_http_client::HttpResponse;
use futures::StreamExt;
use serde_json::Value;
use serde_json::from_value;
use tokio::runtime::Handle;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::debug;

use crate::client::ExecServerError;
use crate::client::Inner;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::MAX_HTTP_BODY_DELTA_BYTES;
use crate::rpc::RpcNotificationSender;

pub(crate) const MAX_QUEUED_HTTP_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENCODED_HTTP_BODY_DELTA_BYTES: usize = MAX_HTTP_BODY_DELTA_BYTES.div_ceil(3) * 4;

pub(crate) struct QueuedHttpBodyDelta {
    notification: HttpRequestBodyDeltaNotification,
    _byte_permit: Option<OwnedSemaphorePermit>,
}

impl QueuedHttpBodyDelta {
    pub(crate) fn new(
        notification: HttpRequestBodyDeltaNotification,
        byte_permit: Option<OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            notification,
            _byte_permit: byte_permit,
        }
    }
}

pub(super) struct HttpBodyStreamRegistration {
    inner: Arc<Inner>,
    request_id: String,
    active: bool,
}

enum HttpResponseBodyStreamInner {
    Local {
        body: Pin<Box<dyn futures::Stream<Item = Result<Bytes, HttpError>> + Send>>,
    },
    Remote {
        inner: Arc<Inner>,
        request_id: String,
        next_seq: u64,
        rx: mpsc::Receiver<QueuedHttpBodyDelta>,
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

impl HttpResponseBodyStream {
    pub(super) fn local(response: HttpResponse) -> Self {
        Self {
            inner: HttpResponseBodyStreamInner::Local {
                body: Box::pin(response.bytes_stream()),
            },
        }
    }

    pub(super) fn remote(
        inner: Arc<Inner>,
        request_id: String,
        rx: mpsc::Receiver<QueuedHttpBodyDelta>,
    ) -> Self {
        Self {
            inner: HttpResponseBodyStreamInner::Remote {
                inner,
                request_id,
                next_seq: 1,
                rx,
                pending_eof: false,
                closed: false,
            },
        }
    }

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

                let Some(QueuedHttpBodyDelta {
                    notification: delta,
                    ..
                }) = rx.recv().await
                else {
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

impl HttpBodyStreamRegistration {
    pub(super) fn new(inner: Arc<Inner>, request_id: String) -> Self {
        Self {
            inner,
            request_id,
            active: true,
        }
    }

    pub(super) fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for HttpBodyStreamRegistration {
    /// Removes the route if the stream request future is cancelled before headers return.
    fn drop(&mut self) {
        if self.active {
            spawn_remove_http_body_stream(Arc::clone(&self.inner), self.request_id.clone());
        }
    }
}

async fn finish_remote_stream(inner: &Arc<Inner>, request_id: &str, closed: &mut bool) {
    if *closed {
        return;
    }
    *closed = true;
    inner.remove_http_body_stream(request_id).await;
}

/// Schedules HTTP body route removal from synchronous drop paths.
fn spawn_remove_http_body_stream(inner: Arc<Inner>, request_id: String) {
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            inner.remove_http_body_stream(&request_id).await;
        });
    }
}

pub(super) async fn send_body_delta(
    notifications: &RpcNotificationSender,
    delta: HttpRequestBodyDeltaNotification,
) -> bool {
    notifications
        .notify(HTTP_REQUEST_BODY_DELTA_METHOD, &delta)
        .await
        .is_ok()
}

impl Inner {
    /// Routes one streamed HTTP body notification into its request-local receiver.
    pub(crate) async fn handle_http_body_delta_notification(
        &self,
        params: Option<Value>,
    ) -> Result<(), ExecServerError> {
        let params = params.unwrap_or(Value::Null);
        if params
            .get("deltaBase64")
            .and_then(Value::as_str)
            .is_some_and(|delta| delta.len() > MAX_ENCODED_HTTP_BODY_DELTA_BYTES)
        {
            return Err(ExecServerError::Protocol(format!(
                "http response body delta exceeds {MAX_HTTP_BODY_DELTA_BYTES} bytes"
            )));
        }
        let params: HttpRequestBodyDeltaNotification = from_value(params)?;
        if params.delta.0.len() > MAX_HTTP_BODY_DELTA_BYTES {
            return Err(ExecServerError::Protocol(format!(
                "http response body delta exceeds {MAX_HTTP_BODY_DELTA_BYTES} bytes"
            )));
        }
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
            let queued_bytes = params
                .delta
                .0
                .len()
                .saturating_add(params.error.as_deref().map_or(0, str::len));
            let byte_permit = if queued_bytes == 0 {
                None
            } else {
                u32::try_from(queued_bytes).ok().and_then(|queued_bytes| {
                    Arc::clone(&self.http_body_stream_byte_budget)
                        .try_acquire_many_owned(queued_bytes)
                        .ok()
                })
            };
            if queued_bytes > 0 && byte_permit.is_none() {
                self.record_http_body_stream_failure(
                    &request_id,
                    format!("queued body deltas exceed {MAX_QUEUED_HTTP_BODY_BYTES} bytes"),
                )
                .await;
                self.remove_http_body_stream(&request_id).await;
                debug!(
                    "closing http response stream `{request_id}` after exhausting the queued byte budget"
                );
                return Ok(());
            }
            match tx.try_send(QueuedHttpBodyDelta::new(params, byte_permit)) {
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
    pub(crate) async fn fail_all_http_body_streams(&self, message: String) {
        let _streams_write_guard = self.http_body_streams_write_lock.lock().await;
        let streams = self.http_body_streams.load();
        let streams = streams.as_ref().clone();
        self.http_body_streams.store(Arc::new(HashMap::new()));
        for (request_id, tx) in streams {
            // Failure notifications must wake every stream even when no
            // byte-budget permits remain.
            if tx
                .try_send(QueuedHttpBodyDelta::new(
                    HttpRequestBodyDeltaNotification {
                        request_id: request_id.clone(),
                        seq: 1,
                        delta: Vec::new().into(),
                        done: true,
                        error: Some(message.clone()),
                    },
                    /*byte_permit*/ None,
                ))
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
    pub(super) fn next_http_body_stream_request_id(&self) -> String {
        let id = self
            .http_body_stream_next_id
            .fetch_add(1, Ordering::Relaxed);
        format!("http-{id}")
    }

    /// Registers a request id before issuing a streaming HTTP call.
    pub(super) async fn insert_http_body_stream(
        &self,
        request_id: String,
        tx: mpsc::Sender<QueuedHttpBodyDelta>,
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
    pub(super) async fn remove_http_body_stream(
        &self,
        request_id: &str,
    ) -> Option<mpsc::Sender<QueuedHttpBodyDelta>> {
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
