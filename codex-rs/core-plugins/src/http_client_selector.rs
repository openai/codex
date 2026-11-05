use codex_http_client::NetworkPolicy;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RequestBuilder;
use codex_http_client::RouteAwareClientPool;
use http::Method;
use std::fmt::Debug;
use std::sync::Arc;

/// Builds requests whose URL is also used to resolve their outbound route.
///
/// Implementations must keep route selection coupled to the request URL. Returning a transport
/// client would let callers send a different URL than the one used for route selection.
pub(crate) trait HttpClientSelector: Debug + Send + Sync {
    fn for_current_account(&self) -> Arc<dyn HttpClientSelector>;
    fn request(&self, method: Method, url: &str) -> RequestBuilder;
    fn request_for_account(
        &self,
        method: Method,
        url: &str,
        binding: &NetworkPolicy,
    ) -> Option<RequestBuilder>;
    fn outbound_proxy_policy(&self) -> OutboundProxyPolicy;
}

impl HttpClientSelector for RouteAwareClientPool {
    fn for_current_account(&self) -> Arc<dyn HttpClientSelector> {
        Arc::new(RouteAwareClientPool::for_current_account(self))
    }

    fn request(&self, method: Method, url: &str) -> RequestBuilder {
        RouteAwareClientPool::request(self, method, url)
    }

    fn request_for_account(
        &self,
        method: Method,
        url: &str,
        binding: &NetworkPolicy,
    ) -> Option<RequestBuilder> {
        Some(self.with_account_binding(binding)?.request(method, url))
    }

    fn outbound_proxy_policy(&self) -> OutboundProxyPolicy {
        RouteAwareClientPool::outbound_proxy_policy(self)
    }
}
