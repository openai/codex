//! Network availability shared by terminal-browser tools and panel commands.

use crate::legacy_core::config::Config;

/// Effective network modes relevant to the Carbonyl runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalBrowserNetworkAvailability {
    Direct,
    Restricted,
    ManagedProxyUnsupported,
}

impl TerminalBrowserNetworkAvailability {
    pub(crate) fn from_config(config: &Config) -> Self {
        if !config.permissions.network_sandbox_policy().is_enabled() {
            Self::Restricted
        } else if config.permissions.network.is_some() {
            Self::ManagedProxyUnsupported
        } else {
            Self::Direct
        }
    }

    pub(crate) fn unavailable_message(self) -> Option<&'static str> {
        match self {
            Self::Direct => None,
            Self::Restricted => Some("Terminal browser requires network access for this session."),
            Self::ManagedProxyUnsupported => Some(
                "Terminal browser is unavailable because managed network requirements are not supported yet.",
            ),
        }
    }
}
