use codex_git_utils::GitInfo;
use codex_git_utils::GitSha;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_has_changes;
use codex_git_utils::git_diff_to_remote;
use codex_git_utils::recent_commits;
use codex_git_utils::resolve_root_git_project_for_trust;
use core_test_support::skip_if_sandbox;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::process::Command;

// Helper function to create a test git repository
async fn create_test_git_repo(temp_dir: &TempDir) -> PathBuf {
    let repo_path = temp_dir.path().join("repo");
    fs::create_dir(&repo_path).expect("Failed to create repo dir");
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
async fn test_recent_commits_non_git_directory_returns_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let entries = recent_commits(temp_dir.path(), /*limit*/ 10).await;
    assert!(entries.is_empty(), "expected no commits outside a git repo");
}

#[tokio::test]
async fn test_recent_commits_orders_and_limits() {
    skip_if_sandbox!();
    use tokio::time::Duration;
    use tokio::time::sleep;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;

    // Make three distinct commits with small delays to ensure ordering by timestamp.
    fs::write(repo_path.join("file.txt"), "one").unwrap();
    Command::new("git")
        .args(["add", "file.txt"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "first change"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git commit 1");

    sleep(Duration::from_millis(1100)).await;

    fs::write(repo_path.join("file.txt"), "two").unwrap();
    Command::new("git")
        .args(["add", "file.txt"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git add 2");
    Command::new("git")
        .args(["commit", "-m", "second change"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git commit 2");

    sleep(Duration::from_millis(1100)).await;

    fs::write(repo_path.join("file.txt"), "three").unwrap();
    Command::new("git")
        .args(["add", "file.txt"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git add 3");
    Command::new("git")
        .args(["commit", "-m", "third change"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git commit 3");

    // Request the latest 3 commits; should be our three changes in reverse time order.
    let entries = recent_commits(&repo_path, /*limit*/ 3).await;
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].subject, "third change");
    assert_eq!(entries[1].subject, "second change");
    assert_eq!(entries[2].subject, "first change");
    // Basic sanity on SHA formatting
    for e in entries {
        assert!(e.sha.len() >= 7 && e.sha.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

async fn create_test_git_repo_with_remote(temp_dir: &TempDir) -> (PathBuf, String) {
    let repo_path = create_test_git_repo(temp_dir).await;
    let remote_path = temp_dir.path().join("remote.git");

    Command::new("git")
        .args(["init", "--bare", remote_path.to_str().unwrap()])
        .output()
        .await
        .expect("Failed to init bare remote");

    Command::new("git")
        .args(["remote", "add", "origin", remote_path.to_str().unwrap()])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to add remote");

    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to get branch");
    let branch = String::from_utf8(output.stdout).unwrap().trim().to_string();

    Command::new("git")
        .args(["push", "-u", "origin", &branch])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to push initial commit");

    (repo_path, branch)
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
    let commit_hash = git_info.commit_hash.unwrap().0;
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

    let remote_url_output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to read remote url");
    // Some dev environments rewrite remotes (e.g., force SSH), so compare against
    // whatever URL Git reports instead of a fixed placeholder.
    let expected_remote = String::from_utf8(remote_url_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Should have repository URL
    assert_eq!(git_info.repository_url, Some(expected_remote));
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

#[tokio::test]
async fn test_get_has_changes_non_git_directory_returns_none() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    assert_eq!(get_has_changes(temp_dir.path()).await, None);
}

#[tokio::test]
async fn test_get_has_changes_clean_repo_returns_false() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    assert_eq!(get_has_changes(&repo_path).await, Some(false));
}

#[tokio::test]
async fn test_get_has_changes_with_tracked_change_returns_true() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;

    fs::write(repo_path.join("test.txt"), "updated tracked file").expect("write tracked file");
    assert_eq!(get_has_changes(&repo_path).await, Some(true));
}

#[tokio::test]
async fn test_get_has_changes_with_untracked_change_returns_true() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;

    fs::write(repo_path.join("new_file.txt"), "untracked").expect("write untracked file");
    assert_eq!(get_has_changes(&repo_path).await, Some(true));
}

#[tokio::test]
async fn test_get_git_working_tree_state_clean_repo() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (repo_path, branch) = create_test_git_repo_with_remote(&temp_dir).await;

    let remote_sha = Command::new("git")
        .args(["rev-parse", &format!("origin/{branch}")])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to rev-parse remote");
    let remote_sha = String::from_utf8(remote_sha.stdout)
        .unwrap()
        .trim()
        .to_string();

    let state = git_diff_to_remote(&repo_path)
        .await
        .expect("Should collect working tree state");
    assert_eq!(state.sha, GitSha::new(&remote_sha));
    assert!(state.diff.is_empty());
}

#[tokio::test]
async fn test_get_git_working_tree_state_with_changes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (repo_path, branch) = create_test_git_repo_with_remote(&temp_dir).await;

    let tracked = repo_path.join("test.txt");
    fs::write(&tracked, "modified").unwrap();
    fs::write(repo_path.join("untracked.txt"), "new").unwrap();

    let remote_sha = Command::new("git")
        .args(["rev-parse", &format!("origin/{branch}")])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to rev-parse remote");
    let remote_sha = String::from_utf8(remote_sha.stdout)
        .unwrap()
        .trim()
        .to_string();

    let state = git_diff_to_remote(&repo_path)
        .await
        .expect("Should collect working tree state");
    assert_eq!(state.sha, GitSha::new(&remote_sha));
    assert!(state.diff.contains("test.txt"));
    assert!(state.diff.contains("untracked.txt"));
}

#[tokio::test]
async fn test_get_git_working_tree_state_branch_fallback() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (repo_path, _branch) = create_test_git_repo_with_remote(&temp_dir).await;

    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to create feature branch");
    Command::new("git")
        .args(["push", "-u", "origin", "feature"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to push feature branch");

    Command::new("git")
        .args(["checkout", "-b", "local-branch"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to create local branch");

    let remote_sha = Command::new("git")
        .args(["rev-parse", "origin/feature"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to rev-parse remote");
    let remote_sha = String::from_utf8(remote_sha.stdout)
        .unwrap()
        .trim()
        .to_string();

    let state = git_diff_to_remote(&repo_path)
        .await
        .expect("Should collect working tree state");
    assert_eq!(state.sha, GitSha::new(&remote_sha));
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_returns_none_outside_repo() {
    let tmp = TempDir::new().expect("tempdir");
    assert!(
        resolve_root_git_project_for_trust(tmp.path())
            .await
            .is_none()
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_regular_repo_returns_repo_root() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let expected = std::fs::canonicalize(&repo_path).unwrap();

    assert_eq!(
        resolve_root_git_project_for_trust(&repo_path).await,
        Some(expected.clone())
    );
    let nested = repo_path.join("sub/dir");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        resolve_root_git_project_for_trust(&nested).await,
        Some(expected)
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_detects_worktree_and_returns_main_root() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;

    // Create a linked worktree
    let wt_root = temp_dir.path().join("wt");
    let _ = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            wt_root.to_str().unwrap(),
            "-b",
            "feature/x",
        ])
        .current_dir(&repo_path)
        .output()
        .expect("git worktree add");

    let expected = std::fs::canonicalize(&repo_path).ok();
    let got = resolve_root_git_project_for_trust(&wt_root)
        .await
        .and_then(|path| std::fs::canonicalize(path).ok());
    assert_eq!(got, expected);
    let nested = wt_root.join("nested/sub");
    std::fs::create_dir_all(&nested).unwrap();
    let got_nested = resolve_root_git_project_for_trust(&nested)
        .await
        .and_then(|path| std::fs::canonicalize(path).ok());
    assert_eq!(got_nested, expected);
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_detects_worktree_pointer_without_git_command() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path().join("repo");
    let common_dir = repo_root.join(".git");
    let worktree_git_dir = common_dir.join("worktrees").join("feature-x");
    let worktree_root = tmp.path().join("wt");
    std::fs::create_dir_all(&worktree_git_dir).unwrap();
    std::fs::create_dir_all(&worktree_root).unwrap();
    std::fs::create_dir_all(worktree_root.join("nested")).unwrap();
    std::fs::write(
        worktree_root.join(".git"),
        format!("gitdir: {}\n", worktree_git_dir.display()),
    )
    .unwrap();

    let expected = std::fs::canonicalize(&repo_root).unwrap();
    assert_eq!(
        resolve_root_git_project_for_trust(&worktree_root).await,
        Some(expected.clone())
    );
    assert_eq!(
        resolve_root_git_project_for_trust(&worktree_root.join("nested")).await,
        Some(expected)
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_non_worktrees_gitdir_returns_none() {
    let tmp = TempDir::new().expect("tempdir");
    let proj = tmp.path().join("proj");
    std::fs::create_dir_all(proj.join("nested")).unwrap();

    // `.git` is a file but does not point to a worktrees path
    std::fs::write(
        proj.join(".git"),
        format!(
            "gitdir: {}\n",
            tmp.path().join("some/other/location").display()
        ),
    )
    .unwrap();

    assert!(resolve_root_git_project_for_trust(&proj).await.is_none());
    assert!(
        resolve_root_git_project_for_trust(&proj.join("nested"))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn test_get_git_working_tree_state_unpushed_commit() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let (repo_path, branch) = create_test_git_repo_with_remote(&temp_dir).await;

    let remote_sha = Command::new("git")
        .args(["rev-parse", &format!("origin/{branch}")])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to rev-parse remote");
    let remote_sha = String::from_utf8(remote_sha.stdout)
        .unwrap()
        .trim()
        .to_string();

    fs::write(repo_path.join("test.txt"), "updated").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to add file");
    Command::new("git")
        .args(["commit", "-m", "local change"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("Failed to commit");

    let state = git_diff_to_remote(&repo_path)
        .await
        .expect("Should collect working tree state");
    assert_eq!(state.sha, GitSha::new(&remote_sha));
    assert!(state.diff.contains("updated"));
}

#[test]
fn test_git_info_serialization() {
    let git_info = GitInfo {
        commit_hash: Some(GitSha::new("abc123def456")),
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
