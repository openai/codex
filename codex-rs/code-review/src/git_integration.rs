//! Git integration for automatic commits

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::fixer::FixReport;
use crate::session::ImprovementRecord;

/// Git integration manager
pub struct GitIntegration {
    repo_path: PathBuf,
    auto_commit: bool,
    commit_prefix: String,
}

impl GitIntegration {
    /// Create a new Git integration
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            auto_commit: false,
            commit_prefix: "[codex-review]".to_string(),
        }
    }

    /// Enable auto-commit
    pub fn with_auto_commit(mut self, enabled: bool) -> Self {
        self.auto_commit = enabled;
        self
    }

    /// Set commit message prefix
    pub fn with_commit_prefix(mut self, prefix: String) -> Self {
        self.commit_prefix = prefix;
        self
    }

    /// Check if current directory is a git repository
    pub async fn is_git_repo(&self) -> bool {
        Command::new("git")
            .arg("rev-parse")
            .arg("--git-dir")
            .current_dir(&self.repo_path)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get current branch name
    pub async fn current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get current branch")?;

        if !output.status.success() {
            anyhow::bail!("Not in a git repository");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Create a backup branch
    pub async fn create_backup_branch(&self, base_name: &str) -> Result<String> {
        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let branch_name = format!("{}-backup-{}", base_name, timestamp);

        info!("Creating backup branch: {}", branch_name);

        let status = Command::new("git")
            .arg("branch")
            .arg(&branch_name)
            .current_dir(&self.repo_path)
            .status()
            .await
            .context("Failed to create backup branch")?;

        if !status.success() {
            anyhow::bail!("Failed to create backup branch");
        }

        Ok(branch_name)
    }

    /// Stage a file
    pub async fn stage_file(&self, file_path: &Path) -> Result<()> {
        debug!("Staging file: {:?}", file_path);

        let status = Command::new("git")
            .arg("add")
            .arg(file_path)
            .current_dir(&self.repo_path)
            .status()
            .await
            .context("Failed to stage file")?;

        if !status.success() {
            anyhow::bail!("Git add failed for {:?}", file_path);
        }

        Ok(())
    }

    /// Commit staged changes
    pub async fn commit(&self, message: &str) -> Result<String> {
        info!("Committing changes: {}", message);

        let full_message = format!("{} {}", self.commit_prefix, message);

        let output = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg(&full_message)
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git commit failed: {}", stderr);
        }

        // Get commit hash
        let hash_output = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(&self.repo_path)
            .output()
            .await?;

        let commit_hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();

        Ok(commit_hash)
    }

    /// Commit a fix report
    pub async fn commit_fix(&self, report: &FixReport) -> Result<String> {
        if !self.auto_commit {
            debug!("Auto-commit disabled, skipping");
            return Ok(String::new());
        }

        if !report.changed {
            debug!("No changes to commit");
            return Ok(String::new());
        }

        // Stage the file
        self.stage_file(&report.file_path).await?;

        // Create commit message
        let message = format!(
            "fix: {} in {:?}

Applied {} style fixes and {} suggestions

{}",
            if report.style_fixes_applied > 0 {
                "Auto-fix style issues"
            } else {
                "Apply improvements"
            },
            report.file_path.file_name().unwrap_or_default(),
            report.style_fixes_applied,
            report.suggestions_applied.len(),
            report.suggestions_applied
                .iter()
                .map(|s| format!("- {}", s.description))
                .collect::<Vec<_>>()
                .join("\n")
        );

        self.commit(&message).await
    }

    /// Commit an improvement record
    pub async fn commit_improvement(&self, improvement: &ImprovementRecord) -> Result<String> {
        if !self.auto_commit {
            return Ok(String::new());
        }

        self.stage_file(&improvement.file_path).await?;

        let message = format!(
            "{}: {}

{}

Confidence: {:.1}%",
            improvement.improvement_type,
            improvement.description,
            improvement.file_path.display(),
            improvement.confidence * 100.0
        );

        self.commit(&message).await
    }

    /// Get repository status
    pub async fn status(&self) -> Result<GitStatus> {
        let output = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get git status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut status = GitStatus::default();

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }

            let status_code = &line[0..2];
            match status_code.trim() {
                "M" => status.modified += 1,
                "A" => status.added += 1,
                "D" => status.deleted += 1,
                "?" | "??" => status.untracked += 1,
                _ => {}
            }
        }

        Ok(status)
    }

    /// Check if there are uncommitted changes
    pub async fn has_changes(&self) -> Result<bool> {
        let status = self.status().await?;
        Ok(status.has_changes())
    }

    /// Get recent commits
    pub async fn recent_commits(&self, count: usize) -> Result<Vec<CommitInfo>> {
        let output = Command::new("git")
            .arg("log")
            .arg(format!("-{}", count))
            .arg("--pretty=format:%H%n%s%n%an%n%ae%n%ad")
            .arg("--date=iso")
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get git log")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut commits = Vec::new();
        let lines: Vec<&str> = stdout.lines().collect();

        for chunk in lines.chunks(5) {
            if chunk.len() >= 5 {
                commits.push(CommitInfo {
                    hash: chunk[0].to_string(),
                    message: chunk[1].to_string(),
                    author: chunk[2].to_string(),
                    email: chunk[3].to_string(),
                    date: chunk[4].to_string(),
                });
            }
        }

        Ok(commits)
    }
}

/// Git repository status
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    pub modified: usize,
    pub added: usize,
    pub deleted: usize,
    pub untracked: usize,
}

impl GitStatus {
    pub fn has_changes(&self) -> bool {
        self.modified > 0 || self.added > 0 || self.deleted > 0
    }

    pub fn total_changes(&self) -> usize {
        self.modified + self.added + self.deleted + self.untracked
    }
}

/// Commit information
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub date: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_git_repo() -> Result<TempDir> {
        let temp = TempDir::new()?;

        Command::new("git")
            .arg("init")
            .current_dir(temp.path())
            .status()
            .await?;

        Command::new("git")
            .arg("config")
            .arg("user.name")
            .arg("Test User")
            .current_dir(temp.path())
            .status()
            .await?;

        Command::new("git")
            .arg("config")
            .arg("user.email")
            .arg("test@example.com")
            .current_dir(temp.path())
            .status()
            .await?;

        Ok(temp)
    }

    #[tokio::test]
    async fn test_is_git_repo() {
        let temp = setup_git_repo().await.unwrap();
        let git = GitIntegration::new(temp.path().to_path_buf());
        assert!(git.is_git_repo().await);
    }

    #[tokio::test]
    async fn test_current_branch() {
        let temp = setup_git_repo().await.unwrap();
        let git = GitIntegration::new(temp.path().to_path_buf());
        let branch = git.current_branch().await.unwrap();
        assert!(!branch.is_empty());
    }

    #[tokio::test]
    async fn test_status() {
        let temp = setup_git_repo().await.unwrap();
        let git = GitIntegration::new(temp.path().to_path_buf());

        // Create a file
        tokio::fs::write(temp.path().join("test.txt"), "content").await.unwrap();

        let status = git.status().await.unwrap();
        assert!(status.untracked > 0 || status.modified > 0);
    }
}
