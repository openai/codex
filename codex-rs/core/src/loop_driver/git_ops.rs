//! Git operations for loop driver context passing.
//!
//! This module provides async wrappers around `codex-git` synchronous operations.

use crate::error::CodexErr;
use std::path::Path;
use std::path::PathBuf;

/// Convert GitToolingError to CodexErr.
fn git_err_to_codex(e: codex_git::GitToolingError) -> CodexErr {
    CodexErr::Fatal(format!("Git operation failed: {e}"))
}

/// Get current HEAD commit ID.
///
/// Wraps `codex_git::get_head_commit` in async.
pub async fn get_head_commit(cwd: &Path) -> Result<String, CodexErr> {
    let cwd = cwd.to_path_buf();
    tokio::task::spawn_blocking(move || codex_git::get_head_commit(&cwd))
        .await
        .map_err(|e| CodexErr::Fatal(format!("Failed to spawn git task: {e}")))?
        .map_err(git_err_to_codex)
}

/// Get uncommitted changes.
///
/// Wraps `codex_git::get_uncommitted_changes` in async.
pub async fn get_uncommitted_changes(cwd: &Path) -> Result<Vec<String>, CodexErr> {
    let cwd = cwd.to_path_buf();
    tokio::task::spawn_blocking(move || codex_git::get_uncommitted_changes(&cwd))
        .await
        .map_err(|e| CodexErr::Fatal(format!("Failed to spawn git task: {e}")))?
        .map_err(git_err_to_codex)
}

/// Commit if there are changes.
///
/// Wraps `codex_git::commit_all` in async.
/// Returns commit ID or None if no changes.
pub async fn commit_if_needed(cwd: &Path, message: &str) -> Result<Option<String>, CodexErr> {
    let cwd = cwd.to_path_buf();
    let message = message.to_string();
    tokio::task::spawn_blocking(move || codex_git::commit_all(&cwd, &message))
        .await
        .map_err(|e| CodexErr::Fatal(format!("Failed to spawn git task: {e}")))?
        .map_err(git_err_to_codex)
}

/// Read plan file if exists.
///
/// This is not a git operation, but kept here for convenience.
/// Reads the most recently modified .md file from `.codex/plans/`.
pub fn read_plan_file_if_exists(cwd: &Path) -> Option<String> {
    let plans_dir = cwd.join(".codex").join("plans");
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

    #[test]
    fn test_read_plan_file_no_dir() {
        let temp = TempDir::new().unwrap();
        let result = read_plan_file_if_exists(temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_read_plan_file_with_plan() {
        let temp = TempDir::new().unwrap();
        let plans_dir = temp.path().join(".codex").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        std::fs::write(plans_dir.join("test-plan.md"), "# Plan\n1. Do X").unwrap();

        let result = read_plan_file_if_exists(temp.path());
        assert!(result.is_some());
        assert!(result.unwrap().contains("# Plan"));
    }
}
