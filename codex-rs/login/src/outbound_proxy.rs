use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;

/// Auth-layer adapter around client-owned proxy policy.
///
/// `AuthConfig` carries this value while endpoint resolution and platform details remain in the
/// client layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRouteConfig {
    http_client_factory: HttpClientFactory,
}

impl AuthRouteConfig {
    pub fn respect_system_proxy() -> Self {
        Self {
            http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        }
    }

    pub(crate) fn http_client_factory(&self) -> &HttpClientFactory {
        &self.http_client_factory
    }
}
