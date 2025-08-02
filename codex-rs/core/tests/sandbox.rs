#![cfg(any(target_os = "macos", target_os = "linux"))]
#![expect(clippy::expect_used)]

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use assert_cmd::cargo::cargo_bin;
#[cfg(target_os = "linux")]
use codex_core::exec::spawn_command_under_linux_sandbox;
use codex_core::protocol::SandboxPolicy;
#[cfg(target_os = "macos")]
use codex_core::seatbelt::spawn_command_under_seatbelt;
use codex_core::spawn::CODEX_SANDBOX_ENV_VAR;
use codex_core::spawn::StdioPolicy;
use tempfile::TempDir;

struct TestScenario {
    repo_parent: PathBuf,
    file_outside_repo: PathBuf,
    repo_root: PathBuf,
    file_in_repo_root: PathBuf,
    file_in_dot_git_dir: PathBuf,
}

struct TestExpectations {
    file_outside_repo_is_writable: bool,
    file_in_repo_root_is_writable: bool,
    file_in_dot_git_dir_is_writable: bool,
}

impl TestScenario {
    async fn run_test(&self, policy: &SandboxPolicy, expectations: TestExpectations) {
        if std::env::var(CODEX_SANDBOX_ENV_VAR) == Ok("seatbelt".to_string()) {
            eprintln!("{CODEX_SANDBOX_ENV_VAR} is set to 'seatbelt', skipping test.");
            return;
        }

        #[cfg(target_os = "linux")]
        {
            // If the sandbox helper is not functional in this environment,
            // skip the test rather than fail.
            let exe = cargo_bin("codex-linux-sandbox");
            let mut child = match spawn_command_under_linux_sandbox(
                exe,
                vec!["/usr/bin/true".to_string()],
                &SandboxPolicy::ReadOnly,
                std::env::current_dir().expect("should be able to get current dir"),
                StdioPolicy::RedirectForShellTool,
                HashMap::new(),
            )
            .await
            {
                Ok(child) => child,
                Err(_) => {
                    eprintln!("codex-linux-sandbox unavailable, skipping test.");
                    return;
                }
            };
            if !child
                .wait()
                .await
                .expect("should be able to wait for child process")
                .success()
            {
                eprintln!("codex-linux-sandbox failed, skipping test.");
                return;
            }
        }

        assert_eq!(
            touch(&self.file_outside_repo, policy).await,
            expectations.file_outside_repo_is_writable
        );
        assert_eq!(
            self.file_outside_repo.exists(),
            expectations.file_outside_repo_is_writable
        );

        assert_eq!(
            touch(&self.file_in_repo_root, policy).await,
            expectations.file_in_repo_root_is_writable
        );
        assert_eq!(
            self.file_in_repo_root.exists(),
            expectations.file_in_repo_root_is_writable
        );

        assert_eq!(
            touch(&self.file_in_dot_git_dir, policy).await,
            expectations.file_in_dot_git_dir_is_writable
        );
        assert_eq!(
            self.file_in_dot_git_dir.exists(),
            expectations.file_in_dot_git_dir_is_writable
        );
    }
}

/// If the user has added a workspace root that is not a Git repo root, then
/// the user has to specify `--skip-git-repo-check` or go through some
/// interstitial that indicates they are taking on some risk because Git
/// cannot be used to backup their work before the agent begins.
///
/// Because the user has agreed to this risk, we do not try find all .git
/// folders in the workspace and block them (though we could change our
/// position on this in the future).
#[tokio::test]
async fn if_parent_of_repo_is_writable_then_dot_git_folder_is_writable() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let test_scenario = create_test_scenario(&tmp);
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![test_scenario.repo_parent.clone()],
        network_access: false,
        include_default_writable_roots: false,
    };

    test_scenario
        .run_test(
            &policy,
            TestExpectations {
                file_outside_repo_is_writable: true,
                file_in_repo_root_is_writable: true,
                file_in_dot_git_dir_is_writable: true,
            },
        )
        .await;
}

