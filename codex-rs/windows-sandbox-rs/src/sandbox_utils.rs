//! Shared helper utilities for Windows sandbox setup.
//!
//! These helpers centralize small pieces of setup logic used across both legacy and
//! elevated paths, including unified_exec sessions and capture flows. They cover
//! codex home directory creation and git safe.directory injection so sandboxed
//! users can run git inside a repo owned by the primary user.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Walk upward from `start` to locate the git worktree root (supports gitfile redirects).
fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = dunce::canonicalize(start).ok()?;
    loop {
        let marker = cur.join(".git");
        if marker.is_dir() {
            return Some(cur);
        }
        if marker.is_file() {
            if let Ok(txt) = std::fs::read_to_string(&marker)
                && let Some(rest) = txt.trim().strip_prefix("gitdir:")
            {
                let gitdir = rest.trim();
                let resolved = if Path::new(gitdir).is_absolute() {
                    PathBuf::from(gitdir)
                } else {
                    cur.join(gitdir)
                };
                return resolved.parent().map(Path::to_path_buf).or(Some(cur));
            }
            return Some(cur);
        }
        let parent = cur.parent()?;
        if parent == cur {
            return None;
        }
        cur = parent.to_path_buf();
    }
}

/// Ensure the sandbox codex home directory exists.
pub fn ensure_codex_home_exists(p: &Path) -> Result<()> {
    std::fs::create_dir_all(p)?;
    Ok(())
}

/// Adds a git safe.directory entry to the environment when running inside a repository.
/// git will not otherwise allow the Sandbox user to run git commands on the repo directory
/// which is owned by the primary user.
pub fn inject_git_safe_directory(env_map: &mut HashMap<String, String>, cwd: &Path) {
    if let Some(git_root) = find_git_root(cwd) {
        let mut cfg_count: usize = env_map
            .get("GIT_CONFIG_COUNT")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let git_path = git_root.to_string_lossy().replace("\\\\", "/");
        env_map.insert(
            format!("GIT_CONFIG_KEY_{cfg_count}"),
            "safe.directory".to_string(),
        );
        env_map.insert(format!("GIT_CONFIG_VALUE_{cfg_count}"), git_path);
        cfg_count += 1;
        env_map.insert("GIT_CONFIG_COUNT".to_string(), cfg_count.to_string());
    }
}
