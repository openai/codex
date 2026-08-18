use super::preapproved_permission_profile;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

fn file_system_permissions(entries: Vec<FileSystemSandboxEntry>) -> AdditionalPermissionProfile {
    AdditionalPermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries,
            glob_scan_max_depth: None,
        }),
        ..Default::default()
    }
}

#[test]
fn preapproval_accepts_reordered_replay_of_one_accumulated_grant() {
    let cwd = tempdir().expect("tempdir");
    let root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
    let write = FileSystemSandboxEntry::new(root.clone().into(), FileSystemAccessMode::Write);
    let mut granted = file_system_permissions(vec![
        FileSystemSandboxEntry::new(root.join("secrets").into(), FileSystemAccessMode::Deny),
        write.clone(),
        FileSystemSandboxEntry::new(root.join("readonly").into(), FileSystemAccessMode::Read),
    ]);
    granted.network = Some(NetworkPermissions {
        enabled: Some(true),
    });
    let replay = file_system_permissions(vec![write]);
    let effective = merge_permission_profiles(Some(&replay), Some(&granted)).expect("permissions");

    assert_eq!(
        preapproved_permission_profile(&effective, &granted, cwd.path()),
        Some(granted)
    );
}

#[test]
fn preapproval_requires_fresh_read_and_write_beneath_a_deny() {
    let cwd = tempdir().expect("tempdir");
    let root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
    let granted = file_system_permissions(vec![
        FileSystemSandboxEntry::new(root.clone().into(), FileSystemAccessMode::Write),
        FileSystemSandboxEntry::new(root.join("secrets").into(), FileSystemAccessMode::Deny),
    ]);

    let preapproved = [FileSystemAccessMode::Read, FileSystemAccessMode::Write].map(|access| {
        let requested = file_system_permissions(vec![FileSystemSandboxEntry::new(
            root.join("secrets/token.txt").into(),
            access,
        )]);
        let effective =
            merge_permission_profiles(Some(&requested), Some(&granted)).expect("permissions");

        preapproved_permission_profile(&effective, &granted, cwd.path())
    });

    assert_eq!(preapproved, [None, None]);
}

#[test]
fn preapproval_fails_closed_when_materialization_rejects_both_profiles() {
    let cwd = tempdir().expect("tempdir");
    let invalid = file_system_permissions(vec![FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: "**/secrets".to_string(),
        },
        FileSystemAccessMode::Write,
    )]);

    assert_eq!(
        preapproved_permission_profile(&invalid, &invalid, cwd.path()),
        None
    );
}
