use std::collections::BTreeSet;

use codex_protocol::models::MacOsAutomationPermission;
use codex_protocol::models::MacOsPreferencesPermission;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;

/// Merges macOS seatbelt profile extensions by taking the permissive union of
/// each permission field.
pub(crate) fn merge_macos_seatbelt_profile_extensions(
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
            macos_accessibility: base.macos_accessibility || permissions.macos_accessibility,
            macos_calendar: base.macos_calendar || permissions.macos_calendar,
        }),
        None => Some(permissions.clone()),
    }
}

pub(crate) fn intersect_macos_seatbelt_profile_extensions(
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
                macos_accessibility: requested.macos_accessibility && granted.macos_accessibility,
                macos_calendar: requested.macos_calendar && granted.macos_calendar,
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
mod tests {
    use super::intersect_macos_automation_permission;
    use super::intersect_macos_seatbelt_profile_extensions;
    use super::merge_macos_seatbelt_profile_extensions;
    use super::union_macos_automation_permission;
    use super::union_macos_preferences_permission;
    use codex_protocol::models::MacOsAutomationPermission;
    use codex_protocol::models::MacOsPreferencesPermission;
    use codex_protocol::models::MacOsSeatbeltProfileExtensions;
    use pretty_assertions::assert_eq;

    #[test]
    fn merge_extensions_widens_permissions() {
        let base = MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadOnly,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Calendar".to_string(),
            ]),
            macos_accessibility: false,
            macos_calendar: false,
        };
        let requested = MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
                "com.apple.Calendar".to_string(),
            ]),
            macos_accessibility: true,
            macos_calendar: true,
        };

        let merged =
            merge_macos_seatbelt_profile_extensions(Some(&base), Some(&requested)).expect("merge");

        assert_eq!(
            merged,
            MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Calendar".to_string(),
                    "com.apple.Notes".to_string(),
                ]),
                macos_accessibility: true,
                macos_calendar: true,
            }
        );
    }

    #[test]
    fn union_macos_preferences_permission_does_not_downgrade() {
        let base = MacOsPreferencesPermission::ReadWrite;
        let requested = MacOsPreferencesPermission::ReadOnly;

        let merged = union_macos_preferences_permission(&base, &requested);

        assert_eq!(merged, MacOsPreferencesPermission::ReadWrite);
    }

    #[test]
    fn union_macos_automation_permission_all_is_dominant() {
        let base = MacOsAutomationPermission::BundleIds(vec!["com.apple.Notes".to_string()]);
        let requested = MacOsAutomationPermission::All;

        let merged = union_macos_automation_permission(&base, &requested);

        assert_eq!(merged, MacOsAutomationPermission::All);
    }

    #[test]
    fn intersect_macos_automation_permission_keeps_common_bundle_ids() {
        let requested = MacOsAutomationPermission::BundleIds(vec![
            "com.apple.Notes".to_string(),
            "com.apple.Calendar".to_string(),
        ]);
        let granted = MacOsAutomationPermission::BundleIds(vec!["com.apple.Notes".to_string()]);

        let intersected = intersect_macos_automation_permission(&requested, &granted);

        assert_eq!(
            intersected,
            MacOsAutomationPermission::BundleIds(vec!["com.apple.Notes".to_string()])
        );
    }

    #[test]
    fn intersect_macos_seatbelt_profile_extensions_preserves_default_grant() {
        let requested = MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
            ]),
            macos_accessibility: true,
            macos_calendar: true,
        };
        let granted = MacOsSeatbeltProfileExtensions::default();

        let intersected =
            intersect_macos_seatbelt_profile_extensions(Some(requested), Some(granted));

        assert_eq!(intersected, Some(MacOsSeatbeltProfileExtensions::default()));
    }
}
