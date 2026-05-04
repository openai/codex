use super::*;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

#[test]
fn command_profile_preserves_configured_deny_read_restrictions() {
    let readable_entry = FileSystemSandboxEntry {
        path: FileSystemPath::Path {
            path: test_path_buf("/tmp/project").abs(),
        },
        access: FileSystemAccessMode::Read,
    };
    let deny_entry = FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "/tmp/project/**/*.env".to_string(),
        },
        access: FileSystemAccessMode::None,
    };
    let mut file_system_sandbox_policy =
        FileSystemSandboxPolicy::restricted(vec![readable_entry.clone()]);
    let mut configured_file_system_sandbox_policy =
        FileSystemSandboxPolicy::restricted(vec![deny_entry.clone()]);
    configured_file_system_sandbox_policy.glob_scan_max_depth = Some(2);

    CommandExecRequestProcessor::preserve_configured_deny_read_restrictions(
        &mut file_system_sandbox_policy,
        &configured_file_system_sandbox_policy,
    );

    let mut expected = FileSystemSandboxPolicy::restricted(vec![readable_entry, deny_entry]);
    expected.glob_scan_max_depth = Some(2);
    assert_eq!(file_system_sandbox_policy, expected);
}
