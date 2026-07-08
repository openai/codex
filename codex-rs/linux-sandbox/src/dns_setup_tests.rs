use super::*;
use crate::bwrap::BwrapArgs;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn unsets_loader_environment_before_inner_command() {
    let mut bwrap_args = BwrapArgs {
        args: vec![
            "bwrap".to_string(),
            "--".to_string(),
            "/bin/true".to_string(),
        ],
        preserved_files: Vec::new(),
        synthetic_mount_targets: Vec::new(),
        protected_create_targets: Vec::new(),
    };
    append_bwrap_args(
        &mut bwrap_args,
        ResolvConfMount {
            file: tempfile::tempfile().expect("temporary resolver configuration"),
            path: PathBuf::from("/etc/resolv.conf"),
        },
    );

    let separator = bwrap_args
        .args
        .iter()
        .position(|arg| arg == "--")
        .expect("command separator");
    let setup_args = &bwrap_args.args[..separator];
    for key in ["LD_AUDIT", "LD_LIBRARY_PATH", "LD_PRELOAD"] {
        assert!(
            setup_args
                .windows(2)
                .any(|args| args == ["--unsetenv", key]),
            "{key} must be unset before the privileged DNS setup command"
        );
    }
}

#[test]
fn rejects_read_denied_resolver_paths() {
    let logical_path = PathBuf::from("/etc/resolv.conf");
    let target_path = PathBuf::from("/run/systemd/resolve/stub-resolv.conf");
    for denied_path in [&logical_path, &target_path] {
        let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: AbsolutePathBuf::from_absolute_path(denied_path.clone())
                        .expect("absolute resolver configuration"),
                },
                access: FileSystemAccessMode::Deny,
            },
        ]);

        let err = ensure_resolver_paths_allowed(
            &file_system_sandbox_policy,
            Path::new("/"),
            &logical_path,
            &target_path,
        )
        .expect_err("read-denied resolver configuration must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }
}
