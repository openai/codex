use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;

use crate::NetworkDomainPermissions;
use crate::NetworkMode;
use crate::NetworkProxyAuditMetadata;
use crate::NetworkProxyConfig;
use crate::NetworkUnixSocketPermissions;

/// Executor-local proxy launch inputs transported with one process start.
///
/// Unlike [`crate::ManagedNetworkSandboxContext`], this describes how the executor should create
/// proxy listeners. The sandbox context is materialized only after those listeners are running.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RemoteNetworkProxyLaunchConfig {
    pub proxy: RemoteNetworkProxyConfig,
    #[serde(default)]
    pub audit_metadata: NetworkProxyAuditMetadata,
    #[serde(default)]
    pub environment_id: Option<String>,
    #[serde(default)]
    pub execution_id: Option<String>,
}

impl RemoteNetworkProxyLaunchConfig {
    pub fn new(proxy: RemoteNetworkProxyConfig) -> Self {
        Self {
            proxy,
            audit_metadata: NetworkProxyAuditMetadata::default(),
            environment_id: None,
            execution_id: None,
        }
    }

    pub fn with_audit_metadata(mut self, audit_metadata: NetworkProxyAuditMetadata) -> Self {
        self.audit_metadata = audit_metadata;
        self
    }

    pub fn for_execution(mut self, environment_id: String, execution_id: String) -> Self {
        self.environment_id = Some(environment_id);
        self.execution_id = Some(execution_id);
        self
    }
}

/// Effective network proxy settings that are safe to send to a remote executor.
///
/// Listener addresses are deliberately omitted because the executor chooses its own loopback
/// ports. MITM, credential injection, and hooks are not represented so their configuration cannot
/// cross the exec-server boundary accidentally.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RemoteNetworkProxyConfig {
    pub enabled: bool,
    pub enable_socks5: bool,
    pub enable_socks5_udp: bool,
    pub allow_upstream_proxy: bool,
    pub dangerously_allow_all_unix_sockets: bool,
    pub mode: NetworkMode,
    pub domains: Option<NetworkDomainPermissions>,
    pub unix_sockets: Option<NetworkUnixSocketPermissions>,
    pub allow_local_binding: bool,
    /// Whether the executor may send policy misses back to the client for a decision.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub request_policy_decisions: bool,
}

impl RemoteNetworkProxyConfig {
    pub fn from_effective_config(config: &NetworkProxyConfig) -> Result<Self> {
        let settings = &config.network;
        ensure!(
            !settings.mitm
                && !settings.credential_broker
                && !settings.dangerously_allow_plaintext_credential_injection
                && settings.mitm_hooks.is_empty(),
            "remote exec-server network proxy does not support MITM, credential injection, or MITM hooks"
        );
        Ok(Self {
            enabled: settings.enabled,
            enable_socks5: settings.enable_socks5,
            enable_socks5_udp: settings.enable_socks5_udp,
            allow_upstream_proxy: settings.allow_upstream_proxy,
            dangerously_allow_all_unix_sockets: settings.dangerously_allow_all_unix_sockets,
            mode: settings.mode,
            domains: settings.domains.clone(),
            unix_sockets: settings.unix_sockets.clone(),
            allow_local_binding: settings.allow_local_binding,
            request_policy_decisions: false,
        })
    }

    pub fn into_network_proxy_config(self) -> NetworkProxyConfig {
        let mut config = NetworkProxyConfig::default();
        config.network.enabled = self.enabled;
        config.network.enable_socks5 = self.enable_socks5;
        config.network.enable_socks5_udp = self.enable_socks5_udp;
        config.network.allow_upstream_proxy = self.allow_upstream_proxy;
        config.network.dangerously_allow_all_unix_sockets = self.dangerously_allow_all_unix_sockets;
        config.network.mode = self.mode;
        config.network.domains = self.domains;
        config.network.unix_sockets = self.unix_sockets;
        config.network.allow_local_binding = self.allow_local_binding;
        config
    }
}

#[cfg(test)]
#[path = "remote_config_tests.rs"]
mod tests;
