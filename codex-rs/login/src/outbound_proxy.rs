use codex_http_client::HttpClientFactory;

/// Auth-layer adapter around client-owned proxy policy.
///
/// `AuthConfig` carries this value while endpoint resolution and platform details remain in the
/// client layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRouteConfig {
    http_client_factory: HttpClientFactory,
}

impl AuthRouteConfig {
    /// Adapts an application-resolved HTTP client factory for auth requests.
    pub fn from_http_client_factory(http_client_factory: HttpClientFactory) -> Self {
        Self {
            http_client_factory,
        }
    }

    /// Returns the HTTP client factory represented by this routing configuration.
    pub fn http_client_factory(&self) -> &HttpClientFactory {
        &self.http_client_factory
    }
}
