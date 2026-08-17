use crate::NetworkDomainPermissions;
use crate::NetworkProxyConfig;
use crate::NetworkUnixSocketPermissions;
use serde::Deserialize;
use serde::Serialize;

/// Traffic restrictions supplied by the owner of one execution environment.
///
/// Proxy enablement, listeners, network mode, MITM, and credentials remain owned by the
/// controller's network-proxy runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentNetworkPolicy {
    pub domains: Option<NetworkDomainPermissions>,
    pub unix_sockets: Option<NetworkUnixSocketPermissions>,
    pub allow_upstream_proxy: bool,
    pub dangerously_allow_all_unix_sockets: bool,
    pub allow_local_binding: bool,
    pub managed_allowed_domains_only: bool,
}

impl EnvironmentNetworkPolicy {
    /// Captures portable traffic restrictions without exposing controller runtime settings.
    pub fn from_config(config: &NetworkProxyConfig, managed_allowed_domains_only: bool) -> Self {
        Self {
            domains: config.domains.clone(),
            unix_sockets: config.unix_sockets.clone(),
            allow_upstream_proxy: config.allow_upstream_proxy,
            dangerously_allow_all_unix_sockets: config.dangerously_allow_all_unix_sockets,
            allow_local_binding: config.allow_local_binding,
            managed_allowed_domains_only,
        }
    }
}
