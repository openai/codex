use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use dunce::canonicalize as canonicalize_path;
use serde::Deserialize;
use tracing::warn;

use crate::config::Constrained;
use crate::config::Permissions;
use crate::config::types::ShellEnvironmentPolicy;
use crate::protocol::AskForApproval;
use crate::protocol::NetworkAccess;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;
#[cfg(target_os = "macos")]
use crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions;
#[cfg(not(target_os = "macos"))]
type MacOsSeatbeltProfileExtensions = ();

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct SkillManifestPermissions {
    #[serde(default)]
    pub(crate) network: bool,
    #[serde(default)]
    pub(crate) file_system: SkillManifestFileSystemPermissions,
    #[serde(default)]
    pub(crate) macos: SkillManifestMacOsPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct SkillManifestFileSystemPermissions {
    #[serde(default)]
    pub(crate) read: Vec<String>,
    #[serde(default)]
    pub(crate) write: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct SkillManifestMacOsPermissions {
    #[serde(default)]
    pub(crate) preferences: Option<MacOsPreferencesValue>,
    #[serde(default)]
    pub(crate) automations: Option<MacOsAutomationValue>,
    #[serde(default)]
    pub(crate) accessibility: bool,
    #[serde(default)]
    pub(crate) calendar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum MacOsPreferencesValue {
    Bool(bool),
    Mode(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum MacOsAutomationValue {
    Bool(bool),
    BundleIds(Vec<String>),
}

pub(crate) fn compile_permission_profile(
    skill_dir: &Path,
    permissions: Option<SkillManifestPermissions>,
) -> Option<Permissions> {
    let permissions = permissions?;
    let fs_read = normalize_permission_paths(
        skill_dir,
        &permissions.file_system.read,
        "permissions.file_system.read",
    );
    let fs_write = normalize_permission_paths(
        skill_dir,
        &permissions.file_system.write,
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
            network_access: permissions.network,
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
    let macos_seatbelt_profile_extensions =
        build_macos_seatbelt_profile_extensions(&permissions.macos);

    Some(Permissions {
        approval_policy: Constrained::allow_any(AskForApproval::Never),
        sandbox_policy: Constrained::allow_any(sandbox_policy),
        network: None,
        shell_environment_policy: ShellEnvironmentPolicy::default(),
        windows_sandbox_mode: None,
        macos_seatbelt_profile_extensions,
    })
}

pub(crate) fn extend_sandbox_policy(
    base: &SandboxPolicy,
    extension: &SandboxPolicy,
) -> SandboxPolicy {
    match (base, extension) {
        (SandboxPolicy::DangerFullAccess, _) | (_, SandboxPolicy::DangerFullAccess) => {
            SandboxPolicy::DangerFullAccess
        }
        (
            SandboxPolicy::ExternalSandbox {
                network_access: base_network,
            },
            SandboxPolicy::ExternalSandbox {
                network_access: extension_network,
            },
        ) => SandboxPolicy::ExternalSandbox {
            network_access: if base_network.is_enabled() || extension_network.is_enabled() {
                NetworkAccess::Enabled
            } else {
                NetworkAccess::Restricted
            },
        },
        (SandboxPolicy::ExternalSandbox { .. }, _) | (_, SandboxPolicy::ExternalSandbox { .. }) => {
            SandboxPolicy::ExternalSandbox {
                network_access: if base.has_full_network_access()
                    || extension.has_full_network_access()
                {
                    NetworkAccess::Enabled
                } else {
                    NetworkAccess::Restricted
                },
            }
        }
        (
            SandboxPolicy::ReadOnly {
                access: base_access,
            },
            SandboxPolicy::ReadOnly {
                access: extension_access,
            },
        ) => SandboxPolicy::ReadOnly {
            access: extend_read_only_access(base_access, extension_access),
        },
        (
            SandboxPolicy::ReadOnly {
                access: base_access,
            },
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            },
        ) => SandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots.clone(),
            read_only_access: extend_read_only_access(base_access, read_only_access),
            network_access: *network_access,
            exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
            exclude_slash_tmp: *exclude_slash_tmp,
        },
        (
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            },
            SandboxPolicy::ReadOnly {
                access: extension_access,
            },
        ) => SandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots.clone(),
            read_only_access: extend_read_only_access(read_only_access, extension_access),
            network_access: *network_access,
            exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
            exclude_slash_tmp: *exclude_slash_tmp,
        },
        (
            SandboxPolicy::WorkspaceWrite {
                writable_roots: base_writable_roots,
                read_only_access: base_read_only_access,
                network_access: base_network_access,
                exclude_tmpdir_env_var: base_exclude_tmpdir_env_var,
                exclude_slash_tmp: base_exclude_slash_tmp,
            },
            SandboxPolicy::WorkspaceWrite {
                writable_roots: extension_writable_roots,
                read_only_access: extension_read_only_access,
                network_access: extension_network_access,
                exclude_tmpdir_env_var: extension_exclude_tmpdir_env_var,
                exclude_slash_tmp: extension_exclude_slash_tmp,
            },
        ) => SandboxPolicy::WorkspaceWrite {
            writable_roots: extend_absolute_roots(base_writable_roots, extension_writable_roots),
            read_only_access: extend_read_only_access(
                base_read_only_access,
                extension_read_only_access,
            ),
            network_access: *base_network_access || *extension_network_access,
            exclude_tmpdir_env_var: *base_exclude_tmpdir_env_var
                && *extension_exclude_tmpdir_env_var,
            exclude_slash_tmp: *base_exclude_slash_tmp && *extension_exclude_slash_tmp,
        },
    }
}

fn extend_read_only_access(base: &ReadOnlyAccess, extension: &ReadOnlyAccess) -> ReadOnlyAccess {
    match (base, extension) {
        (ReadOnlyAccess::FullAccess, _) | (_, ReadOnlyAccess::FullAccess) => {
            ReadOnlyAccess::FullAccess
        }
        (
            ReadOnlyAccess::Restricted {
                include_platform_defaults: base_include_platform_defaults,
                readable_roots: base_readable_roots,
            },
            ReadOnlyAccess::Restricted {
                include_platform_defaults: extension_include_platform_defaults,
                readable_roots: extension_readable_roots,
            },
        ) => ReadOnlyAccess::Restricted {
            include_platform_defaults: *base_include_platform_defaults
                || *extension_include_platform_defaults,
            readable_roots: extend_absolute_roots(base_readable_roots, extension_readable_roots),
        },
    }
}

fn extend_absolute_roots(
    base_roots: &[AbsolutePathBuf],
    extension_roots: &[AbsolutePathBuf],
) -> Vec<AbsolutePathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for root in base_roots.iter().chain(extension_roots) {
        if seen.insert(root.to_path_buf()) {
            roots.push(root.clone());
        }
    }
    roots
}

fn normalize_permission_paths(
    skill_dir: &Path,
    values: &[String],
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
    value: &str,
    field: &str,
) -> Option<AbsolutePathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        warn!("ignoring {field}: value is empty");
        return None;
    }

    let expanded = expand_home(trimmed);
    let path = PathBuf::from(expanded);
    let absolute = if path.is_absolute() {
        path
    } else {
        skill_dir.join(path)
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

fn expand_home(path: &str) -> String {
    if path == "~" {
        if let Some(home) = home_dir() {
            return home.to_string_lossy().to_string();
        }
        return path.to_string();
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}

#[cfg(target_os = "macos")]
fn build_macos_seatbelt_profile_extensions(
    permissions: &SkillManifestMacOsPermissions,
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
        macos_accessibility: permissions.accessibility,
        macos_calendar: permissions.calendar,
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
    _: &SkillManifestMacOsPermissions,
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
    use super::SkillManifestFileSystemPermissions;
    #[cfg(target_os = "macos")]
    use super::SkillManifestMacOsPermissions;
    use super::SkillManifestPermissions;
    use super::compile_permission_profile;
    use super::extend_sandbox_policy;
    use crate::config::Constrained;
    use crate::config::Permissions;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::protocol::AskForApproval;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
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

        let profile = compile_permission_profile(
            &skill_dir,
            Some(SkillManifestPermissions {
                network: true,
                file_system: SkillManifestFileSystemPermissions {
                    read: vec![
                        "./data".to_string(),
                        "./data".to_string(),
                        "scripts/../data".to_string(),
                    ],
                    write: vec!["./output".to_string()],
                },
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
            Some(SkillManifestPermissions {
                network: true,
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
            Some(SkillManifestPermissions {
                network: true,
                file_system: SkillManifestFileSystemPermissions {
                    read: vec!["./data".to_string()],
                    write: Vec::new(),
                },
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
    fn extend_sandbox_policy_merges_read_only_into_workspace_write() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let base_read_root =
            AbsolutePathBuf::try_from(tempdir.path().join("base-read")).expect("absolute path");
        let extension_write_root =
            AbsolutePathBuf::try_from(tempdir.path().join("extension-write"))
                .expect("absolute path");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![base_read_root.clone()],
                },
            },
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![extension_write_root.clone()],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: Vec::new(),
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![extension_write_root],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![base_read_root],
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        );
    }

    #[test]
    fn extend_sandbox_policy_unions_workspace_roots_and_network_access() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let shared_root =
            AbsolutePathBuf::try_from(tempdir.path().join("shared")).expect("absolute path");
        let base_root =
            AbsolutePathBuf::try_from(tempdir.path().join("base")).expect("absolute path");
        let extension_root =
            AbsolutePathBuf::try_from(tempdir.path().join("extension")).expect("absolute path");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root.clone(), base_root.clone()],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root.clone(), extension_root.clone()],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: Vec::new(),
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root, base_root, extension_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
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
            Some(SkillManifestPermissions {
                macos: SkillManifestMacOsPermissions {
                    preferences: Some(super::MacOsPreferencesValue::Mode("readwrite".to_string())),
                    automations: Some(super::MacOsAutomationValue::BundleIds(vec![
                        "com.apple.Notes".to_string(),
                    ])),
                    accessibility: true,
                    calendar: true,
                },
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

        let profile =
            compile_permission_profile(&skill_dir, Some(SkillManifestPermissions::default()))
                .expect("profile");

        assert_eq!(
            profile.macos_seatbelt_profile_extensions,
            Some(crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions::default())
        );
    }
}
