//! Functionality to persist a Codex conversation *rollout* – a linear list of
//! [`ResponseItem`] objects exchanged during a session – to disk so that
//! sessions can be replayed or inspected later (mirrors the behaviour of the
//! upstream TypeScript implementation).

use std::fs::File;
use std::fs::{self};
use std::io::Error as IoError;
use std::path::Path;

use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration as TokioDuration;
use tokio::time::timeout;

use uuid::Uuid;

use crate::config::Config;
use crate::models::ResponseItem;

/// Folder inside `~/.codex` that holds saved rollouts.
const SESSIONS_SUBDIR: &str = "sessions";

#[derive(Serialize)]
struct GitInfo {
    /// Current commit hash (SHA)
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_hash: Option<String>,
    /// Current branch name
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    /// Repository URL (if available from remote)
    #[serde(skip_serializing_if = "Option::is_none")]
    repository_url: Option<String>,
}

#[derive(Serialize)]
struct SessionMeta {
    id: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitInfo>,
}

/// Timeout for git commands to prevent freezing on large repositories
const GIT_COMMAND_TIMEOUT: TokioDuration = TokioDuration::from_secs(3);

/// Run a git command with a timeout to prevent blocking on large repositories
async fn run_git_command_with_timeout(args: &[&str], cwd: &Path) -> Option<std::process::Output> {
    let result = timeout(
        GIT_COMMAND_TIMEOUT,
        Command::new("git").args(args).current_dir(cwd).output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Some(output),
        _ => None, // Timeout or error
    }
}

/// Collect git repository information from the given working directory using command-line git.
/// Returns None if no git repository is found or if git operations fail.
/// Uses timeouts to prevent freezing on large repositories.
/// All git commands (except the initial repo check) run in parallel for better performance.
async fn collect_git_info(cwd: &Path) -> Option<GitInfo> {
    // Check if we're in a git repository first
    let is_git_repo = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd)
        .await?
        .status
        .success();

    if !is_git_repo {
        return None;
    }

    // Run all git info collection commands in parallel
    let (commit_result, branch_result, url_result) = tokio::join!(
        run_git_command_with_timeout(&["rev-parse", "HEAD"], cwd),
        run_git_command_with_timeout(&["rev-parse", "--abbrev-ref", "HEAD"], cwd),
        run_git_command_with_timeout(&["remote", "get-url", "origin"], cwd)
    );

    let mut git_info = GitInfo {
        commit_hash: None,
        branch: None,
        repository_url: None,
    };

    // Process commit hash
    if let Some(output) = commit_result {
        if output.status.success() {
            if let Ok(hash) = String::from_utf8(output.stdout) {
                git_info.commit_hash = Some(hash.trim().to_string());
            }
        }
    }

    // Process branch name
    if let Some(output) = branch_result {
        if output.status.success() {
            if let Ok(branch) = String::from_utf8(output.stdout) {
                let branch = branch.trim();
                if branch != "HEAD" {
                    git_info.branch = Some(branch.to_string());
                }
            }
        }
    }

    // Process repository URL
    if let Some(output) = url_result {
        if output.status.success() {
            if let Ok(url) = String::from_utf8(output.stdout) {
                git_info.repository_url = Some(url.trim().to_string());
            }
        }
    }

    Some(git_info)
}

/// Records all [`ResponseItem`]s for a session and flushes them to disk after
/// every update.
///
/// Rollouts are recorded as JSONL and can be inspected with tools such as:
///
/// ```ignore
/// $ jq -C . ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// $ fx ~/.codex/sessions/rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl
/// ```
#[derive(Clone)]
pub(crate) struct RolloutRecorder {
    tx: Sender<String>,
}

impl RolloutRecorder {
    /// Attempt to create a new [`RolloutRecorder`]. If the sessions directory
    /// cannot be created or the rollout file cannot be opened we return the
    /// error so the caller can decide whether to disable persistence.
    pub async fn new(
        config: &Config,
        uuid: Uuid,
        instructions: Option<String>,
        cwd: &Path,
    ) -> std::io::Result<Self> {
        let LogFileInfo {
            file,
            session_id,
            timestamp,
        } = create_log_file(config, uuid)?;

        // Build the static session metadata JSON first.
        let timestamp_format: &[FormatItem] = format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        );
        let timestamp = timestamp
            .format(timestamp_format)
            .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

        // Collect git repository information
        let git_info = collect_git_info(cwd).await;

        let meta = SessionMeta {
            timestamp,
            id: session_id.to_string(),
            instructions,
            git: git_info,
        };

        // A reasonably-sized bounded channel. If the buffer fills up the send
        // future will yield, which is fine – we only need to ensure we do not
        // perform *blocking* I/O on the caller's thread.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);

        // Spawn a Tokio task that owns the file handle and performs async
        // writes. Using `tokio::fs::File` keeps everything on the async I/O
        // driver instead of blocking the runtime.
        tokio::task::spawn(async move {
            let mut file = tokio::fs::File::from_std(file);

            while let Some(line) = rx.recv().await {
                // Write line + newline, then flush to disk.
                if let Err(e) = file.write_all(line.as_bytes()).await {
                    tracing::warn!("rollout writer: failed to write line: {e}");
                    break;
                }
                if let Err(e) = file.write_all(b"\n").await {
                    tracing::warn!("rollout writer: failed to write newline: {e}");
                    break;
                }
                if let Err(e) = file.flush().await {
                    tracing::warn!("rollout writer: failed to flush: {e}");
                    break;
                }
            }
        });

