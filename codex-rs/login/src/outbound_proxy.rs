use codex_http_client::HttpClientFactory;

/// Auth-layer adapter around client-owned proxy policy.
///
/// `AuthConfig` carries this value while endpoint resolution and platform details remain in the
/// client layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRouteConfig {
    http_client_factory: HttpClientFactory,
    local_bootstrap_factory: Option<HttpClientFactory>,
}

impl AuthRouteConfig {
    /// Adapts an application-resolved HTTP client factory for auth requests.
    pub fn from_http_client_factory(http_client_factory: HttpClientFactory) -> Self {
        Self {
            http_client_factory,
            local_bootstrap_factory: None,
        }
    }

    /// Installs the configuration owner's local-only policy for auth discovery.
    /// Only auth-owned endpoint constructors can access this factory.
    pub fn with_local_bootstrap_factory(mut self, factory: HttpClientFactory) -> Self {
        self.local_bootstrap_factory = Some(factory);
        self
    }

    pub(crate) fn authentication_factory(&self, endpoint: &str) -> HttpClientFactory {
        let Some(factory) = &self.local_bootstrap_factory else {
            return self.http_client_factory.clone();
        };
        let endpoints = endpoint.parse().into_iter().collect();
        factory.clone().with_network_policy(
            factory
                .network_policy()
                .clone()
                .restrict_to_endpoints(endpoints),
        )
    }

    /// Returns the HTTP client factory represented by this routing configuration.
    pub fn http_client_factory(&self) -> &HttpClientFactory {
        &self.http_client_factory
    }
}
