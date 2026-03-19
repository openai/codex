use crate::acl::add_deny_write_ace;
use crate::path_normalization::canonicalize_path;
use anyhow::Result;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;

pub fn is_command_cwd_root(root: &Path, canonical_command_cwd: &Path) -> bool {
    canonicalize_path(root) == canonical_command_cwd
}

/// # Safety
/// Caller must ensure `psid` is a valid SID pointer.
pub unsafe fn protect_workspace_codex_dir(cwd: &Path, psid: *mut c_void) -> Result<bool> {
    protect_root_subdir(cwd, psid, ".codex", MissingPathPolicy::CreateDir)
}

/// # Safety
/// Caller must ensure `psid` is a valid SID pointer.
pub unsafe fn protect_workspace_agents_dir(cwd: &Path, psid: *mut c_void) -> Result<bool> {
    protect_root_subdir(cwd, psid, ".agents", MissingPathPolicy::ExistingOnly)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MissingPathPolicy {
    ExistingOnly,
    CreateDir,
}

fn prepare_root_subdir(
    root: &Path,
    subdir: &str,
    missing_path_policy: MissingPathPolicy,
) -> Result<Option<PathBuf>> {
    let path = root.join(subdir);
    if path.exists() {
        return Ok(Some(path));
    }

    match missing_path_policy {
        MissingPathPolicy::ExistingOnly => Ok(None),
        MissingPathPolicy::CreateDir => {
            // Windows deny ACEs require an existing path, so reserve `.codex` eagerly.
            std::fs::create_dir_all(&path)?;
            Ok(Some(path))
        }
    }
}

unsafe fn protect_root_subdir(
    root: &Path,
    psid: *mut c_void,
    subdir: &str,
    missing_path_policy: MissingPathPolicy,
) -> Result<bool> {
    let Some(path) = prepare_root_subdir(root, subdir, missing_path_policy)? else {
        return Ok(false);
    };
    add_deny_write_ace(&path, psid)
}

#[cfg(test)]
mod tests {
    use super::MissingPathPolicy;
    use super::prepare_root_subdir;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn reserves_missing_codex_dir_for_protection() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace root");

        let prepared = prepare_root_subdir(&root, ".codex", MissingPathPolicy::CreateDir)
            .expect("prepare path");
        let expected = Some(root.join(".codex"));

        assert_eq!(expected, prepared);
        assert!(root.join(".codex").is_dir());
    }

    #[test]
    fn skips_missing_agents_dir_when_not_reserved() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace root");

        let prepared = prepare_root_subdir(&root, ".agents", MissingPathPolicy::ExistingOnly)
            .expect("prepare path");

        assert_eq!(None, prepared);
        assert!(!root.join(".agents").exists());
    }

    #[test]
    fn preserves_existing_protected_path_without_recreating_it() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("workspace");
        let codex_dir = root.join(".codex");
        std::fs::create_dir_all(&codex_dir).expect("codex dir");

        let prepared = prepare_root_subdir(&root, ".codex", MissingPathPolicy::CreateDir)
            .expect("prepare path");

        assert_eq!(Some(codex_dir), prepared);
    }
}
