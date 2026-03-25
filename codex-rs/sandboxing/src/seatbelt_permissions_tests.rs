use super::MacOsAutomationPermission;
use super::MacOsContactsPermission;
use super::MacOsPreferencesPermission;
use super::MacOsSeatbeltProfileExtensions;
use super::build_seatbelt_extensions;

#[test]
fn preferences_read_only_emits_read_clauses_only() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_preferences: MacOsPreferencesPermission::ReadOnly,
        ..Default::default()
    });
    assert!(policy.policy.contains("(allow user-preference-read)"));
    assert!(!policy.policy.contains("(allow user-preference-write)"));
}

#[test]
fn preferences_read_write_emits_write_clauses() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_preferences: MacOsPreferencesPermission::ReadWrite,
        ..Default::default()
    });
    assert!(policy.policy.contains("(allow user-preference-read)"));
    assert!(policy.policy.contains("(allow user-preference-write)"));
    assert!(
        policy.policy.contains(
            "(allow ipc-posix-shm-write-create (ipc-posix-name-prefix \"apple.cfprefs.\"))"
        )
    );
}

#[test]
fn automation_all_emits_unscoped_appleevents() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_automation: MacOsAutomationPermission::All,
        ..Default::default()
    });
    assert!(policy.policy.contains("(allow appleevent-send)"));
    assert!(policy.policy.contains("com.apple.coreservices.appleevents"));
}

#[test]
fn automation_bundle_ids_are_normalized_and_scoped() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_automation: MacOsAutomationPermission::BundleIds(vec![
            " com.apple.Notes ".to_string(),
            "com.apple.Calendar".to_string(),
            "bad bundle".to_string(),
            "com.apple.Notes".to_string(),
        ]),
        ..Default::default()
    });
    assert!(
        policy
            .policy
            .contains("(appleevent-destination \"com.apple.Calendar\")")
    );
    assert!(
        policy
            .policy
            .contains("(appleevent-destination \"com.apple.Notes\")")
    );
    assert!(!policy.policy.contains("bad bundle"));
    assert!(policy.policy.contains("com.apple.coreservices.appleevents"));
}

#[test]
fn launch_services_emit_launch_clauses() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_launch_services: true,
        ..Default::default()
    });
    assert!(
        policy
            .policy
            .contains("com.apple.coreservices.launchservicesd")
    );
    assert!(policy.policy.contains("com.apple.lsd.mapdb"));
    assert!(
        policy
            .policy
            .contains("com.apple.coreservices.quarantine-resolver")
    );
    assert!(policy.policy.contains("com.apple.lsd.modifydb"));
    assert!(policy.policy.contains("(allow lsopen)"));
}

#[test]
fn accessibility_and_calendar_emit_mach_lookups() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_accessibility: true,
        macos_calendar: true,
        ..Default::default()
    });
    assert!(policy.policy.contains("com.apple.axserver"));
    assert!(policy.policy.contains("com.apple.CalendarAgent"));
}

#[test]
fn reminders_emit_calendar_agent_and_remindd_lookups() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_reminders: true,
        ..Default::default()
    });
    assert!(policy.policy.contains("com.apple.CalendarAgent"));
    assert!(policy.policy.contains("com.apple.remindd"));
}

#[test]
fn contacts_read_only_emit_contacts_read_clauses() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_contacts: MacOsContactsPermission::ReadOnly,
        ..Default::default()
    });

    assert!(
        policy
            .policy
            .contains("(subpath \"/System/Library/Address Book Plug-Ins\")")
    );
    assert!(
        policy
            .policy
            .contains("(subpath (param \"ADDRESSBOOK_DIR\"))")
    );
    assert!(policy.policy.contains("com.apple.contactsd.persistence"));
    assert!(policy.policy.contains("com.apple.accountsd.accountmanager"));
    assert!(!policy.policy.contains("com.apple.securityd.xpc"));
    assert!(
        policy
            .dir_params
            .iter()
            .any(|(key, _)| key == "ADDRESSBOOK_DIR")
    );
}

#[test]
fn contacts_read_write_emit_write_clauses() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
        macos_contacts: MacOsContactsPermission::ReadWrite,
        ..Default::default()
    });

    assert!(policy.policy.contains("(subpath \"/var/folders\")"));
    assert!(policy.policy.contains("(subpath \"/private/var/folders\")"));
    assert!(policy.policy.contains("com.apple.securityd.xpc"));
}

#[test]
fn default_extensions_emit_preferences_read_only_policy() {
    let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions::default());
    assert!(policy.policy.contains("(allow user-preference-read)"));
    assert!(!policy.policy.contains("(allow user-preference-write)"));
}
