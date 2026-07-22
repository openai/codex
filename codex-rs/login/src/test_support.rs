//! Test-only helpers exposed for cross-crate integration tests.
//!
//! Production code should receive an [`AuthRouteConfig`](crate::AuthRouteConfig) adapted from the
//! application's resolved HTTP client factory instead of depending on this module.

use crate::AuthRouteConfig;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;

/// Returns auth routing that preserves the transport's built-in proxy behavior.
pub fn transport_default_auth_route_config() -> AuthRouteConfig {
    AuthRouteConfig::from_http_client_factory(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ))
}
