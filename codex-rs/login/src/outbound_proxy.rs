use codex_client::OutboundProxyConfig;
use codex_client::OutboundProxyMode;
use codex_config::types::SystemProxyFeatureConfigToml;
use codex_config::types::SystemProxyFeatureModeToml;

/// Stable route-selection policy for Codex-owned clients.
///
/// Call sites should accept this type instead of lower-level resolver
/// details. Config parsing decides whether to construct one, and the client
/// layer remains responsible for platform-specific proxy resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthRouteConfig {
    route_config: Option<OutboundProxyConfig>,
}

impl AuthRouteConfig {
    pub fn auto() -> Self {
        Self::from_outbound_proxy_mode(OutboundProxyMode::Auto)
    }

    pub fn env() -> Self {
        Self::from_outbound_proxy_mode(OutboundProxyMode::Env)
    }

    pub fn system() -> Self {
        Self::from_outbound_proxy_mode(OutboundProxyMode::System)
    }

    pub fn direct() -> Self {
        Self::from_outbound_proxy_mode(OutboundProxyMode::Direct)
    }

    /// Returns the shared outbound proxy policy for route-aware clients.
    pub fn route_config(&self) -> Option<&OutboundProxyConfig> {
        self.route_config.as_ref()
    }

    /// Returns whether clients need a proxy-aware transport rather than a direct socket.
    pub fn requires_route_aware_transport(&self) -> bool {
        self.route_config
            .as_ref()
            .is_some_and(|config| config.mode() != OutboundProxyMode::Direct)
    }

    fn from_outbound_proxy_mode(mode: OutboundProxyMode) -> Self {
        Self {
            route_config: Some(OutboundProxyConfig::new(mode)),
        }
    }
}

pub fn auth_route_config_from_system_proxy_config(
    system_proxy: &SystemProxyFeatureConfigToml,
) -> AuthRouteConfig {
    match system_proxy.mode.unwrap_or_default() {
        SystemProxyFeatureModeToml::Auto => AuthRouteConfig::auto(),
        SystemProxyFeatureModeToml::Env => AuthRouteConfig::env(),
        SystemProxyFeatureModeToml::System => AuthRouteConfig::system(),
        SystemProxyFeatureModeToml::Direct => AuthRouteConfig::direct(),
    }
}

/// Returns the auth route config for the system-proxy startup path.
pub fn bootstrap_auth_route_config_from_system_proxy_config(
    system_proxy: Option<&SystemProxyFeatureConfigToml>,
) -> Option<AuthRouteConfig> {
    system_proxy.map(auth_route_config_from_system_proxy_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn system_proxy_feature_config_maps_modes_and_startup_gate_together() {
        use SystemProxyFeatureModeToml as SystemProxyFeatureMode;

        assert_eq!(
            bootstrap_auth_route_config_from_system_proxy_config(/*system_proxy*/ None),
            None
        );

        let cases = [
            (None, AuthRouteConfig::auto()),
            (Some(SystemProxyFeatureMode::Auto), AuthRouteConfig::auto()),
            (Some(SystemProxyFeatureMode::Env), AuthRouteConfig::env()),
            (
                Some(SystemProxyFeatureMode::Direct),
                AuthRouteConfig::direct(),
            ),
            (
                Some(SystemProxyFeatureMode::System),
                AuthRouteConfig::system(),
            ),
        ];

        for (mode, expected_config) in cases {
            let system_proxy = SystemProxyFeatureConfigToml {
                enabled: Some(true),
                mode,
            };
            assert_eq!(
                auth_route_config_from_system_proxy_config(&system_proxy),
                expected_config
            );
            assert_eq!(
                bootstrap_auth_route_config_from_system_proxy_config(Some(&system_proxy)),
                Some(expected_config)
            );
        }

        assert!(AuthRouteConfig::auto().requires_route_aware_transport());
        assert!(AuthRouteConfig::env().requires_route_aware_transport());
        assert!(AuthRouteConfig::system().requires_route_aware_transport());
        assert!(!AuthRouteConfig::direct().requires_route_aware_transport());
        assert!(!AuthRouteConfig::default().requires_route_aware_transport());
    }
}
