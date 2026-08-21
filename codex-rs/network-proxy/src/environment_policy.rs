use crate::NetworkDomainPermissions;
use crate::NetworkProxyConfig;
use crate::NetworkUnixSocketPermission;
use crate::NetworkUnixSocketPermissions;
use serde::Deserialize;
use serde::Serialize;

/// Traffic restrictions supplied by the owner of one execution environment.
///
/// Proxy enablement, listeners, network mode, MITM, and credentials remain outside
/// attachment-owned traffic policy.
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

    /// Applies attachment-owned traffic settings while preserving inherited denials and proxy setup.
    pub fn apply_to(&self, config: &mut NetworkProxyConfig) {
        // Use the owner's domain rules without dropping controller denials.
        let inherited_denials = config.denied_domains().unwrap_or_default();
        config.domains.clone_from(&self.domains);
        for domain in inherited_denials {
            config.upsert_domain_permission(
                domain,
                crate::NetworkDomainPermission::Deny,
                crate::normalize_host,
            );
        }
        let inherited_sockets = config.unix_sockets.take().unwrap_or_default();
        let mut effective_sockets = self.unix_sockets.clone().unwrap_or_default();

        // "Allow all" cannot override a socket denied by either policy.
        let inherited_permits_all = config.dangerously_allow_all_unix_sockets
            && !inherited_sockets
                .entries
                .values()
                .any(|permission| matches!(permission, NetworkUnixSocketPermission::Deny));
        let owner_permits_all = self.dangerously_allow_all_unix_sockets
            && !effective_sockets
                .entries
                .values()
                .any(|permission| matches!(permission, NetworkUnixSocketPermission::Deny));

        // Keep shared socket grants; controller denials always take priority.
        effective_sockets.entries.retain(|path, permission| {
            matches!(permission, NetworkUnixSocketPermission::Deny)
                || inherited_permits_all
                || matches!(
                    inherited_sockets.entries.get(path),
                    Some(NetworkUnixSocketPermission::Allow)
                )
        });
        for (path, permission) in inherited_sockets.entries {
            if owner_permits_all || matches!(permission, NetworkUnixSocketPermission::Deny) {
                effective_sockets.entries.insert(path, permission);
            }
        }

        // Enable permissions only when both controller and owner allow them.
        config.unix_sockets = (!effective_sockets.entries.is_empty()).then_some(effective_sockets);
        config.dangerously_allow_all_unix_sockets = inherited_permits_all && owner_permits_all;
        config.allow_upstream_proxy &= self.allow_upstream_proxy;
        config.allow_local_binding &= self.allow_local_binding;
    }
}
