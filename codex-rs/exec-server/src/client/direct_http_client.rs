//! Direct HTTP implementation for the `HttpClient` capability.
//!
//! This code runs where the actual network request should originate:
//! - in a local environment, that means the orchestrator process
//! - in a remote environment, that means the remote runtime after the
//!   orchestrator has forwarded `http/request` over JSON-RPC

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

use super::HttpResponseBodyStream;
use super::body_stream::send_body_delta;
use crate::HttpClient;
use crate::client::ExecServerError;
use crate::protocol::HttpHeader;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

/// Direct HTTP implementation for whichever process is responsible for issuing
/// the real network request.
#[derive(Clone, Default)]
pub(crate) struct DirectHttpClient;

/// Local streaming response state held between the initial HTTP response and
/// downstream body-delta forwarding.
pub(crate) struct PendingDirectHttpBodyStream {
    pub(crate) request_id: String,
    pub(crate) response: reqwest::Response,
}

/// Validates `http/request` parameters and runs the actual `reqwest` call used
/// by the exec-server route and the local [`HttpClient`] backend.
pub(crate) struct DirectHttpRequestRunner {
    client: reqwest::Client,
}

impl DirectHttpClient {
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

impl HttpClient for DirectHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        async move {
            let runner = DirectHttpRequestRunner::new(params.timeout_ms)
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
            let runner = DirectHttpRequestRunner::new(params.timeout_ms)
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

impl DirectHttpRequestRunner {
    pub(crate) fn new(timeout_ms: Option<u64>) -> Result<Self, JSONRPCErrorError> {
        let client = DirectHttpClient::build_client(timeout_ms)
            .map_err(|error| internal_error(error.to_string()))?;
        Ok(Self { client })
    }

    pub(crate) async fn run(
        &self,
        params: HttpRequestParams,
    ) -> Result<(HttpRequestResponse, Option<PendingDirectHttpBodyStream>), JSONRPCErrorError> {
        let method = Method::from_bytes(params.method.as_bytes())
            .map_err(|error| invalid_params(format!("http/request method is invalid: {error}")))?;
        let url = Url::parse(&params.url)
            .map_err(|error| invalid_params(format!("http/request url is invalid: {error}")))?;
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
            .map_err(|error| internal_error(format!("http/request failed: {error}")))?;
        let status = response.status().as_u16();
        let headers = Self::response_headers(response.headers());

        if params.stream_response {
            return Ok((
                HttpRequestResponse {
                    status,
                    headers,
                    body: Vec::new().into(),
                },
                Some(PendingDirectHttpBodyStream {
                    request_id: params.request_id,
                    response,
                }),
            ));
        }

        let body = response.bytes().await.map_err(|error| {
            internal_error(format!(
                "failed to read http/request response body: {error}"
            ))
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

    pub(crate) async fn stream_body(
        pending_stream: PendingDirectHttpBodyStream,
        notifications: RpcNotificationSender,
    ) {
        let PendingDirectHttpBodyStream {
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
                Err(error) => {
                    let _ = send_body_delta(
                        &notifications,
                        HttpRequestBodyDeltaNotification {
                            request_id,
                            seq,
                            delta: Vec::new().into(),
                            done: true,
                            error: Some(error.to_string()),
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

    fn build_headers(headers: Vec<HttpHeader>) -> Result<HeaderMap, JSONRPCErrorError> {
        let mut header_map = HeaderMap::new();
        for header in headers {
            let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|error| {
                invalid_params(format!("http/request header name is invalid: {error}"))
            })?;
            let value = HeaderValue::from_str(&header.value).map_err(|error| {
                invalid_params(format!(
                    "http/request header value is invalid for {}: {error}",
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
}
