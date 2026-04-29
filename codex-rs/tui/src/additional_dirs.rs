use codex_protocol::models::PermissionProfile;
use std::path::PathBuf;

/// Returns a warning describing why `--add-dir` entries will be ignored for the
/// resolved permission profile. The caller is responsible for presenting the
/// warning to the user (for example, printing to stderr).
pub fn add_dir_warning_message(
    additional_dirs: &[PathBuf],
    permission_profile: &PermissionProfile,
    cwd: &std::path::Path,
) -> Option<String> {
    if additional_dirs.is_empty() {
        return None;
    }

    if matches!(
        permission_profile,
        PermissionProfile::Disabled | PermissionProfile::External { .. }
    ) {
        return None;
    }

    let file_system_policy = permission_profile.file_system_sandbox_policy();
    if file_system_policy.has_full_disk_write_access() {
        return None;
    }

    if file_system_policy.can_write_path_with_cwd(cwd, cwd) {
        return None;
    }

    Some(format_warning(additional_dirs))
}

fn format_warning(additional_dirs: &[PathBuf]) -> String {
    let joined_paths = additional_dirs
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Ignoring --add-dir ({joined_paths}) because the effective permissions do not allow additional writable roots. Switch to workspace-write or danger-full-access to allow them."
    )
}

#[cfg(test)]
mod tests {
    use super::add_dir_warning_message;
    use codex_protocol::models::ManagedFileSystemPermissions;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::protocol::NetworkSandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn returns_none_for_workspace_write() {
        let profile = PermissionProfile::workspace_write();
        let dirs = vec![PathBuf::from("/tmp/example")];
        assert_eq!(
            add_dir_warning_message(&dirs, &profile, Path::new("/tmp/project")),
            None
        );
    }

    #[test]
    fn returns_none_for_danger_full_access() {
        let profile = PermissionProfile::Disabled;
        let dirs = vec![PathBuf::from("/tmp/example")];
        assert_eq!(
            add_dir_warning_message(&dirs, &profile, Path::new("/tmp/project")),
            None
        );
    }

    #[test]
    fn returns_none_for_external_sandbox() {
        let profile = PermissionProfile::External {
            network: NetworkSandboxPolicy::Enabled,
        };
        let dirs = vec![PathBuf::from("/tmp/example")];
        assert_eq!(
            add_dir_warning_message(&dirs, &profile, Path::new("/tmp/project")),
            None
        );
    }

    #[test]
    fn warns_for_read_only() {
        let profile = PermissionProfile::read_only();
        let dirs = vec![PathBuf::from("relative"), PathBuf::from("/abs")];
        let message = add_dir_warning_message(&dirs, &profile, Path::new("/tmp/project"))
            .expect("expected warning for read-only sandbox");
        assert_eq!(
            message,
            "Ignoring --add-dir (relative, /abs) because the effective permissions do not allow additional writable roots. Switch to workspace-write or danger-full-access to allow them."
        );
    }

    #[test]
    fn warns_when_profile_can_write_elsewhere_but_not_cwd() {
        let profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Path {
                            path: "/tmp/writable".try_into().expect("absolute path"),
                        },
                        access: FileSystemAccessMode::Write,
                    },
                ],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };
        let dirs = vec![PathBuf::from("/tmp/extra")];

        assert_eq!(
            add_dir_warning_message(&dirs, &profile, Path::new("/tmp/project")),
            Some("Ignoring --add-dir (/tmp/extra) because the effective permissions do not allow additional writable roots. Switch to workspace-write or danger-full-access to allow them.".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_additional_dirs() {
        let profile = PermissionProfile::read_only();
        let dirs: Vec<PathBuf> = Vec::new();
        assert_eq!(
            add_dir_warning_message(&dirs, &profile, Path::new("/tmp/project")),
            None
        );
    }
}
