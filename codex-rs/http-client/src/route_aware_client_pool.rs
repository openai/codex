mod execution;

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use http::HeaderMap;
use http::Method;
use http::StatusCode;
use reqwest::IntoUrl;

use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClient;
use crate::HttpClientBuilder;
use crate::HttpClientFactory;
use crate::NetworkPolicyDenied;
use crate::OutboundProxyPolicy;
use crate::OutboundProxyRoute;
use crate::RequestBuilder;
use crate::RouteFailureClass;
use crate::client::HttpClientBackend;
use crate::client::TransportClient;
use crate::client_builder::ProxyRouting;
use crate::tls_backend_fallback::RustlsClientCache;

const MAX_CACHED_ROUTES: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CustomCaFallback {
    LegacyDirect,
    #[default]
    Disabled,
    LegacyTransportDefault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedTlsBackend {
    TransportDefault,
    RustlsFallback,
}

/// Reuses transport clients by resolved route while selecting a route for every request URL.
///
/// Resolves the initial route from the complete input URL, before reqwest handles URL credentials.
/// Redirects are followed through the pool as new requests, so
/// each hop gets its own route decision while connections are still reused by route.
#[derive(Clone)]
pub struct RouteAwareClientPool {
    http_client_factory: HttpClientFactory,
    route_class: ClientRouteClass,
    pub(crate) client_builder: HttpClientBuilder,
    default_headers: HeaderMap,
    custom_ca_fallback: CustomCaFallback,
    clients: Arc<Mutex<HashMap<OutboundProxyRoute, TransportClient>>>,
    client_build: Arc<tokio::sync::Mutex<()>>,
    rustls_clients: Option<RustlsClientCache>,
}

impl fmt::Debug for RouteAwareClientPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteAwareClientPool")
            .field("http_client_factory", &self.http_client_factory)
            .field("route_class", &self.route_class)
            .finish_non_exhaustive()
    }
}

/// Error returned when selecting a route or constructing its pooled HTTP client.
#[derive(Debug, thiserror::Error)]
pub enum RouteAwareClientPoolError {
    #[error("failed to resolve the outbound proxy route: {0}")]
    Resolve(#[source] io::Error),
    #[error(transparent)]
    Build(#[from] BuildRouteAwareHttpClientError),
    #[error("HTTP transport construction task failed: {0}")]
    BuildTask(#[source] tokio::task::JoinError),
}

/// Error returned while building, routing, or sending a route-aware request.
#[derive(Debug, thiserror::Error)]
pub enum RouteAwareRequestError {
    #[error(transparent)]
    Policy(#[from] NetworkPolicyDenied),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    Route(#[from] RouteAwareClientPoolError),
    #[error("failed to build route-aware request: {0}")]
    Build(String),
    #[error("redirect target uses unsupported URL scheme: {0}")]
    UnsupportedRedirectScheme(String),
    #[error("too many redirects")]
    TooManyRedirects,
    #[error("route-aware request timed out")]
    Timeout,
}

impl RouteAwareRequestError {
    /// Classifies transport, proxy, and certificate failures without exposing request details.
    pub fn failure_class(&self) -> Option<RouteFailureClass> {
        if self.is_timeout() {
            return Some(RouteFailureClass::ConnectTimeout);
        }
        if self.status() == Some(StatusCode::PROXY_AUTHENTICATION_REQUIRED) {
            return Some(RouteFailureClass::ProxyAuthenticationRequired);
        }
        if let Self::Route(RouteAwareClientPoolError::Resolve(error)) = self
            && let Some(source) = error.get_ref()
            && source.is::<rustls::Error>()
        {
            return Some(RouteFailureClass::TlsError);
        }

        let mut source: Option<&(dyn std::error::Error + 'static)> = Some(self);
        while let Some(error) = source {
            if error.downcast_ref::<rustls::Error>().is_some()
                || error.downcast_ref::<native_tls::Error>().is_some()
            {
                return Some(RouteFailureClass::TlsError);
            }
            if error.to_string() == "tunnel error: proxy authorization required" {
                return Some(RouteFailureClass::ProxyAuthenticationRequired);
            }
            source = error.source();
        }

        match self {
            Self::Route(RouteAwareClientPoolError::Build(
                BuildRouteAwareHttpClientError::CustomCa(_)
                | BuildRouteAwareHttpClientError::ExplicitTls(_),
            )) => Some(RouteFailureClass::TlsError),
            Self::Route(RouteAwareClientPoolError::Build(
                BuildRouteAwareHttpClientError::InvalidProxyConfig { .. },
            )) => Some(RouteFailureClass::InvalidProxyConfig),
            Self::Route(RouteAwareClientPoolError::Resolve(_)) => {
                Some(RouteFailureClass::ProxyResolutionUnavailable)
            }
            Self::Route(RouteAwareClientPoolError::BuildTask(_))
            | Self::Request(_)
            | Self::Policy(_)
            | Self::Build(_)
            | Self::UnsupportedRedirectScheme(_)
            | Self::TooManyRedirects
            | Self::Timeout => None,
        }
    }

    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Request(error) => error.status(),
            Self::Route(_)
            | Self::Policy(_)
            | Self::Build(_)
            | Self::UnsupportedRedirectScheme(_)
            | Self::TooManyRedirects
            | Self::Timeout => None,
        }
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout) || matches!(self, Self::Request(error) if error.is_timeout())
    }

    pub fn is_connect(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_connect())
    }

    pub fn is_builder(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_builder())
    }

