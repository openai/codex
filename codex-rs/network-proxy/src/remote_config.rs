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
}

impl RemoteNetworkProxyConfig {
    pub fn from_effective_config(config: &NetworkProxyConfig) -> Result<Self> {
        ensure!(
            !config.enabled
                || (!config.mitm
                    && !config.credential_broker
                    && !config.dangerously_allow_plaintext_credential_injection
                    && config.mitm_hooks.is_empty()),
            "remote exec-server network proxy does not support MITM, credential injection, or MITM hooks"
        );
        Ok(Self {
            enabled: config.enabled,
            enable_socks5: config.enable_socks5,
            enable_socks5_udp: config.enable_socks5_udp,
            allow_upstream_proxy: config.allow_upstream_proxy,
            dangerously_allow_all_unix_sockets: config.dangerously_allow_all_unix_sockets,
            mode: config.mode,
            domains: config.domains.clone(),
            unix_sockets: config.unix_sockets.clone(),
            allow_local_binding: config.allow_local_binding,
        })
    }

    pub(crate) fn into_network_proxy_config(self) -> NetworkProxyConfig {
        NetworkProxyConfig {
            enabled: self.enabled,
            enable_socks5: self.enable_socks5,
            enable_socks5_udp: self.enable_socks5_udp,
            allow_upstream_proxy: self.allow_upstream_proxy,
            dangerously_allow_all_unix_sockets: self.dangerously_allow_all_unix_sockets,
            mode: self.mode,
            domains: self.domains,
            unix_sockets: self.unix_sockets,
            allow_local_binding: self.allow_local_binding,
            ..NetworkProxyConfig::default()
        }
    }
}

#[cfg(test)]
#[path = "remote_config_tests.rs"]
mod tests;
