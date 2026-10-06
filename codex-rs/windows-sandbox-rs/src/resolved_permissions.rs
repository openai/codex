use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath::Root;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Windows-local view of the runtime permission profile.
///
/// Most Windows sandbox code needs resolved runtime permissions plus a few
/// Windows-specific path conventions, not the user/config-facing
/// `PermissionProfile` enum itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWindowsSandboxPermissions {
    file_system: FileSystemSandboxPolicy,
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
    workspace_roots: &[AbsolutePathBuf],
    cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Result<WindowsSandboxTokenMode> {
    let permissions =
        ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
            permission_profile,
            workspace_roots,
        )?;
    if permissions.file_system.has_full_disk_write_access() {
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
    pub fn try_from_permission_profile(permission_profile: &PermissionProfile) -> Result<Self> {
        if !matches!(permission_profile, PermissionProfile::Managed { .. }) {
            anyhow::bail!(
                "only managed permission profiles can be enforced by the Windows sandbox"
            );
        }
        let (file_system, network) = permission_profile.to_runtime_permissions();
        if !matches!(file_system.kind, FileSystemSandboxKind::Restricted) {
            anyhow::bail!(
                "only restricted managed filesystem permissions can be enforced by the Windows sandbox"
            );
        }
        Ok(Self {
            file_system,
            network,
        })
    }

    /// Resolves a managed permission profile and binds symbolic `:workspace_roots`
    /// entries to the workspace roots supplied by the caller.
    pub fn try_from_permission_profile_for_workspace_roots(
        permission_profile: &PermissionProfile,
        workspace_roots: &[AbsolutePathBuf],
    ) -> Result<Self> {
        let mut permissions = Self::try_from_permission_profile(permission_profile)?;
        permissions.file_system = permissions
            .file_system
            .materialize_project_roots_with_workspace_roots(workspace_roots);
        Ok(permissions)
    }

    pub(crate) fn should_apply_network_block(&self) -> bool {
        !self.network.is_enabled()
    }

    pub(crate) fn network_policy(&self) -> NetworkSandboxPolicy {
        self.network
    }

    /// Rejects filesystem policies that the elevated Windows sandbox cannot
    /// enforce safely.
    pub fn validate_elevated_filesystem_policy(&self, cwd: &Path) -> Result<()> {
        let root = cwd
            .ancestors()
            .last()
            .ok_or_else(|| anyhow::anyhow!("command cwd has no filesystem root"))?;
        let root_is_denied = ReadDenyMatcher::try_new_for_local_paths(&self.file_system, cwd)
            .map_err(anyhow::Error::msg)?
            .is_some_and(|matcher| matcher.is_local_path_read_denied(root));
        if !self.file_system.can_read_local_path_with_cwd(root, cwd) || root_is_denied {
            anyhow::bail!("elevated Windows sandbox requires effective `:root` read access");
        }
        Ok(())
    }

    pub(crate) fn has_full_disk_read_access(&self) -> bool {
        self.file_system.has_full_disk_read_access()
    }

    pub(crate) fn has_symbolic_root_read_access(&self, cwd: &Path) -> bool {
        self.file_system.entries.iter().any(|entry| {
            matches!(&entry.path, FileSystemPath::Special { value: Root })
                && entry.access.can_read()
        }) && cwd
            .ancestors()
            .last()
            .is_some_and(|root| self.file_system.can_read_local_path_with_cwd(root, cwd))
    }

    pub(crate) fn include_platform_defaults(&self) -> bool {
        self.file_system.include_platform_defaults()
    }

    pub(crate) fn readable_roots_for_cwd(&self, cwd: &Path) -> Vec<PathBuf> {
        self.file_system
            .get_readable_roots_with_cwd(cwd)
            .into_iter()
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
        cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> Vec<WindowsWritableRoot> {
        let mut file_system = self.file_system.clone();
        resolve_workload_temp_paths(&mut file_system, env_map);
        file_system
            .get_writable_roots_with_cwd(cwd)
            .into_iter()
            .map(|root| WindowsWritableRoot {
                root: root.root.into_path_buf(),
                read_only_subpaths: root
                    .read_only_subpaths
                    .into_iter()
                    .map(AbsolutePathBuf::into_path_buf)
                    .collect(),
            })
            .collect()
    }
}

/// Replace `:tmpdir` with absolute TEMP/TMP paths from the completed Windows
/// workload environment. Use the same key order as the child's environment
/// block; never use host TEMP/TMP. Preserve access and missing-path rules so
/// the ordinary policy evaluator still handles read/deny carveouts.
pub fn resolve_workload_temp_paths(
    file_system: &mut FileSystemSandboxPolicy,
    workload_env: &HashMap<String, String>,
) {
    use codex_protocol::permissions::FileSystemSpecialPath;

    file_system.entries = std::mem::take(&mut file_system.entries)
        .into_iter()
        .flat_map(|entry| match &entry.path {
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            } => crate::process::ordered_env_entries(workload_env)
                .into_iter()
                .filter(|(key, value)| {
                    (key.eq_ignore_ascii_case("TEMP") || key.eq_ignore_ascii_case("TMP"))
                        && Path::new(value).is_absolute()
                })
                .filter_map(|(_, value)| AbsolutePathBuf::from_absolute_path(value).ok())
                .map(|path| FileSystemSandboxEntry {
                    path: path.into(),
                    ..entry.clone()
                })
                .collect(),
            FileSystemPath::Special {
                value: FileSystemSpecialPath::SlashTmp,
            } => Vec::new(),
            _ => vec![entry],
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ManagedFileSystemPermissions;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::project_roots_glob_pattern;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn workspace_roots_for(root: &Path) -> Vec<AbsolutePathBuf> {
        vec![AbsolutePathBuf::from_absolute_path(root).expect("absolute workspace root")]
    }

    fn permission_profile_with_entries(entries: Vec<FileSystemSandboxEntry>) -> PermissionProfile {
        PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        }
    }

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

        let permissions = ResolvedWindowsSandboxPermissions::try_from_permission_profile(
            &PermissionProfile::workspace_write(),
        )
        .expect("managed permission profile");
        let roots = permissions
            .writable_roots_for_cwd(&cwd, &env_map)
            .into_iter()
            .map(|root| root.root)
            .collect::<std::collections::HashSet<_>>();

        let expected_roots = [
            dunce::canonicalize(&temp_dir).expect("canonicalize temp dir"),
            dunce::canonicalize(&cwd).expect("canonicalize cwd"),
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

        assert_eq!(expected_roots, roots);
    }

    #[test]
    fn explicit_empty_or_missing_override_does_not_fall_back_to_temp() {
        let dir = TempDir::new().unwrap();
        let cwd = dir.path().join("work");
        let codex_home = dir.path().join("codex-home");
        let temp = dir.path().join("temp");
        for path in [&cwd, &codex_home, &temp] {
            std::fs::create_dir_all(path).unwrap();
        }
        let env = HashMap::from([("Temp".into(), temp.to_string_lossy().into_owned())]);
        let profile = permission_profile_with_entries(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                FileSystemAccessMode::Write,
            ),
        ]);
        let permissions =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile(&profile).unwrap();
        assert_eq!(
            token_mode_for_permission_profile(&profile, &[], &cwd, &env).unwrap(),
            WindowsSandboxTokenMode::WritableRootsCapability
        );
        for overrides in [vec![], vec![dir.path().join("missing")]] {
            assert!(
                crate::setup::effective_write_roots_for_permissions(
                    &permissions,
                    &cwd,
                    &env,
                    &codex_home,
                    Some(&overrides)
                )
                .is_empty(),
                "invalid override {overrides:?}"
            );
        }
    }

    #[test]
    fn unavailable_windows_temp_never_creates_write_capabilities() {
        let dir = TempDir::new().unwrap();
        let cwd = dir.path().join("work");
        std::fs::create_dir_all(&cwd).unwrap();
        let profile = permission_profile_with_entries(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                FileSystemAccessMode::Write,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::SlashTmp,
                },
                FileSystemAccessMode::Write,
            ),
        ]);
        let explicit_empty = HashMap::from([
            ("TEMP".into(), String::new()),
            ("TMP".into(), "relative".into()),
        ]);
        assert_eq!(
            token_mode_for_permission_profile(&profile, &[], &cwd, &explicit_empty).unwrap(),
            WindowsSandboxTokenMode::ReadOnlyCapability
        );
        assert_eq!(
            token_mode_for_permission_profile(&profile, &[], &cwd, &HashMap::new()).unwrap(),
            WindowsSandboxTokenMode::ReadOnlyCapability
        );
    }

    #[test]
    fn permission_profile_workspace_root_uses_runtime_workspace_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let workspace_root = tmp.path().join("workspace");
        let command_cwd = workspace_root.join("subdir");
        std::fs::create_dir_all(&command_cwd).expect("create command cwd");

        let permission_profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                }],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };
        let workspace_roots = workspace_roots_for(workspace_root.as_path());
        let permissions =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
                &permission_profile,
                workspace_roots.as_slice(),
            )
            .expect("managed permission profile");

        let roots = permissions
            .writable_roots_for_cwd(&command_cwd, &HashMap::new())
            .into_iter()
            .map(|root| root.root)
            .collect::<Vec<_>>();

        assert_eq!(
            roots,
            vec![dunce::canonicalize(&workspace_root).expect("canonical workspace root")]
        );
    }

    #[test]
    fn permission_profile_workspace_roots_expand_all_runtime_workspace_roots() {
        let tmp = TempDir::new().expect("tempdir");
        let first = AbsolutePathBuf::from_absolute_path(tmp.path().join("first"))
            .expect("absolute first root");
        let second = AbsolutePathBuf::from_absolute_path(tmp.path().join("second"))
            .expect("absolute second root");
        let permission_profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                        },
                        access: FileSystemAccessMode::Write,
                        missing_path_behavior: None,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::project_roots(Some(".git".into())),
                        },
                        access: FileSystemAccessMode::Deny,
                        missing_path_behavior: None,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::GlobPattern {
                            pattern: project_roots_glob_pattern(Path::new("**/*.env")),
                        },
                        access: FileSystemAccessMode::Deny,
                        missing_path_behavior: None,
                    },
                ],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };

        let permissions =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
                &permission_profile,
                &[first.clone(), second.clone()],
            )
            .expect("managed permission profile");

        assert_eq!(
            permissions.file_system,
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: first.clone().into(),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: second.clone().into(),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: first.join(".git").into(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: second.join(".git").into(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: AbsolutePathBuf::resolve_path_against_base(
                            "**/*.env",
                            first.as_path(),
                        )
                        .to_string_lossy()
                        .into_owned(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: AbsolutePathBuf::resolve_path_against_base(
                            "**/*.env",
                            second.as_path(),
                        )
                        .to_string_lossy()
                        .into_owned(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
            ])
        );
    }

    #[test]
    fn token_mode_for_profile_without_writable_roots_uses_readonly_capability() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = tmp.path().join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let workspace_roots = workspace_roots_for(cwd.as_path());

        let token_mode = token_mode_for_permission_profile(
            &PermissionProfile::read_only(),
            workspace_roots.as_slice(),
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
        let workspace_roots = workspace_roots_for(cwd.as_path());

        let token_mode = token_mode_for_permission_profile(
            &PermissionProfile::workspace_write(),
            workspace_roots.as_slice(),
            &cwd,
            &HashMap::new(),
        )
        .expect("token mode");

        assert_eq!(WindowsSandboxTokenMode::WritableRootsCapability, token_mode);
    }

    #[test]
    fn permission_profile_rejects_disabled_profiles() {
        let err = ResolvedWindowsSandboxPermissions::try_from_permission_profile(
            &PermissionProfile::Disabled,
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

        let err =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile(&permission_profile)
                .expect_err("unrestricted profile should not resolve for sandbox enforcement");

        assert!(
            err.to_string()
                .contains("only restricted managed filesystem permissions can be enforced")
        );
    }

    #[test]
    fn elevated_filesystem_policy_requires_effective_root_read_access() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = tmp.path().join("workspace");
        let denied_path = tmp.path().join("private");
        std::fs::create_dir_all(&cwd).expect("create cwd");

        let default_root_denied = ResolvedWindowsSandboxPermissions::try_from_permission_profile(
            &permission_profile_with_entries(vec![FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                FileSystemAccessMode::Write,
            )]),
        )
        .expect("managed permission profile");
        let root_read_with_carveout =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile(
                &permission_profile_with_entries(vec![
                    FileSystemSandboxEntry::new(
                        FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        FileSystemAccessMode::Read,
                    ),
                    FileSystemSandboxEntry::new(
                        FileSystemPath::Path {
                            path: AbsolutePathBuf::from_absolute_path(&denied_path)
                                .expect("absolute denied path")
                                .into(),
                        },
                        FileSystemAccessMode::Deny,
                    ),
                ]),
            )
            .expect("managed permission profile");
        let root = cwd.ancestors().last().expect("filesystem root");
        let root_read_with_explicit_path =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile(
                &permission_profile_with_entries(vec![FileSystemSandboxEntry::new(
                    FileSystemPath::Path {
                        path: AbsolutePathBuf::from_absolute_path(root)
                            .expect("absolute root")
                            .into(),
                    },
                    FileSystemAccessMode::Read,
                )]),
            )
            .expect("managed permission profile");
        let root_read_with_root_deny_glob =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile(
                &permission_profile_with_entries(vec![
                    FileSystemSandboxEntry::new(
                        FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        FileSystemAccessMode::Read,
                    ),
                    FileSystemSandboxEntry::new(
                        FileSystemPath::GlobPattern {
                            pattern: root.join("**").display().to_string(),
                        },
                        FileSystemAccessMode::Deny,
                    ),
                ]),
            )
            .expect("managed permission profile");

        assert!(
            default_root_denied
                .validate_elevated_filesystem_policy(&cwd)
                .is_err()
        );
        assert!(
            root_read_with_carveout
                .validate_elevated_filesystem_policy(&cwd)
                .is_ok()
        );
        assert!(
            root_read_with_explicit_path
                .validate_elevated_filesystem_policy(&cwd)
                .is_ok()
        );
        assert!(
            root_read_with_root_deny_glob
                .validate_elevated_filesystem_policy(&cwd)
                .is_err()
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
                    missing_path_behavior: None,
                }],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };
        let workspace_roots = workspace_roots_for(cwd.as_path());

        let err = token_mode_for_permission_profile(
            &permission_profile,
            workspace_roots.as_slice(),
            &cwd,
            &HashMap::new(),
        )
        .expect_err("full disk writes should not resolve to a token mode");

        assert!(
            err.to_string()
                .contains("full-disk filesystem writes, which cannot be enforced")
        );
    }
}
