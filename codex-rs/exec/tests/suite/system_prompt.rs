#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_overrides_system_prompt_from_arg() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let raw_instructions = "Custom instructions!   ";

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "done"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once_match(&server, any(), body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--system-prompt")
        .arg(raw_instructions)
        .arg("-C")
        .arg(test.cwd_path())
        .arg("summarize the repo")
        .assert()
        .success();

    let request = response_mock.single_request();
    let payload = request.body_json();
    let instructions = payload
        .get("instructions")
        .and_then(|value| value.as_str())
        .expect("instructions field missing from request");
    assert_eq!(instructions, raw_instructions.trim());

    Ok(())
}
