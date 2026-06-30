use super::*;
use crate::get_has_changes;
use crate::git_diff_to_remote;
use pretty_assertions::assert_eq;
use std::path::Path;
use tokio::process::Command as TokioCommand;

#[test]
fn rejects_untrusted_nonempty_helper_values() {
    assert!(!config_output_has_untrusted_executable_helpers(b""));
    assert!(!config_output_has_untrusted_executable_helpers(b"\0"));
    assert!(config_output_has_untrusted_executable_helpers(
        b"local\0filter.example.clean\nhelper\0"
    ));
    assert!(config_output_has_untrusted_executable_helpers(
        b"command\0merge.example.driver\nhelper\0"
    ));
    assert!(config_output_has_untrusted_executable_helpers(
        b"global\0merge.example.driver\nhelper\0"
    ));
    assert!(config_output_has_untrusted_executable_helpers(
        b"system\0merge.example.driver\nhelper\0"
    ));
    assert!(config_output_has_untrusted_executable_helpers(
        b"worktree\0filter.example.process\nhelper\0"
    ));
}

#[test]
fn allows_trusted_or_disabled_helper_values() {
    assert!(!config_output_has_untrusted_executable_helpers(
        b"global\0filter.lfs.process\ngit-lfs filter-process\0"
    ));
    assert!(!config_output_has_untrusted_executable_helpers(
        b"local\0filter.example.clean\n\0"
    ));
    assert!(!config_output_has_untrusted_executable_helpers(
        b"global\0merge.disabled.driver\n\0"
    ));
}

#[test]
fn rejects_malformed_probe_output() {
    assert!(config_output_has_untrusted_executable_helpers(b"local\0"));
    assert!(config_output_has_untrusted_executable_helpers(
        b"local\0filter.example.clean\0"
    ));
}

async fn run_git(repo_path: &Path, args: &[&str]) {
    let output = TokioCommand::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .expect("run git command");
    assert!(
        output.status.success(),
        "git command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn create_test_git_repo(temp_dir: &tempfile::TempDir) -> std::path::PathBuf {
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).expect("create repo dir");
    run_git(&repo_path, &["init"]).await;
    run_git(&repo_path, &["config", "user.name", "Test User"]).await;
    run_git(&repo_path, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(repo_path.join("test.txt"), "test content").expect("write test file");
    run_git(&repo_path, &["add", "."]).await;
    run_git(&repo_path, &["commit", "-m", "initial"]).await;
    repo_path
}

async fn configure_clean_filter(repo_path: &Path, tracked_path: &str) {
    std::fs::write(
        repo_path.join(".gitattributes"),
        format!("{tracked_path} filter=x=y\n"),
    )
    .expect("write attributes");
    run_git(repo_path, &["add", ".gitattributes"]).await;
    run_git(repo_path, &["commit", "-m", "attributes"]).await;
    run_git(
        repo_path,
        &[
            "config",
            "filter.x=y.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;

    let tracked_file = repo_path.join(tracked_path);
    let contents = std::fs::read_to_string(&tracked_file).expect("read tracked file");
    std::thread::sleep(std::time::Duration::from_secs(1));
    std::fs::write(tracked_file, contents).expect("refresh tracked file");
}

async fn configured_filter_ran(repo_path: &Path) -> bool {
    let output = TokioCommand::new("git")
        .args(["config", "--get", "codex.filterran"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("read filter marker");
    output.status.success()
}

async fn add_submodule_with_clean_filter(parent: &Path) {
    let source = tempfile::tempdir().expect("submodule source");
    let source_path = source.path();
    run_git(source_path, &["init"]).await;
    run_git(source_path, &["config", "user.name", "Test User"]).await;
    run_git(source_path, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(source_path.join("nested.txt"), "original\n").expect("nested file");
    std::fs::write(
        source_path.join(".gitattributes"),
        "nested.txt filter=codex-test\n",
    )
    .expect("nested attributes");
    run_git(source_path, &["add", "."]).await;
    run_git(source_path, &["commit", "-m", "seed"]).await;

    run_git(
        parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            source_path.to_str().expect("source path"),
            "nested",
        ],
    )
    .await;
    run_git(parent, &["commit", "-m", "add submodule"]).await;
    let nested = parent.join("nested");
    run_git(
        &nested,
        &[
            "config",
            "filter.codex-test.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;
    std::fs::write(nested.join("nested.txt"), "modified\n").expect("dirty nested file");
}

async fn add_origin_and_push(repo_path: &Path, remote_path: &Path) -> String {
    run_git(
        remote_path.parent().expect("remote parent"),
        &["init", "--bare", remote_path.to_str().expect("remote path")],
    )
    .await;
    run_git(
        repo_path,
        &[
            "remote",
            "add",
            "origin",
            remote_path.to_str().expect("remote path"),
        ],
    )
    .await;
    let branch_output = TokioCommand::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("read branch");
    assert!(branch_output.status.success(), "read branch");
    let branch = String::from_utf8(branch_output.stdout)
        .expect("branch utf8")
        .trim()
        .to_string();
    run_git(repo_path, &["push", "-u", "origin", &branch]).await;
    branch
}

#[tokio::test]
async fn get_has_changes_rejects_configured_clean_filter_without_running_it() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    configure_clean_filter(&repo_path, "test.txt").await;

    assert_eq!(get_has_changes(&repo_path).await, None);
    assert!(!configured_filter_ran(&repo_path).await);
}

#[tokio::test]
async fn get_has_changes_does_not_enter_dirty_submodules() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    add_submodule_with_clean_filter(&repo_path).await;

    assert_eq!(get_has_changes(&repo_path).await, Some(false));
    assert!(!configured_filter_ran(&repo_path.join("nested")).await);
}

#[tokio::test]
async fn git_diff_to_remote_does_not_enter_dirty_submodules() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    add_submodule_with_clean_filter(&repo_path).await;
    run_git(&repo_path, &["config", "diff.submodule", "diff"]).await;
    let remote_path = temp_dir.path().join("remote.git");
    add_origin_and_push(&repo_path, &remote_path).await;

    assert!(git_diff_to_remote(&repo_path).await.is_some());
    assert!(!configured_filter_ran(&repo_path.join("nested")).await);
}

#[tokio::test]
async fn git_diff_to_remote_rejects_configured_clean_filter_without_running_it() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let remote_path = temp_dir.path().join("remote.git");
    let branch = add_origin_and_push(&repo_path, &remote_path).await;

    configure_clean_filter(&repo_path, "test.txt").await;
    run_git(&repo_path, &["push", "origin", &branch]).await;

    assert!(git_diff_to_remote(&repo_path).await.is_none());
    assert!(!configured_filter_ran(&repo_path).await);
}
