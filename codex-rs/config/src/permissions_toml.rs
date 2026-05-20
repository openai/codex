use std::collections::BTreeMap;

use crate::merge::merge_toml_values;
use codex_network_proxy::NetworkDomainPermission as ProxyNetworkDomainPermission;
use codex_network_proxy::NetworkMode;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkUnixSocketPermission as ProxyNetworkUnixSocketPermission;
use codex_network_proxy::normalize_host;
use codex_protocol::permissions::FileSystemAccessMode;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use toml::Value as TomlValue;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct PermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, PermissionProfileToml>,
}

impl PermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn resolve_profile<F>(
        &self,
        profile_name: &str,
        mut parent_profile: F,
    ) -> Result<ResolvedPermissionProfileToml, PermissionProfileResolutionError>
    where
        F: FnMut(&str) -> Option<PermissionProfileToml>,
    {
        let mut profile_names = Vec::new();
        let mut profiles = Vec::new();
        let mut next_profile_name = profile_name.to_string();
        let mut referenced_by: Option<String> = None;

        loop {
            if let Some(cycle_start) = profile_names
                .iter()
                .position(|name| name == &next_profile_name)
            {
                let cycle = profile_names[cycle_start..]
                    .iter()
                    .cloned()
                    .chain(std::iter::once(next_profile_name))
                    .collect::<Vec<_>>();
                return Err(PermissionProfileResolutionError::Cycle { cycle });
            }

            let profile = self
                .entries
                .get(&next_profile_name)
                .cloned()
                .or_else(|| parent_profile(&next_profile_name))
                .ok_or_else(|| {
                    referenced_by.as_deref().map_or_else(
                        || PermissionProfileResolutionError::UndefinedProfile {
                            profile_name: next_profile_name.clone(),
                        },
                        |referenced_by| {
                            if next_profile_name.starts_with(':') {
                                PermissionProfileResolutionError::UnsupportedBuiltInParent {
                                    profile_name: referenced_by.to_string(),
                                    parent_profile_name: next_profile_name.clone(),
                                }
                            } else {
                                PermissionProfileResolutionError::UndefinedParent {
                                    profile_name: referenced_by.to_string(),
                                    parent_profile_name: next_profile_name.clone(),
                                }
                            }
                        },
                    )
                })?;
            let parent_profile_name = profile.extends.clone();

            profile_names.push(next_profile_name.clone());

            if let Some(parent_profile_name) = parent_profile_name {
                profiles.push(profile);
                referenced_by = Some(next_profile_name);
                next_profile_name = parent_profile_name;
                continue;
            }

            let profile = profiles
                .into_iter()
                .rev()
                .try_fold(profile, merge_permission_profiles)?;
            return Ok(ResolvedPermissionProfileToml {
                profile,
                inherited_profile_names: profile_names.into_iter().skip(1).collect(),
            });
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PermissionProfileToml {
    pub description: Option<String>,
    pub extends: Option<String>,
    pub workspace_roots: Option<WorkspaceRootsToml>,
    pub filesystem: Option<FilesystemPermissionsToml>,
    pub network: Option<NetworkToml>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPermissionProfileToml {
    pub profile: PermissionProfileToml,
    /// Names of profiles inherited while resolving `profile`, ordered from the
    /// selected profile's direct parent to the farthest ancestor.
    ///
    /// Callers use this to preserve which built-in baseline contributed the
    /// resolved permissions after the parent profiles have been merged away.
    pub inherited_profile_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PermissionProfileResolutionError {
    #[error("default_permissions refers to undefined profile `{profile_name}`")]
    UndefinedProfile { profile_name: String },
    #[error(
        "permissions profile `{profile_name}` extends undefined profile `{parent_profile_name}`"
    )]
    UndefinedParent {
        profile_name: String,
        parent_profile_name: String,
    },
    #[error(
        "permissions profile `{profile_name}` cannot extend unsupported built-in profile `{parent_profile_name}`"
    )]
    UnsupportedBuiltInParent {
        profile_name: String,
        parent_profile_name: String,
    },
    #[error(
        "permissions profile inheritance cycle detected: {}",
        cycle.join(" -> ")
    )]
    Cycle { cycle: Vec<String> },
    #[error("failed to serialize permissions profile while resolving inheritance: {source}")]
    SerializeProfileToml {
        #[source]
        source: toml::ser::Error,
    },
    #[error(
        "failed to deserialize merged permissions profile while resolving inheritance: {source}"
    )]
    DeserializeProfileToml {
        #[source]
        source: toml::de::Error,
    },
}

fn merge_permission_profiles(
    mut parent: PermissionProfileToml,
    mut child: PermissionProfileToml,
) -> Result<PermissionProfileToml, PermissionProfileResolutionError> {
    let merges_network_domains = parent
        .network
        .as_ref()
        .and_then(|network| network.domains.as_ref())
        .is_some()
        && child
            .network
            .as_ref()
            .and_then(|network| network.domains.as_ref())
            .is_some();

    // Description and inheritance metadata belong to the selected profile
    // declaration, so an inherited profile must not fill those gaps.
    parent.description = None;
    parent.extends = None;

    if merges_network_domains {
        normalize_profile_network_domains(&mut parent);
        normalize_profile_network_domains(&mut child);
    }

    let mut merged = TomlValue::try_from(parent)
        .map_err(|source| PermissionProfileResolutionError::SerializeProfileToml { source })?;
    let child = TomlValue::try_from(child)
        .map_err(|source| PermissionProfileResolutionError::SerializeProfileToml { source })?;
    merge_toml_values(&mut merged, &child);
    merged
        .try_into()
        .map_err(|source| PermissionProfileResolutionError::DeserializeProfileToml { source })
}

fn normalize_profile_network_domains(profile: &mut PermissionProfileToml) {
    let Some(domains) = profile
        .network
        .as_mut()
        .and_then(|network| network.domains.as_mut())
    else {
        return;
    };

    let entries = std::mem::take(&mut domains.entries);
    domains.entries = entries
        .into_iter()
        .map(|(pattern, permission)| (normalize_host(&pattern), permission))
        .collect();
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
pub struct WorkspaceRootsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

impl WorkspaceRootsToml {
    pub fn enabled_roots(&self) -> impl Iterator<Item = &String> {
        self.entries
            .iter()
            .filter_map(|(path, enabled)| (*enabled).then_some(path))
    }
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum NetworkModeSchema {
    Limited,
    Full,
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
