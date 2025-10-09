#![cfg(target_os = "macos")]

//! Tests for the macOS sandboxing that are specific to Seatbelt.
//! Tests that apply to both Mac and Linux sandboxing should go in sandbox.rs.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use codex_core::protocol::SandboxPolicy;
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
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
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
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
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

/// Verify that user lookups via `pwd.getpwuid(os.getuid())` work under the
/// seatbelt sandbox. Prior to allowing the necessary mach‑lookup for
/// OpenDirectory libinfo, this would fail with `KeyError: getpwuid(): uid not found`.
#[tokio::test]
async fn python_getpwuid_works_under_seatbelt() {
    if std::env::var(CODEX_SANDBOX_ENV_VAR) == Ok("seatbelt".to_string()) {
        eprintln!("{CODEX_SANDBOX_ENV_VAR} is set to 'seatbelt', skipping test.");
        return;
    }

    // For local dev.
    if which::which("python3").is_err() {
        eprintln!("python3 not found in PATH, skipping test.");
        return;
    }

    // ReadOnly is sufficient here since we are only exercising user lookup.
    let policy = SandboxPolicy::ReadOnly;
    let command_cwd = std::env::current_dir().expect("getcwd");
    let sandbox_cwd = command_cwd.clone();

    let mut child = spawn_command_under_seatbelt(
        vec![
            "python3".to_string(),
            "-c".to_string(),
            // Print the passwd struct; success implies lookup worked.
            "import pwd, os; print(pwd.getpwuid(os.getuid()))".to_string(),
        ],
        command_cwd,
        &policy,
        sandbox_cwd.as_path(),
        StdioPolicy::RedirectForShellTool,
        HashMap::new(),
    )
    .await
    .expect("should be able to spawn python under seatbelt");

    let status = child
        .wait()
        .await
        .expect("should be able to wait for child process");
    assert!(status.success(), "python exited with {status:?}");
}

#[expect(clippy::expect_used)]
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

#[expect(clippy::expect_used)]
/// Note that `path` must be absolute.
async fn touch(path: &Path, policy: &SandboxPolicy) -> bool {
    assert!(path.is_absolute(), "Path must be absolute: {path:?}");
    let command_cwd = std::env::current_dir().expect("getcwd");
    let sandbox_cwd = command_cwd.clone();
    let mut child = spawn_command_under_seatbelt(
        vec![
            "/usr/bin/touch".to_string(),
            path.to_string_lossy().to_string(),
        ],
        command_cwd,
        policy,
        sandbox_cwd.as_path(),
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
