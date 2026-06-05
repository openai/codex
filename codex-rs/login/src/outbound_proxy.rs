use codex_client::OutboundProxyConfig;
use codex_client::OutboundProxyMode;
use codex_config::types::SystemProxyFeatureConfigToml;
use codex_config::types::SystemProxyFeatureModeToml;

/// Stable route-selection policy for auth-owned clients.
///
/// Auth call sites should accept this type instead of lower-level resolver
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

    pub(crate) fn route_config(&self) -> Option<&OutboundProxyConfig> {
        self.route_config.as_ref()
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

/// Returns the auth route config for the explicit platform system-proxy startup path.
pub fn bootstrap_auth_route_config_from_system_proxy_config(
    system_proxy: Option<&SystemProxyFeatureConfigToml>,
) -> Option<AuthRouteConfig> {
    let system_proxy = system_proxy?;
    if system_proxy.mode != Some(SystemProxyFeatureModeToml::System) {
        return None;
    }

    // Legacy startup auth builders only need the explicit platform system-proxy selection here.
    Some(auth_route_config_from_system_proxy_config(system_proxy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn system_proxy_feature_config_maps_modes_and_startup_gate_together() {
        use SystemProxyFeatureModeToml as SystemProxyFeatureMode;

        assert_eq!(
            bootstrap_auth_route_config_from_system_proxy_config(None),
            None
        );

        let system_config = AuthRouteConfig {
            route_config: Some(OutboundProxyConfig::new(OutboundProxyMode::System)),
        };
        let cases = [
            (None, AuthRouteConfig::auto(), None),
            (
                Some(SystemProxyFeatureMode::Auto),
                AuthRouteConfig::auto(),
                None,
            ),
            (
                Some(SystemProxyFeatureMode::Env),
                AuthRouteConfig::env(),
                None,
            ),
            (
                Some(SystemProxyFeatureMode::Direct),
                AuthRouteConfig::direct(),
                None,
            ),
            (
                Some(SystemProxyFeatureMode::System),
                system_config.clone(),
                Some(system_config),
            ),
        ];

        for (mode, expected_config, expected_startup_config) in cases {
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
                expected_startup_config
            );
        }
    }
}
