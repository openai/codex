//! Startup-sync-specific HTTP transport selection.
//!
//! Curated plugin startup sync normally uses git, so its HTTP path is also a recovery path for
//! machines where git is unavailable or fails. Under `ReqwestDefault`, that recovery path must
//! preserve the legacy `codex_login::default_client::create_client_without_request_logging()`
//! behavior: invalid custom-CA configuration is logged and falls back to a normal client instead
//! of making HTTP sync fail as well.
//!
//! When `RespectSystemProxy` is enabled, however, every concrete request URL—including download
//! URLs returned by another endpoint—must be routed through `RouteAwareClientPool` so PAC and
//! operating-system proxy settings are respected. The route-aware pool intentionally surfaces
//! client-construction errors and therefore cannot provide the legacy custom-CA fallback for free.
//!
//! `StartupSyncHttpClient` keeps those two policies behind one request API without making lenient
//! custom-CA handling a global HTTP-client behavior. This module selects the transport only;
//! startup-sync request helpers remain responsible for applying the standard Codex headers.

use std::sync::Arc;
use std::time::Duration;

use crate::http_client_selector::HttpClientSelector;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpResponse;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RequestBuilder;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestBuilder;
use codex_login::default_client::create_client_without_request_logging;
use http::HeaderMap;
use http::Method;

pub(super) enum StartupSyncHttpClient {
    Default(HttpClient),
    RouteAware(Arc<dyn HttpClientSelector>),
}

impl StartupSyncHttpClient {
    pub(super) fn new(http_client_factory: &HttpClientFactory) -> Self {
        match http_client_factory.outbound_proxy_policy() {
            OutboundProxyPolicy::ReqwestDefault => {
                Self::Default(create_client_without_request_logging())
            }
            OutboundProxyPolicy::RespectSystemProxy => {
                let http_clients =
                    RouteAwareClientPool::with_chatgpt_cloudflare_cookies_without_request_logging(
                        http_client_factory.clone(),
                        ClientRouteClass::Api,
                    );
                Self::RouteAware(Arc::new(http_clients))
            }
        }
    }

    #[cfg(test)]
    pub(super) fn route_aware(http_clients: Arc<dyn HttpClientSelector>) -> Self {
        Self::RouteAware(http_clients)
    }

    pub(super) fn request(&self, method: Method, url: &str) -> StartupSyncRequestBuilder {
        match self {
            Self::Default(client) => {
                StartupSyncRequestBuilder::Default(client.request(method, url))
            }
            Self::RouteAware(http_clients) => {
                StartupSyncRequestBuilder::RouteAware(http_clients.request(method, url))
            }
        }
    }
}

pub(super) enum StartupSyncRequestBuilder {
    Default(RequestBuilder),
    RouteAware(RouteAwareRequestBuilder),
}

impl StartupSyncRequestBuilder {
    pub(super) fn timeout(self, timeout: Duration) -> Self {
        match self {
            Self::Default(request) => Self::Default(request.timeout(timeout)),
            Self::RouteAware(request) => Self::RouteAware(request.timeout(timeout)),
        }
    }

    pub(super) fn header(self, key: &'static str, value: &'static str) -> Self {
        match self {
            Self::Default(request) => Self::Default(request.header(key, value)),
            Self::RouteAware(request) => Self::RouteAware(request.header(key, value)),
        }
    }

    pub(super) fn headers(self, headers: HeaderMap) -> Self {
        match self {
            Self::Default(request) => Self::Default(request.headers(headers)),
            Self::RouteAware(request) => Self::RouteAware(request.headers(headers)),
        }
    }

    pub(super) async fn send(self) -> Result<HttpResponse, String> {
        match self {
            Self::Default(request) => request.send().await.map_err(|err| err.to_string()),
            Self::RouteAware(request) => request.send().await.map_err(|err| err.to_string()),
        }
    }
}
