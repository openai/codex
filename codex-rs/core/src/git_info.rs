use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use tokio::process::Command;
use tokio::time::Duration as TokioDuration;
use tokio::time::timeout;

/// Timeout for git commands to prevent freezing on large repositories
const GIT_COMMAND_TIMEOUT: TokioDuration = TokioDuration::from_secs(5);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitInfo {
    /// Current commit hash (SHA)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Current branch name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Repository URL (if available from remote)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

/// Collect git repository information from the given working directory using command-line git.
/// Returns None if no git repository is found or if git operations fail.
/// Uses timeouts to prevent freezing on large repositories.
/// All git commands (except the initial repo check) run in parallel for better performance.
pub async fn collect_git_info(cwd: &Path) -> Option<GitInfo> {
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
    if let Some(output) = commit_result
        && output.status.success()
        && let Ok(hash) = String::from_utf8(output.stdout)
    {
        git_info.commit_hash = Some(hash.trim().to_string());
    }

    // Process branch name
    if let Some(output) = branch_result
        && output.status.success()
        && let Ok(branch) = String::from_utf8(output.stdout)
    {
        let branch = branch.trim();
        if branch != "HEAD" {
            git_info.branch = Some(branch.to_string());
        }
    }

    // Process repository URL
    if let Some(output) = url_result
        && output.status.success()
        && let Ok(url) = String::from_utf8(output.stdout)
    {
        git_info.repository_url = Some(url.trim().to_string());
    }

    Some(git_info)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper function to create a test git repository
    async fn create_test_git_repo(temp_dir: &TempDir) -> PathBuf {
        let repo_path = temp_dir.path().to_path_buf();
        let envs = vec![
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ];

        // Initialize git repo
        Command::new("git")
            .envs(envs.clone())
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to init git repo");

        // Configure git user (required for commits)
        Command::new("git")
            .envs(envs.clone())
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user name");

        Command::new("git")
            .envs(envs.clone())
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user email");

        // Create a test file and commit it
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        Command::new("git")
            .envs(envs.clone())
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add files");

        Command::new("git")
            .envs(envs.clone())
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
