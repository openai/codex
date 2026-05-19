use super::CloudRequirementsFragmentSource;
use super::merge_output_source;
use crate::NetworkDomainPermissionsToml;
use crate::NetworkRequirementsToml;
use crate::NetworkUnixSocketPermissionsToml;
use crate::Sourced;

// Network scalar fields are first-wins in bundle order: once a higher-priority
// layer sets a scalar, lower-priority layers cannot change it.
//
// Domain permissions and Unix socket permissions are the notable exception: the
// final value is a union of entries from every cloud layer, not the map from a
// single highest-priority layer. This lets admins split allow/deny entries
// across cloud layers.
//
// When multiple layers define the same key, the highest-priority layer wins for
// that key. Lower-priority layers can add new entries, but they cannot change an
// existing entry from a higher-priority layer.

pub(super) fn merge_network(
    target: &mut Option<Sourced<NetworkRequirementsToml>>,
    incoming: Option<NetworkRequirementsToml>,
    source_ref: &CloudRequirementsFragmentSource,
) {
    let Some(incoming) = incoming.filter(|network| network != &NetworkRequirementsToml::default())
    else {
        return;
    };
    let Some(existing) = target.as_mut() else {
        *target = Some(Sourced::new(incoming, source_ref.requirement_source()));
        return;
    };

    if merge_network_requirements(&mut existing.value, incoming) {
        merge_output_source(&mut existing.source, source_ref);
    }
}

fn merge_network_requirements(
    existing: &mut NetworkRequirementsToml,
    incoming: NetworkRequirementsToml,
) -> bool {
    // Destructure without `..` so every new network field gets an explicit
    // cloud-composition rule.
    let NetworkRequirementsToml {
        enabled,
        http_port,
        socks_port,
        allow_upstream_proxy,
        dangerously_allow_non_loopback_proxy,
        dangerously_allow_all_unix_sockets,
        domains,
        managed_allowed_domains_only,
        unix_sockets,
        allow_local_binding,
    } = incoming;

    let mut changed = false;
    changed |= fill_optional(&mut existing.enabled, enabled);
    changed |= fill_optional(&mut existing.http_port, http_port);
    changed |= fill_optional(&mut existing.socks_port, socks_port);
    changed |= fill_optional(&mut existing.allow_upstream_proxy, allow_upstream_proxy);
    changed |= fill_optional(
        &mut existing.dangerously_allow_non_loopback_proxy,
        dangerously_allow_non_loopback_proxy,
    );
    changed |= fill_optional(
        &mut existing.dangerously_allow_all_unix_sockets,
        dangerously_allow_all_unix_sockets,
    );
    changed |= merge_domain_permissions(&mut existing.domains, domains);
    changed |= fill_optional(
        &mut existing.managed_allowed_domains_only,
        managed_allowed_domains_only,
    );
    changed |= merge_unix_socket_permissions(&mut existing.unix_sockets, unix_sockets);
    changed |= fill_optional(&mut existing.allow_local_binding, allow_local_binding);
    changed
}

fn fill_optional<T>(target: &mut Option<T>, incoming: Option<T>) -> bool {
    if target.is_none()
        && let Some(value) = incoming
    {
        *target = Some(value);
        return true;
    }
    false
}

fn merge_domain_permissions(
    existing: &mut Option<NetworkDomainPermissionsToml>,
    incoming: Option<NetworkDomainPermissionsToml>,
) -> bool {
    let Some(incoming) = incoming.filter(|permissions| !permissions.is_empty()) else {
        return false;
    };
    let Some(existing) = existing.as_mut() else {
        *existing = Some(incoming);
        return true;
    };

    // Insert all domain entries from every layer into one final map. New domain
    // patterns are appended; duplicate patterns keep the value from the
    // highest-priority layer.
    let mut changed = false;
    for (domain, permission) in incoming.entries {
        if let std::collections::btree_map::Entry::Vacant(entry) = existing.entries.entry(domain) {
            entry.insert(permission);
            changed = true;
        }
    }
    changed
}

fn merge_unix_socket_permissions(
    existing: &mut Option<NetworkUnixSocketPermissionsToml>,
    incoming: Option<NetworkUnixSocketPermissionsToml>,
) -> bool {
    let Some(incoming) = incoming.filter(|permissions| !permissions.is_empty()) else {
        return false;
    };
    let Some(existing) = existing.as_mut() else {
        *existing = Some(incoming);
        return true;
    };

    // Insert all Unix socket entries from every layer into one final map. New
    // socket paths are appended; duplicate paths keep the value from the
    // highest-priority layer.
    let mut changed = false;
    for (path, permission) in incoming.entries {
        if let std::collections::btree_map::Entry::Vacant(entry) = existing.entries.entry(path) {
            entry.insert(permission);
            changed = true;
        }
    }
    changed
}