    pub fn is_body(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_body())
    }

    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_request())
    }

    pub fn url_mut(&mut self) -> Option<&mut reqwest::Url> {
        match self {
            Self::Request(error) => error.url_mut(),
            _ => None,
        }
    }

    /// Removes a request URL from the underlying transport error before it is logged or returned.
    ///
    /// Use this for requests whose URL can contain credentials, such as signed blob uploads.
    pub fn without_url(self) -> Self {
        match self {
            Self::Request(error) => Self::Request(error.without_url()),
            other => other,
        }
    }
}

impl RouteAwareClientPool {
    pub(crate) fn request_logging_enabled(&self) -> bool {
        self.client_builder.request_logging_enabled()
    }

    pub fn outbound_proxy_policy(&self) -> OutboundProxyPolicy {
        self.http_client_factory.outbound_proxy_policy()
    }

    pub fn allows_system_proxy_fallback(&self) -> bool {
        self.http_client_factory.allows_system_proxy_fallback()
    }

    /// Changes routing while preserving transport settings and clients cached by resolved route.
    pub fn with_outbound_proxy_policy(mut self, policy: OutboundProxyPolicy) -> Self {
        self.http_client_factory = self.http_client_factory.with_outbound_proxy_policy(policy);
        self
    }

    /// Exposes the ordinary request builder while sending through this policy-aware pool.
    pub fn into_client(self) -> HttpClient {
        HttpClient {
            backend: HttpClientBackend::Routed(Arc::new(self)),
        }
    }

    /// Creates a pool with the shared default HTTP transport settings.
    pub fn new(http_client_factory: HttpClientFactory, route_class: ClientRouteClass) -> Self {
        Self::with_builder(http_client_factory, route_class, HttpClientBuilder::new())
    }

