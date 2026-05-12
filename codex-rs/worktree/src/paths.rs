use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use sha2::Digest;

pub fn codex_worktrees_root(codex_home: &Path) -> PathBuf {
    codex_home.join("worktrees")
}

pub fn is_managed_worktree_path(path: &Path, codex_home: &Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = codex_worktrees_root(codex_home)
        .canonicalize()
        .unwrap_or_else(|_| codex_worktrees_root(codex_home));
    path.starts_with(root)
}

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

pub fn sanitize_branch_for_path(branch: &str) -> Result<String> {
    let sanitized = branch.replace(['/', '\\'], "-");
    if sanitized.trim().is_empty() {
        anyhow::bail!("branch name must produce a non-empty worktree path segment");
    }
    Ok(sanitized)
}

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
