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
use futures::StreamExt;
use reqwest::Response;
use serde_json::Value;
use serde_json::from_value;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::debug;

use crate::client::ExecServerError;
use crate::client::Inner;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::rpc::RpcNotificationSender;

pub(super) struct HttpBodyStreamRegistration {
    inner: Arc<Inner>,
    request_id: String,
    active: bool,
}

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

impl HttpResponseBodyStream {
    pub(super) fn local(response: Response) -> Self {
        Self {
            inner: HttpResponseBodyStreamInner::Local {
                body: Box::pin(response.bytes_stream()),
            },
        }
    }

    pub(super) fn remote(
        inner: Arc<Inner>,
        request_id: String,
        rx: mpsc::Receiver<HttpRequestBodyDeltaNotification>,
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
    pub(crate) async fn fail_all_http_body_streams(&self, message: String) {
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
    pub(super) async fn remove_http_body_stream(
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
