use codex_client::OutboundProxyConfig;
use codex_client::OutboundProxyMode;
use codex_config::types::NetworkConfigToml;
use codex_config::types::NetworkProxyMode;

pub(crate) fn outbound_proxy_config_from_network_config(
    network: &NetworkConfigToml,
) -> OutboundProxyConfig {
    let mode = match network.proxy_mode.unwrap_or_default() {
        NetworkProxyMode::Auto => OutboundProxyMode::Auto,
        NetworkProxyMode::Env => OutboundProxyMode::Env,
        // Keep non-Windows users on the legacy path until the resolver-backed
        // system proxy rollout expands beyond Windows.
        NetworkProxyMode::System if system_proxy_mode_enabled() => OutboundProxyMode::System,
        NetworkProxyMode::System => OutboundProxyMode::Auto,
        NetworkProxyMode::Direct => OutboundProxyMode::Direct,
    };
    OutboundProxyConfig {
        mode,
        proxy_url: network.proxy_url.clone(),
    }
}

/// Returns the auth proxy config for the explicit Windows system-proxy startup path.
pub fn windows_system_proxy_config_from_network_config(
    network: Option<&NetworkConfigToml>,
) -> Option<OutboundProxyConfig> {
    let network = network?;
    if !system_proxy_mode_enabled()
        || network.proxy_mode != Some(NetworkProxyMode::System)
        || network.proxy_url.is_some()
    {
        return None;
    }

    // Legacy startup auth builders only need the explicit Windows system-proxy selection here.
    Some(outbound_proxy_config_from_network_config(network))
}

const fn system_proxy_mode_enabled() -> bool {
    cfg!(target_os = "windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_mode_preserves_legacy_auto_path() {
        let network = NetworkConfigToml::default();

        let config = outbound_proxy_config_from_network_config(&network);

        assert_eq!(config.mode, OutboundProxyMode::Auto);
    }

    #[test]
    fn explicit_system_mode_is_windows_only() {
        let network = NetworkConfigToml {
            proxy_mode: Some(NetworkProxyMode::System),
            proxy_url: None,
        };

        let config = outbound_proxy_config_from_network_config(&network);

        let expected = if cfg!(target_os = "windows") {
            OutboundProxyMode::System
        } else {
            OutboundProxyMode::Auto
        };
        assert_eq!(config.mode, expected);
    }

    #[test]
    fn startup_system_proxy_config_is_scoped_to_windows_pac_path() {
        let network = NetworkConfigToml {
            proxy_mode: Some(NetworkProxyMode::System),
            proxy_url: None,
        };

        let config = windows_system_proxy_config_from_network_config(Some(&network));

        if cfg!(target_os = "windows") {
            assert_eq!(
                config,
                Some(outbound_proxy_config_from_network_config(&network))
            );
        } else {
            assert_eq!(config, None);
        }
    }

    #[test]
    fn startup_system_proxy_config_skips_concrete_proxy_override() {
        let network = NetworkConfigToml {
            proxy_mode: Some(NetworkProxyMode::System),
            proxy_url: Some("http://proxy.example.test:8080".to_string()),
        };

        assert_eq!(
            windows_system_proxy_config_from_network_config(Some(&network)),
            None
        );
    }
}
