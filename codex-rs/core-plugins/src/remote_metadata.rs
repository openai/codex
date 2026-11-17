//! Compares installed runtime inputs separately from remote catalog metadata.
//!
//! Runtime invalidation tracks plugin identity, enablement, canonical connector ownership, and the active bundle identity.
//! Catalog release, policy, and display changes still need to be published to catalog consumers.

use crate::remote::ActiveRemotePluginBundle;
use crate::remote::RemoteInstalledPlugin;
use crate::remote::RemoteMarketplace;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// The account metadata consumed when building installed plugin runtimes.
#[derive(PartialEq, Eq)]
struct InstalledPluginRuntimeState<'a> {
    marketplace_name: &'a str,
    id: &'a str,
    name: &'a str,
    canonical_app_id: Option<&'a str>,
    enabled: bool,
    active_bundle: Option<&'a ActiveRemotePluginBundle>,
}

impl<'a> From<&'a RemoteInstalledPlugin> for InstalledPluginRuntimeState<'a> {
    fn from(plugin: &'a RemoteInstalledPlugin) -> Self {
        Self {
            marketplace_name: &plugin.marketplace_name,
            id: &plugin.id,
            name: &plugin.name,
            canonical_app_id: plugin.canonical_app_id.as_deref(),
            enabled: plugin.enabled,
            active_bundle: plugin
                .enabled
                .then_some(plugin.active_bundle.as_ref())
                .flatten(),
        }
    }
}

impl InstalledPluginRuntimeState<'_> {
    /// Report only changed field names, never their values.
    fn changed_fields(self, current: Self) -> impl Iterator<Item = &'static str> {
        let Self {
            marketplace_name,
            id,
            name,
            canonical_app_id,
            enabled,
            active_bundle,
        } = self;
        [
            (
                "marketplace_name",
                marketplace_name != current.marketplace_name,
            ),
            ("id", id != current.id),
            ("name", name != current.name),
            (
                "canonical_app_id",
                canonical_app_id != current.canonical_app_id,
            ),
            ("enabled", enabled != current.enabled),
            (
                "active_directory_version",
                active_bundle.map(|bundle| &bundle.directory_version)
                    != current
                        .active_bundle
                        .map(|bundle| &bundle.directory_version),
            ),
            (
                "active_remote_plugin_id",
                active_bundle.and_then(|bundle| bundle.remote_plugin_id.as_deref())
                    != current
                        .active_bundle
                        .and_then(|bundle| bundle.remote_plugin_id.as_deref()),
            ),
        ]
        .into_iter()
        .filter_map(|(field, changed)| changed.then_some(field))
    }
}

pub(crate) fn installed_plugin_metadata_eq(
    previous: &[RemoteInstalledPlugin],
    current: &[RemoteInstalledPlugin],
) -> bool {
    previous
        .iter()
        .map(InstalledPluginRuntimeState::from)
        .eq(current.iter().map(InstalledPluginRuntimeState::from))
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
            let mut fields: Vec<_> = InstalledPluginRuntimeState::from(prior)
                .changed_fields(InstalledPluginRuntimeState::from(plugin))
                .collect();
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
