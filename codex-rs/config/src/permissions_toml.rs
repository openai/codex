use std::collections::BTreeMap;

use codex_network_proxy::InjectedHeaderConfig;
use codex_network_proxy::MitmHookActionsConfig;
use codex_network_proxy::MitmHookBodyConfig;
use codex_network_proxy::MitmHookConfig;
use codex_network_proxy::MitmHookMatchConfig;
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

#[derive(Serialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NetworkMitmToml {
    pub enabled: Option<bool>,
    pub hooks: Option<BTreeMap<String, NetworkMitmHookToml>>,
    pub actions: Option<BTreeMap<String, NetworkMitmActionToml>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkMitmTomlUnchecked {
    pub enabled: Option<bool>,
    pub hooks: Option<BTreeMap<String, NetworkMitmHookToml>>,
    pub actions: Option<BTreeMap<String, NetworkMitmActionToml>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct NetworkMitmHookToml {
    pub host: String,
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
    #[serde(default)]
    pub query: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub headers: BTreeMap<String, Vec<String>>,
    #[schemars(with = "Option<MitmHookBodyConfigSchema>")]
    pub body: Option<MitmHookBodyConfig>,
    pub action: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum NetworkModeSchema {
    Limited,
    Full,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(default)]
pub struct NetworkMitmActionToml {
    pub strip_request_headers: Vec<String>,
    pub inject_request_headers: Vec<NetworkMitmInjectedHeaderToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(default)]
pub struct NetworkMitmInjectedHeaderToml {
    pub name: String,
    pub secret_env_var: Option<String>,
    pub secret_file: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(transparent)]
struct MitmHookBodyConfigSchema(pub serde_json::Value);

impl<'de> Deserialize<'de> for NetworkMitmToml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let unchecked = NetworkMitmTomlUnchecked::deserialize(deserializer)?;
        let mitm = Self {
            enabled: unchecked.enabled,
            hooks: unchecked.hooks,
            actions: unchecked.actions,
        };
        mitm.validate_action_references()
            .map_err(serde::de::Error::custom)?;
        Ok(mitm)
    }
}

impl NetworkMitmToml {
    fn validate_action_references(&self) -> Result<(), String> {
        if let Some(actions) = self.actions.as_ref() {
            for (action_name, action) in actions {
                if action.is_empty() {
                    return Err(format!(
                        "network.mitm.actions.{action_name} must define at least one operation"
                    ));
                }
            }
        }

        let Some(hooks) = self.hooks.as_ref() else {
            return Ok(());
        };

        for (hook_name, hook) in hooks {
            if hook.action.is_empty() {
                return Err(format!(
                    "network.mitm.hooks.{hook_name}.action must not be empty"
                ));
            }

            for action_name in &hook.action {
                if self
                    .actions
                    .as_ref()
                    .is_none_or(|actions| !actions.contains_key(action_name))
                {
                    return Err(format!(
                        "network.mitm.hooks.{hook_name}.action references undefined action `{action_name}`"
                    ));
                }
            }
        }

        Ok(())
    }
}

impl NetworkMitmActionToml {
    fn is_empty(&self) -> bool {
        self.strip_request_headers.is_empty() && self.inject_request_headers.is_empty()
    }
}

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
                config.network.mitm_hooks = hooks
                    .values()
                    .map(|hook| hook.to_runtime(mitm.actions.as_ref()))
                    .collect();
            }
        }
    }

    pub fn to_network_proxy_config(&self) -> NetworkProxyConfig {
        let mut config = NetworkProxyConfig::default();
        self.apply_to_network_proxy_config(&mut config);
        config
    }
}

impl NetworkMitmHookToml {
    fn to_runtime(
        &self,
        actions_by_name: Option<&BTreeMap<String, NetworkMitmActionToml>>,
    ) -> MitmHookConfig {
        MitmHookConfig {
            host: self.host.clone(),
            matcher: MitmHookMatchConfig {
                methods: self.methods.clone(),
                path_prefixes: self.path_prefixes.clone(),
                query: self.query.clone(),
                headers: self.headers.clone(),
                body: self.body.clone(),
            },
            actions: self.selected_actions(actions_by_name),
        }
    }

    fn selected_actions(
        &self,
        actions_by_name: Option<&BTreeMap<String, NetworkMitmActionToml>>,
    ) -> MitmHookActionsConfig {
        let Some(actions_by_name) = actions_by_name else {
            return MitmHookActionsConfig::default();
        };

        let mut selected = MitmHookActionsConfig::default();
        for action_name in &self.action {
            if let Some(action) = actions_by_name.get(action_name) {
                selected
                    .strip_request_headers
                    .extend(action.strip_request_headers.clone());
                selected.inject_request_headers.extend(
                    action
                        .inject_request_headers
                        .iter()
                        .map(NetworkMitmInjectedHeaderToml::to_runtime),
                );
            }
        }
        selected
    }
}

impl NetworkMitmInjectedHeaderToml {
    fn to_runtime(&self) -> InjectedHeaderConfig {
        InjectedHeaderConfig {
            name: self.name.clone(),
            secret_env_var: self.secret_env_var.clone(),
            secret_file: self.secret_file.clone(),
            prefix: self.prefix.clone(),
        }
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
