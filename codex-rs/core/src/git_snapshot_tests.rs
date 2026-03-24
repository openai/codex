use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::process::Command;

use crate::git_snapshot::prepare_workspace_snapshot_commit;

async fn run_git(repo_path: &std::path::Path, args: &[&str]) -> Vec<u8> {
    let git_config_global = repo_path.join("empty-git-config");
    std::fs::write(&git_config_global, "").expect("write empty git config");
    let output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", &git_config_global)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

async fn init_repo() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir_all(&repo_path).expect("create repo");

    run_git(&repo_path, &["init"]).await;
    std::fs::write(repo_path.join("README.md"), "hello\n").expect("write README");
    run_git(&repo_path, &["add", "."]).await;
    run_git(
        &repo_path,
        &[
            "-c",
            "user.name=Test User",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "initial",
        ],
    )
    .await;

    (temp_dir, repo_path)
}

#[tokio::test]
async fn workspace_snapshot_commit_returns_head_for_clean_repo() {
    let (_temp_dir, repo_path) = init_repo().await;

    let head = String::from_utf8(run_git(&repo_path, &["rev-parse", "HEAD"]).await)
        .expect("head should be valid utf-8")
        .trim()
        .to_string();

    let snapshot = prepare_workspace_snapshot_commit(&repo_path)
        .await
        .expect("snapshot hash");

    assert_eq!(snapshot, head);
}

#[tokio::test]
async fn workspace_snapshot_commit_includes_untracked_files() {
    let (_temp_dir, repo_path) = init_repo().await;
    std::fs::write(repo_path.join("notes.txt"), "new file\n").expect("write untracked file");

    let snapshot = prepare_workspace_snapshot_commit(&repo_path)
        .await
        .expect("snapshot hash");
    let file_contents =
        String::from_utf8(run_git(&repo_path, &["show", &format!("{snapshot}:notes.txt")]).await)
            .expect("snapshot file should be valid utf-8");

    assert_eq!(file_contents, "new file\n");
}

#[tokio::test]
async fn workspace_snapshot_commit_is_deterministic_for_same_dirty_tree() {
    let (_temp_dir, repo_path) = init_repo().await;
    std::fs::write(repo_path.join("README.md"), "updated\n").expect("update README");

    let first = prepare_workspace_snapshot_commit(&repo_path)
        .await
        .expect("first snapshot hash");
    let second = prepare_workspace_snapshot_commit(&repo_path)
        .await
        .expect("second snapshot hash");

    assert_eq!(first, second);
}
