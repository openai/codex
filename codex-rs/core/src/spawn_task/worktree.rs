use std::path::PathBuf;

use anyhow::Result;
use tokio::process::Command;
use tracing::info;
use tracing::warn;

/// Information about a created worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Task ID this worktree belongs to.
    pub task_id: String,
    /// Path to the worktree directory.
    pub worktree_path: PathBuf,
    /// Git branch name.
    pub branch_name: String,
    /// Base branch it was created from.
    pub base_branch: String,
}

/// Manager for git worktrees used by spawn tasks.
///
/// GENERIC: This manager handles worktrees for ALL spawn task types
/// (SpawnAgent, SpawnWorkflow, future types).
pub struct WorktreeManager {
    /// Path to ~/.codex/
    codex_home: PathBuf,
    /// Path to the original project.
    project_root: PathBuf,
}

impl WorktreeManager {
    pub fn new(codex_home: PathBuf, project_root: PathBuf) -> Self {
        Self {
            codex_home,
            project_root,
        }
    }

    /// Create a git worktree for a spawn task.
    ///
    /// Creates a new branch with the task_id as name and a worktree
    /// at ~/.codex/spawn-tasks/worktrees/{task_id}/
    ///
    /// Works for ANY spawn task type (Agent, Workflow, etc.)
    pub async fn create_worktree(
        &self,
        task_id: &str,
        base_branch: Option<&str>,
    ) -> Result<WorktreeInfo> {
        let worktree_path = self.worktrees_dir().join(task_id);
        let branch_name = task_id.to_string();

        // Check if worktree already exists
        if worktree_path.exists() {
            anyhow::bail!("Worktree for task '{}' already exists", task_id);
        }

        // Create worktrees directory if needed
        tokio::fs::create_dir_all(self.worktrees_dir()).await?;

        // Detect base branch if not specified (async)
        let base = match base_branch {
            Some(b) => b.to_string(),
            None => self.detect_default_branch().await,
        };

        info!(
            task_id = %task_id,
            base_branch = %base,
            worktree_path = %worktree_path.display(),
            "Creating git worktree"
        );

        // Create worktree with new branch (async to avoid blocking runtime)
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap_or_default(),
                "-b",
                &branch_name,
                &base,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create worktree: {}", stderr);
        }

        Ok(WorktreeInfo {
            task_id: task_id.to_string(),
            worktree_path,
            branch_name,
            base_branch: base,
        })
    }

    /// Remove a git worktree and its branch.
    ///
    /// Works for ANY spawn task type (Agent, Workflow, etc.)
    pub async fn cleanup_worktree(&self, task_id: &str) -> Result<()> {
        let worktree_path = self.worktrees_dir().join(task_id);

        info!(task_id = %task_id, "Removing git worktree");

        // Remove worktree (async)
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args([
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap_or_default(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            warn!(
                task_id = %task_id,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "Failed to remove worktree (may already be removed)"
            );
        }

        // Delete the branch (async)
        let _ = Command::new("git")
            .current_dir(&self.project_root)
            .args(["branch", "-D", task_id])
            .output()
            .await;

        Ok(())
    }

    /// Get the worktrees directory.
    pub fn worktrees_dir(&self) -> PathBuf {
        self.codex_home.join("spawn-tasks").join("worktrees")
    }

    /// Detect the default branch (async to avoid blocking runtime).
    async fn detect_default_branch(&self) -> String {
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                if let Some(branch) = s.trim().strip_prefix("refs/remotes/origin/") {
                    return branch.to_string();
                }
            }
        }

        // Fall back to trying 'main', then 'master'
        if self.branch_exists("main").await {
            return "main".to_string();
        }
        if self.branch_exists("master").await {
            return "master".to_string();
        }

        // Default to HEAD
        "HEAD".to_string()
    }

    /// Check if a branch exists.
    async fn branch_exists(&self, branch: &str) -> bool {
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args(["rev-parse", "--verify", branch])
            .output()
            .await;

        matches!(output, Ok(o) if o.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktrees_dir_path() {
        let manager = WorktreeManager::new(
            PathBuf::from("/home/user/.codex"),
            PathBuf::from("/home/user/project"),
        );

        assert_eq!(
            manager.worktrees_dir(),
            PathBuf::from("/home/user/.codex/spawn-tasks/worktrees")
        );
    }
}
