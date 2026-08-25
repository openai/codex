#![allow(clippy::expect_used)]

use codex_worktree::WorktreeSettings;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;

fn desktop_config(entries: [(&str, Value); 3]) -> HashMap<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

#[test]
fn default_settings_match_desktop_worktree_defaults() {
    let codex_home = tempfile::tempdir().expect("create Codex home");

    let settings = WorktreeSettings::from_desktop_config(codex_home.path(), /*desktop*/ None)
        .expect("load default worktree settings");

    assert_eq!(
        settings,
        WorktreeSettings {
            root: codex_home.path().join("worktrees"),
            auto_cleanup_enabled: true,
            keep_count: 15,
        }
    );
}

#[test]
fn desktop_settings_use_existing_root_and_retention_keys() {
    let codex_home = tempfile::tempdir().expect("create Codex home");
    let custom_root = codex_home.path().join("custom-managed-worktrees");
    let desktop = desktop_config([
        (
            "git-worktree-root",
            json!(custom_root.to_string_lossy().into_owned()),
        ),
        ("worktree-auto-cleanup-enabled", json!(false)),
        ("worktree-keep-count", json!(4)),
    ]);

    let settings = WorktreeSettings::from_desktop_config(codex_home.path(), Some(&desktop))
        .expect("load existing Desktop worktree settings");

    assert_eq!(
        settings,
        WorktreeSettings {
            root: custom_root,
            auto_cleanup_enabled: false,
            keep_count: 4,
        }
    );
}

#[test]
fn desktop_settings_reject_invalid_root_and_retention_values() {
    let codex_home = tempfile::tempdir().expect("create Codex home");
    let invalid_configs = [
        HashMap::from([("git-worktree-root".to_owned(), json!("relative/worktrees"))]),
        HashMap::from([("worktree-auto-cleanup-enabled".to_owned(), json!("yes"))]),
        HashMap::from([("worktree-keep-count".to_owned(), json!(0))]),
        HashMap::from([("worktree-keep-count".to_owned(), json!(-1))]),
        HashMap::from([("worktree-keep-count".to_owned(), json!(1.5))]),
    ];

    for desktop in invalid_configs {
        assert!(
            WorktreeSettings::from_desktop_config(codex_home.path(), Some(&desktop)).is_err(),
            "invalid Desktop worktree settings were accepted: {desktop:?}",
        );
    }
}
