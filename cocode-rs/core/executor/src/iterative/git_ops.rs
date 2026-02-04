//! Git operations for iterative executor context passing.
//!
//! This module provides async wrappers around `cocode-git` synchronous operations.

use std::path::Path;
use std::path::PathBuf;

use crate::error::Result;
use crate::error::executor_error::*;

/// Get current HEAD commit ID.
///
/// Wraps `cocode_git::get_head_commit` in async.
pub async fn get_head_commit(cwd: &Path) -> Result<String> {
    let cwd = cwd.to_path_buf();
    tokio::task::spawn_blocking(move || cocode_git::get_head_commit(&cwd))
        .await
        .map_err(|e| {
            TaskSpawnSnafu {
                message: format!("Failed to spawn git task: {e}"),
            }
            .build()
        })?
        .map_err(|e| {
            GitSnafu {
                message: format!("Failed to get HEAD commit: {e}"),
            }
            .build()
        })
}

/// Get uncommitted changes.
///
/// Wraps `cocode_git::get_uncommitted_changes` in async.
pub async fn get_uncommitted_changes(cwd: &Path) -> Result<Vec<String>> {
    let cwd = cwd.to_path_buf();
    tokio::task::spawn_blocking(move || cocode_git::get_uncommitted_changes(&cwd))
        .await
        .map_err(|e| {
            TaskSpawnSnafu {
                message: format!("Failed to spawn git task: {e}"),
            }
            .build()
        })?
        .map_err(|e| {
            GitSnafu {
                message: format!("Failed to get uncommitted changes: {e}"),
            }
            .build()
        })
}

/// Commit if there are changes.
///
/// Wraps `cocode_git::commit_all` in async.
/// Returns commit ID or None if no changes.
pub async fn commit_if_needed(cwd: &Path, message: &str) -> Result<Option<String>> {
    let cwd = cwd.to_path_buf();
    let message = message.to_string();
    tokio::task::spawn_blocking(move || cocode_git::commit_all(&cwd, &message))
        .await
        .map_err(|e| {
            TaskSpawnSnafu {
                message: format!("Failed to spawn git task: {e}"),
            }
            .build()
        })?
        .map_err(|e| {
            GitSnafu {
                message: format!("Failed to commit changes: {e}"),
            }
            .build()
        })
}

/// Read plan file if exists.
///
/// Reads the most recently modified .md file from `.cocode/plans/` directory.
pub fn read_plan_file_if_exists(cwd: &Path) -> Option<String> {
    let plans_dir = cwd.join(".cocode").join("plans");
    if !plans_dir.exists() {
        return None;
    }

    let entries = match std::fs::read_dir(&plans_dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    let mut latest_file: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    match &latest_file {
                        None => latest_file = Some((path, modified)),
                        Some((_, prev_time)) if modified > *prev_time => {
                            latest_file = Some((path, modified));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    latest_file.and_then(|(path, _)| std::fs::read_to_string(&path).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_get_uncommitted_changes_empty() {
        let temp = TempDir::new().unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let changes = get_uncommitted_changes(temp.path()).await.unwrap();
        assert!(changes.is_empty());
    }

    #[tokio::test]
    async fn test_get_uncommitted_changes_with_file() {
        let temp = TempDir::new().unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Create a file
        std::fs::write(temp.path().join("test.txt"), "content").unwrap();

        let changes = get_uncommitted_changes(temp.path()).await.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("test.txt"));
    }

    #[tokio::test]
    async fn test_get_head_commit() {
        let temp = TempDir::new().unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Configure git user for commit
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(temp.path().join("test.txt"), "content").unwrap();
        StdCommand::new("git")
            .args(["add", "-A"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let commit = get_head_commit(temp.path()).await.unwrap();
        assert!(!commit.is_empty());
        assert!(commit.len() >= 7); // Git SHA is at least 7 chars
    }

    #[test]
    fn test_read_plan_file_no_dir() {
        let temp = TempDir::new().unwrap();
        let result = read_plan_file_if_exists(temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_read_plan_file_with_plan() {
        let temp = TempDir::new().unwrap();
        let plans_dir = temp.path().join(".cocode").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        std::fs::write(plans_dir.join("test-plan.md"), "# Plan\n1. Do X").unwrap();

        let result = read_plan_file_if_exists(temp.path());
        assert!(result.is_some());
        assert!(result.unwrap().contains("# Plan"));
    }

    #[tokio::test]
    async fn test_commit_if_needed_no_changes() {
        let temp = TempDir::new().unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let result = commit_if_needed(temp.path(), "test commit").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_commit_if_needed_with_changes() {
        let temp = TempDir::new().unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Configure git user for commit
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Create a file
        std::fs::write(temp.path().join("test.txt"), "content").unwrap();

        let result = commit_if_needed(temp.path(), "[iter-0] Test commit")
            .await
            .unwrap();
        assert!(result.is_some());
        let commit_id = result.unwrap();
        assert!(!commit_id.is_empty());
    }
}
