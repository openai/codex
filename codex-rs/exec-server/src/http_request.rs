//! Executor-owned HTTP request runner.
//!
//! The remote MCP path uses this module to make HTTP requests from the same
//! environment that owns stdio process execution. Buffered responses return as a
//! normal JSON-RPC result, while streaming responses split headers and body
//! frames so Streamable HTTP clients can process SSE data before EOF.

use std::time::Duration;

use codex_app_server_protocol::JSONRPCErrorError;
use futures::StreamExt;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;

use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HttpHeader;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

/// Default timeout for executor HTTP requests when the protocol omits one.
const DEFAULT_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct PendingHttpBodyStream {
    pub(crate) request_id: String,
    response: reqwest::Response,
}

/// Runs one executor HTTP request and returns the JSON-RPC response payload.
///
/// When `stream_response` is set, the returned body is empty and the response
/// bytes are emitted as ordered `http/request/bodyDelta` notifications.
pub(crate) async fn run_http_request(
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

    let request_id = if params.stream_response {
        Some(params.request_id.clone().ok_or_else(|| {
            invalid_params("http/request streamResponse requires requestId".to_string())
        })?)
    } else {
        None
    };
    let headers = build_headers(params.headers)?;
    let client = {
        let client_builder = match params.timeout_ms {
            None => reqwest::Client::builder().timeout(DEFAULT_HTTP_REQUEST_TIMEOUT),
            Some(None) => reqwest::Client::builder(),
            Some(Some(timeout_ms)) => {
                reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms))
            }
        };
        client_builder.build()
    }
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
    let headers = response_headers(response.headers());

    if let Some(request_id) = request_id {
        return Ok((
            HttpRequestResponse {
                status,
                headers,
                body: Vec::new().into(),
            },
            Some(PendingHttpBodyStream {
                request_id,
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

/// Converts protocol headers into a reqwest header map while preserving repeats.
fn build_headers(headers: Vec<HttpHeader>) -> Result<HeaderMap, JSONRPCErrorError> {
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

/// Converts response headers back into protocol headers with UTF-8 values only.
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

/// Bridges one reqwest byte stream to ordered JSON-RPC notifications.
pub(crate) async fn stream_http_body(
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

/// Sends one streamed response-body notification.
///
/// Returns `false` when the JSON-RPC connection has closed, letting the stream
/// task stop without treating disconnects as executor errors.
async fn send_body_delta(
    notifications: &RpcNotificationSender,
    delta: HttpRequestBodyDeltaNotification,
) -> bool {
    notifications
        .notify(HTTP_REQUEST_BODY_DELTA_METHOD, &delta)
        .await
        .is_ok()
}
