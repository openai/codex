//! Path and identifier helpers for managed worktrees.
//!
//! These helpers centralize the naming rules that must stay consistent across CLI, TUI, app-server,
//! and tests. They do not touch the filesystem except when canonicalizing paths for containment
//! checks; creation and removal remain in the manager module.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use sha2::Digest;

/// Returns the Codex-home root used for app-style managed worktrees.
///
/// This path is a discovery root, not proof of ownership. Callers must still read metadata before
/// removing or binding a worktree under it.
pub fn codex_worktrees_root(codex_home: &Path) -> PathBuf {
    codex_home.join("worktrees")
}

/// Returns true when a path is under the Codex-home managed worktree root.
///
/// The check canonicalizes both sides when possible so symlinks and macOS private-var aliases do not
/// make an owned worktree look external. A true result should be combined with metadata checks
/// before destructive operations.
pub fn is_managed_worktree_path(path: &Path, codex_home: &Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = codex_worktrees_root(codex_home)
        .canonicalize()
        .unwrap_or_else(|_| codex_worktrees_root(codex_home));
    path.starts_with(root)
}

/// Produces a short ASCII slug for display, search, and metadata.
///
/// Slugs are intentionally lossy and must not be used as the only identifier for deletion when
/// multiple worktrees can share a name-like prefix. An empty result is rejected because it would
/// make picker rows and metadata ambiguous.
pub fn slugify_name(name: &str) -> Result<String> {
    let slug = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(12)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        anyhow::bail!("worktree name must contain at least one ASCII letter or digit");
    }
    Ok(slug)
}

/// Converts a branch name into a single safe sibling-directory segment.
///
/// This preserves most characters but replaces path separators so branch names cannot escape the
/// sibling worktree directory. Git branch validity is checked separately by the manager before
/// creation.
pub fn sanitize_branch_for_path(branch: &str) -> Result<String> {
    let sanitized = branch.replace(['/', '\\'], "-");
    if sanitized.trim().is_empty() {
        anyhow::bail!("branch name must produce a non-empty worktree path segment");
    }
    Ok(sanitized)
}

/// Builds a stable repository fingerprint from Git identity inputs.
///
/// The common Git directory keeps linked worktrees for the same repository grouped together, while
/// the optional origin URL reduces collisions when paths are similar across different clones.
pub fn repo_fingerprint(common_git_dir: &Path, origin_url: Option<&str>) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(common_git_dir.to_string_lossy().as_bytes());
    if let Some(origin_url) = origin_url {
        hasher.update(b"\0");
        hasher.update(origin_url.as_bytes());
    }
    let digest = hasher.finalize();
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Returns the sibling Git worktree root for a branch.
///
/// The path follows the Worktrunk-style repo.branch naming convention. Callers should pass the
/// primary repository root, not an existing sibling worktree root, or the new path will be anchored
/// beside the wrong checkout.
pub fn sibling_worktree_git_root(repo_root: &Path, branch: &str) -> Result<PathBuf> {
    let repo_name = repo_root
        .file_name()
        .context("source repository root has no directory name")?;
    let parent = repo_root
        .parent()
        .context("source repository root has no parent directory")?;
    let sanitized_branch = sanitize_branch_for_path(branch)?;
    let dirname = format!("{}.{}", repo_name.to_string_lossy(), sanitized_branch);
    Ok(parent.join(dirname))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn slugify_name_keeps_short_ascii_slug() -> Result<()> {
        assert_eq!(slugify_name("Fix parser tests!")?, "fix-parser-tests");
        Ok(())
    }

    #[test]
    fn sanitize_branch_for_path_matches_worktrunk_style() -> Result<()> {
        assert_eq!(
            sanitize_branch_for_path("feature/auth\\windows")?,
            "feature-auth-windows"
        );
        Ok(())
    }

    #[test]
    fn sibling_worktree_path_matches_worktrunk_default() -> Result<()> {
        assert_eq!(
            sibling_worktree_git_root(Path::new("/Users/me/code/codex"), "feature/auth")?,
            PathBuf::from("/Users/me/code/codex.feature-auth")
        );
        Ok(())
    }
}
