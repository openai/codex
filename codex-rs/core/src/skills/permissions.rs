use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsAutomationValue;
use codex_protocol::models::MacOsPermissions;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsPreferencesValue;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use dunce::canonicalize as canonicalize_path;
use tracing::warn;

use crate::config::Constrained;
use crate::config::Permissions;
use crate::config::types::ShellEnvironmentPolicy;
use crate::protocol::AskForApproval;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;
#[cfg(target_os = "macos")]
use crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions;
#[cfg(not(target_os = "macos"))]
type MacOsSeatbeltProfileExtensions = ();

pub(crate) fn compile_permission_profile(
    skill_dir: &Path,
    permissions: Option<PermissionProfile>,
) -> Option<Permissions> {
    let PermissionProfile {
        network,
        file_system,
        macos,
    } = permissions?;
    let file_system = file_system.unwrap_or_default();
    let fs_read = normalize_permission_paths(
        skill_dir,
        file_system.read.as_deref().unwrap_or_default(),
        "permissions.file_system.read",
    );
    let fs_write = normalize_permission_paths(
        skill_dir,
        file_system.write.as_deref().unwrap_or_default(),
        "permissions.file_system.write",
    );
    let sandbox_policy = if !fs_write.is_empty() {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: fs_write,
            read_only_access: if fs_read.is_empty() {
                ReadOnlyAccess::FullAccess
            } else {
                ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: fs_read,
                }
            },
            network_access: network.unwrap_or_default(),
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    } else if !fs_read.is_empty() {
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: fs_read,
            },
        }
    } else {
        // Default sandbox policy
        SandboxPolicy::new_read_only_policy()
    };
    let macos_permissions = macos.unwrap_or_default();
    let macos_seatbelt_profile_extensions =
        build_macos_seatbelt_profile_extensions(&macos_permissions);

    Some(Permissions {
        approval_policy: Constrained::allow_any(AskForApproval::Never),
        sandbox_policy: Constrained::allow_any(sandbox_policy),
        network: None,
        allow_login_shell: true,
        shell_environment_policy: ShellEnvironmentPolicy::default(),
        windows_sandbox_mode: None,
        macos_seatbelt_profile_extensions,
    })
}

fn normalize_permission_paths(
    skill_dir: &Path,
    values: &[PathBuf],
    field: &str,
) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
        let Some(path) = normalize_permission_path(skill_dir, value, field) else {
            continue;
        };
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }

    paths
}

fn normalize_permission_path(
    skill_dir: &Path,
    value: &Path,
    field: &str,
) -> Option<AbsolutePathBuf> {
    let value = value.to_string_lossy();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        warn!("ignoring {field}: value is empty");
        return None;
    }

    let expanded = expand_home(trimmed);
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        skill_dir.join(expanded)
    };
    let normalized = normalize_lexically(&absolute);
    let canonicalized = canonicalize_path(&normalized).unwrap_or(normalized);
    match AbsolutePathBuf::from_absolute_path(&canonicalized) {
        Ok(path) => Some(path),
        Err(error) => {
            warn!("ignoring {field}: expected absolute path, got {canonicalized:?}: {error}");
            None
        }
    }
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
        return PathBuf::from(path);
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

#[cfg(target_os = "macos")]
fn build_macos_seatbelt_profile_extensions(
    permissions: &MacOsPermissions,
) -> Option<MacOsSeatbeltProfileExtensions> {
    let defaults = MacOsSeatbeltProfileExtensions::default();

    let extensions = MacOsSeatbeltProfileExtensions {
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
    };
    Some(extensions)
}

