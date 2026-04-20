use std::collections::BTreeMap;

use codex_network_proxy::MitmHookConfig;
use codex_network_proxy::NetworkDomainPermission as ProxyNetworkDomainPermission;
use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkUnixSocketPermission as ProxyNetworkUnixSocketPermission;
use codex_network_proxy::normalize_host;
use codex_protocol::permissions::FileSystemAccessMode;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct PermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, PermissionProfileToml>,
}

impl PermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PermissionProfileToml {
    pub filesystem: Option<FilesystemPermissionsToml>,
    pub network: Option<NetworkToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct FilesystemPermissionsToml {
    /// Optional maximum depth for expanding unreadable glob patterns on
    /// platforms that snapshot glob matches before sandbox startup.
    #[schemars(range(min = 1))]
    pub glob_scan_max_depth: Option<usize>,
    #[serde(flatten)]
    pub entries: BTreeMap<String, FilesystemPermissionToml>,
}

impl FilesystemPermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum FilesystemPermissionToml {
    Access(FileSystemAccessMode),
    Scoped(BTreeMap<String, FileSystemAccessMode>),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct NetworkDomainPermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, NetworkDomainPermissionToml>,
}

impl NetworkDomainPermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn allowed_domains(&self) -> Option<Vec<String>> {
        let allowed_domains: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, permission)| matches!(permission, NetworkDomainPermissionToml::Allow))
            .map(|(pattern, _)| pattern.clone())
            .collect();
        (!allowed_domains.is_empty()).then_some(allowed_domains)
    }

    pub fn denied_domains(&self) -> Option<Vec<String>> {
        let denied_domains: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, permission)| matches!(permission, NetworkDomainPermissionToml::Deny))
            .map(|(pattern, _)| pattern.clone())
            .collect();
        (!denied_domains.is_empty()).then_some(denied_domains)
    }
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum NetworkDomainPermissionToml {
    Allow,
    Deny,
}

impl std::fmt::Display for NetworkDomainPermissionToml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let permission = match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        };
        f.write_str(permission)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct NetworkUnixSocketPermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, NetworkUnixSocketPermissionToml>,
}

impl NetworkUnixSocketPermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn allow_unix_sockets(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, permission)| matches!(permission, NetworkUnixSocketPermissionToml::Allow))
            .map(|(path, _)| path.clone())
            .collect()
    }
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum NetworkUnixSocketPermissionToml {
    Allow,
    None,
}

impl std::fmt::Display for NetworkUnixSocketPermissionToml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let permission = match self {
            Self::Allow => "allow",
            Self::None => "none",
        };
        f.write_str(permission)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NetworkToml {
    pub enabled: Option<bool>,
    pub proxy_url: Option<String>,
    pub enable_socks5: Option<bool>,
    pub socks_url: Option<String>,
    pub enable_socks5_udp: Option<bool>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    #[schemars(with = "Option<NetworkModeSchema>")]
    pub mode: Option<NetworkMode>,
    pub domains: Option<NetworkDomainPermissionsToml>,
    pub unix_sockets: Option<NetworkUnixSocketPermissionsToml>,
    pub allow_local_binding: Option<bool>,
    pub mitm: Option<NetworkMitmToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NetworkMitmToml {
    pub enabled: Option<bool>,
    #[schemars(with = "Option<Vec<MitmHookConfigSchema>>")]
    pub hooks: Option<Vec<MitmHookConfig>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum NetworkModeSchema {
    Limited,
    Full,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(default)]
struct MitmHookConfigSchema {
    pub host: String,
    #[serde(rename = "match", default)]
    pub matcher: MitmHookMatchConfigSchema,
    #[serde(default)]
    pub actions: MitmHookActionsConfigSchema,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(default)]
struct MitmHookMatchConfigSchema {
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
    pub query: BTreeMap<String, Vec<String>>,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body: Option<MitmHookBodyConfigSchema>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(default)]
struct MitmHookActionsConfigSchema {
    pub strip_request_headers: Vec<String>,
    pub inject_request_headers: Vec<InjectedHeaderConfigSchema>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(default)]
struct InjectedHeaderConfigSchema {
    pub name: String,
    pub secret_env_var: Option<String>,
    pub secret_file: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(transparent)]
struct MitmHookBodyConfigSchema(pub serde_json::Value);

impl NetworkToml {
    pub fn apply_to_network_proxy_config(&self, config: &mut NetworkProxyConfig) {
        if let Some(enabled) = self.enabled {
            config.network.enabled = enabled;
        }
        if let Some(proxy_url) = self.proxy_url.as_ref() {
            config.network.proxy_url = proxy_url.clone();
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
        if let Some(dangerously_allow_all_unix_sockets) = self.dangerously_allow_all_unix_sockets {
            config.network.dangerously_allow_all_unix_sockets = dangerously_allow_all_unix_sockets;
        }
        if let Some(mode) = self.mode {
            config.network.mode = mode;
        }
        if let Some(domains) = self.domains.as_ref() {
            overlay_network_domain_permissions(config, domains);
        }
        if let Some(unix_sockets) = self.unix_sockets.as_ref() {
            let mut proxy_unix_sockets = config.network.unix_sockets.take().unwrap_or_default();
            for (path, permission) in &unix_sockets.entries {
                let permission = match permission {
                    NetworkUnixSocketPermissionToml::Allow => {
                        ProxyNetworkUnixSocketPermission::Allow
                    }
                    NetworkUnixSocketPermissionToml::None => ProxyNetworkUnixSocketPermission::None,
                };
                proxy_unix_sockets.entries.insert(path.clone(), permission);
            }
            config.network.unix_sockets =
                (!proxy_unix_sockets.entries.is_empty()).then_some(proxy_unix_sockets);
        }
        if let Some(allow_local_binding) = self.allow_local_binding {
            config.network.allow_local_binding = allow_local_binding;
        }
        if let Some(mitm) = self.mitm.as_ref() {
            if let Some(enabled) = mitm.enabled {
                config.network.mitm = enabled;
            }
            if let Some(hooks) = mitm.hooks.as_ref() {
                config.network.mitm_hooks = hooks.clone();
            }
        }
    }

    pub fn to_network_proxy_config(&self) -> NetworkProxyConfig {
        let mut config = NetworkProxyConfig::default();
        self.apply_to_network_proxy_config(&mut config);
        config
    }
}

pub fn overlay_network_domain_permissions(
    config: &mut NetworkProxyConfig,
    domains: &NetworkDomainPermissionsToml,
) {
    for (pattern, permission) in &domains.entries {
        let permission = match permission {
            NetworkDomainPermissionToml::Allow => ProxyNetworkDomainPermission::Allow,
            NetworkDomainPermissionToml::Deny => ProxyNetworkDomainPermission::Deny,
        };
        config
            .network
            .upsert_domain_permission(pattern.clone(), permission, normalize_host);
    }
}
