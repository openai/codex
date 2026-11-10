//! Compares the metadata that determines installed-plugin behavior.
//!
//! Display metadata (including image URLs and store capability badges) is excluded. Callers must
//! still publish the unmodified payload so display consumers receive fresh metadata.

use crate::remote::RemoteInstalledPlugin;
use crate::remote::RemoteMarketplace;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Use the same comparisons for invalidation and its diagnostics. Expose field names only.
fn changed_fields<'a>(
    previous: &'a RemoteInstalledPlugin,
    current: &'a RemoteInstalledPlugin,
) -> impl Iterator<Item = &'static str> + 'a {
    macro_rules! compare {
        ($($field:ident),+ $(,)?) => {
            [$( (stringify!($field), previous.$field != current.$field) ),+]
        };
    }
    compare!(
        marketplace_name,
        id,
        name,
        canonical_app_id,
        version,
        installed_at,
        enabled,
        install_policy,
        install_policy_source,
        must_show_installation_interstitial,
        auth_policy,
        availability,
        disabled_reason,
        eligible_plan_types,
    )
    .into_iter()
    .filter_map(|(field, changed)| changed.then_some(field))
}

pub(crate) fn installed_plugin_metadata_eq(
    previous: &[RemoteInstalledPlugin],
    current: &[RemoteInstalledPlugin],
) -> bool {
    previous.len() == current.len()
        && previous
            .iter()
            .zip(current)
            .all(|(previous, current)| changed_fields(previous, current).next().is_none())
}

/// Called only for a changed snapshot. Match by remote ID so an insertion does
/// not misattribute every subsequent plugin's fields to its neighbor.
pub(crate) fn installed_plugin_metadata_changes(
    previous: &[RemoteInstalledPlugin],
    current: &[RemoteInstalledPlugin],
) -> BTreeMap<String, Vec<&'static str>> {
    let mut old: HashMap<_, _> = previous
        .iter()
        .enumerate()
        .map(|(index, p)| (p.id.as_str(), (index, p)))
        .collect();
    // A removed plugin shifts subsequent indices without reordering the surviving list.
    let same_members =
        previous.len() == current.len() && current.iter().all(|p| old.contains_key(p.id.as_str()));
    let mut changes = BTreeMap::new();
    for (index, plugin) in current.iter().enumerate() {
        let fields = if let Some((old_index, prior)) = old.remove(plugin.id.as_str()) {
            let mut fields: Vec<_> = changed_fields(prior, plugin).collect();
            if old_index != index && same_members {
                fields.push("position");
            }
            fields
        } else {
            vec!["added"]
        };
        if !fields.is_empty() {
            changes.insert(plugin.id.clone(), fields);
        }
    }
    for id in old.into_keys() {
        changes.insert(id.to_owned(), vec!["removed"]);
    }
    changes
}

/// Compare installed catalog identity, versions, enablement, policy and availability.
///
/// Plugin display order is ignored. Marketplace ordering remains significant.
/// Callers must still store the unmodified current catalog for display consumers.
pub fn remote_catalog_metadata_eq(
    previous: &[RemoteMarketplace],
    current: &[RemoteMarketplace],
) -> bool {
    previous.len() == current.len()
        && previous.iter().zip(current).all(|(previous, current)| {
            if previous.name != current.name || previous.plugins.len() != current.plugins.len() {
                return false;
            }
            // Core sorts this view by display name, which can change independently of behavior.
            let mut previous = previous.plugins.iter().collect::<Vec<_>>();
            let mut current = current.plugins.iter().collect::<Vec<_>>();
            previous.sort_unstable_by(|a, b| a.remote_plugin_id.cmp(&b.remote_plugin_id));
            current.sort_unstable_by(|a, b| a.remote_plugin_id.cmp(&b.remote_plugin_id));
            previous
                .into_iter()
                .zip(current)
                .all(|(previous, current)| {
                    previous.id == current.id
                        && previous.remote_plugin_id == current.remote_plugin_id
                        && previous.name == current.name
                        && previous.version == current.version
                        && previous.local_version == current.local_version
                        && previous.installed == current.installed
                        && previous.installed_at == current.installed_at
                        && previous.enabled == current.enabled
                        && previous.install_policy == current.install_policy
                        && previous.install_policy_source == current.install_policy_source
                        && previous.must_show_installation_interstitial
                            == current.must_show_installation_interstitial
                        && previous.auth_policy == current.auth_policy
                        && previous.availability == current.availability
                        && previous.disabled_reason == current.disabled_reason
                        && previous.eligible_plan_types == current.eligible_plan_types
                })
        })
}

#[cfg(test)]
#[path = "remote_metadata_tests.rs"]
mod tests;