#[cfg(target_os = "macos")]
fn resolve_macos_preferences_permission(
    value: Option<&MacOsPreferencesValue>,
    default: crate::seatbelt_permissions::MacOsPreferencesPermission,
) -> crate::seatbelt_permissions::MacOsPreferencesPermission {
    use crate::seatbelt_permissions::MacOsPreferencesPermission;

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
                warn!(
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
    default: crate::seatbelt_permissions::MacOsAutomationPermission,
) -> crate::seatbelt_permissions::MacOsAutomationPermission {
    use crate::seatbelt_permissions::MacOsAutomationPermission;

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

#[cfg(not(target_os = "macos"))]
fn build_macos_seatbelt_profile_extensions(
    _: &MacOsPermissions,
) -> Option<MacOsSeatbeltProfileExtensions> {
    None
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::compile_permission_profile;
    use crate::config::Constrained;
    use crate::config::Permissions;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::protocol::AskForApproval;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use codex_protocol::models::FileSystemPermissions;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsAutomationValue;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPermissions;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPreferencesValue;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn compile_permission_profile_normalizes_paths() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(skill_dir.join("scripts")).expect("skill dir");
        let read_dir = skill_dir.join("data");
        fs::create_dir_all(&read_dir).expect("read dir");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(PermissionProfile {
                network: Some(true),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![
                        PathBuf::from("./data"),
                        PathBuf::from("./data"),
                        PathBuf::from("scripts/../data"),
                    ]),
                    write: Some(vec![PathBuf::from("./output")]),
                }),
                ..Default::default()
            }),
        )
        .expect("profile");

        assert_eq!(
            profile,
            Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(SandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![
                        AbsolutePathBuf::try_from(skill_dir.join("output"))
                            .expect("absolute output path")
                    ],
                    read_only_access: ReadOnlyAccess::Restricted {
                        include_platform_defaults: true,
                        readable_roots: vec![
                            AbsolutePathBuf::try_from(
                                dunce::canonicalize(&read_dir).unwrap_or(read_dir)
                            )
                            .expect("absolute read path")
                        ],
                    },
                    network_access: true,
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                }),
                network: None,
                allow_login_shell: true,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                windows_sandbox_mode: None,
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: Some(
                    crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default(),
                ),
                #[cfg(not(target_os = "macos"))]
                macos_seatbelt_profile_extensions: None,
            }
        );
    }

    #[test]
    fn compile_permission_profile_without_permissions_has_empty_profile() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(&skill_dir, None);

        assert_eq!(profile, None);
    }

    #[test]
    fn compile_permission_profile_with_network_only_uses_read_only_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(PermissionProfile {
                network: Some(true),
                ..Default::default()
            }),
        )
        .expect("profile");

        assert_eq!(
            profile,
            Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
                network: None,
                allow_login_shell: true,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                windows_sandbox_mode: None,
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: Some(
                    crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default(),
                ),
                #[cfg(not(target_os = "macos"))]
                macos_seatbelt_profile_extensions: None,
            }
        );
    }

    #[test]
    fn compile_permission_profile_with_network_and_read_only_paths_uses_read_only_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        let read_dir = skill_dir.join("data");
        fs::create_dir_all(&read_dir).expect("read dir");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(PermissionProfile {
                network: Some(true),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![PathBuf::from("./data")]),
                    write: Some(Vec::new()),
                }),
                ..Default::default()
            }),
        )
        .expect("profile");

        assert_eq!(
            profile,
            Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(SandboxPolicy::ReadOnly {
                    access: ReadOnlyAccess::Restricted {
                        include_platform_defaults: true,
                        readable_roots: vec![
                            AbsolutePathBuf::try_from(
                                dunce::canonicalize(&read_dir).unwrap_or(read_dir)
                            )
                            .expect("absolute read path")
                        ],
                    },
                }),
                network: None,
                allow_login_shell: true,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                windows_sandbox_mode: None,
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: Some(
                    crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default(),
                ),
                #[cfg(not(target_os = "macos"))]
                macos_seatbelt_profile_extensions: None,
            }
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn compile_permission_profile_builds_macos_permission_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(
            &skill_dir,
            Some(PermissionProfile {
                macos: Some(MacOsPermissions {
                    preferences: Some(MacOsPreferencesValue::Mode("readwrite".to_string())),
                    automations: Some(MacOsAutomationValue::BundleIds(vec![
                        "com.apple.Notes".to_string(),
                    ])),
                    accessibility: Some(true),
                    calendar: Some(true),
                }),
                ..Default::default()
            }),
        )
        .expect("profile");

        assert_eq!(
            profile.macos_seatbelt_profile_extensions,
            Some(
                crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions {
                    macos_preferences:
                        crate::seatbelt_permissions::MacOsPreferencesPermission::ReadWrite,
                    macos_automation:
                        crate::seatbelt_permissions::MacOsAutomationPermission::BundleIds(vec![
                            "com.apple.Notes".to_string()
                        ],),
                    macos_accessibility: true,
                    macos_calendar: true,
                }
            )
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn compile_permission_profile_uses_macos_defaults_when_values_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(&skill_dir, Some(PermissionProfile::default()))
            .expect("profile");

        assert_eq!(
            profile.macos_seatbelt_profile_extensions,
            Some(crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default())
        );
    }
}