/// When the writable root is the root of a Git repository (as evidenced by the
/// presence of a .git folder), then the .git folder should be read-only if
/// the policy is `WorkspaceWrite`.
#[tokio::test]
async fn if_git_repo_is_writable_root_then_dot_git_folder_is_read_only() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let test_scenario = create_test_scenario(&tmp);
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![test_scenario.repo_root.clone()],
        network_access: false,
        include_default_writable_roots: false,
    };

    test_scenario
        .run_test(
            &policy,
            TestExpectations {
                file_outside_repo_is_writable: false,
                file_in_repo_root_is_writable: true,
                file_in_dot_git_dir_is_writable: false,
            },
        )
        .await;
}

/// Under DangerFullAccess, all writes should be permitted anywhere on disk,
/// including inside the .git folder.
#[tokio::test]
async fn danger_full_access_allows_all_writes() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let test_scenario = create_test_scenario(&tmp);
    let policy = SandboxPolicy::DangerFullAccess;

    test_scenario
        .run_test(
            &policy,
            TestExpectations {
                file_outside_repo_is_writable: true,
                file_in_repo_root_is_writable: true,
                file_in_dot_git_dir_is_writable: true,
            },
        )
        .await;
}

/// Under ReadOnly, writes should not be permitted anywhere on disk.
#[tokio::test]
async fn read_only_forbids_all_writes() {
    let tmp = TempDir::new().expect("should be able to create temp dir");
    let test_scenario = create_test_scenario(&tmp);
    let policy = SandboxPolicy::ReadOnly;

    test_scenario
        .run_test(
            &policy,
            TestExpectations {
                file_outside_repo_is_writable: false,
                file_in_repo_root_is_writable: false,
                file_in_dot_git_dir_is_writable: false,
            },
        )
        .await;
}

fn create_test_scenario(tmp: &TempDir) -> TestScenario {
    let repo_parent = tmp.path().to_path_buf();
    let repo_root = repo_parent.join("repo");
    let dot_git_dir = repo_root.join(".git");

    std::fs::create_dir(&repo_root).expect("should be able to create repo root");
    std::fs::create_dir(&dot_git_dir).expect("should be able to create .git dir");

    TestScenario {
        file_outside_repo: repo_parent.join("outside.txt"),
        repo_parent,
        file_in_repo_root: repo_root.join("repo_file.txt"),
        repo_root,
        file_in_dot_git_dir: dot_git_dir.join("dot_git_file.txt"),
    }
}

/// Note that `path` must be absolute.
async fn touch(path: &Path, policy: &SandboxPolicy) -> bool {
    assert!(path.is_absolute(), "Path must be absolute: {path:?}");
    #[cfg(target_os = "macos")]
    {
        let mut child = spawn_command_under_seatbelt(
            vec![
                "/usr/bin/touch".to_string(),
                path.to_string_lossy().to_string(),
            ],
            policy,
            std::env::current_dir().expect("should be able to get current dir"),
            StdioPolicy::RedirectForShellTool,
            HashMap::new(),
        )
        .await
        .expect("should be able to spawn command under seatbelt");
        child
            .wait()
            .await
            .expect("should be able to wait for child process")
            .success()
    }
    #[cfg(target_os = "linux")]
    {
        let exe = cargo_bin("codex-linux-sandbox");
        let mut child = spawn_command_under_linux_sandbox(
            exe,
            vec![
                "/usr/bin/touch".to_string(),
                path.to_string_lossy().to_string(),
            ],
            policy,
            std::env::current_dir().expect("should be able to get current dir"),
            StdioPolicy::RedirectForShellTool,
            HashMap::new(),
        )
        .await
        .expect("should be able to spawn command under linux sandbox");
        child
            .wait()
            .await
            .expect("should be able to wait for child process")
            .success()
    }
}
