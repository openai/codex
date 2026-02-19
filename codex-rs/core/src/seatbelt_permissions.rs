#![cfg(target_os = "macos")]

use std::collections::BTreeSet;
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MacOsPreferencesPermission {
    // IMPORTANT: ReadOnly needs to be the default because it's the security-sensitive default.
    // it's important for allowing cf prefs to work.
    #[default]
    ReadOnly,
    ReadWrite,
    None,
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

pub(crate) fn merge_macos_seatbelt_profile_extensions(
    base: Option<&MacOsSeatbeltProfileExtensions>,
    extension: Option<&MacOsSeatbeltProfileExtensions>,
) -> Option<MacOsSeatbeltProfileExtensions> {
    match (base, extension) {
        (None, None) => None,
        (Some(base), None) => Some(base.clone().normalized()),
        (None, Some(extension)) => Some(extension.clone().normalized()),
        (Some(base), Some(extension)) => {
            let base = base.normalized();
            let extension = extension.normalized();
            Some(
                MacOsSeatbeltProfileExtensions {
                    macos_preferences: merge_macos_preferences_permission(
                        &base.macos_preferences,
                        &extension.macos_preferences,
                    ),
                    macos_automation: merge_macos_automation_permission(
                        &base.macos_automation,
                        &extension.macos_automation,
                    ),
                    macos_accessibility: base.macos_accessibility && extension.macos_accessibility,
                    macos_calendar: base.macos_calendar && extension.macos_calendar,
                }
                .normalized(),
            )
        }
    }
}

fn merge_macos_preferences_permission(
    base: &MacOsPreferencesPermission,
    extension: &MacOsPreferencesPermission,
) -> MacOsPreferencesPermission {
    fn rank(permission: &MacOsPreferencesPermission) -> u8 {
        match permission {
            MacOsPreferencesPermission::None => 0,
            MacOsPreferencesPermission::ReadOnly => 1,
            MacOsPreferencesPermission::ReadWrite => 2,
        }
    }

    if rank(extension) < rank(base) {
        extension.clone()
    } else {
        base.clone()
    }
}

fn merge_macos_automation_permission(
    base: &MacOsAutomationPermission,
    extension: &MacOsAutomationPermission,
) -> MacOsAutomationPermission {
    match (base, extension) {
        (MacOsAutomationPermission::None, _) | (_, MacOsAutomationPermission::None) => {
            MacOsAutomationPermission::None
        }
        (MacOsAutomationPermission::All, other) | (other, MacOsAutomationPermission::All) => {
            other.clone()
        }
        (
            MacOsAutomationPermission::BundleIds(base_ids),
            MacOsAutomationPermission::BundleIds(extension_ids),
        ) => {
            let base_ids = base_ids.iter().cloned().collect::<BTreeSet<_>>();
            let extension_ids = extension_ids.iter().cloned().collect::<BTreeSet<_>>();
            let intersection = base_ids
                .intersection(&extension_ids)
                .cloned()
                .collect::<Vec<_>>();
            if intersection.is_empty() {
                MacOsAutomationPermission::None
            } else {
                MacOsAutomationPermission::BundleIds(intersection)
            }
        }
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
    use super::merge_macos_seatbelt_profile_extensions;

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
    fn default_extensions_emit_preferences_read_only_policy() {
        let policy = build_seatbelt_extensions(&MacOsSeatbeltProfileExtensions::default());
        assert!(policy.policy.contains("(allow user-preference-read)"));
        assert!(!policy.policy.contains("(allow user-preference-write)"));
    }

    #[test]
    fn merge_extensions_intersects_permissions() {
        let base = MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadOnly,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
                "com.apple.Calendar".to_string(),
            ]),
            macos_accessibility: false,
            macos_calendar: true,
        };
        let extension = MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Reminders".to_string(),
                "com.apple.Calendar".to_string(),
            ]),
            macos_accessibility: true,
            macos_calendar: false,
        };

        let merged =
            merge_macos_seatbelt_profile_extensions(Some(&base), Some(&extension)).expect("merged");

        assert_eq!(
            merged.macos_preferences,
            MacOsPreferencesPermission::ReadOnly
        );
        assert_eq!(
            merged.macos_automation,
            MacOsAutomationPermission::BundleIds(vec!["com.apple.Calendar".to_string(),])
        );
        assert!(!merged.macos_accessibility);
        assert!(!merged.macos_calendar);
    }

    #[test]
    fn merge_extensions_all_intersects_to_other_side() {
        let base = MacOsSeatbeltProfileExtensions {
            macos_automation: MacOsAutomationPermission::All,
            ..Default::default()
        };
        let extension = MacOsSeatbeltProfileExtensions {
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
            ]),
            ..Default::default()
        };

        let merged =
            merge_macos_seatbelt_profile_extensions(Some(&base), Some(&extension)).expect("merged");
        assert_eq!(
            merged.macos_automation,
            MacOsAutomationPermission::BundleIds(vec!["com.apple.Notes".to_string(),])
        );
    }

    #[test]
    fn merge_extensions_none_intersects_to_none() {
        let base = MacOsSeatbeltProfileExtensions {
            macos_automation: MacOsAutomationPermission::All,
            ..Default::default()
        };
        let extension = MacOsSeatbeltProfileExtensions {
            macos_automation: MacOsAutomationPermission::None,
            ..Default::default()
        };

        let merged =
            merge_macos_seatbelt_profile_extensions(Some(&base), Some(&extension)).expect("merged");
        assert_eq!(merged.macos_automation, MacOsAutomationPermission::None);
    }

    #[test]
    fn merge_extensions_normalizes_single_source() {
        let extension = MacOsSeatbeltProfileExtensions {
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                " com.apple.Notes ".to_string(),
                "com.apple.Notes".to_string(),
            ]),
            ..Default::default()
        };

        let merged =
            merge_macos_seatbelt_profile_extensions(None, Some(&extension)).expect("merged");
        assert_eq!(
            merged.macos_automation,
            MacOsAutomationPermission::BundleIds(vec!["com.apple.Notes".to_string()])
        );
    }
}
