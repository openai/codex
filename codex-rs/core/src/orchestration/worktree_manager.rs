//! Git worktree manager for competition-style development
//! 
//! Creates isolated worktrees for each AI agent to work independently,
//! then merges the best result back to the main branch.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub agent: String,
}

pub struct WorktreeManager {
    repo_path: PathBuf,
    worktree_base: PathBuf,
}

impl WorktreeManager {
    pub fn new(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();
        let worktree_base = repo_path.join(".codex-worktrees");

        // Create worktree base directory if it doesn't exist
        if !worktree_base.exists() {
            std::fs::create_dir_all(&worktree_base)
                .context("Failed to create worktree base directory")?;
        }

        Ok(Self {
            repo_path,
            worktree_base,
        })
    }

    /// Create a new worktree for an agent
    pub fn create_worktree(&self, agent_name: &str, task_id: &str) -> Result<WorktreeInfo> {
        let branch_name = format!("codex/{}/{}", agent_name, task_id);
        let worktree_name = format!("{}_{}", agent_name, task_id);
        let worktree_path = self.worktree_base.join(&worktree_name);

        // Remove existing worktree if present
        if worktree_path.exists() {
            self.remove_worktree(&worktree_name)?;
        }

        // Create new branch
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["branch", &branch_name])
            .output()
            .context("Failed to create branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "already exists" error
            if !stderr.contains("already exists") {
                anyhow::bail!("Git branch creation failed: {}", stderr);
            }
        }

        // Create worktree
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                &branch_name,
            ])
            .output()
            .context("Failed to create worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git worktree creation failed: {}", stderr);
        }

        Ok(WorktreeInfo {
            name: worktree_name,
            path: worktree_path,
            branch: branch_name,
            agent: agent_name.to_string(),
        })
    }

    /// Remove a worktree and its branch
    pub fn remove_worktree(&self, worktree_name: &str) -> Result<()> {
        let worktree_path = self.worktree_base.join(worktree_name);

        if worktree_path.exists() {
            let output = Command::new("git")
                .current_dir(&self.repo_path)
                .args(["worktree", "remove", worktree_path.to_str().unwrap(), "--force"])
                .output()
                .context("Failed to remove worktree")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Warning: Failed to remove worktree: {}", stderr);
            }
        }

        Ok(())
    }

    /// List all active worktrees
    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .context("Failed to list worktrees")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git worktree list failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();

        for chunk in stdout.split("\n\n") {
            let lines: Vec<&str> = chunk.lines().collect();
            if lines.len() < 2 {
                continue;
            }

            let path = lines[0].trim_start_matches("worktree ");
            let branch = lines[1].trim_start_matches("branch ");

            // Only include our managed worktrees
            if let Some(name) = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
            {
                if path.contains(".codex-worktrees") {
                    let agent = name.split('_').next().unwrap_or("unknown").to_string();

                    worktrees.push(WorktreeInfo {
                        name: name.to_string(),
                        path: PathBuf::from(path),
                        branch: branch.to_string(),
                        agent,
                    });
                }
            }
        }

        Ok(worktrees)
    }

    /// Merge a worktree branch back to main
    pub fn merge_worktree(&self, worktree_info: &WorktreeInfo, target_branch: &str) -> Result<()> {
        // Switch to target branch
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["checkout", target_branch])
            .output()
            .context("Failed to checkout target branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git checkout failed: {}", stderr);
        }

        // Merge worktree branch
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["merge", &worktree_info.branch, "--no-ff"])
            .output()
            .context("Failed to merge worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git merge failed: {}", stderr);
        }

        Ok(())
    }

    /// Clean up all managed worktrees
    pub fn cleanup_all(&self) -> Result<()> {
        let worktrees = self.list_worktrees()?;

        for worktree in worktrees {
            self.remove_worktree(&worktree.name)?;
        }

        Ok(())
    }
}

