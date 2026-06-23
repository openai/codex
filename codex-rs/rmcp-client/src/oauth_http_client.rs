use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use oauth2::HttpRequest;
use oauth2::HttpResponse;
use reqwest::header::HeaderMap;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthHttpClientError;
use rmcp::transport::auth::OAuthHttpClientFuture;
use rmcp::transport::auth::OAuthHttpRedirectPolicy;
use rmcp::transport::auth::OAuthHttpRequest;

const MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
static NEXT_OAUTH_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct OAuthHttpClientAdapter {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
}

impl OAuthHttpClientAdapter {
    pub(crate) fn new(http_client: Arc<dyn HttpClient>, default_headers: HeaderMap) -> Self {
        Self {
            http_client,
            default_headers,
        }
    }

    async fn execute_request(
        &self,
        request: HttpRequest,
        redirect_policy: OAuthHttpRedirectPolicy,
        timeout: Option<Duration>,
    ) -> Result<HttpResponse, OAuthHttpClientError> {
        let redirect_policy = match redirect_policy {
            OAuthHttpRedirectPolicy::Follow => HttpRedirectPolicy::Follow,
            OAuthHttpRedirectPolicy::Stop => HttpRedirectPolicy::Stop,
            _ => {
                return Err(OAuthHttpClientError::new(
                    "unsupported OAuth HTTP redirect policy",
                ));
            }
        };
        let (parts, body) = request.into_parts();
        let mut headers = self.default_headers.clone();
        for name in parts.headers.keys() {
            headers.remove(name);
        }
        headers.extend(parts.headers);
        let headers = headers
            .iter()
            .map(|(name, value)| {
                Ok(HttpHeader {
                    name: name.as_str().to_string(),
                    value: value
                        .to_str()
                        .map_err(|error| OAuthHttpClientError::new(error.to_string()))?
                        .to_string(),
                })
            })
            .collect::<Result<Vec<_>, OAuthHttpClientError>>()?;
        let timeout_ms = timeout.map(|timeout| {
            u64::try_from(timeout.as_millis())
                .unwrap_or(u64::MAX)
                .max(1)
        });
        let request_id = NEXT_OAUTH_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let (response, mut body_stream) = self
            .http_client
            .http_request_stream(HttpRequestParams {
                method: parts.method.to_string(),
                url: parts.uri.to_string(),
                headers,
                body: (!body.is_empty()).then_some(body.into()),
                timeout_ms,
                redirect_policy,
                request_id: format!("oauth-request-{request_id}"),
                stream_response: true,
            })
            .await
            .map_err(|error| OAuthHttpClientError::new(error.to_string()))?;
        let mut body = Vec::new();
        while let Some(chunk) = body_stream
            .recv()
            .await
            .map_err(|error| OAuthHttpClientError::new(error.to_string()))?
        {
            if chunk.len() > MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES - body.len() {
                return Err(OAuthHttpClientError::new(format!(
                    "OAuth HTTP response body exceeds {MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES} bytes"
                )));
            }
            body.extend_from_slice(&chunk);
        }
        let mut builder = oauth2::http::Response::builder().status(response.status);
        for header in response.headers {
            builder = builder.header(header.name, header.value);
        }
        builder
            .body(body)
            .map_err(|error| OAuthHttpClientError::new(error.to_string()))
    }
}

impl OAuthHttpClient for OAuthHttpClientAdapter {
    fn execute(&self, request: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        Box::pin(self.execute_request(request.request, request.redirect_policy, request.timeout))
    }
}