    /// Creates a pool that returns redirect responses without following them.
    ///
    /// This applies both when reqwest owns redirect handling and when the pool follows redirects
    /// manually so each hop can receive its own proxy-route decision.
    pub fn new_without_redirects(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new().without_redirects(),
        )
    }

    /// Creates a no-redirect pool without request URL or response-header diagnostics.
    pub fn new_without_redirects_or_request_logging(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new()
                .without_redirects()
                .without_request_logging(),
        )
    }

    /// Creates a pool whose clients limit only connection establishment.
    ///
    /// The timeout applies to every client built for a resolved route, including redirect hops.
    pub fn with_connect_timeout(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
        connect_timeout: Duration,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new().connect_timeout(connect_timeout),
        )
    }

    /// Configures protocol-specific transport behavior without bypassing destination checks.
    pub fn with_builder(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
        mut client_builder: HttpClientBuilder,
    ) -> Self {
        let default_headers = client_builder.default_headers.take().unwrap_or_default();
        Self {
            http_client_factory,
            route_class,
            client_builder,
            default_headers,
            custom_ca_fallback: CustomCaFallback::Disabled,
            clients: Arc::new(Mutex::new(HashMap::new())),
            client_build: Arc::default(),
            rustls_clients: None,
        }
    }

    /// Starts a new account-scoped operation while sharing the existing connection cache.
    /// Capture this before resolving credentials or reading account-specific content.
    pub fn for_current_account(&self) -> Self {
        let mut pool = self.clone();
        pool.http_client_factory = pool.http_client_factory.clone().with_network_policy(
            pool.http_client_factory
                .network_policy()
                .clone()
                .for_current_account(),
        );
        pool
    }

    /// Retries recognized TLS protocol-negotiation failures once using rustls.
    ///
    /// Successful fallback is remembered for each HTTPS origin and resolved outbound route.
    /// Rustls clients are reused across fallback destinations that share a route, while other
    /// destinations retain the existing transport-default backend.
    pub fn with_tls_backend_fallback(mut self) -> Self {
        self.rustls_clients = Some(RustlsClientCache::default());
        self
    }

    /// Creates a pool with the shared defaults but without URL or response-header diagnostics.
    pub fn new_without_request_logging(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new().without_request_logging(),
        )
    }

    /// Preserves the legacy custom-CA fallback for transport-default proxy routes.
    ///
    /// Use this only when migrating a client that already continued with system roots after a
    /// custom-CA construction failure. System-proxy routes still propagate construction errors.
    pub fn with_legacy_custom_ca_fallback(mut self) -> Self {
        self.custom_ca_fallback = CustomCaFallback::LegacyTransportDefault;
        self
    }

    /// Preserves the legacy sandbox client's direct routing and custom-CA fallback.
    pub fn with_legacy_direct_proxy_and_custom_ca_fallback(mut self) -> Self {
        self.custom_ca_fallback = CustomCaFallback::LegacyDirect;
        self
    }

    /// Creates a pool that retains the Cloudflare cookies required by ChatGPT endpoints.
    pub fn with_chatgpt_cloudflare_cookies(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_chatgpt_cloudflare_cookies_and_default_headers(
            http_client_factory,
            route_class,
            HeaderMap::new(),
        )
    }

    /// Creates a pool with ChatGPT Cloudflare cookies and default headers on every route.
    pub fn with_chatgpt_cloudflare_cookies_and_default_headers(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
        default_headers: HeaderMap,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new()
                .default_headers(default_headers)
                .with_chatgpt_cloudflare_cookie_store(),
        )
    }

    /// Creates a no-redirect pool that retains the Cloudflare cookies required by ChatGPT
    /// endpoints.
    pub fn with_chatgpt_cloudflare_cookies_without_redirects(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new()
                .with_chatgpt_cloudflare_cookie_store()
                .without_redirects(),
        )
    }

    /// Creates a no-redirect ChatGPT Cloudflare-cookie pool without request diagnostics.
    pub fn with_chatgpt_cloudflare_cookies_without_redirects_or_request_logging(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new()
                .with_chatgpt_cloudflare_cookie_store()
                .without_redirects()
                .without_request_logging(),
        )
    }

    /// Creates a ChatGPT Cloudflare-cookie pool without URL or response-header diagnostics.
    pub fn with_chatgpt_cloudflare_cookies_without_request_logging(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new()
                .with_chatgpt_cloudflare_cookie_store()
                .without_request_logging(),
        )
    }

    pub fn get<U>(&self, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::GET, url)
    }

    pub fn post<U>(&self, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::POST, url)
    }

    pub fn put<U>(&self, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::PUT, url)
    }

    pub fn delete<U>(&self, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::DELETE, url)
    }

    pub fn request<U>(&self, method: Method, url: U) -> RequestBuilder
    where
        U: IntoUrl,
    {
        RequestBuilder::routed(Arc::new(self.clone()), method, url)
    }

    async fn client_for_url_with_resolver<F, Fut>(
        &self,
        request_url: &str,
        resolve_route: F,
    ) -> Result<(OutboundProxyRoute, TransportClient, SelectedTlsBackend), RouteAwareClientPoolError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
        let route = if self.custom_ca_fallback == CustomCaFallback::LegacyDirect {
            OutboundProxyRoute::Direct
        } else {
            resolve_route(request_url.to_string())
                .await
                .map_err(RouteAwareClientPoolError::Resolve)?
        };
        if let Some(rustls_clients) = self.rustls_clients.as_ref()
            && let Ok(url) = reqwest::Url::parse(request_url)
            && rustls_clients.requires_rustls(&url, &route)
            && let Some(client) = rustls_clients.client_for_route(&route)
        {
            return Ok((route, client, SelectedTlsBackend::RustlsFallback));
        }
        {
            let clients = match self.clients.lock() {
                Ok(clients) => clients,
                Err(error) => {
                    panic!("route-aware client cache lock should not be poisoned: {error}")
                }
            };
            if let Some(client) = clients.get(&route) {
                return Ok((route, client.clone(), SelectedTlsBackend::TransportDefault));
            }
        }

        let build_permit = Arc::clone(&self.client_build).lock_owned().await;
        {
            let clients = self.clients.lock().unwrap_or_else(|error| {
                panic!("route-aware client cache lock should not be poisoned: {error}")
            });
            if let Some(client) = clients.get(&route) {
                return Ok((route, client.clone(), SelectedTlsBackend::TransportDefault));
            }
        }
        let client_builder = if self.follows_redirects_manually() {
            self.client_builder.clone().without_redirects()
        } else {
            self.client_builder.clone()
        };
        let pool = self.clone();
        let build_route = route.clone();
        let client = tokio::task::spawn_blocking(move || {
            // A timed-out caller must not release the slot or discard a successful build.
            let _build_permit = build_permit;
            let client = match (
                pool.http_client_factory.outbound_proxy_policy(),
                pool.custom_ca_fallback,
            ) {
                (_, CustomCaFallback::LegacyDirect) => {
                    Ok(client_builder.build_with_custom_ca_fallback(ProxyRouting::Direct))
                }
                (OutboundProxyPolicy::ReqwestDefault, CustomCaFallback::LegacyTransportDefault) => {
                    Ok(
                        client_builder
                            .build_with_custom_ca_fallback(ProxyRouting::TransportDefault),
                    )
                }
                (OutboundProxyPolicy::ReqwestDefault, CustomCaFallback::Disabled)
                | (OutboundProxyPolicy::RespectSystemProxy, CustomCaFallback::Disabled)
                | (
                    OutboundProxyPolicy::RespectSystemProxy,
                    CustomCaFallback::LegacyTransportDefault,
                ) => client_builder.build_for_resolved_route(
                    &pool.http_client_factory,
                    pool.route_class,
                    &build_route,
                ),
            }?;
            let mut clients = pool.clients.lock().unwrap_or_else(|error| {
                panic!("route-aware client cache lock should not be poisoned: {error}")
            });
            if clients.len() >= MAX_CACHED_ROUTES
                && let Some(route_to_evict) = clients.keys().next().cloned()
            {
                clients.remove(&route_to_evict);
            }
            clients.insert(build_route, client.clone());
            Ok::<_, BuildRouteAwareHttpClientError>(client)
        })
        .await
        .map_err(RouteAwareClientPoolError::BuildTask)??;
        Ok((route, client, SelectedTlsBackend::TransportDefault))
    }

    fn follows_redirects_manually(&self) -> bool {
        self.client_builder.follows_redirects()
            && (self.http_client_factory.outbound_proxy_policy()
                == OutboundProxyPolicy::RespectSystemProxy
                || self.http_client_factory.network_policy().is_managed()
                || self.rustls_clients.is_some())
    }

    fn rustls_client_for_route(
        &self,
        route: &OutboundProxyRoute,
    ) -> Result<TransportClient, RouteAwareClientPoolError> {
        let mut client_builder = self.client_builder.clone().with_rustls_tls();
        if self.follows_redirects_manually() {
            client_builder = client_builder.without_redirects();
        }
        client_builder
            .build_for_resolved_route(&self.http_client_factory, self.route_class, route)
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "route_aware_client_pool_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "route_aware_tls_fallback_tests.rs"]
mod tls_fallback_tests;

#[cfg(test)]
#[path = "route_aware_policy_tests.rs"]
mod policy_tests;
