use std::sync::Arc;
use std::time::Duration;

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
use tokio::sync::mpsc;

use super::ExecServerClient;
use super::ExecServerError;
use crate::HttpClient;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpHeader;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

#[path = "http_body_stream.rs"]
mod body_stream;

use body_stream::HttpBodyStreamRegistration;
pub use body_stream::HttpResponseBodyStream;
use body_stream::send_body_delta;

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
                HttpResponseBodyStream::local(pending_stream.response),
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
