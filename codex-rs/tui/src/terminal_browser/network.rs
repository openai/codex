//! Network availability shared by terminal-browser tools and panel commands.

use std::net::SocketAddr;

use crate::legacy_core::config::Config;
use crate::session_state::SessionNetworkProxyRuntime;

/// Effective network modes relevant to the Carbonyl runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalBrowserNetworkAvailability {
    Direct,
    ManagedProxy { http_addr: SocketAddr },
    Restricted,
    ManagedProxyUnavailable,
    ManagedProxyMitmUnsupported,
}

impl TerminalBrowserNetworkAvailability {
    pub(crate) fn from_config_and_runtime(
        config: &Config,
        network_proxy: Option<&SessionNetworkProxyRuntime>,
    ) -> Self {
        if !config.permissions.network_sandbox_policy().is_enabled() {
            Self::Restricted
        } else if let Some(network) = config.permissions.network.as_ref() {
            if network.mitm_enabled() {
                return Self::ManagedProxyMitmUnsupported;
            }
            let Some(network_proxy) = network_proxy else {
                return Self::ManagedProxyUnavailable;
            };
            if network_proxy.mitm {
                return Self::ManagedProxyMitmUnsupported;
            }
            let Ok(http_addr) = network_proxy.http_addr.parse::<SocketAddr>() else {
                return Self::ManagedProxyUnavailable;
            };
            if !http_addr.ip().is_loopback() || http_addr.port() == 0 {
                return Self::ManagedProxyUnavailable;
            }
            Self::ManagedProxy { http_addr }
        } else {
            Self::Direct
        }
    }

    pub(crate) fn dynamic_tools_supported(config: &Config) -> bool {
        config.permissions.network_sandbox_policy().is_enabled()
            && config
                .permissions
                .network
                .as_ref()
                .is_none_or(|network| !network.mitm_enabled())
    }

    pub(crate) fn unavailable_message(self) -> Option<&'static str> {
        match self {
            Self::Direct | Self::ManagedProxy { .. } => None,
            Self::Restricted => Some("Terminal browser requires network access for this session."),
            Self::ManagedProxyUnavailable => Some(
                "Terminal browser is unavailable because the managed network proxy is not ready for this session.",
            ),
            Self::ManagedProxyMitmUnsupported => Some(
                "Terminal browser is unavailable because managed TLS interception is not supported.",
            ),
        }
    }
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
