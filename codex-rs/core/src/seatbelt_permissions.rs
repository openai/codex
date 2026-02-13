#![cfg(target_os = "macos")]

use std::collections::BTreeSet;
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MacOsPreferencesPermission {
    #[default]
    None,
    ReadOnly,
    ReadWrite,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MacOsAutomationPermission {
    #[default]
    None,
    All,
    BundleIds(Vec<String>),
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacOsSeatbeltProfileExtensions {
    pub macos_preferences: MacOsPreferencesPermission,
    pub macos_automation: MacOsAutomationPermission,
    pub macos_accessibility: bool,
    pub macos_calendar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SeatbeltExtensionPolicy {
    pub(crate) policy: String,
    pub(crate) dir_params: Vec<(String, PathBuf)>,
}

impl MacOsSeatbeltProfileExtensions {
    pub fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        if let MacOsAutomationPermission::BundleIds(bundle_ids) = &self.macos_automation {
            let bundle_ids = normalize_bundle_ids(bundle_ids);
            normalized.macos_automation = if bundle_ids.is_empty() {
                MacOsAutomationPermission::None
            } else {
                MacOsAutomationPermission::BundleIds(bundle_ids)
            };
        }
        normalized
    }
}

pub(crate) fn build_seatbelt_extensions(
    extensions: &MacOsSeatbeltProfileExtensions,
) -> SeatbeltExtensionPolicy {
    let extensions = extensions.normalized();
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
                "(allow mach-lookup\n  (global-name \"com.apple.coreservices.launchservicesd\")\n  (global-name \"com.apple.coreservices.appleevents\"))"
                    .to_string(),
            );
            clauses.push("(allow appleevent-send)".to_string());
        }
        MacOsAutomationPermission::BundleIds(bundle_ids) => {
            if !bundle_ids.is_empty() {
                clauses.push(
                    "(allow mach-lookup (global-name \"com.apple.coreservices.appleevents\"))"
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

    if extensions.macos_accessibility {
        clauses.push("(allow mach-lookup (local-name \"com.apple.axserver\"))".to_string());
    }

    if extensions.macos_calendar {
        clauses.push("(allow mach-lookup (global-name \"com.apple.CalendarAgent\"))".to_string());
    }

    if clauses.is_empty() {
        SeatbeltExtensionPolicy::default()
    } else {
        SeatbeltExtensionPolicy {
            policy: format!(
                "; macOS permission profile extensions\n{}\n",
                clauses.join("\n")
            ),
            dir_params: Vec::new(),
        }
    }
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

fn is_valid_bundle_id(bundle_id: &str) -> bool {
    if bundle_id.len() < 3 || !bundle_id.contains('.') {
        return false;
    }
    bundle_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::MacOsAutomationPermission;
    use super::MacOsPreferencesPermission;
    use super::MacOsSeatbeltProfileExtensions;
    use super::build_seatbelt_extensions;
    use pretty_assertions::assert_eq;

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
        assert!(policy.policy.contains(
            "(allow ipc-posix-shm-write-create (ipc-posix-name-prefix \"apple.cfprefs.\"))"
        ));
    }

    #[test]
    fn automation_all_emits_unscoped_appleevents() {
        let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions {
            macos_automation: MacOsAutomationPermission::All,
            ..Default::default()
        });
        assert!(policy.policy.contains("(allow appleevent-send)"));
        assert!(
            policy
                .policy
                .contains("com.apple.coreservices.launchservicesd")
        );
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
    fn empty_extensions_emit_empty_policy() {
        let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions::default());
        assert_eq!(policy.policy, "");
    }
}
