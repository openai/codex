#![cfg(unix)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::str::contains;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_last_message_does_not_follow_symlink() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let target_path = test.cwd_path().join("target.md");
    let output_path = test.cwd_path().join("output.md");
    std::fs::write(&target_path, "original")?;
    std::os::unix::fs::symlink(&target_path, &output_path)?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "replacement"),
        responses::ev_completed("resp1"),
    ]);
    let _response_mock = responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("write a response")
        .assert()
        .success()
        .stderr(contains("Failed to write last message file"));

    assert_eq!(std::fs::read_to_string(&target_path)?, "original");
    Ok(())
}
