use std::collections::HashMap;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;

use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClient;
use crate::HttpClientFactory;
use crate::OutboundProxyRoute;
use crate::with_chatgpt_cloudflare_cookie_store;

const MAX_CACHED_ROUTES: usize = 16;

/// Reuses reqwest clients by resolved route while evaluating the exact URL of every request.
///
/// PAC and system-proxy selection remains request-specific, but requests that resolve to the same
/// direct or proxy route share one connection pool. Named constructors apply transport-wide
/// settings such as the ChatGPT Cloudflare cookie store without exposing reqwest to callers.
#[derive(Clone)]
pub struct RouteAwareClientPool {
    http_client_factory: HttpClientFactory,
    route_class: ClientRouteClass,
    builder_factory: Arc<dyn Fn() -> reqwest::ClientBuilder + Send + Sync>,
    clients: Arc<Mutex<HashMap<OutboundProxyRoute, HttpClient>>>,
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

impl RouteAwareClientPool {
    /// Creates a pool with the shared default HTTP transport settings.
    pub fn new(http_client_factory: HttpClientFactory, route_class: ClientRouteClass) -> Self {
        Self::with_builder_factory(http_client_factory, route_class, reqwest::Client::builder)
    }

    /// Creates a pool that retains the Cloudflare cookies required by ChatGPT endpoints.
    pub fn with_chatgpt_cloudflare_cookies(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
    ) -> Self {
        Self::with_builder_factory(http_client_factory, route_class, || {
            with_chatgpt_cloudflare_cookie_store(reqwest::Client::builder())
        })
    }

    pub(crate) fn with_builder_factory(
        http_client_factory: HttpClientFactory,
        route_class: ClientRouteClass,
        builder_factory: impl Fn() -> reqwest::ClientBuilder + Send + Sync + 'static,
    ) -> Self {
        Self {
            http_client_factory,
            route_class,
            builder_factory: Arc::new(builder_factory),
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns a pooled client configured for the proxy route selected for `request_url`.
    pub async fn client_for_url(
        &self,
        request_url: &str,
    ) -> Result<HttpClient, RouteAwareClientPoolError> {
        let route = self
            .http_client_factory
            .resolve_proxy_route_async(request_url.to_string())
            .await
            .map_err(RouteAwareClientPoolError::Resolve)?;
        let clients = match self.clients.lock() {
            Ok(clients) => clients,
            Err(error) => panic!("route-aware client cache lock should not be poisoned: {error}"),
        };
        if let Some(client) = clients.get(&route) {
            return Ok(client.clone());
        }
        drop(clients);

        let client = self
            .http_client_factory
            .build_reqwest_client_for_resolved_route(
                (self.builder_factory)(),
                self.route_class,
                &route,
            )?;
        let client = HttpClient::new(client);
        let mut clients = match self.clients.lock() {
            Ok(clients) => clients,
            Err(error) => panic!("route-aware client cache lock should not be poisoned: {error}"),
        };
        if let Some(existing_client) = clients.get(&route) {
            return Ok(existing_client.clone());
        }
        if clients.len() >= MAX_CACHED_ROUTES
            && let Some(route_to_evict) = clients.keys().next().cloned()
        {
            clients.remove(&route_to_evict);
        }
        clients.insert(route, client.clone());
        Ok(client)
    }
}

#[cfg(test)]
#[path = "route_aware_client_pool_tests.rs"]
mod tests;
