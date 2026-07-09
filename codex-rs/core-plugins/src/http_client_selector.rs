use codex_http_client::HttpClient;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareClientPoolError;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

pub(crate) type HttpClientSelectionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HttpClient, RouteAwareClientPoolError>> + Send + 'a>>;

/// Selects an HTTP client after resolving the outbound route for an exact request URL.
///
/// Implementations are expected to preserve request-specific route selection while reusing
/// transport clients when multiple URLs resolve to the same route.
pub(crate) trait HttpClientSelector: Debug + Send + Sync {
    fn client_for_url<'a>(&'a self, request_url: &'a str) -> HttpClientSelectionFuture<'a>;
}

impl HttpClientSelector for RouteAwareClientPool {
    fn client_for_url<'a>(&'a self, request_url: &'a str) -> HttpClientSelectionFuture<'a> {
        Box::pin(RouteAwareClientPool::client_for_url(self, request_url))
    }
}
