use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApplyPatchFileApprovalKey {
    kind: &'static str,
    path: AbsolutePathBuf,
}

impl ApplyPatchFileApprovalKey {
    fn new(path: AbsolutePathBuf) -> Self {
        Self {
            kind: "applyPatchFile",
            path,
        }
    }
}

pub(crate) fn approval_keys_for_action(
    action: &ApplyPatchAction,
) -> Vec<ApplyPatchFileApprovalKey> {
    let mut keys = Vec::new();
    let cwd = action.cwd.as_path();

    for (path, change) in action.changes() {
        if let Some(key) = approval_key_for_path(cwd, path) {
            keys.push(key);
        }

        if let ApplyPatchFileChange::Update { move_path, .. } = change
            && let Some(dest) = move_path
            && let Some(key) = approval_key_for_path(cwd, dest)
        {
            keys.push(key);
        }
    }

    keys
}

fn approval_key_for_path(cwd: &Path, path: &Path) -> Option<ApplyPatchFileApprovalKey> {
    AbsolutePathBuf::resolve_path_against_base(path, cwd)
        .ok()
        .map(ApplyPatchFileApprovalKey::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_apply_patch::MaybeApplyPatchVerified;
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

        let keys = approval_keys_for_action(&action);
        assert_eq!(keys.len(), 2);
    }
}
