use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::StatusCode;
use http::header::CONTENT_TYPE;
use http::header::PROXY_AUTHORIZATION;
use reqwest::IntoUrl;
use serde::Serialize;

use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClient;
use crate::HttpClientFactory;
use crate::OutboundProxyPolicy;
use crate::OutboundProxyRoute;
use crate::route_aware_redirect::MAX_REDIRECTS;
use crate::route_aware_redirect::insert_referer;
use crate::route_aware_redirect::is_redirect;
use crate::route_aware_redirect::redirect_request;
use crate::route_aware_redirect::redirect_url;
use crate::route_aware_redirect::remove_sensitive_headers;
use crate::with_chatgpt_cloudflare_cookie_store;

const MAX_CACHED_ROUTES: usize = 16;

/// Reuses transport clients by resolved route while selecting a route for every request URL.
///
/// Request creation stays on the pool so the URL used for PAC or system-proxy resolution cannot
/// differ from the URL that is sent. Redirects are followed through the pool as new requests, so
/// each hop gets its own route decision while connections are still reused by route.
#[derive(Clone)]
pub struct RouteAwareClientPool {
    http_client_factory: HttpClientFactory,
    route_class: ClientRouteClass,
    builder_factory: Arc<dyn Fn() -> reqwest::ClientBuilder + Send + Sync>,
    request_logging: PoolRequestLogging,
    clients: Arc<Mutex<HashMap<OutboundProxyRoute, HttpClient>>>,
}

impl fmt::Debug for RouteAwareClientPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteAwareClientPool")
            .field("http_client_factory", &self.http_client_factory)
            .field("route_class", &self.route_class)
            .field("request_logging", &self.request_logging)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PoolRequestLogging {
    Enabled,
    Disabled,
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

    pub async fn send(self) -> Result<reqwest::Response, RouteAwareRequestError> {
        self.pool.send(self.request?).await
    }
}

impl RouteAwareClientPool {
    pub fn outbound_proxy_policy(&self) -> OutboundProxyPolicy {
        self.http_client_factory.outbound_proxy_policy()
    }

    /// Creates a pool with the shared default HTTP transport settings.
    pub fn new(http_client_factory: HttpClientFactory, route_class: ClientRouteClass) -> Self {
        Self::with_builder_factory(
            http_client_factory,
            route_class,
            reqwest::Client::builder,
            PoolRequestLogging::Enabled,
        )
    }

    /// Creates a pool with the shared defaults but without URL or response-header diagnostics.
    pub fn new_without_request_logging(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder_factory(
            http_client_factory,
            route_class,
            reqwest::Client::builder,
            PoolRequestLogging::Disabled,
        )
    }

    /// Creates a pool that retains the Cloudflare cookies required by ChatGPT endpoints.
    pub fn with_chatgpt_cloudflare_cookies(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder_factory(
            http_client_factory,
            route_class,
            || with_chatgpt_cloudflare_cookie_store(reqwest::Client::builder()),
            PoolRequestLogging::Enabled,
        )
    }

