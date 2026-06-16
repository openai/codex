use codex_client::OutboundProxyConfig;

/// Auth-owned policy marker that keeps platform-specific resolution in the client layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRouteConfig {
    route_config: OutboundProxyConfig,
}

impl AuthRouteConfig {
    pub fn respect_system_proxy() -> Self {
        Self {
            route_config: OutboundProxyConfig::respect_system_proxy(),
        }
    }

    pub(crate) fn route_config(&self) -> &OutboundProxyConfig {
        &self.route_config
    }
}
