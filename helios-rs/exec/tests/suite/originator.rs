#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use helios_core::default_client::HELIOS_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR;
use core_test_support::responses;
use core_test_support::test_helios_exec::test_helios_exec;
use wiremock::matchers::header;

/// Verify that when the server reports an error, `helios-exec` exits with a
/// non-zero status code so automation can detect failures.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_helios_exec_originator() -> anyhow::Result<()> {
    let test = test_helios_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Hello, world!"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once_match(&server, header("Originator", "helios_exec"), body).await;

    test.cmd_with_server(&server)
        .env_remove(HELIOS_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .code(0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supports_originator_override() -> anyhow::Result<()> {
    let test = test_helios_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Hello, world!"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once_match(&server, header("Originator", "helios_exec_override"), body)
        .await;

    test.cmd_with_server(&server)
        .env("HELIOS_INTERNAL_ORIGINATOR_OVERRIDE", "helios_exec_override")
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .code(0);

    Ok(())
}
