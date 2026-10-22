use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use crate::CredentialProviderConfig;
use crate::NetworkDomainPermissions;
use crate::NetworkMode;
use crate::NetworkProxyAuditMetadata;
use crate::NetworkProxyConfig;
use crate::NetworkUnixSocketPermissions;
use crate::credential_broker::CredentialBroker;

/// Credential mappings retained by one executor session across process launches.
/// Only mappings are shared; proxy listeners, network policy, and approvals remain per command.
#[derive(Default)]
pub struct RemoteCredentialBrokerCache {
    brokers: Mutex<Vec<CachedCredentialBroker>>,
}

struct CachedCredentialBroker {
    environment_id: Option<String>,
    conversation_id: Option<String>,
    config: NetworkProxyConfig,
    broker: CredentialBroker,
}

impl RemoteCredentialBrokerCache {
    pub(crate) fn broker_if_enabled(
        &self,
        environment_id: Option<&str>,
        conversation_id: Option<&str>,
        config: &NetworkProxyConfig,
    ) -> Option<CredentialBroker> {
        if !config.credential_broker {
            return None;
        }
        // Retain broker configuration and the command's destination context so ordinary
        // network-policy changes preserve placeholders, while destination changes cannot
        // invalidate another command's mappings.
        let config = NetworkProxyConfig {
            credential_broker: config.credential_broker,
            allow_local_binding: Some(config.allow_local_binding()),
            credential_broker_openai_host: config.credential_broker_openai_host.clone(),
            credential_broker_context: config.credential_broker_context.clone(),
            credential_providers: config.credential_providers.clone(),
            ..NetworkProxyConfig::default()
        };
        let mut brokers = self
            .brokers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = brokers.iter().find(|cached| {
            cached.environment_id.as_deref() == environment_id
                && cached.conversation_id.as_deref() == conversation_id
                && cached.config == config
        }) {
            return Some(cached.broker.clone());
        }
        let broker = CredentialBroker::new(config.credential_broker);
        broker.configure(&config);
        brokers.push(CachedCredentialBroker {
            environment_id: environment_id.map(str::to_owned),
            conversation_id: conversation_id.map(str::to_owned),
            config,
            broker: broker.clone(),
        });
        Some(broker)
    }
}

/// Credential masking supported by an executor or requested for a remote launch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RemoteCredentialMasking {
    #[default]
    Disabled,
    /// Mask only when the configuration contains no custom providers.
    BuiltInOnly,
    WithConfiguredProviders,
}

/// Masking fields shared with independently deployed executors.
#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
struct CredentialMaskingConfig {
    #[serde(
        rename = "credentialMasking",
        skip_serializing_if = "std::ops::Not::not"
    )]
    enabled: bool,
    #[serde(
        rename = "credentialProviders",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    providers: BTreeMap<String, CredentialProviderConfig>,
}

impl CredentialMaskingConfig {
    fn deserialize_normalized<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let mut config = Self::deserialize(deserializer)?;
        // Discard provider rules sent with masking off, even if we forward this configuration.
        if !config.enabled {
            config.providers.clear();
        }
        Ok(config)
    }
}

impl fmt::Debug for CredentialMaskingConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialMaskingConfig")
            .field("enabled", &self.enabled)
            .field("credential_provider_count", &self.providers.len())
            .finish()
    }
}

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
    /// Controller-side policy decision budget. The executor adds transport overhead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_decision_timeout_ms: Option<u64>,
}

impl RemoteNetworkProxyLaunchConfig {
    pub fn new(proxy: RemoteNetworkProxyConfig) -> Self {
        Self {
            proxy,
            audit_metadata: NetworkProxyAuditMetadata::default(),
            environment_id: None,
            execution_id: None,
            policy_decision_timeout_ms: None,
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
/// ports. Masking carries provider rules; credential values and destination context are resolved
/// on the executor. CA configuration and MITM hooks stay on their execution host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RemoteNetworkProxyConfig {
    pub enabled: bool,
    #[serde(
        flatten,
        deserialize_with = "CredentialMaskingConfig::deserialize_normalized"
    )]
    credential_masking: CredentialMaskingConfig,
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
            config.mitm_ca.is_none()
                && (!config.enabled
                    || (!config.mitm
                        && !config.credential_broker
                        && !config.dangerously_allow_plaintext_credential_injection
                        && config.mitm_hooks.is_empty())),
            "remote exec-server network proxy does not support MITM, external CAs, credential injection, or MITM hooks"
        );
        Ok(Self {
            enabled: config.enabled,
            credential_masking: CredentialMaskingConfig::default(),
            enable_socks5: config.enable_socks5,
            enable_socks5_udp: config.enable_socks5_udp,
            allow_upstream_proxy: config.allow_upstream_proxy,
            dangerously_allow_all_unix_sockets: config
                .dangerously_allow_all_unix_sockets
                .unwrap_or(false),
            mode: config.mode,
            domains: config.domains.clone(),
            unix_sockets: config.unix_sockets.clone(),
            allow_local_binding: config.allow_local_binding(),
        })
    }

    /// Enable masking with the given provider rules, or just the built-ins if empty.
    pub fn enable_credential_masking(
        &mut self,
        providers: BTreeMap<String, CredentialProviderConfig>,
    ) {
        self.credential_masking = CredentialMaskingConfig {
            enabled: true,
            providers,
        };
    }

    /// Whether the executor should mask credentials for this launch.
    pub fn credential_masking_enabled(&self) -> bool {
        self.credential_masking.enabled
    }

    pub(crate) fn into_network_proxy_config(self) -> NetworkProxyConfig {
        let mut config = NetworkProxyConfig {
            enabled: self.enabled,
            enable_socks5: self.enable_socks5,
            enable_socks5_udp: self.enable_socks5_udp,
            allow_upstream_proxy: self.allow_upstream_proxy,
            dangerously_allow_all_unix_sockets: Some(self.dangerously_allow_all_unix_sockets),
            mode: self.mode,
            domains: self.domains,
            unix_sockets: self.unix_sockets,
            allow_local_binding: Some(self.allow_local_binding),
            ..NetworkProxyConfig::default()
        };
        if self.credential_masking.enabled {
            config.set_credential_broker_enabled(/*enabled*/ true);
            config.credential_providers = self.credential_masking.providers;
        }
        config
    }
}

#[cfg(test)]
#[path = "remote_config_tests.rs"]
mod tests;
