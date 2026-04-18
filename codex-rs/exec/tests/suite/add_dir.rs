#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use std::fs;

/// Verify that the --add-dir flag is accepted and the command runs successfully.
/// This test confirms the CLI argument is properly wired up.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_add_dir_flag() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Task completed"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    // Create temporary directories to use with --add-dir
    let temp_dir1 = tempfile::tempdir()?;
    let temp_dir2 = tempfile::tempdir()?;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--add-dir")
        .arg(temp_dir1.path())
        .arg("--add-dir")
        .arg(temp_dir2.path())
        .arg("test with additional directories")
        .assert()
        .code(0);

    Ok(())
}

/// Verify that multiple --add-dir flags can be specified.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepts_multiple_add_dir_flags() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Multiple directories accepted"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    let temp_dir1 = tempfile::tempdir()?;
    let temp_dir2 = tempfile::tempdir()?;
    let temp_dir3 = tempfile::tempdir()?;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--add-dir")
        .arg(temp_dir1.path())
        .arg("--add-dir")
        .arg(temp_dir2.path())
        .arg("--add-dir")
        .arg(temp_dir3.path())
        .arg("test with three directories")
        .assert()
        .code(0);

    Ok(())
}

/// Verify that --add-dir grants write access to the specified directory when
/// workspace-write sandboxing is active.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_dir_allows_writes_under_workspace_write() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let writable_dir = tempfile::tempdir()?;
    let target_file = writable_dir.path().join("created-by-codex.txt");
    let args = serde_json::json!({
        "command": [
            "bash",
            "-lc",
            "printf add-dir-ok > \"$1\"",
            "bash",
            target_file.to_string_lossy(),
        ],
    });

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("response_1"),
                ev_function_call(
                    "shell_add_dir_write",
                    "shell",
                    &serde_json::to_string(&args)?,
                ),
                ev_completed("response_1"),
            ]),
            sse(vec![
                ev_response_created("response_2"),
                responses::ev_assistant_message("response_2", "Done"),
                ev_completed("response_2"),
            ]),
        ],
    )
    .await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--add-dir")
        .arg(writable_dir.path())
        .arg("create a file in the added directory")
        .assert()
        .code(0);

    assert_eq!(fs::read_to_string(target_file)?, "add-dir-ok");

    Ok(())
}
