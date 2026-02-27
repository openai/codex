use std::path::PathBuf;

use codex_protocol::models::FileSystemPermissions;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsAutomationValue;
use codex_protocol::models::MacOsPermissions;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsPreferencesValue;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use dunce::canonicalize as canonicalize_path;
use serde::Deserialize;
use tracing::warn;

use crate::config::Constrained;
use crate::config::Permissions;
use crate::config::types::ShellEnvironmentPolicy;
use crate::protocol::AskForApproval;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SkillFileSystemPermissions {
    pub read: Option<Vec<AbsolutePathBuf>>,
    pub write: Option<Vec<AbsolutePathBuf>>,
}

impl SkillFileSystemPermissions {
    pub(crate) fn is_empty(&self) -> bool {
        self.read.is_none() && self.write.is_none()
    }
}

impl From<&SkillFileSystemPermissions> for FileSystemPermissions {
    fn from(value: &SkillFileSystemPermissions) -> Self {
        Self {
            read: value
                .read
                .as_ref()
                .map(|paths| absolute_paths_to_path_bufs(paths)),
            write: value
                .write
                .as_ref()
                .map(|paths| absolute_paths_to_path_bufs(paths)),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SkillPermissionProfile {
    pub network: Option<bool>,
    pub file_system: Option<SkillFileSystemPermissions>,
    pub macos: Option<MacOsPermissions>,
}

impl SkillPermissionProfile {
    pub(crate) fn is_empty(&self) -> bool {
        self.network.is_none()
            && self
                .file_system
                .as_ref()
                .map(SkillFileSystemPermissions::is_empty)
                .unwrap_or(true)
            && self
                .macos
                .as_ref()
                .map(MacOsPermissions::is_empty)
                .unwrap_or(true)
    }
}

impl From<&SkillPermissionProfile> for PermissionProfile {
    fn from(value: &SkillPermissionProfile) -> Self {
        Self {
            network: value.network,
            file_system: value.file_system.as_ref().map(FileSystemPermissions::from),
            macos: value.macos.clone(),
        }
    }
}

impl From<SkillPermissionProfile> for PermissionProfile {
    fn from(value: SkillPermissionProfile) -> Self {
        Self::from(&value)
    }
}

pub(crate) fn compile_permission_profile(
    permissions: Option<&SkillPermissionProfile>,
) -> Option<Permissions> {
    let permissions = permissions?;
    let file_system = permissions
        .file_system
        .as_ref()
        .cloned()
        .unwrap_or_default();
    let fs_read = canonicalize_permission_paths(
        file_system.read.as_deref().unwrap_or_default(),
        "permissions.file_system.read",
    );
    let fs_write = canonicalize_permission_paths(
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
            network_access: permissions.network.unwrap_or_default(),
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
    let macos_permissions = permissions.macos.clone().unwrap_or_default();
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

fn absolute_paths_to_path_bufs(values: &[AbsolutePathBuf]) -> Vec<PathBuf> {
    values.iter().map(AbsolutePathBuf::to_path_buf).collect()
}

fn canonicalize_permission_paths(values: &[AbsolutePathBuf], field: &str) -> Vec<AbsolutePathBuf> {
    values
        .iter()
        .filter_map(|value| {
            let canonicalized = canonicalize_path(value.as_path()).unwrap_or(value.to_path_buf());
            match AbsolutePathBuf::from_absolute_path(&canonicalized) {
                Ok(path) => Some(path),
                Err(error) => {
                    warn!(
                        "ignoring {field}: expected absolute path, got {canonicalized:?}: {error}"
                    );
                    None
                }
            }
        })
        .fold(Vec::new(), |mut paths, path| {
            if !paths.contains(&path) {
                paths.push(path);
            }
            paths
        })
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

#[cfg(test)]
mod tests {
    use super::SkillFileSystemPermissions;
    use super::SkillPermissionProfile;
    use super::compile_permission_profile;
    use crate::config::Constrained;
    use crate::config::Permissions;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::protocol::AskForApproval;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsAutomationValue;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPermissions;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPreferencesValue;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::fs;

    #[test]
    fn compile_permission_profile_normalizes_paths() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(skill_dir.join("scripts")).expect("skill dir");
        let read_dir = skill_dir.join("data");
        fs::create_dir_all(&read_dir).expect("read dir");

        let profile = compile_permission_profile(Some(&SkillPermissionProfile {
            network: Some(true),
            file_system: Some(SkillFileSystemPermissions {
                read: Some(vec![
                    AbsolutePathBuf::try_from(skill_dir.join("data")).expect("read path"),
                    AbsolutePathBuf::try_from(skill_dir.join("data")).expect("read path"),
                    AbsolutePathBuf::try_from(skill_dir.join("scripts/../data"))
                        .expect("normalized read path"),
                ]),
                write: Some(vec![
                    AbsolutePathBuf::try_from(skill_dir.join("output"))
                        .expect("absolute output path"),
                ]),
            }),
            ..Default::default()
        }))
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
        let profile = compile_permission_profile(None);

        assert_eq!(profile, None);
    }

    #[test]
    fn compile_permission_profile_with_network_only_uses_read_only_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("skill dir");

        let profile = compile_permission_profile(Some(&SkillPermissionProfile {
            network: Some(true),
            ..Default::default()
        }))
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

        let profile = compile_permission_profile(Some(&SkillPermissionProfile {
            network: Some(true),
            file_system: Some(SkillFileSystemPermissions {
                read: Some(vec![
                    AbsolutePathBuf::try_from(skill_dir.join("data")).expect("absolute read path"),
                ]),
                write: Some(Vec::new()),
            }),
            ..Default::default()
        }))
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

        let profile = compile_permission_profile(Some(&SkillPermissionProfile {
            macos: Some(MacOsPermissions {
                preferences: Some(MacOsPreferencesValue::Mode("readwrite".to_string())),
                automations: Some(MacOsAutomationValue::BundleIds(vec![
                    "com.apple.Notes".to_string(),
                ])),
                accessibility: Some(true),
                calendar: Some(true),
            }),
            ..Default::default()
        }))
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

        let profile =
            compile_permission_profile(Some(&SkillPermissionProfile::default())).expect("profile");

        assert_eq!(
            profile.macos_seatbelt_profile_extensions,
            Some(crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default())
        );
    }
}
