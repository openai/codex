#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use codex_core::auth::CODEX_API_KEY_ENV_VAR;
use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::sleep;

/// Verify that SIGINT (Ctrl+C) during exec mode exits with code 130.
/// Priority: interrupted > error > success
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exits_130_on_sigint() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;

    // Mock long-running response (no response.done)
    let body = responses::sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp_test", "status": "in_progress"}
        }),
        serde_json::json!({
            "type": "response.output_item.delta",
            "response_id": "resp_test",
            "delta": {"type": "thinking", "content": "Processing..."}
        }),
    ]);
    responses::mount_sse(&server, body).await;

    let bin_path = assert_cmd::cargo::cargo_bin("codex-exec");
    let base_url = format!("{}/v1", server.uri());
    let mut cmd = tokio::process::Command::new(&bin_path);
    cmd.current_dir(test.cwd_path())
        .env("CODEX_HOME", test.home_path())
        .env(CODEX_API_KEY_ENV_VAR, "dummy")
        .env("OPENAI_BASE_URL", &base_url)
        .arg("--skip-git-repo-check")
        .arg("test prompt")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    sleep(Duration::from_millis(1500)).await;

    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGINT);
        }
    }

    let status = child.wait().await?;

    assert_eq!(
        status.code(),
        Some(130),
        "Expected exit code 130 on SIGINT, got {:?}",
        status.code()
    );

    Ok(())
}

/// Verify that SIGINT takes precedence over errors.
/// If both error events and SIGINT occur, exit code should be 130.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigint_takes_precedence_over_error() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;

    // Mock response with error event, but we'll interrupt with SIGINT
    let body = responses::sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp_test", "status": "in_progress"}
        }),
        serde_json::json!({
            "type": "response.output_item.delta",
            "response_id": "resp_test",
            "delta": {"type": "thinking", "content": "Processing..."}
        }),
        serde_json::json!({
            "type": "response.failed",
            "response": {
                "id": "resp_test",
                "error": {"code": "internal_error", "message": "synthetic error"}
            }
        }),
    ]);
    responses::mount_sse(&server, body).await;

    let bin_path = assert_cmd::cargo::cargo_bin("codex-exec");
    let base_url = format!("{}/v1", server.uri());
    let mut cmd = tokio::process::Command::new(&bin_path);
    cmd.current_dir(test.cwd_path())
        .env("CODEX_HOME", test.home_path())
        .env(CODEX_API_KEY_ENV_VAR, "dummy")
        .env("OPENAI_BASE_URL", &base_url)
        .arg("--skip-git-repo-check")
        .arg("test prompt")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    sleep(Duration::from_millis(1500)).await;

    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGINT);
        }
    }

    let status = child.wait().await?;

    // Even if errors occurred, SIGINT takes precedence
    assert_eq!(
        status.code(),
        Some(130),
        "Expected SIGINT precedence: exit 130, got {:?}",
        status.code()
    );

    Ok(())
}
