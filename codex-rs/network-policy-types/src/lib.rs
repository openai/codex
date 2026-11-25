//! Network policy and credential definitions shared by the protocol, configuration, and proxy.
//! The proxy owns policy enforcement and credential validation.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::collections::BTreeMap;

/// Managed proxy requirements supplied by the owner of one execution environment.
///
/// The owner can require proxy routing without granting direct network access.
/// Listeners, network mode, MITM, and credentials remain controller-owned.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentNetworkPolicy {
    /// Requires managed proxy routing even when direct network access is restricted.
    /// False or omission leaves activation to the controller and command permissions.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub requires_proxy: bool,
    /// Route outbound TCP from managed Linux sandbox commands through the managed SOCKS5 proxy.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub transparent_tcp: bool,
    pub domains: Option<NetworkDomainPermissions>,
    /// Managed host selectors inherited by every execution in this environment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_domains: Vec<String>,
    pub unix_sockets: Option<NetworkUnixSocketPermissions>,
    pub allow_upstream_proxy: bool,
    pub dangerously_allow_all_unix_sockets: bool,
    /// Omission inherits local-binding policy; an explicit false remains a restriction.
    pub allow_local_binding: Option<bool>,
    pub managed_allowed_domains_only: bool,
}

/// Variant order encodes effective precedence for duplicate patterns:
/// `None < Allow < Deny`, so deny wins over allow when entries conflict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum NetworkDomainPermission {
    None,
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkDomainPermissionEntry {
    pub pattern: String,
    pub permission: NetworkDomainPermission,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkDomainPermissions {
    pub entries: Vec<NetworkDomainPermissionEntry>,
}

impl Serialize for NetworkDomainPermissions {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.effective_entries()
            .into_iter()
            .map(|entry| (entry.pattern, entry.permission))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NetworkDomainPermissions {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = BTreeMap::<String, NetworkDomainPermission>::deserialize(deserializer)?
            .into_iter()
            .map(|(pattern, permission)| NetworkDomainPermissionEntry {
                pattern,
                permission,
            })
            .collect();
        Ok(Self { entries })
    }
}

impl NetworkDomainPermissions {
    /// Resolve duplicate patterns by precedence, preserving their first-seen order.
    pub fn effective_entries(&self) -> Vec<NetworkDomainPermissionEntry> {
        let mut order = Vec::new();
        let mut effective_permissions = BTreeMap::new();

        for entry in &self.entries {
            if !effective_permissions.contains_key(&entry.pattern) {
                order.push(entry.pattern.clone());
            }

            let permission = effective_permissions
                .entry(entry.pattern.clone())
                .or_insert(entry.permission);
            if entry.permission > *permission {
                *permission = entry.permission;
            }
        }

        order
            .into_iter()
            .filter_map(|pattern| {
                effective_permissions.remove(&pattern).map(|permission| {
                    NetworkDomainPermissionEntry {
                        pattern,
                        permission,
                    }
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkUnixSocketPermission {
    Allow,
    Deny,
}

/// Socket policy keys are preserved across controller and executor hosts.
/// Allow entries must be NUL-free and absolute according to the executor OS;
/// Deny entries are retained without path validation or expansion.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NetworkUnixSocketPermissions {
    #[serde(flatten)]
    pub entries: BTreeMap<String, NetworkUnixSocketPermission>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPolicyDecision {
    Deny,
    Ask,
}

impl NetworkPolicyDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDecisionSource {
    BaselinePolicy,
    ModeGuard,
    ProxyState,
    Decider,
}

impl NetworkDecisionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselinePolicy => "baseline_policy",
            Self::ModeGuard => "mode_guard",
            Self::ProxyState => "proxy_state",
            Self::Decider => "decider",
        }
    }
}

/// Declarative description of an environment-backed credential family.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CredentialProviderConfig {
    pub env: Vec<String>,
    pub patterns: Vec<String>,
    /// URL prefixes authorized for injection. Bare loopback hosts imply HTTP; other bare hosts
    /// imply HTTPS.
    pub url_prefixes: Vec<String>,
    /// Environment variable containing an additional URL prefix or hostname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_prefix_from_env: Option<String>,
    pub auth: Vec<CredentialAuthMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

/// Authentication formats supported by declarative credential providers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialAuthMethod {
    Bearer,
    Token,
    Basic,
    Header,
}
