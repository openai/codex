//! Project proxy settings into environment policies and compose them with controller restrictions.

use crate::EnvironmentNetworkPolicy;
use crate::NetworkProxyConfig;
use crate::NetworkUnixSocketPermission;

/// Captures proxy activation and traffic restrictions without controller runtime settings.
pub fn environment_policy_from_config(
    config: &NetworkProxyConfig,
    managed_allowed_domains_only: bool,
) -> EnvironmentNetworkPolicy {
    EnvironmentNetworkPolicy {
        requires_proxy: config.enabled,
        transparent_tcp: config.transparent_tcp,
        domains: config.domains.clone(),
        review_domains: config.review_domains.clone(),
        unix_sockets: config.unix_sockets.clone(),
        allow_upstream_proxy: config.allow_upstream_proxy,
        dangerously_allow_all_unix_sockets: config
            .dangerously_allow_all_unix_sockets
            .unwrap_or(false),
        allow_local_binding: config.allow_local_binding,
        managed_allowed_domains_only,
    }
}

/// Applies attachment-owned traffic settings while preserving inherited denials and proxy setup.
pub fn apply_environment_policy(
    policy: &EnvironmentNetworkPolicy,
    config: &mut NetworkProxyConfig,
) {
    config.transparent_tcp |= policy.transparent_tcp;
    // Neither owner nor controller may remove the other's review selectors.
    for pattern in &policy.review_domains {
        if !config
            .review_domains
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(pattern))
        {
            config.review_domains.push(pattern.clone());
        }
    }
    // Use the owner's domain rules without dropping controller denials.
    let inherited_denials = config.denied_domains().unwrap_or_default();
    config.domains.clone_from(&policy.domains);
    for domain in inherited_denials {
        config.upsert_domain_permission(
            domain,
            crate::NetworkDomainPermission::Deny,
            crate::normalize_host,
        );
    }
    // An omitted controller policy leaves socket permissions to the attachment;
    // an explicitly empty socket map still supplies a restrictive ceiling.
    let inherited_allow_all = config
        .dangerously_allow_all_unix_sockets
        .unwrap_or(config.unix_sockets.is_none());
    let inherited_sockets = config.unix_sockets.take().unwrap_or_default();
    let mut effective_sockets = policy.unix_sockets.clone().unwrap_or_default();

    // "Allow all" cannot override a socket denied by either policy.
    let inherited_permits_all = inherited_allow_all
        && !inherited_sockets
            .entries
            .values()
            .any(|permission| matches!(permission, NetworkUnixSocketPermission::Deny));
    let owner_permits_all = policy.dangerously_allow_all_unix_sockets
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

    // Socket and upstream-proxy permissions require both controller and owner grants.
    config.unix_sockets = (!effective_sockets.entries.is_empty()).then_some(effective_sockets);
    config.dangerously_allow_all_unix_sockets = Some(inherited_permits_all && owner_permits_all);
    config.allow_upstream_proxy &= policy.allow_upstream_proxy;
    // Either explicit denial wins; defer unresolved defaults to the executor.
    config.allow_local_binding = match (config.allow_local_binding, policy.allow_local_binding) {
        (Some(controller), Some(owner)) => Some(controller && owner),
        (controller, owner) => controller.or(owner),
    };
}