        let recorder = Self { tx };
        // Ensure SessionMeta is the first item in the file.
        recorder.record_item(&meta).await?;
        Ok(recorder)
    }

    /// Append `items` to the rollout file.
    pub(crate) async fn record_items(&self, items: &[ResponseItem]) -> std::io::Result<()> {
        for item in items {
            match item {
                // Note that function calls may look a bit strange if they are
                // "fully qualified MCP tool calls," so we could consider
                // reformatting them in that case.
                ResponseItem::Message { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::FunctionCallOutput { .. } => {}
                ResponseItem::Reasoning { .. } | ResponseItem::Other => {
                    // These should never be serialized.
                    continue;
                }
            }
            self.record_item(item).await?;
        }
        Ok(())
    }

    async fn record_item(&self, item: &impl Serialize) -> std::io::Result<()> {
        // Serialize the item to JSON first so that the writer thread only has
        // to perform the actual write.
        let json = serde_json::to_string(item)
            .map_err(|e| IoError::other(format!("failed to serialize response items: {e}")))?;

        self.tx
            .send(json)
            .await
            .map_err(|e| IoError::other(format!("failed to queue rollout item: {e}")))
    }
}

struct LogFileInfo {
    /// Opened file handle to the rollout file.
    file: File,

    /// Session ID (also embedded in filename).
    session_id: Uuid,

    /// Timestamp for the start of the session.
    timestamp: OffsetDateTime,
}

fn create_log_file(config: &Config, session_id: Uuid) -> std::io::Result<LogFileInfo> {
    // Resolve ~/.codex/sessions/YYYY/MM/DD and create it if missing.
    let timestamp = OffsetDateTime::now_local()
        .map_err(|e| IoError::other(format!("failed to get local time: {e}")))?;
    let mut dir = config.codex_home.clone();
    dir.push(SESSIONS_SUBDIR);
    dir.push(timestamp.year().to_string());
    dir.push(format!("{:02}", u8::from(timestamp.month())));
    dir.push(format!("{:02}", timestamp.day()));
    fs::create_dir_all(&dir)?;

    // Custom format for YYYY-MM-DDThh-mm-ss. Use `-` instead of `:` for
    // compatibility with filesystems that do not allow colons in filenames.
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let date_str = timestamp
        .format(format)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;

    let filename = format!("rollout-{date_str}-{session_id}.jsonl");

    let path = dir.join(filename);
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;

    Ok(LogFileInfo {
        file,
        session_id,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]

    use super::*;

    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper function to create a test git repository
    async fn create_test_git_repo(temp_dir: &TempDir) -> PathBuf {
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to init git repo");

        // Configure git user (required for commits)
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user name");

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user email");

        // Create a test file and commit it
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add files");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to commit");

        repo_path
    }

    #[tokio::test]
    async fn test_collect_git_info_non_git_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = collect_git_info(temp_dir.path()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_collect_git_info_git_repository() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have commit hash
        assert!(git_info.commit_hash.is_some());
        let commit_hash = git_info.commit_hash.unwrap();
        assert_eq!(commit_hash.len(), 40); // SHA-1 hash should be 40 characters
        assert!(commit_hash.chars().all(|c| c.is_ascii_hexdigit()));

        // Should have branch (likely "main" or "master")
        assert!(git_info.branch.is_some());
        let branch = git_info.branch.unwrap();
        assert!(branch == "main" || branch == "master");

        // Repository URL might be None for local repos without remote
        // This is acceptable behavior
    }

    #[tokio::test]
    async fn test_collect_git_info_with_remote() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Add a remote origin
        Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add remote");

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have repository URL
        assert_eq!(
            git_info.repository_url,
            Some("https://github.com/example/repo.git".to_string())
        );
    }

    #[tokio::test]
    async fn test_collect_git_info_detached_head() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Get the current commit hash
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to get HEAD");
        let commit_hash = String::from_utf8(output.stdout).unwrap().trim().to_string();

        // Checkout the commit directly (detached HEAD)
        Command::new("git")
            .args(["checkout", &commit_hash])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to checkout commit");

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have commit hash
        assert!(git_info.commit_hash.is_some());
        // Branch should be None for detached HEAD (since rev-parse --abbrev-ref HEAD returns "HEAD")
        assert!(git_info.branch.is_none());
    }

    #[tokio::test]
    async fn test_collect_git_info_with_branch() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create and checkout a new branch
        Command::new("git")
            .args(["checkout", "-b", "feature-branch"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to create branch");

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have the new branch name
        assert_eq!(git_info.branch, Some("feature-branch".to_string()));
    }

    #[test]
    fn test_git_info_serialization() {
        let git_info = GitInfo {
            commit_hash: Some("abc123def456".to_string()),
            branch: Some("main".to_string()),
            repository_url: Some("https://github.com/example/repo.git".to_string()),
        };

        let json = serde_json::to_string(&git_info).expect("Should serialize GitInfo");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse JSON");

        assert_eq!(parsed["commit_hash"], "abc123def456");
        assert_eq!(parsed["branch"], "main");
        assert_eq!(
            parsed["repository_url"],
            "https://github.com/example/repo.git"
        );
    }

    #[test]
    fn test_git_info_serialization_with_nones() {
        let git_info = GitInfo {
            commit_hash: None,
            branch: None,
            repository_url: None,
        };

        let json = serde_json::to_string(&git_info).expect("Should serialize GitInfo");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse JSON");

        // Fields with None values should be omitted due to skip_serializing_if
        assert!(!parsed.as_object().unwrap().contains_key("commit_hash"));
        assert!(!parsed.as_object().unwrap().contains_key("branch"));
        assert!(!parsed.as_object().unwrap().contains_key("repository_url"));
    }
}
