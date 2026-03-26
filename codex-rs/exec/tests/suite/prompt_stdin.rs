#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_appends_piped_stdin_to_prompt_argument() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("-m")
        .arg("gpt-5.1")
        .arg("Summarize this concisely")
        .write_stdin("my output\n")
        .assert()
        .success();

    let request = response_mock.single_request();
    assert!(
        request.has_message_with_input_texts("user", |texts| {
            texts == ["Summarize this concisely\n\n<stdin>\nmy output\n</stdin>".to_string()]
        }),
        "request should include a user message with the prompt plus piped stdin context"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_ignores_empty_piped_stdin_when_prompt_argument_is_present() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("-m")
        .arg("gpt-5.1")
        .arg("Summarize this concisely")
        .write_stdin("")
        .assert()
        .success();

    let request = response_mock.single_request();
    assert!(
        request.has_message_with_input_texts("user", |texts| texts
            == ["Summarize this concisely".to_string()]),
        "request should preserve the prompt when stdin is empty"
    );

    Ok(())
}
