use super::intersect_macos_automation_permission;
use super::intersect_macos_seatbelt_profile_extensions;
use super::merge_macos_seatbelt_profile_extensions;
use super::union_macos_automation_permission;
use super::union_macos_contacts_permission;
use super::union_macos_preferences_permission;
use codex_protocol::models::MacOsAutomationPermission;
use codex_protocol::models::MacOsContactsPermission;
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
        macos_launch_services: false,
        macos_accessibility: false,
        macos_calendar: false,
        macos_reminders: false,
        macos_contacts: MacOsContactsPermission::ReadOnly,
    };
    let requested = MacOsSeatbeltProfileExtensions {
        macos_preferences: MacOsPreferencesPermission::ReadWrite,
        macos_automation: MacOsAutomationPermission::BundleIds(vec![
            "com.apple.Notes".to_string(),
            "com.apple.Calendar".to_string(),
        ]),
        macos_launch_services: true,
        macos_accessibility: true,
        macos_calendar: true,
        macos_reminders: true,
        macos_contacts: MacOsContactsPermission::ReadWrite,
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
            macos_launch_services: true,
            macos_accessibility: true,
            macos_calendar: true,
            macos_reminders: true,
            macos_contacts: MacOsContactsPermission::ReadWrite,
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
        macos_automation: MacOsAutomationPermission::BundleIds(vec!["com.apple.Notes".to_string()]),
        macos_launch_services: false,
        macos_accessibility: true,
        macos_calendar: true,
        macos_reminders: false,
        macos_contacts: MacOsContactsPermission::None,
    };
    let granted = MacOsSeatbeltProfileExtensions::default();

    let intersected = intersect_macos_seatbelt_profile_extensions(Some(requested), Some(granted));

    assert_eq!(intersected, Some(MacOsSeatbeltProfileExtensions::default()));
}

#[test]
fn union_macos_contacts_permission_does_not_downgrade() {
    let base = MacOsContactsPermission::ReadWrite;
    let requested = MacOsContactsPermission::ReadOnly;

    let merged = union_macos_contacts_permission(&base, &requested);

    assert_eq!(merged, MacOsContactsPermission::ReadWrite);
}
