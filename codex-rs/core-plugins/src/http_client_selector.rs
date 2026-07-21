use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestBuilder;
use http::Method;
use std::fmt::Debug;

/// Builds requests whose URL is also used to resolve their outbound route.
///
/// Implementations must keep route selection coupled to the request URL. Returning a transport
/// client would let callers send a different URL than the one used for route selection.
pub(crate) trait HttpClientSelector: Debug + Send + Sync {
    fn request(&self, method: Method, url: &str) -> RouteAwareRequestBuilder;
    fn outbound_proxy_policy(&self) -> OutboundProxyPolicy;
}

impl HttpClientSelector for RouteAwareClientPool {
    fn request(&self, method: Method, url: &str) -> RouteAwareRequestBuilder {
        RouteAwareClientPool::request(self, method, url)
    }

    fn outbound_proxy_policy(&self) -> OutboundProxyPolicy {
        RouteAwareClientPool::outbound_proxy_policy(self)
    }
}
