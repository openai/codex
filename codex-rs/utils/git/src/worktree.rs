/// Git worktree management for concurrent editing
///
/// Enables multiple Codex instances to work on different branches simultaneously
/// using Git worktrees. Each instance gets its own working directory linked to
/// a unique branch, avoiding file conflicts.
use crate::errors::GitToolingError;
use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Git worktree information
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Path to the worktree directory
    pub path: PathBuf,
    /// Branch name (e.g., "codex-agent-123")
    pub branch: String,
    /// Base commit SHA
    pub base_commit: String,
}

/// Worktree configuration
#[derive(Debug, Clone)]
pub struct WorktreeConfig {
    /// Repository root path
    pub repo_root: PathBuf,
    /// Worktree prefix (e.g., "codex-agent")
    pub worktree_prefix: String,
    /// Instance ID (e.g., agent ID, session ID)
    pub instance_id: String,
}

impl Worktree {
    /// Create a new worktree for concurrent editing
    ///
    /// Creates a new Git worktree at `.codex/worktrees/{prefix}-{instance_id}`
    /// with a new branch `{prefix}/{instance_id}` based on the current HEAD.
    pub fn create(config: WorktreeConfig) -> Result<Self> {
        let repo_root = &config.repo_root;

        // Get current HEAD commit
        let base_commit = get_current_commit(repo_root)?;

        // Create worktree path
        let worktree_dir = repo_root.join(".codex").join("worktrees");
        std::fs::create_dir_all(&worktree_dir).context("Failed to create worktrees directory")?;

        let worktree_name = format!("{}-{}", config.worktree_prefix, config.instance_id);
        let worktree_path = worktree_dir.join(&worktree_name);

        // Create branch name
        let branch = format!("{}/{}", config.worktree_prefix, config.instance_id);

        // Check if worktree already exists
        if worktree_path.exists() {
            // Try to remove it first
            Self::remove_internal(&worktree_path, repo_root)?;
        }

        // Create new worktree with new branch
        let output = Command::new("git")
            .arg("worktree")
            .arg("add")
            .arg("-b")
            .arg(&branch)
            .arg(&worktree_path)
            .arg(&base_commit)
            .current_dir(repo_root)
            .output()
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            return Err(GitToolingError::GitCommandFailed {
                command: format!(
                    "git worktree add -b {} {} {}",
                    branch,
                    worktree_path.display(),
                    base_commit
                ),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            }
            .into());
        }

