use crate::util::resolve_path;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub(crate) struct PatchApprovalStore {
    approved_paths: HashSet<PathBuf>,
}

impl PatchApprovalStore {
    pub fn approve_action(&mut self, action: &ApplyPatchAction, cwd: &Path) {
        for (path, change) in action.changes() {
            self.approved_paths.insert(resolve_and_normalize(cwd, path));
            if let ApplyPatchFileChange::Update { move_path, .. } = change
                && let Some(dest) = move_path
            {
                self.approved_paths.insert(resolve_and_normalize(cwd, dest));
            }
        }
    }

    pub fn is_action_approved(&self, action: &ApplyPatchAction, cwd: &Path) -> bool {
        for (path, change) in action.changes() {
            if !self
                .approved_paths
                .contains(&resolve_and_normalize(cwd, path))
            {
                return false;
            }
            if let ApplyPatchFileChange::Update { move_path, .. } = change
                && let Some(dest) = move_path
                && !self
                    .approved_paths
                    .contains(&resolve_and_normalize(cwd, dest))
            {
                return false;
            }
        }
        true
    }
}

fn resolve_and_normalize(cwd: &Path, path: &PathBuf) -> PathBuf {
    let abs = resolve_path(cwd, path);
    normalize_path_components(&abs)
}

fn normalize_path_components(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn parse_action(cwd: &Path, patch: &str) -> ApplyPatchAction {
        let argv = vec!["apply_patch".to_string(), patch.to_string()];
        match codex_apply_patch::maybe_parse_apply_patch_verified(&argv, cwd) {
            codex_apply_patch::MaybeApplyPatchVerified::Body(action) => action,
            other => panic!("expected patch body, got: {other:?}"),
        }
    }

    #[test]
    fn approved_for_session_covers_all_touched_files() {
        let tmp = TempDir::new().expect("tmp");
        let cwd = tmp.path();

        let patch_two_files = r#"*** Begin Patch
*** Add File: a.txt
+hello
*** Add File: b.txt
+world
*** End Patch"#;
        let action = parse_action(cwd, patch_two_files);

        let mut store = PatchApprovalStore::default();
        assert!(!store.is_action_approved(&action, cwd));

        store.approve_action(&action, cwd);
        assert!(store.is_action_approved(&action, cwd));

        let patch_a_only = r#"*** Begin Patch
*** Add File: a.txt
+again
*** End Patch"#;
        let action_a = parse_action(cwd, patch_a_only);
        assert!(store.is_action_approved(&action_a, cwd));

        let patch_c_only = r#"*** Begin Patch
*** Add File: c.txt
+nope
*** End Patch"#;
        let action_c = parse_action(cwd, patch_c_only);
        assert!(!store.is_action_approved(&action_c, cwd));
    }
}
