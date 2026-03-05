use std::cmp::Ordering;
use std::collections::BTreeSet;

use codex_protocol::models::MacOsAutomationPermission;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsAutomationValue;
use codex_protocol::models::MacOsPermissions;
use codex_protocol::models::MacOsPreferencesPermission;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsPreferencesValue;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;

pub(crate) fn merge_macos_seatbelt_profile_extensions(
    base: Option<&MacOsSeatbeltProfileExtensions>,
    permissions: Option<&MacOsPermissions>,
) -> Option<MacOsSeatbeltProfileExtensions> {
    let Some(permissions) = permissions else {
        return base.cloned();
    };

    let requested = build_macos_seatbelt_profile_extensions(permissions)?;
    match base {
        Some(base) => Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: merge_macos_preferences_permission(
                &base.macos_preferences,
                &requested.macos_preferences,
            ),
            macos_automation: merge_macos_automation_permission(
                &base.macos_automation,
                &requested.macos_automation,
            ),
            macos_accessibility: base.macos_accessibility || requested.macos_accessibility,
            macos_calendar: base.macos_calendar || requested.macos_calendar,
        }),
        None => Some(requested),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn build_macos_seatbelt_profile_extensions(
    permissions: &MacOsPermissions,
) -> Option<MacOsSeatbeltProfileExtensions> {
    let defaults = MacOsSeatbeltProfileExtensions::default();

    Some(MacOsSeatbeltProfileExtensions {
        macos_preferences: resolve_macos_preferences_permission(
            permissions.preferences.as_ref(),
            defaults.macos_preferences,
        ),
        macos_automation: resolve_macos_automation_permission(
            permissions.automations.as_ref(),
            defaults.macos_automation,
        ),
        macos_accessibility: permissions
            .accessibility
            .unwrap_or(defaults.macos_accessibility),
        macos_calendar: permissions.calendar.unwrap_or(defaults.macos_calendar),
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn build_macos_seatbelt_profile_extensions(
    _: &MacOsPermissions,
) -> Option<MacOsSeatbeltProfileExtensions> {
    None
}

/// Merges two preferences permissions by keeping the more permissive one.
///
/// The larger rank wins: `None < ReadOnly < ReadWrite`. When both sides have
/// the same rank, this keeps `base`.
fn merge_macos_preferences_permission(
    base: &MacOsPreferencesPermission,
    requested: &MacOsPreferencesPermission,
) -> MacOsPreferencesPermission {
    match preferences_permission_rank(base).cmp(&preferences_permission_rank(requested)) {
        Ordering::Less => requested.clone(),
        Ordering::Equal | Ordering::Greater => base.clone(),
    }
}

fn preferences_permission_rank(permission: &MacOsPreferencesPermission) -> u8 {
    match permission {
        MacOsPreferencesPermission::None => 0,
        MacOsPreferencesPermission::ReadOnly => 1,
        MacOsPreferencesPermission::ReadWrite => 2,
    }
}

/// Merges two automation permissions by keeping the more permissive result.
///
/// `All` wins over everything, `None` yields to the other side, and two bundle
/// ID allowlists are unioned together.
fn merge_macos_automation_permission(
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

#[cfg(target_os = "macos")]
fn resolve_macos_preferences_permission(
    value: Option<&MacOsPreferencesValue>,
    default: MacOsPreferencesPermission,
) -> MacOsPreferencesPermission {
    match value {
        Some(MacOsPreferencesValue::Bool(true)) => MacOsPreferencesPermission::ReadOnly,
        Some(MacOsPreferencesValue::Bool(false)) => MacOsPreferencesPermission::None,
        Some(MacOsPreferencesValue::Mode(mode)) => {
            let mode = mode.trim();
            if mode.eq_ignore_ascii_case("readonly") || mode.eq_ignore_ascii_case("read-only") {
                MacOsPreferencesPermission::ReadOnly
            } else if mode.eq_ignore_ascii_case("readwrite")
                || mode.eq_ignore_ascii_case("read-write")
            {
                MacOsPreferencesPermission::ReadWrite
            } else {
                tracing::warn!(
                    "ignoring permissions.macos.preferences: expected true/false, readonly, or readwrite"
                );
                default
            }
        }
        None => default,
    }
}

#[cfg(target_os = "macos")]
fn resolve_macos_automation_permission(
    value: Option<&MacOsAutomationValue>,
    default: MacOsAutomationPermission,
) -> MacOsAutomationPermission {
    match value {
        Some(MacOsAutomationValue::Bool(true)) => MacOsAutomationPermission::All,
        Some(MacOsAutomationValue::Bool(false)) => MacOsAutomationPermission::None,
        Some(MacOsAutomationValue::BundleIds(bundle_ids)) => {
            let bundle_ids = bundle_ids
                .iter()
                .map(|bundle_id| bundle_id.trim())
                .filter(|bundle_id| !bundle_id.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<String>>();
            if bundle_ids.is_empty() {
                MacOsAutomationPermission::None
            } else {
                MacOsAutomationPermission::BundleIds(bundle_ids)
            }
        }
        None => default,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::build_macos_seatbelt_profile_extensions;
    #[cfg(target_os = "macos")]
    use super::merge_macos_seatbelt_profile_extensions;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsAutomationPermission;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsAutomationValue;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPermissions;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPreferencesPermission;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPreferencesValue;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsSeatbeltProfileExtensions;
    #[cfg(target_os = "macos")]
    use pretty_assertions::assert_eq;

    #[cfg(target_os = "macos")]
    #[test]
    fn build_extensions_uses_legacy_preferences_default() {
        let extensions =
            build_macos_seatbelt_profile_extensions(&MacOsPermissions::default()).expect("build");

        assert_eq!(
            extensions,
            MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadOnly,
                ..Default::default()
            }
        );
    }

    #[cfg(target_os = "macos")]
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
        let requested = MacOsPermissions {
            preferences: Some(MacOsPreferencesValue::Mode("readwrite".to_string())),
            automations: Some(MacOsAutomationValue::BundleIds(vec![
                "com.apple.Notes".to_string(),
                "com.apple.Calendar".to_string(),
            ])),
            accessibility: Some(true),
            calendar: Some(true),
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
}
