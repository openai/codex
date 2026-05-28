use anyhow::Context;
use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::WritableRoot;
use codex_sandboxing::EffectiveFilesystemPermissions;
use codex_sandboxing::FilesystemPermissionsContext;
use codex_sandboxing::FilesystemPermissionsMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Windows-local view of the runtime permission profile.
///
/// Most Windows sandbox code needs resolved runtime permissions plus a few
/// Windows-specific path conventions, not the user/config-facing
/// `PermissionProfile` enum itself.
#[derive(Debug)]
pub struct ResolvedWindowsSandboxPermissions {
    effective_file_system: EffectiveFilesystemPermissions,
    writable_roots: Vec<WritableRoot>,
    has_writable_tmpdir_entry: bool,
    network: NetworkSandboxPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowsWritableRoot {
    pub(crate) root: PathBuf,
    pub(crate) read_only_subpaths: Vec<PathBuf>,
}

/// Restricted-token family needed to enforce a Windows permission profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsSandboxTokenMode {
    ReadOnlyCapability,
    WritableRootsCapability,
}

/// Chooses the restricted-token family needed for a managed permission profile.
pub fn token_mode_for_permission_profile(
    permission_profile: &PermissionProfile,
    cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Result<WindowsSandboxTokenMode> {
    let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_cwd(
        permission_profile,
        cwd,
    )?;
    if permissions
        .effective_file_system
        .has_full_disk_write_access()
    {
        anyhow::bail!(
            "permission profile requests full-disk filesystem writes, which cannot be enforced by the Windows sandbox"
        );
    }
    if permissions.writable_roots_for_cwd(cwd, env_map).is_empty() {
        Ok(WindowsSandboxTokenMode::ReadOnlyCapability)
    } else {
        Ok(WindowsSandboxTokenMode::WritableRootsCapability)
    }
}

impl ResolvedWindowsSandboxPermissions {
    /// Resolves a managed permission profile for the Windows compatibility boundary.
    ///
    /// Normal runtime callers provide already-materialized workspace roots. For
    /// callers that still provide symbolic `:workspace_roots`, this adapter
    /// explicitly binds them to the permission root supplied by the caller.
    pub fn try_from_permission_profile_for_cwd(
        permission_profile: &PermissionProfile,
        cwd: &Path,
    ) -> Result<Self> {
        if !matches!(permission_profile, PermissionProfile::Managed { .. }) {
            anyhow::bail!(
                "only managed permission profiles can be enforced by the Windows sandbox"
            );
        }
        let permission_profile_cwd = AbsolutePathBuf::from_absolute_path(cwd)
            .context("permission profile cwd must be absolute for the Windows sandbox")?;
        let permission_profile = permission_profile
            .clone()
            .materialize_project_roots_with_workspace_roots(std::slice::from_ref(
                &permission_profile_cwd,
            ));
        let (file_system, network) = permission_profile.to_runtime_permissions();
        if !matches!(file_system.kind, FileSystemSandboxKind::Restricted) {
            anyhow::bail!(
                "only restricted managed filesystem permissions can be enforced by the Windows sandbox"
            );
        }
        let effective_file_system = EffectiveFilesystemPermissions::from_profile(
            &permission_profile,
            FilesystemPermissionsContext {
                policy_evaluation_cwd: &permission_profile_cwd,
            },
        )?;
        let has_writable_tmpdir_entry =
            file_system
                .entries
                .iter()
                .any(|FileSystemSandboxEntry { path, access }| {
                    matches!(
                        path,
                        FileSystemPath::Special {
                            value: codex_protocol::permissions::FileSystemSpecialPath::Tmpdir,
                        }
                    ) && access.can_write()
                });
        let mut windows_writable_file_system = file_system;
        windows_writable_file_system
            .entries
            .retain(|FileSystemSandboxEntry { path, .. }| {
                !matches!(
                    path,
                    FileSystemPath::Special {
                        value: codex_protocol::permissions::FileSystemSpecialPath::Tmpdir
                            | codex_protocol::permissions::FileSystemSpecialPath::SlashTmp,
                    }
                )
            });
        let windows_writable_profile =
            PermissionProfile::from_runtime_permissions(&windows_writable_file_system, network);
        let writable_roots = EffectiveFilesystemPermissions::from_profile(
            &windows_writable_profile,
            FilesystemPermissionsContext {
                policy_evaluation_cwd: &permission_profile_cwd,
            },
        )?
        .writable_roots;

        Ok(Self {
            effective_file_system,
            writable_roots,
            has_writable_tmpdir_entry,
            network,
        })
    }

    pub(crate) fn should_apply_network_block(&self) -> bool {
        !self.network.is_enabled()
    }

    pub(crate) fn network_policy(&self) -> NetworkSandboxPolicy {
        self.network
    }

    pub(crate) fn is_enforceable_by_windows_sandbox(&self) -> bool {
        matches!(
            self.effective_file_system.mode,
            FilesystemPermissionsMode::Restricted
        )
    }

    pub(crate) fn has_full_disk_read_access(&self) -> bool {
        self.effective_file_system.has_full_disk_read_access()
    }

    pub(crate) fn include_platform_defaults(&self) -> bool {
        self.effective_file_system.include_platform_defaults
    }

    pub(crate) fn readable_roots_for_cwd(&self, _cwd: &Path) -> Vec<PathBuf> {
        self.effective_file_system
            .readable_roots
            .iter()
            .cloned()
            .map(AbsolutePathBuf::into_path_buf)
            .collect()
    }

    pub(crate) fn uses_write_capabilities_for_cwd(
        &self,
        cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> bool {
        !self.writable_roots_for_cwd(cwd, env_map).is_empty()
    }

    pub(crate) fn writable_roots_for_cwd(
        &self,
        _cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> Vec<WindowsWritableRoot> {
        let mut roots = self
            .writable_roots
            .iter()
            .cloned()
            .map(|root| WindowsWritableRoot {
                root: root.root.into_path_buf(),
                read_only_subpaths: root
                    .read_only_subpaths
                    .into_iter()
                    .map(AbsolutePathBuf::into_path_buf)
                    .collect(),
            })
            .collect::<Vec<_>>();

        if self.has_writable_tmpdir_entry {
            roots.extend(windows_temp_env_roots(env_map).into_iter().map(|root| {
                WindowsWritableRoot {
                    root,
                    read_only_subpaths: Vec::new(),
                }
            }));
        }

        roots
    }
}

fn windows_temp_env_roots(env_map: &HashMap<String, String>) -> Vec<PathBuf> {
    ["TEMP", "TMP"]
        .into_iter()
        .filter_map(|key| {
            env_map
                .get(key)
                .map(|value| PathBuf::from(value.as_str()))
                .or_else(|| std::env::var_os(key).map(PathBuf::from))
        })
        .filter(|path| path.is_absolute())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ManagedFileSystemPermissions;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn permission_profile_workspace_write_uses_windows_temp_env_vars() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = tmp.path().join("workspace");
        let temp_dir = tmp.path().join("temp");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let mut env_map = HashMap::new();
        env_map.insert("TEMP".to_string(), temp_dir.to_string_lossy().to_string());
        env_map.insert("TMP".to_string(), temp_dir.to_string_lossy().to_string());

        let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_cwd(
            &PermissionProfile::workspace_write(),
            &cwd,
        )
        .expect("managed permission profile");
        let roots = permissions
            .writable_roots_for_cwd(&cwd, &env_map)
            .into_iter()
            .map(|root| root.root)
            .collect::<std::collections::HashSet<_>>();

        let expected_roots = [
            temp_dir,
            dunce::canonicalize(&cwd).expect("canonicalize cwd"),
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

        assert_eq!(expected_roots, roots);
    }

    #[test]
    fn permission_profile_workspace_root_stays_bound_to_profile_cwd() {
        let tmp = TempDir::new().expect("tempdir");
        let profile_cwd = tmp.path().join("workspace");
        let command_cwd = profile_cwd.join("subdir");
        std::fs::create_dir_all(&command_cwd).expect("create command cwd");

        let permission_profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    access: FileSystemAccessMode::Write,
                }],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };
        let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_cwd(
            &permission_profile,
            &profile_cwd,
        )
        .expect("managed permission profile");

        let roots = permissions
            .writable_roots_for_cwd(&command_cwd, &HashMap::new())
            .into_iter()
            .map(|root| root.root)
            .collect::<Vec<_>>();

        assert_eq!(
            roots,
            vec![dunce::canonicalize(&profile_cwd).expect("canonical profile cwd")]
        );
    }

    #[test]
    fn token_mode_for_profile_without_writable_roots_uses_readonly_capability() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = tmp.path().join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");

        let token_mode = token_mode_for_permission_profile(
            &PermissionProfile::read_only(),
            &cwd,
            &HashMap::new(),
        )
        .expect("token mode");

        assert_eq!(WindowsSandboxTokenMode::ReadOnlyCapability, token_mode);
    }

    #[test]
    fn token_mode_for_profile_with_writable_roots_uses_write_capabilities() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = tmp.path().join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");

        let token_mode = token_mode_for_permission_profile(
            &PermissionProfile::workspace_write(),
            &cwd,
            &HashMap::new(),
        )
        .expect("token mode");

        assert_eq!(WindowsSandboxTokenMode::WritableRootsCapability, token_mode);
    }

    #[test]
    fn permission_profile_rejects_disabled_profiles() {
        let err = ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_cwd(
            &PermissionProfile::Disabled,
            Path::new("/"),
        )
        .expect_err("disabled profile should not resolve for sandbox enforcement");

        assert!(
            err.to_string()
                .contains("only managed permission profiles can be enforced")
        );
    }

    #[test]
    fn permission_profile_rejects_unrestricted_managed_filesystem() {
        let permission_profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Unrestricted,
            network: NetworkSandboxPolicy::Restricted,
        };

        let err = ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_cwd(
            &permission_profile,
            Path::new("/"),
        )
        .expect_err("unrestricted profile should not resolve for sandbox enforcement");

        assert!(
            err.to_string()
                .contains("only restricted managed filesystem permissions can be enforced")
        );
    }

    #[test]
    fn token_mode_rejects_full_disk_write_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = tmp.path().join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let permission_profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Write,
                }],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };

        let err = token_mode_for_permission_profile(&permission_profile, &cwd, &HashMap::new())
            .expect_err("full disk writes should not resolve to a token mode");

        assert!(
            err.to_string()
                .contains("full-disk filesystem writes, which cannot be enforced")
        );
    }
}