    /// Creates a ChatGPT Cloudflare-cookie pool without URL or response-header diagnostics.
    pub fn with_chatgpt_cloudflare_cookies_without_request_logging(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder_factory(
            http_client_factory,
            route_class,
            || with_chatgpt_cloudflare_cookie_store(reqwest::Client::builder()),
            PoolRequestLogging::Disabled,
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

    fn with_builder_factory(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
        builder_factory: impl Fn() -> reqwest::ClientBuilder + Send + Sync + 'static,
        request_logging: PoolRequestLogging,
    ) -> Self {
        Self {
            http_client_factory,
            route_class,
            builder_factory: Arc::new(builder_factory),
            request_logging,
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn send(
        &self,
        request: reqwest::Request,
    ) -> Result<reqwest::Response, RouteAwareRequestError> {
        let http_client_factory = self.http_client_factory.clone();
        self.send_with_resolver(request, move |request_url| {
            let http_client_factory = http_client_factory.clone();
            async move {
                http_client_factory
                    .resolve_proxy_route_async(request_url)
                    .await
            }
        })
        .await
    }

    async fn send_with_resolver<F, Fut>(
        &self,
        mut request: reqwest::Request,
        resolve_route: F,
    ) -> Result<reqwest::Response, RouteAwareRequestError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
        let request_method = request.method().clone();
        let request_url = request.url().to_string();
        let follows_redirects_manually = self.http_client_factory.outbound_proxy_policy()
            == OutboundProxyPolicy::RespectSystemProxy;
        let timeout_deadline = request
            .timeout()
            .copied()
            .map(|timeout| tokio::time::Instant::now() + timeout);
        let mut redirects = 0;
        let mut previous_route = None;
        loop {
            let current_url = request.url().clone();
            let (current_route, client) = match timeout_deadline {
                Some(timeout_deadline) => tokio::time::timeout_at(
                    timeout_deadline,
                    self.client_for_url_with_resolver(current_url.as_str(), &resolve_route),
                )
                .await
                .map_err(|_| RouteAwareRequestError::Timeout)??,
                None => {
                    self.client_for_url_with_resolver(current_url.as_str(), &resolve_route)
                        .await?
                }
            };
            if previous_route
                .as_ref()
                .is_some_and(|previous_route| previous_route != &current_route)
            {
                request.headers_mut().remove(PROXY_AUTHORIZATION);
            }
            previous_route = Some(current_route);
            if let Some(timeout_deadline) = timeout_deadline {
                let remaining = timeout_deadline
                    .checked_duration_since(tokio::time::Instant::now())
                    .ok_or(RouteAwareRequestError::Timeout)?;
                if remaining.is_zero() {
                    return Err(RouteAwareRequestError::Timeout);
                }
                *request.timeout_mut() = Some(remaining);
            }
            let method = request.method().clone();
            let headers = request.headers().clone();
            let version = request.version();
            let timeout = request.timeout().copied();
            let replay = request.try_clone();
            let execute_request = async {
                if follows_redirects_manually {
                    client.execute_without_request_logging(request).await
                } else {
                    client.execute(request).await
                }
            };
            let response = match match timeout_deadline {
                Some(timeout_deadline) => {
                    tokio::time::timeout_at(timeout_deadline, execute_request)
                        .await
                        .map_err(|_| RouteAwareRequestError::Timeout)?
                }
                None => execute_request.await,
            } {
                Ok(response) => response,
                Err(error) => {
                    if follows_redirects_manually {
                        client.log_error_summary(&request_method, &request_url, &error);
                    }
                    return Err(error.into());
                }
            };
            let status = response.status();
            if !is_redirect(status) {
                if follows_redirects_manually {
                    client.log_response(&request_method, &request_url, &response);
                }
                return Ok(response);
            }
            let Some(next_url) = redirect_url(&response) else {
                if follows_redirects_manually {
                    client.log_response(&request_method, &request_url, &response);
                }
                return Ok(response);
            };
            let Some(mut next_request) =
                redirect_request(status, method, headers, version, timeout, replay, next_url)
            else {
                if follows_redirects_manually {
                    client.log_response(&request_method, &request_url, &response);
                }
                return Ok(response);
            };
            let next_request_url = next_request.url().clone();
            if !matches!(next_request_url.scheme(), "http" | "https") {
                return Err(RouteAwareRequestError::UnsupportedRedirectScheme(
                    next_request_url.scheme().to_string(),
                ));
            }
            if redirects >= MAX_REDIRECTS {
                return Err(RouteAwareRequestError::TooManyRedirects);
            }
            remove_sensitive_headers(next_request.headers_mut(), &current_url, &next_request_url);
            insert_referer(next_request.headers_mut(), &current_url, &next_request_url);
            request = next_request;
            redirects += 1;
        }
    }

    async fn client_for_url_with_resolver<F, Fut>(
        &self,
        request_url: &str,
        resolve_route: F,
    ) -> Result<(OutboundProxyRoute, HttpClient), RouteAwareClientPoolError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
        let route = resolve_route(request_url.to_string())
            .await
            .map_err(RouteAwareClientPoolError::Resolve)?;
        let clients = match self.clients.lock() {
            Ok(clients) => clients,
            Err(error) => panic!("route-aware client cache lock should not be poisoned: {error}"),
        };
        if let Some(client) = clients.get(&route) {
            return Ok((route, client.clone()));
        }
        drop(clients);

        let builder = (self.builder_factory)();
        let builder = match self.http_client_factory.outbound_proxy_policy() {
            OutboundProxyPolicy::ReqwestDefault => builder,
            OutboundProxyPolicy::RespectSystemProxy => {
                builder.redirect(reqwest::redirect::Policy::none())
            }
        };
        let client = self
            .http_client_factory
            .build_reqwest_client_for_resolved_route(builder, self.route_class, &route)?;
        let client = match self.request_logging {
            PoolRequestLogging::Enabled => HttpClient::new(client),
            PoolRequestLogging::Disabled => HttpClient::new_without_request_logging(client),
        };
        let mut clients = match self.clients.lock() {
            Ok(clients) => clients,
            Err(error) => panic!("route-aware client cache lock should not be poisoned: {error}"),
        };
        if let Some(existing_client) = clients.get(&route) {
            return Ok((route, existing_client.clone()));
        }
        if clients.len() >= MAX_CACHED_ROUTES
            && let Some(route_to_evict) = clients.keys().next().cloned()
        {
            clients.remove(&route_to_evict);
        }
        clients.insert(route.clone(), client.clone());
        Ok((route, client))
    }
}

#[cfg(test)]
#[path = "route_aware_client_pool_tests.rs"]
mod tests;
