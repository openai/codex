use crate::acl::add_deny_write_ace;
use anyhow::Result;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn is_command_cwd_root(root: &Path, canonical_command_cwd: &Path) -> bool {
    canonicalize_path(root) == canonical_command_cwd
}

/// # Safety
/// Caller must ensure `psid` is a valid SID pointer.
pub unsafe fn protect_workspace_codex_dir(cwd: &Path, psid: *mut c_void) -> Result<bool> {
    let cwd_codex = cwd.join(".codex");
    if cwd_codex.is_dir() {
        add_deny_write_ace(&cwd_codex, psid)
    } else {
        Ok(false)
    }
}
