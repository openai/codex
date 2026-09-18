mod execution;

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use futures::TryStream;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::StatusCode;
use http::header::CONTENT_TYPE;
use reqwest::IntoUrl;
use serde::Serialize;

use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClient;
use crate::HttpClientBuilder;
use crate::HttpClientFactory;
use crate::OutboundProxyPolicy;
use crate::OutboundProxyRoute;
use crate::RouteFailureClass;
use crate::tls_backend_fallback::RustlsClientCache;

const MAX_CACHED_ROUTES: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CustomCaFallback {
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
/// Request creation stays on the pool so the URL used for PAC or system-proxy resolution cannot
/// differ from the URL that is sent. Redirects are followed through the pool as new requests, so
/// each hop gets its own route decision while connections are still reused by route.
#[derive(Clone)]
pub struct RouteAwareClientPool {
    http_client_factory: HttpClientFactory,
    route_class: ClientRouteClass,
    client_builder: HttpClientBuilder,
    custom_ca_fallback: CustomCaFallback,
    clients: Arc<Mutex<HashMap<OutboundProxyRoute, HttpClient>>>,
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
}

/// Error returned while building, routing, or sending a route-aware request.
#[derive(Debug, thiserror::Error)]
pub enum RouteAwareRequestError {
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
                BuildRouteAwareHttpClientError::CustomCa(_),
            )) => Some(RouteFailureClass::TlsError),
            Self::Route(RouteAwareClientPoolError::Build(
                BuildRouteAwareHttpClientError::InvalidProxyConfig { .. },
            )) => Some(RouteFailureClass::InvalidProxyConfig),
            Self::Route(RouteAwareClientPoolError::Resolve(_)) => {
                Some(RouteFailureClass::ProxyResolutionUnavailable)
            }
            Self::Request(_)
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

    pub fn is_body(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_body())
    }

    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_request())
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

#[must_use = "requests are not sent unless `send` is awaited"]
pub struct RouteAwareRequestBuilder {
    pool: RouteAwareClientPool,
    request: Result<reqwest::Request, RouteAwareRequestError>,
}

impl fmt::Debug for RouteAwareRequestBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let request = self.request.as_ref().ok();
        formatter
            .debug_struct("RouteAwareRequestBuilder")
            .field("pool", &self.pool)
            .field("method", &request.map(reqwest::Request::method))
            .field("url", &request.map(|_| "<redacted>"))
            .finish_non_exhaustive()
    }
}

impl RouteAwareRequestBuilder {
    fn new<U>(pool: RouteAwareClientPool, method: Method, url: U) -> Self
    where
        U: IntoUrl,
    {
        let request = url
            .into_url()
            .map(|url| reqwest::Request::new(method, url))
            .map_err(RouteAwareRequestError::Request);
        Self { pool, request }
    }

    pub fn headers(mut self, headers: HeaderMap) -> Self {
        if let Ok(request) = &mut self.request {
            request.headers_mut().extend(headers);
        }
        self
    }

    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        if let Ok(request) = &mut self.request {
            let header = HeaderName::try_from(key)
                .map_err(Into::into)
                .and_then(|key| {
                    HeaderValue::try_from(value)
                        .map(|value| (key, value))
                        .map_err(Into::into)
                });
            match header {
                Ok((key, value)) => {
                    request.headers_mut().append(key, value);
                }
                Err(error) => {
                    self.request = Err(RouteAwareRequestError::Build(error.to_string()));
                }
            }
        }
        self
    }

    /// Sets a timeout for the request as a whole.
    ///
    /// The budget starts before outbound-route resolution and covers selecting or constructing a
    /// pooled client, establishing a connection, sending the request, and awaiting the response.
    /// Use [`HttpClientBuilder::connect_timeout`] when only connection establishment should be
    /// bounded.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        if let Ok(request) = &mut self.request {
            *request.timeout_mut() = Some(timeout);
        }
        self
    }

    pub fn json<T>(mut self, value: &T) -> Self
    where
        T: ?Sized + Serialize,
    {
        if let Ok(request) = &mut self.request {
            match serde_json::to_vec(value) {
                Ok(body) => {
                    if !request.headers().contains_key(CONTENT_TYPE) {
                        request
                            .headers_mut()
                            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    }
                    *request.body_mut() = Some(body.into());
                }
                Err(error) => {
                    self.request = Err(RouteAwareRequestError::Build(error.to_string()));
                }
            }
        }
        self
    }

    pub fn body<B>(mut self, body: B) -> Self
    where
        B: Into<reqwest::Body>,
    {
        if let Ok(request) = &mut self.request {
            *request.body_mut() = Some(body.into());
        }
        self
    }

    /// Sets a streaming request body without exposing the underlying HTTP implementation.
    pub fn body_stream<S>(mut self, stream: S) -> Self
    where
        S: TryStream + Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        if let Ok(request) = &mut self.request {
            *request.body_mut() = Some(reqwest::Body::wrap_stream(stream));
        }
        self
    }

    pub async fn send(self) -> Result<reqwest::Response, RouteAwareRequestError> {
        self.pool.send(self.request?).await
    }
}

