//! Restricts MCP HTTP redirects to the configured server's origin.
//!
//! MCP requests can carry sensitive headers and tool-call bodies. Following a
//! cross-origin redirect would send them to another server or an internal
//! service, so this transport validates each redirect before following it.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use futures::FutureExt;
use futures::future::BoxFuture;
use http::HeaderValue;
use http::StatusCode;
use url::Url;

const MAX_REDIRECTS: usize = 10;

pub(crate) struct SameOriginRedirectHttpClient {
    inner: Arc<dyn HttpClient>,
}

enum RedirectResponse {
    Buffered(HttpRequestResponse),
    Streaming(HttpRequestResponse, HttpResponseBodyStream),
}

impl SameOriginRedirectHttpClient {
    pub(crate) fn new(inner: Arc<dyn HttpClient>) -> Self {
        Self { inner }
    }

    async fn execute(
        &self,
        mut params: HttpRequestParams,
    ) -> Result<RedirectResponse, ExecServerError> {
        // Inspect each Location ourselves before the underlying client can send
        // sensitive request data to the redirect destination.
        params.redirect_policy = HttpRedirectPolicy::Stop;
        let mut current_url = Url::parse(&params.url)
            .map_err(|error| ExecServerError::HttpRequest(error.to_string()))?;
        let original_origin = current_url.origin();
        // Redirect hops share the original timeout instead of restarting it.
        let deadline = params
            .timeout_ms
            .map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms));
        let mut redirects = 0;

        loop {
            if let Some(deadline) = deadline {
                let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                    return Err(ExecServerError::HttpRequest(
                        "MCP HTTP request timed out".to_string(),
                    ));
                };
                params.timeout_ms = Some(
                    u64::try_from(remaining.as_millis())
                        .unwrap_or(u64::MAX)
                        .max(1),
                );
            }

            let result = if params.stream_response {
                let (response, stream) = self.inner.http_request_stream(params.clone()).await?;
                RedirectResponse::Streaming(response, stream)
            } else {
                RedirectResponse::Buffered(self.inner.http_request(params.clone()).await?)
            };
            let response = match &result {
                RedirectResponse::Buffered(response) | RedirectResponse::Streaming(response, _) => {
                    response
                }
            };
            let status = StatusCode::from_u16(response.status).ok();
            if !matches!(
                status,
                Some(
                    StatusCode::MOVED_PERMANENTLY
                        | StatusCode::FOUND
                        | StatusCode::SEE_OTHER
                        | StatusCode::TEMPORARY_REDIRECT
                        | StatusCode::PERMANENT_REDIRECT
                )
            ) {
                return Ok(result);
            }

            let Some(location) = response
                .headers
                .iter()
                .find(|header| header.name.eq_ignore_ascii_case("location"))
                .map(|header| header.value.as_str())
            else {
                return Ok(result);
            };
            let next_url = current_url
                .join(location)
                .map_err(|error| ExecServerError::HttpRequest(error.to_string()))?;
            // Compare every hop with the configured origin so redirect chains
            // cannot escape it through a later cross-origin Location.
            if next_url.origin() != original_origin {
                return Err(ExecServerError::HttpRequest(
                    "MCP HTTP redirect to a different origin is not allowed".to_string(),
                ));
            }
            // A hostname can resolve to a different address on the next HTTP
            // connection. HTTPS authenticates the hostname; localhost and IP
            // literals do not depend on attacker-controlled DNS.
            if next_url.scheme() == "http"
                && matches!(next_url.host(), Some(url::Host::Domain(host)) if host != "localhost")
            {
                return Err(ExecServerError::HttpRequest(
                    "MCP HTTP redirects for non-loopback hostnames require HTTPS".to_string(),
                ));
            }
            if redirects >= MAX_REDIRECTS {
                return Err(ExecServerError::HttpRequest(
                    "MCP HTTP request exceeded the redirect limit".to_string(),
                ));
            }

            // Preserve normal HTTP behavior: 307/308 retain the method and body;
            // 303 and legacy 301/302 POST redirects switch to a bodyless GET.
            let drop_body = matches!(
                status,
                Some(StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND)
            ) && params.method.eq_ignore_ascii_case("POST")
                || status == Some(StatusCode::SEE_OTHER);
            if drop_body {
                if !params.method.eq_ignore_ascii_case("HEAD") {
                    params.method = "GET".to_string();
                }
                params.body = None;
                params.headers.retain(|header| {
                    ![
                        "content-type",
                        "content-length",
                        "content-encoding",
                        "transfer-encoding",
                    ]
                    .iter()
                    .any(|name| header.name.eq_ignore_ascii_case(name))
                });
            }

            // HTTPS keeps application proxy credentials inside the tunnel to
            // the already-validated origin. Plaintext routes can change across
            // requests, so their proxy credentials must never be replayed.
            params.headers.retain(|header| {
                (current_url.scheme() == "https"
                    || !header.name.eq_ignore_ascii_case("proxy-authorization"))
                    && !header.name.eq_ignore_ascii_case("referer")
            });
            let mut referer = current_url.clone();
            let _ = referer.set_username("");
            let _ = referer.set_password(None);
            referer.set_fragment(None);
            if HeaderValue::from_str(referer.as_str()).is_ok() {
                params.headers.push(HttpHeader {
                    name: "referer".to_string(),
                    value: referer.to_string(),
                });
            }

            params.url = next_url.to_string();
            current_url = next_url;
            redirects += 1;
        }
    }
}

impl HttpClient for SameOriginRedirectHttpClient {
    fn http_request(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        async move {
            params.stream_response = false;
            if params.redirect_policy == HttpRedirectPolicy::Stop {
                return self.inner.http_request(params).await;
            }
            match self.execute(params).await? {
                RedirectResponse::Buffered(response) => Ok(response),
                RedirectResponse::Streaming(_, _) => Err(ExecServerError::Protocol(
                    "MCP buffered HTTP request returned a streamed response".to_string(),
                )),
            }
        }
        .boxed()
    }

    fn http_request_stream(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        async move {
            params.stream_response = true;
            if params.redirect_policy == HttpRedirectPolicy::Stop {
                return self.inner.http_request_stream(params).await;
            }
            match self.execute(params).await? {
                RedirectResponse::Streaming(response, stream) => Ok((response, stream)),
                RedirectResponse::Buffered(_) => Err(ExecServerError::Protocol(
                    "MCP streamed HTTP request returned a buffered response".to_string(),
                )),
            }
        }
        .boxed()
    }
}

#[cfg(test)]
#[path = "http_client_redirect_tests.rs"]
mod tests;
