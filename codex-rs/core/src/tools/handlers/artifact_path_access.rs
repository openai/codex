use codex_protocol::protocol::FileSystemSandboxPolicy;
use std::path::Path;

pub(super) fn path_is_readable(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    path: &Path,
) -> bool {
    if file_system_sandbox_policy
        .get_unreadable_roots_with_cwd(cwd)
        .iter()
        .any(|root| path.starts_with(root.as_path()))
    {
        return false;
    }

    if file_system_sandbox_policy.has_full_disk_read_access() {
        return true;
    }

    file_system_sandbox_policy
        .get_readable_roots_with_cwd(cwd)
        .iter()
        .any(|root| path.starts_with(root.as_path()))
}

pub(super) fn path_is_writable(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    path: &Path,
) -> bool {
    if file_system_sandbox_policy
        .get_unreadable_roots_with_cwd(cwd)
        .iter()
        .any(|root| path.starts_with(root.as_path()))
    {
        return false;
    }

    if file_system_sandbox_policy.has_full_disk_write_access() {
        return true;
    }

    file_system_sandbox_policy
        .get_writable_roots_with_cwd(cwd)
        .iter()
        .any(|root| root.is_path_writable(path))
}

#[cfg(test)]
mod tests {
    use super::path_is_readable;
    use super::path_is_writable;
    use codex_protocol::protocol::FileSystemAccessMode;
    use codex_protocol::protocol::FileSystemPath;
    use codex_protocol::protocol::FileSystemSandboxEntry;
    use codex_protocol::protocol::FileSystemSandboxPolicy;
    use codex_protocol::protocol::FileSystemSpecialPath;
    use codex_protocol::protocol::FileSystemSpecialPathKind;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn unreadable_subpaths_override_root_read_access() {
        let cwd = TempDir::new().expect("tempdir");
        let readable = cwd.path().join("readable.txt");
        let blocked = cwd.path().join("blocked.txt");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath {
                        kind: FileSystemSpecialPathKind::Root,
                        subpath: None,
                    },
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath {
                        kind: FileSystemSpecialPathKind::CurrentWorkingDirectory,
                        subpath: Some(PathBuf::from("blocked.txt")),
                    },
                },
                access: FileSystemAccessMode::None,
            },
        ]);

        assert_eq!(path_is_readable(&policy, cwd.path(), &readable), true);
        assert_eq!(path_is_readable(&policy, cwd.path(), &blocked), false);
    }

    #[test]
    fn unreadable_subpaths_override_writable_roots() {
        let cwd = TempDir::new().expect("tempdir");
        let allowed = cwd.path().join("allowed.txt");
        let blocked = cwd.path().join("blocked.txt");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath {
                        kind: FileSystemSpecialPathKind::CurrentWorkingDirectory,
                        subpath: None,
                    },
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath {
                        kind: FileSystemSpecialPathKind::CurrentWorkingDirectory,
                        subpath: Some(PathBuf::from("blocked.txt")),
                    },
                },
                access: FileSystemAccessMode::None,
            },
        ]);

        assert_eq!(path_is_writable(&policy, cwd.path(), &allowed), true);
        assert_eq!(path_is_writable(&policy, cwd.path(), &blocked), false);
    }
}
