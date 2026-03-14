#![cfg(target_os = "macos")]

use std::collections::BTreeSet;
use std::path::PathBuf;

pub use codex_protocol::models::MacOsAutomationPermission;
pub use codex_protocol::models::MacOsContactsPermission;
pub use codex_protocol::models::MacOsPreferencesPermission;
pub use codex_protocol::models::MacOsSeatbeltProfileExtensions;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SeatbeltExtensionPolicy {
    pub(crate) policy: String,
    pub(crate) dir_params: Vec<(String, PathBuf)>,
}

fn normalized_extensions(
    extensions: &MacOsSeatbeltProfileExtensions,
) -> MacOsSeatbeltProfileExtensions {
    let mut normalized = extensions.clone();
    if let MacOsAutomationPermission::BundleIds(bundle_ids) = &extensions.macos_automation {
        let bundle_ids = normalize_bundle_ids(bundle_ids);
        normalized.macos_automation = if bundle_ids.is_empty() {
            MacOsAutomationPermission::None
        } else {
            MacOsAutomationPermission::BundleIds(bundle_ids)
        };
    }
    normalized.macos_mach_services = normalize_mach_services(&extensions.macos_mach_services);

    normalized
}

pub(crate) fn build_seatbelt_extensions(
    extensions: &MacOsSeatbeltProfileExtensions,
) -> SeatbeltExtensionPolicy {
    let extensions = normalized_extensions(extensions);
    let mut clauses = Vec::new();

    match extensions.macos_preferences {
        MacOsPreferencesPermission::None => {}
        MacOsPreferencesPermission::ReadOnly => {
            clauses.push(
                "(allow ipc-posix-shm-read* (ipc-posix-name-prefix \"apple.cfprefs.\"))"
                    .to_string(),
            );
            clauses.push(
                "(allow mach-lookup\n    (global-name \"com.apple.cfprefsd.daemon\")\n    (global-name \"com.apple.cfprefsd.agent\")\n    (local-name \"com.apple.cfprefsd.agent\"))"
                    .to_string(),
            );
            clauses.push("(allow user-preference-read)".to_string());
        }
        MacOsPreferencesPermission::ReadWrite => {
            clauses.push(
                "(allow ipc-posix-shm-read* (ipc-posix-name-prefix \"apple.cfprefs.\"))"
                    .to_string(),
            );
            clauses.push(
                "(allow mach-lookup\n    (global-name \"com.apple.cfprefsd.daemon\")\n    (global-name \"com.apple.cfprefsd.agent\")\n    (local-name \"com.apple.cfprefsd.agent\"))"
                    .to_string(),
            );
            clauses.push("(allow user-preference-read)".to_string());
            clauses.push("(allow user-preference-write)".to_string());
            clauses.push(
                "(allow ipc-posix-shm-write-data (ipc-posix-name-prefix \"apple.cfprefs.\"))"
                    .to_string(),
            );
            clauses.push(
                "(allow ipc-posix-shm-write-create (ipc-posix-name-prefix \"apple.cfprefs.\"))"
                    .to_string(),
            );
        }
    }

    match extensions.macos_automation {
        MacOsAutomationPermission::None => {}
        MacOsAutomationPermission::All => {
            clauses.push(
                "(allow mach-lookup\n  (global-name \"com.apple.coreservices.appleevents\"))"
                    .to_string(),
            );
            clauses.push("(allow appleevent-send)".to_string());
        }
        MacOsAutomationPermission::BundleIds(bundle_ids) => {
            if !bundle_ids.is_empty() {
                clauses.push(
                    "(allow mach-lookup\n  (global-name \"com.apple.coreservices.appleevents\"))"
                        .to_string(),
                );
                let destinations = bundle_ids
                    .iter()
                    .map(|bundle_id| format!("    (appleevent-destination \"{bundle_id}\")"))
                    .collect::<Vec<String>>()
                    .join("\n");
                clauses.push(format!("(allow appleevent-send\n{destinations}\n)"));
            }
        }
    }

    if !extensions.macos_mach_services.is_empty() {
        let services = extensions
            .macos_mach_services
            .iter()
            .map(|service| format!("  (global-name \"{service}\")"))
            .collect::<Vec<String>>()
            .join("\n");
        clauses.push(format!("(allow mach-lookup\n{services}\n)"));
    }

    if extensions.macos_launch_services {
        clauses.push(
            "(allow mach-lookup\n  (global-name \"com.apple.coreservices.launchservicesd\")\n  (global-name \"com.apple.lsd.mapdb\")\n  (global-name \"com.apple.coreservices.quarantine-resolver\")\n  (global-name \"com.apple.lsd.modifydb\"))"
                .to_string(),
        );
        clauses.push("(allow lsopen)".to_string());
    }

    if extensions.macos_accessibility {
        clauses.push("(allow mach-lookup (local-name \"com.apple.axserver\"))".to_string());
    }

    if extensions.macos_calendar {
        clauses.push("(allow mach-lookup (global-name \"com.apple.CalendarAgent\"))".to_string());
    }

    if extensions.macos_reminders {
        clauses.push(
            "(allow mach-lookup\n  (global-name \"com.apple.CalendarAgent\")\n  (global-name \"com.apple.remindd\"))"
                .to_string(),
        );
    }

    let mut dir_params = Vec::new();
    match extensions.macos_contacts {
        MacOsContactsPermission::None => {}
        MacOsContactsPermission::ReadOnly => {
            clauses.push(
                "(allow file-read* file-test-existence\n  (subpath \"/System/Library/Address Book Plug-Ins\")\n  (subpath (param \"ADDRESSBOOK_DIR\")))"
                    .to_string(),
            );
            clauses.push(
                "(allow mach-lookup\n  (global-name \"com.apple.tccd\")\n  (global-name \"com.apple.tccd.system\")\n  (global-name \"com.apple.contactsd.persistence\")\n  (global-name \"com.apple.AddressBook.ContactsAccountsService\")\n  (global-name \"com.apple.contacts.account-caching\")\n  (global-name \"com.apple.accountsd.accountmanager\"))"
                    .to_string(),
            );
            if let Some(addressbook_dir) = addressbook_dir() {
                dir_params.push(("ADDRESSBOOK_DIR".to_string(), addressbook_dir));
            }
        }
        MacOsContactsPermission::ReadWrite => {
            clauses.push(
                "(allow file-read* file-write*\n  (subpath \"/System/Library/Address Book Plug-Ins\")\n  (subpath (param \"ADDRESSBOOK_DIR\"))\n  (subpath \"/var/folders\")\n  (subpath \"/private/var/folders\"))"
                    .to_string(),
            );
            clauses.push(
                "(allow mach-lookup\n  (global-name \"com.apple.tccd\")\n  (global-name \"com.apple.tccd.system\")\n  (global-name \"com.apple.contactsd.persistence\")\n  (global-name \"com.apple.AddressBook.ContactsAccountsService\")\n  (global-name \"com.apple.contacts.account-caching\")\n  (global-name \"com.apple.accountsd.accountmanager\")\n  (global-name \"com.apple.securityd.xpc\"))"
                    .to_string(),
            );
            if let Some(addressbook_dir) = addressbook_dir() {
                dir_params.push(("ADDRESSBOOK_DIR".to_string(), addressbook_dir));
            }
        }
    }

    if clauses.is_empty() {
        SeatbeltExtensionPolicy::default()
    } else {
        SeatbeltExtensionPolicy {
            policy: format!(
                "; macOS permission profile extensions\n{}\n",
                clauses.join("\n")
            ),
            dir_params,
        }
    }
}

fn addressbook_dir() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join("Library/Application Support/AddressBook"))
}

fn normalize_bundle_ids(bundle_ids: &[String]) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for bundle_id in bundle_ids {
        let candidate = bundle_id.trim();
        if is_valid_bundle_id(candidate) {
            unique.insert(candidate.to_string());
        }
    }
    unique.into_iter().collect()
}

fn normalize_mach_services(mach_services: &[String]) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for mach_service in mach_services {
        let candidate = mach_service.trim();
        if is_valid_mach_service(candidate) {
            unique.insert(candidate.to_string());
        }
    }
    unique.into_iter().collect()
}

fn is_valid_bundle_id(bundle_id: &str) -> bool {
    if bundle_id.len() < 3 || !bundle_id.contains('.') {
        return false;
    }
    bundle_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn is_valid_mach_service(mach_service: &str) -> bool {
    if mach_service.len() < 3 || !mach_service.contains('.') {
        return false;
    }
    mach_service
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[cfg(test)]
#[path = "seatbelt_permissions_tests.rs"]
mod tests;
