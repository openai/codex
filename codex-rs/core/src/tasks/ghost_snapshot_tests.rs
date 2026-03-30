use super::*;
use codex_git_utils::LargeUntrackedDir;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn large_untracked_warning_includes_threshold() {
    let report = GhostSnapshotReport {
        large_untracked_dirs: vec![LargeUntrackedDir {
            path: PathBuf::from("models"),
            file_count: 250,
        }],
        ignored_untracked_files: Vec::new(),
    };

    let message = format_large_untracked_warning(Some(200), &report).unwrap();
    assert!(message.contains(">= 200 files"));
}

#[test]
fn large_untracked_warning_disabled_when_threshold_disabled() {
    let report = GhostSnapshotReport {
        large_untracked_dirs: vec![LargeUntrackedDir {
            path: PathBuf::from("models"),
            file_count: 250,
        }],
        ignored_untracked_files: Vec::new(),
    };

    assert_eq!(
        format_large_untracked_warning(/*ignore_large_untracked_dirs*/ None, &report),
        None
    );
}
