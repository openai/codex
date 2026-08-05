use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use http::HeaderMap;
use http::header::AUTHORIZATION;
use oauth2::HttpRequest;
use oauth2::HttpResponse;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthHttpClientError;
use rmcp::transport::auth::OAuthHttpClientFuture;
use rmcp::transport::auth::OAuthHttpRedirectPolicy;
use rmcp::transport::auth::OAuthHttpRequest;

use crate::auth_status::OAuthDiscoveryTimeout;
use crate::http_client_adapter::StreamableHttpRedirectMode;

const MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
static NEXT_OAUTH_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
enum OAuthHttpClientAdapterError {
    #[error("unsupported OAuth HTTP redirect policy")]
    UnsupportedRedirectPolicy,
    #[error("OAuth HTTP response body exceeds {maximum_bytes} bytes")]
    ResponseBodyTooLarge { maximum_bytes: usize },
}

fn oauth_http_client_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> OAuthHttpClientError {
    Box::new(error)
}

#[derive(Clone)]
pub(crate) struct OAuthHttpClientAdapter {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
    timeout: OAuthDiscoveryTimeout,
    has_configured_headers: bool,
    redirect_mode: StreamableHttpRedirectMode,
}

impl OAuthHttpClientAdapter {
    #[cfg(test)]
    pub(crate) fn new(http_client: Arc<dyn HttpClient>, default_headers: HeaderMap) -> Self {
        Self {
            http_client,
            default_headers,
            timeout: OAuthDiscoveryTimeout::Requested,
            has_configured_headers: false,
            redirect_mode: StreamableHttpRedirectMode::Legacy,
        }
    }

    pub(crate) fn new_with_redirect_mode(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        has_configured_headers: bool,
        redirect_mode: StreamableHttpRedirectMode,
    ) -> Self {
        Self {
            http_client,
            default_headers,
            timeout: OAuthDiscoveryTimeout::Requested,
            has_configured_headers,
            redirect_mode,
        }
    }

    pub(crate) fn new_with_max_timeout_and_redirect_mode(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        max_timeout: Duration,
        has_configured_headers: bool,
        redirect_mode: StreamableHttpRedirectMode,
    ) -> Self {
        Self {
            http_client,
            default_headers,
            timeout: OAuthDiscoveryTimeout::Capped(max_timeout),
            has_configured_headers,
            redirect_mode,
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
                return Err(oauth_http_client_error(
                    OAuthHttpClientAdapterError::UnsupportedRedirectPolicy,
                ));
            }
        };
        let (parts, body) = request.into_parts();
        let mut headers = self.default_headers.clone();
        for name in parts.headers.keys() {
            headers.remove(name);
        }
        headers.extend(parts.headers);
        let redirect_policy = oauth_redirect_policy(
            self.redirect_mode,
            &headers,
            self.has_configured_headers,
            redirect_policy,
        );

        let headers = headers
            .iter()
            .map(|(name, value)| {
                Ok(HttpHeader {
                    name: name.as_str().to_string(),
                    value: value.to_str().map_err(oauth_http_client_error)?.to_string(),
                })
            })
            .collect::<Result<Vec<_>, OAuthHttpClientError>>()?;
        let timeout = match self.timeout {
            OAuthDiscoveryTimeout::Requested => timeout,
            OAuthDiscoveryTimeout::Capped(max_timeout) => {
                Some(timeout.map_or(max_timeout, |timeout| timeout.min(max_timeout)))
            }
        };
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
            .map_err(oauth_http_client_error)?;
        let mut body = Vec::new();
        while let Some(chunk) = body_stream.recv().await.map_err(oauth_http_client_error)? {
            if chunk.len() > MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES - body.len() {
                return Err(oauth_http_client_error(
                    OAuthHttpClientAdapterError::ResponseBodyTooLarge {
                        maximum_bytes: MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES,
                    },
                ));
            }
            body.extend_from_slice(&chunk);
        }
        let mut builder = oauth2::http::Response::builder().status(response.status);
        for header in response.headers {
            builder = builder.header(header.name, header.value);
        }
        builder.body(body).map_err(oauth_http_client_error)
    }
}

fn oauth_redirect_policy(
    mode: StreamableHttpRedirectMode,
    headers: &HeaderMap,
    has_configured_headers: bool,
    requested_policy: HttpRedirectPolicy,
) -> HttpRedirectPolicy {
    if mode == StreamableHttpRedirectMode::AgentPluginV1
        && (has_configured_headers || headers.contains_key(AUTHORIZATION))
    {
        HttpRedirectPolicy::Stop
    } else {
        requested_policy
    }
}

impl OAuthHttpClient for OAuthHttpClientAdapter {
    fn execute(&self, request: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        Box::pin(self.execute_request(request.request, request.redirect_policy, request.timeout))
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn agent_plugin_oauth_stops_only_for_sensitive_headers() {
        assert_eq!(
            policy(
                StreamableHttpRedirectMode::AgentPluginV1,
                /*has_configured_headers*/ true,
                /*has_authorization*/ false,
            ),
            HttpRedirectPolicy::Stop
        );
        assert_eq!(
            policy(
                StreamableHttpRedirectMode::AgentPluginV1,
                /*has_configured_headers*/ false,
                /*has_authorization*/ true,
            ),
            HttpRedirectPolicy::Stop
        );
        assert_eq!(
            policy(
                StreamableHttpRedirectMode::AgentPluginV1,
                /*has_configured_headers*/ false,
                /*has_authorization*/ false,
            ),
            HttpRedirectPolicy::Follow
        );
        assert_eq!(
            policy(
                StreamableHttpRedirectMode::Legacy,
                /*has_configured_headers*/ true,
                /*has_authorization*/ true,
            ),
            HttpRedirectPolicy::Follow
        );
    }

    fn policy(
        mode: StreamableHttpRedirectMode,
        has_configured_headers: bool,
        has_authorization: bool,
    ) -> HttpRedirectPolicy {
        let mut headers = HeaderMap::new();
        if has_authorization {
            headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        }
        oauth_redirect_policy(
            mode,
            &headers,
            has_configured_headers,
            HttpRedirectPolicy::Follow,
        )
    }
}
