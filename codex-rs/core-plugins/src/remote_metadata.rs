//! Compares the metadata that determines installed-plugin behavior.
//!
//! Display metadata (including image URLs and store capability badges) is excluded. Callers must
//! still publish the unmodified payload so display consumers receive fresh metadata.

use crate::remote::RemoteInstalledPlugin;
use crate::remote::RemoteMarketplace;

pub(crate) fn installed_plugin_metadata_eq(
    previous: &[RemoteInstalledPlugin],
    current: &[RemoteInstalledPlugin],
) -> bool {
    previous.len() == current.len()
        && previous.iter().zip(current).all(|(previous, current)| {
            previous.marketplace_name == current.marketplace_name
                && previous.id == current.id
                && previous.name == current.name
                && previous.canonical_app_id == current.canonical_app_id
                && previous.version == current.version
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
