//! JSON-RPC-backed `HttpClient` implementation.
//!
//! This code runs in the orchestrator process. It does not issue network
//! requests directly; instead it forwards `http/request` to the remote runtime
//! and then reconstructs streamed bodies from `http/request/bodyDelta`
//! notifications on the shared connection.

use std::sync::Arc;

use futures::FutureExt;
use futures::future::BoxFuture;
use tokio::sync::mpsc;

use super::HttpResponseBodyStream;
use super::body_stream::HttpBodyStreamRegistration;
use crate::HttpClient;
use crate::client::ExecServerClient;
use crate::client::ExecServerError;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;

/// Maximum queued body frames per streamed HTTP response.
const HTTP_BODY_DELTA_CHANNEL_CAPACITY: usize = 256;

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
        let mut registration =
            HttpBodyStreamRegistration::new(Arc::clone(&self.inner), request_id.clone());
        let response = match self.call(HTTP_REQUEST_METHOD, &params).await {
            Ok(response) => response,
            Err(error) => {
                self.inner.remove_http_body_stream(&request_id).await;
                registration.disarm();
                return Err(error);
            }
        };
        registration.disarm();
        Ok((
            response,
            HttpResponseBodyStream::remote(Arc::clone(&self.inner), request_id, rx),
        ))
    }
}

impl HttpClient for ExecServerClient {
    /// Orchestrator-side adapter that forwards buffered HTTP requests to the
    /// remote runtime over the shared JSON-RPC connection.
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        async move { ExecServerClient::http_request(self, params).await }.boxed()
    }

    /// Orchestrator-side adapter that forwards streamed HTTP requests to the
    /// remote runtime and exposes body deltas as a byte stream.
    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        async move { ExecServerClient::http_request_stream(self, params).await }.boxed()
    }
}