        Ok(Self {
            path: worktree_path,
            branch,
            base_commit,
        })
    }

    /// List all worktrees in the repository
    pub fn list(repo_root: &Path) -> Result<Vec<WorktreeInfo>> {
        let output = Command::new("git")
            .arg("worktree")
            .arg("list")
            .arg("--porcelain")
            .current_dir(repo_root)
            .output()
            .context("Failed to execute git worktree list")?;

        if !output.status.success() {
            return Err(GitToolingError::GitCommandFailed {
                command: "git worktree list --porcelain".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            }
            .into());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        parse_worktree_list(&output_str)
    }

    /// Remove this worktree
    pub fn remove(self, repo_root: &Path) -> Result<()> {
        Self::remove_internal(&self.path, repo_root)
    }

    /// Internal worktree removal (can be called on non-existent worktrees)
    fn remove_internal(worktree_path: &Path, repo_root: &Path) -> Result<()> {
        // Remove worktree
        let output = Command::new("git")
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(worktree_path)
            .current_dir(repo_root)
            .output()
            .context("Failed to execute git worktree remove")?;

        if !output.status.success() {
            // If worktree doesn't exist, that's fine
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("is not a working tree") {
                return Err(GitToolingError::GitCommandFailed {
                    command: format!("git worktree remove --force {}", worktree_path.display()),
                    stderr: stderr.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Commit changes in this worktree
    pub fn commit(&self, message: &str) -> Result<String> {
        // Stage all changes
        let output = Command::new("git")
            .arg("add")
            .arg(".")
            .current_dir(&self.path)
            .output()
            .context("Failed to execute git add")?;

        if !output.status.success() {
            return Err(GitToolingError::GitCommandFailed {
                command: "git add .".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            }
            .into());
        }

        // Commit
        let output = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg(message)
            .current_dir(&self.path)
            .output()
            .context("Failed to execute git commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // "nothing to commit" is not an error
            if !stderr.contains("nothing to commit") {
                return Err(GitToolingError::GitCommandFailed {
                    command: format!("git commit -m '{message}'"),
                    stderr: stderr.to_string(),
                }
                .into());
            }
        }

        // Get commit SHA
        get_current_commit(&self.path)
    }

    /// Get diff of this worktree relative to base commit
    pub fn diff(&self) -> Result<String> {
        let output = Command::new("git")
            .arg("diff")
            .arg(&self.base_commit)
            .current_dir(&self.path)
            .output()
            .context("Failed to execute git diff")?;

        if !output.status.success() {
            return Err(GitToolingError::GitCommandFailed {
                command: format!("git diff {}", self.base_commit),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            }
            .into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Merge changes from another branch into this worktree
    pub fn merge(&self, branch: &str) -> Result<MergeResult> {
        let output = Command::new("git")
            .arg("merge")
            .arg(branch)
            .arg("--no-ff")
            .arg("--no-edit")
            .current_dir(&self.path)
            .output()
            .context("Failed to execute git merge")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            // Check for merge conflict
            if stderr.contains("CONFLICT") || stdout.contains("CONFLICT") {
                return Ok(MergeResult::Conflict {
                    conflicts: parse_merge_conflicts(&self.path)?,
                });
            }

            return Err(GitToolingError::GitCommandFailed {
                command: format!("git merge {branch} --no-ff --no-edit"),
                stderr: stderr.to_string(),
            }
            .into());
        }

        Ok(MergeResult::Success {
            commit: get_current_commit(&self.path)?,
        })
    }

    /// Abort a merge in progress
    pub fn abort_merge(&self) -> Result<()> {
        let output = Command::new("git")
            .arg("merge")
            .arg("--abort")
            .current_dir(&self.path)
            .output()
            .context("Failed to execute git merge --abort")?;

        if !output.status.success() {
            return Err(GitToolingError::GitCommandFailed {
                command: "git merge --abort".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            }
            .into());
        }

        Ok(())
    }
}

/// Worktree information from `git worktree list`
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub commit: String,
    pub branch: Option<String>,
}

/// Merge result
#[derive(Debug, Clone)]
pub enum MergeResult {
    Success { commit: String },
    Conflict { conflicts: Vec<PathBuf> },
}

/// Get current commit SHA
fn get_current_commit(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git rev-parse HEAD")?;

    if !output.status.success() {
        return Err(GitToolingError::GitCommandFailed {
            command: "git rev-parse HEAD".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
        .into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Parse `git worktree list --porcelain` output
fn parse_worktree_list(output: &str) -> Result<Vec<WorktreeInfo>> {
    let mut worktrees = Vec::new();
    let mut current_worktree: Option<(PathBuf, String, Option<String>)> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree
            if let Some((path, commit, branch)) = current_worktree.take() {
                worktrees.push(WorktreeInfo {
                    path,
                    commit,
                    branch,
                });
            }

            // Start new worktree
            if let Some(p) = line.strip_prefix("worktree ") {
                let path = PathBuf::from(p);
                current_worktree = Some((path, String::new(), None));
            }
        } else if let Some(c) = line.strip_prefix("HEAD ")
            && let Some((_, ref mut commit, _)) = current_worktree
        {
            *commit = c.to_string();
        } else if let Some(b) = line.strip_prefix("branch ")
            && let Some((_, _, ref mut branch)) = current_worktree
        {
            *branch = Some(b.to_string());
        }
    }

    // Save last worktree
    if let Some((path, commit, branch)) = current_worktree {
        worktrees.push(WorktreeInfo {
            path,
            commit,
            branch,
        });
    }

    Ok(worktrees)
}

/// Parse merge conflicts
fn parse_merge_conflicts(repo_path: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg("--diff-filter=U")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git diff --name-only --diff-filter=U")?;

    if !output.status.success() {
        return Err(GitToolingError::GitCommandFailed {
            command: "git diff --name-only --diff-filter=U".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
        .into());
    }

    let conflicts = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| PathBuf::from(line.trim()))
        .collect();

    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let output = r#"worktree /repo
HEAD abc123
branch refs/heads/main

worktree /repo/.codex/worktrees/codex-agent-xyz
HEAD def456
branch refs/heads/codex-agent/xyz

"#;

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, PathBuf::from("/repo"));
        assert_eq!(worktrees[0].commit, "abc123");
        assert_eq!(worktrees[0].branch.as_deref(), Some("refs/heads/main"));
        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/repo/.codex/worktrees/codex-agent-xyz")
        );
        assert_eq!(worktrees[1].commit, "def456");
        assert_eq!(
            worktrees[1].branch.as_deref(),
            Some("refs/heads/codex-agent/xyz")
        );
    }
}
