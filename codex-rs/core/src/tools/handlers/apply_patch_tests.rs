use super::*;
use codex_apply_patch::MaybeApplyPatchVerified;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn approval_keys_include_move_destination() {
    let tmp = TempDir::new().expect("tmp");
    let cwd = tmp.path();
    std::fs::create_dir_all(cwd.join("old")).expect("create old dir");
    std::fs::create_dir_all(cwd.join("renamed/dir")).expect("create dest dir");
    std::fs::write(cwd.join("old/name.txt"), "old content\n").expect("write old file");
    let patch = r#"*** Begin Patch
*** Update File: old/name.txt
*** Move to: renamed/dir/name.txt
@@
-old content
+new content
*** End Patch"#;
    let argv = vec!["apply_patch".to_string(), patch.to_string()];
    let action = match codex_apply_patch::maybe_parse_apply_patch_verified(&argv, cwd) {
        MaybeApplyPatchVerified::Body(action) => action,
        other => panic!("expected patch body, got: {other:?}"),
    };

    let keys = file_paths_for_action(&action);
    assert_eq!(keys.len(), 2);
}
