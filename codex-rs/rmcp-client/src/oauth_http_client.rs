use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::StatusCode;
use http::header::AUTHORIZATION;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::LOCATION;
use http::header::TRANSFER_ENCODING;
use http::header::USER_AGENT;
use oauth2::HttpRequest;
use oauth2::HttpResponse;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthHttpClientError;
use rmcp::transport::auth::OAuthHttpClientFuture;
use rmcp::transport::auth::OAuthHttpRedirectPolicy;
use rmcp::transport::auth::OAuthHttpRequest;
use url::Origin;
use url::Url;

use crate::auth_status::OAuthDiscoveryTimeout;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::utils::MCP_USER_AGENT;

const MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
const MAX_OAUTH_HTTP_REDIRECTS: usize = 10;
static NEXT_OAUTH_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
enum OAuthHttpClientAdapterError {
    #[error("unsupported OAuth HTTP redirect policy")]
    UnsupportedRedirectPolicy,
    #[error("OAuth HTTP request timed out")]
    TimedOut,
    #[error("OAuth HTTP request exceeded {MAX_OAUTH_HTTP_REDIRECTS} redirects")]
    TooManyRedirects,
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
    resource_origin: Origin,
    timeout: OAuthDiscoveryTimeout,
    has_configured_headers: bool,
    redirect_mode: StreamableHttpRedirectMode,
}

impl OAuthHttpClientAdapter {
    #[cfg(test)]
    pub(crate) fn new(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        resource_url: &str,
    ) -> Self {
        Self::new_with_redirect_mode(
            http_client,
            default_headers,
            resource_url,
            /*has_configured_headers*/ false,
            StreamableHttpRedirectMode::Legacy,
        )
        .expect("OAuth resource URL should be valid")
    }

    pub(crate) fn new_with_redirect_mode(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        resource_url: &str,
        has_configured_headers: bool,
        redirect_mode: StreamableHttpRedirectMode,
    ) -> Result<Self, url::ParseError> {
        Ok(Self {
            http_client,
            default_headers,
            resource_origin: Url::parse(resource_url)?.origin(),
            timeout: OAuthDiscoveryTimeout::Requested,
            has_configured_headers,
            redirect_mode,
        })
    }

    pub(crate) fn new_with_max_timeout_and_redirect_mode(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        resource_url: &str,
        max_timeout: Duration,
        has_configured_headers: bool,
        redirect_mode: StreamableHttpRedirectMode,
    ) -> Result<Self, url::ParseError> {
        Ok(Self {
            http_client,
            default_headers,
            resource_origin: Url::parse(resource_url)?.origin(),
            timeout: OAuthDiscoveryTimeout::Capped(max_timeout),
            has_configured_headers,
            redirect_mode,
        })
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
        let mut request_url =
            Url::parse(&parts.uri.to_string()).map_err(oauth_http_client_error)?;
        let is_resource_origin = request_url.origin() == self.resource_origin;
        let mut headers = if is_resource_origin {
            self.default_headers.clone()
        } else {
            HeaderMap::new()
        };
        for name in parts.headers.keys() {
            headers.remove(name);
        }
        let has_resource_only_headers = is_resource_origin
            && headers.iter().any(|(name, value)| {
                name != USER_AGENT || value != HeaderValue::from_static(MCP_USER_AGENT)
            });
        headers.extend(parts.headers);
        if !is_resource_origin {
            headers.insert(USER_AGENT, HeaderValue::from_static(MCP_USER_AGENT));
        }
        let redirect_policy = oauth_redirect_policy(
            self.redirect_mode,
            &headers,
            self.has_configured_headers,
            redirect_policy,
        );
        // The executor can only follow every redirect or none, so replay credentialed
        // requests ourselves after checking that each destination stays on the resource origin.
        let follow_same_origin_redirects =
            has_resource_only_headers && redirect_policy == HttpRedirectPolicy::Follow;

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
        let deadline = timeout.map(|timeout| Instant::now() + timeout);
        let mut params = HttpRequestParams {
            method: parts.method.to_string(),
            url: parts.uri.to_string(),
            headers,
            body: (!body.is_empty()).then_some(body.into()),
            timeout_ms,
            redirect_policy: if follow_same_origin_redirects {
                HttpRedirectPolicy::Stop
            } else {
                redirect_policy
            },
            request_id: String::new(),
            stream_response: true,
        };
        let mut redirects = 0;
        let (response, body) = loop {
            let request_id = NEXT_OAUTH_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
            params.request_id = format!("oauth-request-{request_id}");
            let (response, mut body_stream) = self
                .http_client
                .http_request_stream(params.clone())
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
            let Ok(status) = StatusCode::from_u16(response.status) else {
                break (response, body);
            };
            if !follow_same_origin_redirects
                || !matches!(
                    status,
                    StatusCode::MOVED_PERMANENTLY
                        | StatusCode::FOUND
                        | StatusCode::SEE_OTHER
                        | StatusCode::TEMPORARY_REDIRECT
                        | StatusCode::PERMANENT_REDIRECT
                )
            {
                break (response, body);
            }
            let Some(next_url) = response
                .headers
                .iter()
                .find(|header| header.name.eq_ignore_ascii_case(LOCATION.as_str()))
                .and_then(|header| request_url.join(&header.value).ok())
                .filter(|url| url.origin() == self.resource_origin)
            else {
                break (response, body);
            };
            if redirects >= MAX_OAUTH_HTTP_REDIRECTS {
                return Err(oauth_http_client_error(
                    OAuthHttpClientAdapterError::TooManyRedirects,
                ));
            }
            if status == StatusCode::SEE_OTHER
                || matches!(status, StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND)
                    && params.method == Method::POST.as_str()
            {
                if params.method != Method::HEAD.as_str() {
                    params.method = Method::GET.to_string();
                }
                params.body = None;
                params.headers.retain(|header| {
                    ![
                        CONTENT_TYPE,
                        CONTENT_LENGTH,
                        CONTENT_ENCODING,
                        TRANSFER_ENCODING,
                    ]
                    .iter()
                    .any(|name| header.name.eq_ignore_ascii_case(name.as_str()))
                });
            }
            params.url = next_url.to_string();
            if let Some(deadline) = deadline {
                let remaining =
                    deadline
                        .checked_duration_since(Instant::now())
                        .ok_or_else(|| {
                            oauth_http_client_error(OAuthHttpClientAdapterError::TimedOut)
                        })?;
                params.timeout_ms = Some(
                    u64::try_from(remaining.as_millis())
                        .unwrap_or(u64::MAX)
                        .max(1),
                );
            }
            request_url = next_url;
            redirects += 1;
        };
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
#[path = "oauth_http_client_security_tests.rs"]
mod security_tests;

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
