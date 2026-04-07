use crate::windows_sandbox::run_setup_refresh_with_extra_read_roots;
use anyhow::Result;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub fn grant_read_root_non_elevated(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_root: &Path,
) -> Result<PathBuf> {
    if !read_root.is_absolute() {
        anyhow::bail!("path must be absolute: {}", read_root.display());
    }
    if !read_root.exists() {
        anyhow::bail!("path does not exist: {}", read_root.display());
    }
    if !read_root.is_dir() {
        anyhow::bail!("path must be a directory: {}", read_root.display());
    }

    let canonical_root = dunce::canonicalize(read_root)?;
    run_setup_refresh_with_extra_read_roots(
        policy,
        policy_cwd,
        command_cwd,
        env_map,
        codex_home,
        vec![canonical_root.clone()],
    )?;
    Ok(canonical_root)
}

#[cfg(test)]
#[path = "windows_sandbox_read_grants_tests.rs"]
mod tests;
