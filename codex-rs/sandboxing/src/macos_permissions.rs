use std::collections::BTreeSet;

use codex_protocol::models::MacOsAutomationPermission;
use codex_protocol::models::MacOsContactsPermission;
use codex_protocol::models::MacOsPreferencesPermission;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;

/// Merges macOS seatbelt profile extensions by taking the permissive union of
/// each permission field.
pub fn merge_macos_seatbelt_profile_extensions(
    base: Option<&MacOsSeatbeltProfileExtensions>,
    permissions: Option<&MacOsSeatbeltProfileExtensions>,
) -> Option<MacOsSeatbeltProfileExtensions> {
    let Some(permissions) = permissions else {
        return base.cloned();
    };

    match base {
        Some(base) => Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: union_macos_preferences_permission(
                &base.macos_preferences,
                &permissions.macos_preferences,
            ),
            macos_automation: union_macos_automation_permission(
                &base.macos_automation,
                &permissions.macos_automation,
            ),
            macos_launch_services: base.macos_launch_services || permissions.macos_launch_services,
            macos_accessibility: base.macos_accessibility || permissions.macos_accessibility,
            macos_calendar: base.macos_calendar || permissions.macos_calendar,
            macos_reminders: base.macos_reminders || permissions.macos_reminders,
            macos_contacts: union_macos_contacts_permission(
                &base.macos_contacts,
                &permissions.macos_contacts,
            ),
        }),
        None => Some(permissions.clone()),
    }
}

pub fn intersect_macos_seatbelt_profile_extensions(
    requested: Option<MacOsSeatbeltProfileExtensions>,
    granted: Option<MacOsSeatbeltProfileExtensions>,
) -> Option<MacOsSeatbeltProfileExtensions> {
    match (requested, granted) {
        (Some(requested), Some(granted)) => {
            let macos_automation = intersect_macos_automation_permission(
                &requested.macos_automation,
                &granted.macos_automation,
            );

            Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: requested.macos_preferences.min(granted.macos_preferences),
                macos_automation,
                macos_launch_services: requested.macos_launch_services
                    && granted.macos_launch_services,
                macos_accessibility: requested.macos_accessibility && granted.macos_accessibility,
                macos_calendar: requested.macos_calendar && granted.macos_calendar,
                macos_reminders: requested.macos_reminders && granted.macos_reminders,
                macos_contacts: requested.macos_contacts.min(granted.macos_contacts),
            })
        }
        _ => None,
    }
}

/// Unions two preferences permissions by keeping the more permissive one.
///
/// The larger rank wins: `None < ReadOnly < ReadWrite`. When both sides have
/// the same rank, this keeps `base`.
fn union_macos_preferences_permission(
    base: &MacOsPreferencesPermission,
    requested: &MacOsPreferencesPermission,
) -> MacOsPreferencesPermission {
    if base < requested {
        requested.clone()
    } else {
        base.clone()
    }
}

fn union_macos_contacts_permission(
    base: &MacOsContactsPermission,
    requested: &MacOsContactsPermission,
) -> MacOsContactsPermission {
    if base < requested {
        requested.clone()
    } else {
        base.clone()
    }
}

/// Unions two automation permissions by keeping the more permissive result.
///
/// `All` wins over everything, `None` yields to the other side, and two bundle
/// ID allowlists are unioned together.
fn union_macos_automation_permission(
    base: &MacOsAutomationPermission,
    requested: &MacOsAutomationPermission,
) -> MacOsAutomationPermission {
    match (base, requested) {
        (MacOsAutomationPermission::All, _) | (_, MacOsAutomationPermission::All) => {
            MacOsAutomationPermission::All
        }
        (MacOsAutomationPermission::None, _) => requested.clone(),
        (_, MacOsAutomationPermission::None) => base.clone(),
        (
            MacOsAutomationPermission::BundleIds(base_bundle_ids),
            MacOsAutomationPermission::BundleIds(requested_bundle_ids),
        ) => MacOsAutomationPermission::BundleIds(
            base_bundle_ids
                .iter()
                .chain(requested_bundle_ids.iter())
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        ),
    }
}

fn intersect_macos_automation_permission(
    requested: &MacOsAutomationPermission,
    granted: &MacOsAutomationPermission,
) -> MacOsAutomationPermission {
    match (requested, granted) {
        (_, MacOsAutomationPermission::None) | (MacOsAutomationPermission::None, _) => {
            MacOsAutomationPermission::None
        }
        (MacOsAutomationPermission::All, granted) => granted.clone(),
        (MacOsAutomationPermission::BundleIds(requested), MacOsAutomationPermission::All) => {
            MacOsAutomationPermission::BundleIds(requested.clone())
        }
        (
            MacOsAutomationPermission::BundleIds(requested),
            MacOsAutomationPermission::BundleIds(granted),
        ) => {
            let bundle_ids = requested
                .iter()
                .filter(|bundle_id| granted.contains(bundle_id))
                .cloned()
                .collect::<Vec<String>>();
            if bundle_ids.is_empty() {
                MacOsAutomationPermission::None
            } else {
                MacOsAutomationPermission::BundleIds(bundle_ids)
            }
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "macos_permissions_tests.rs"]
mod tests;