impl RouteAwareClientPool {
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

    fn with_builder(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
        client_builder: HttpClientBuilder,
    ) -> Self {
        Self {
            http_client_factory,
            route_class,
            client_builder,
            custom_ca_fallback: CustomCaFallback::Disabled,
            clients: Arc::new(Mutex::new(HashMap::new())),
            rustls_clients: None,
        }
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

    /// Creates a pool that retains the Cloudflare cookies required by ChatGPT endpoints.
    pub fn with_chatgpt_cloudflare_cookies(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder(
            http_client_factory,
            route_class,
            HttpClientBuilder::new().with_chatgpt_cloudflare_cookie_store(),
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

    pub fn get<U>(&self, url: U) -> RouteAwareRequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::GET, url)
    }

    pub fn post<U>(&self, url: U) -> RouteAwareRequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::POST, url)
    }

    pub fn put<U>(&self, url: U) -> RouteAwareRequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::PUT, url)
    }

    pub fn delete<U>(&self, url: U) -> RouteAwareRequestBuilder
    where
        U: IntoUrl,
    {
        self.request(Method::DELETE, url)
    }

    pub fn request<U>(&self, method: Method, url: U) -> RouteAwareRequestBuilder
    where
        U: IntoUrl,
    {
        RouteAwareRequestBuilder::new(self.clone(), method, url)
    }

    async fn client_for_url_with_resolver<F, Fut>(
        &self,
        request_url: &str,
        resolve_route: F,
    ) -> Result<(OutboundProxyRoute, HttpClient, SelectedTlsBackend), RouteAwareClientPoolError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
        let route = resolve_route(request_url.to_string())
            .await
            .map_err(RouteAwareClientPoolError::Resolve)?;
        if let Some(rustls_clients) = self.rustls_clients.as_ref()
            && let Ok(url) = reqwest::Url::parse(request_url)
            && rustls_clients.requires_rustls(&url, &route)
            && let Some(client) = rustls_clients.client_for_route(&route)
        {
            return Ok((route, client, SelectedTlsBackend::RustlsFallback));
        }
        let clients = match self.clients.lock() {
            Ok(clients) => clients,
            Err(error) => panic!("route-aware client cache lock should not be poisoned: {error}"),
        };
        if let Some(client) = clients.get(&route) {
            return Ok((route, client.clone(), SelectedTlsBackend::TransportDefault));
        }
        drop(clients);

        let client_builder = if self.follows_redirects_manually() {
            self.client_builder.clone().without_redirects()
        } else {
            self.client_builder.clone()
        };
        #[expect(
            deprecated,
            reason = "explicitly opted-in pools preserve the legacy custom-CA fallback"
        )]
        let client = match (
            self.http_client_factory.outbound_proxy_policy(),
            self.custom_ca_fallback,
        ) {
            (OutboundProxyPolicy::ReqwestDefault, CustomCaFallback::LegacyTransportDefault) => {
                client_builder.build_with_transport_default_proxy_and_custom_ca_fallback()
            }
            (OutboundProxyPolicy::ReqwestDefault, CustomCaFallback::Disabled)
            | (OutboundProxyPolicy::RespectSystemProxy, CustomCaFallback::Disabled)
            | (OutboundProxyPolicy::RespectSystemProxy, CustomCaFallback::LegacyTransportDefault) => {
                client_builder.build_for_resolved_route(
                    &self.http_client_factory,
                    self.route_class,
                    &route,
                )?
            }
        };
        let mut clients = match self.clients.lock() {
            Ok(clients) => clients,
            Err(error) => panic!("route-aware client cache lock should not be poisoned: {error}"),
        };
        if let Some(existing_client) = clients.get(&route) {
            return Ok((
                route,
                existing_client.clone(),
                SelectedTlsBackend::TransportDefault,
            ));
        }
        if clients.len() >= MAX_CACHED_ROUTES
            && let Some(route_to_evict) = clients.keys().next().cloned()
        {
            clients.remove(&route_to_evict);
        }
        clients.insert(route.clone(), client.clone());
        Ok((route, client, SelectedTlsBackend::TransportDefault))
    }

    fn follows_redirects_manually(&self) -> bool {
        self.client_builder.follows_redirects()
            && (self.http_client_factory.outbound_proxy_policy()
                == OutboundProxyPolicy::RespectSystemProxy
                || self.rustls_clients.is_some())
    }

    fn rustls_client_for_route(
        &self,
        route: &OutboundProxyRoute,
    ) -> Result<HttpClient, RouteAwareClientPoolError> {
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
