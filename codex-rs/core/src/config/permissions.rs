use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkProxyConfig;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PermissionsToml {
    /// Network proxy settings from `[permissions.network]`.
    /// User config can enable the proxy; managed requirements may still constrain values.
    pub network: Option<NetworkToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NetworkToml {
    pub enabled: Option<bool>,
    pub proxy_url: Option<String>,
    pub admin_url: Option<String>,
    pub enable_socks5: Option<bool>,
    pub socks_url: Option<String>,
    pub enable_socks5_udp: Option<bool>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_admin: Option<bool>,
    #[schemars(with = "Option<NetworkModeSchema>")]
    pub mode: Option<NetworkMode>,
    pub allowed_domains: Option<Vec<String>>,
    pub denied_domains: Option<Vec<String>>,
    pub allow_unix_sockets: Option<Vec<String>>,
    pub allow_local_binding: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum NetworkModeSchema {
    Limited,
    Full,
}

impl NetworkToml {
    pub(crate) fn apply_to_network_proxy_config(&self, config: &mut NetworkProxyConfig) {
        if let Some(enabled) = self.enabled {
            config.network.enabled = enabled;
        }
        if let Some(proxy_url) = self.proxy_url.as_ref() {
            config.network.proxy_url = proxy_url.clone();
        }
        if let Some(admin_url) = self.admin_url.as_ref() {
            config.network.admin_url = admin_url.clone();
        }
        if let Some(enable_socks5) = self.enable_socks5 {
            config.network.enable_socks5 = enable_socks5;
        }
        if let Some(socks_url) = self.socks_url.as_ref() {
            config.network.socks_url = socks_url.clone();
        }
        if let Some(enable_socks5_udp) = self.enable_socks5_udp {
            config.network.enable_socks5_udp = enable_socks5_udp;
        }
        if let Some(allow_upstream_proxy) = self.allow_upstream_proxy {
            config.network.allow_upstream_proxy = allow_upstream_proxy;
        }
        if let Some(dangerously_allow_non_loopback_proxy) =
            self.dangerously_allow_non_loopback_proxy
        {
            config.network.dangerously_allow_non_loopback_proxy =
                dangerously_allow_non_loopback_proxy;
        }
        if let Some(dangerously_allow_non_loopback_admin) =
            self.dangerously_allow_non_loopback_admin
        {
            config.network.dangerously_allow_non_loopback_admin =
                dangerously_allow_non_loopback_admin;
        }
        if let Some(mode) = self.mode {
            config.network.mode = mode;
        }
        if let Some(allowed_domains) = self.allowed_domains.as_ref() {
            config.network.allowed_domains = allowed_domains.clone();
        }
        if let Some(denied_domains) = self.denied_domains.as_ref() {
            config.network.denied_domains = denied_domains.clone();
        }
        if let Some(allow_unix_sockets) = self.allow_unix_sockets.as_ref() {
            config.network.allow_unix_sockets = allow_unix_sockets.clone();
        }
        if let Some(allow_local_binding) = self.allow_local_binding {
            config.network.allow_local_binding = allow_local_binding;
        }
    }

    pub(crate) fn to_network_proxy_config(&self) -> NetworkProxyConfig {
        let mut config = NetworkProxyConfig::default();
        self.apply_to_network_proxy_config(&mut config);
        config
    }
}

pub(crate) fn network_proxy_config_from_permissions(
    permissions: Option<&PermissionsToml>,
) -> NetworkProxyConfig {
    permissions
        .and_then(|permissions| permissions.network.as_ref())
        .map_or_else(
            NetworkProxyConfig::default,
            NetworkToml::to_network_proxy_config,
        )
}
