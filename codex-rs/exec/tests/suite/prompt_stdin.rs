#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::str::contains;

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

    // echo "my output" | codex exec --skip-git-repo-check -C <cwd> -m gpt-5.1 "Summarize this concisely"
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

    // printf "" | codex exec --skip-git-repo-check -C <cwd> -m gpt-5.1 "Summarize this concisely"
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_dash_prompt_reads_stdin_as_the_prompt() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    // echo "prompt from stdin" | codex exec --skip-git-repo-check -C <cwd> -m gpt-5.1 -
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("-m")
        .arg("gpt-5.1")
        .arg("-")
        .write_stdin("prompt from stdin\n")
        .assert()
        .success();

    let request = response_mock.single_request();
    assert!(
        request.has_message_with_input_texts("user", |texts| {
            texts == ["prompt from stdin\n".to_string()]
        }),
        "dash prompt should preserve the existing forced-stdin behavior"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_without_prompt_argument_reads_piped_stdin_as_the_prompt() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    // echo "prompt from stdin" | codex exec --skip-git-repo-check -C <cwd> -m gpt-5.1
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("-m")
        .arg("gpt-5.1")
        .write_stdin("prompt from stdin\n")
        .assert()
        .success();

    let request = response_mock.single_request();
    assert!(
        request.has_message_with_input_texts("user", |texts| {
            texts == ["prompt from stdin\n".to_string()]
        }),
        "missing prompt argument should preserve the existing piped-stdin prompt behavior"
    );

    Ok(())
}

#[test]
fn exec_without_prompt_argument_rejects_empty_piped_stdin() {
    let test = test_codex_exec();

    // printf "" | codex exec --skip-git-repo-check -C <cwd>
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .write_stdin("")
        .assert()
        .code(1)
        .stderr(contains("No prompt provided via stdin."));
}

#[test]
fn exec_dash_prompt_rejects_empty_piped_stdin() {
    let test = test_codex_exec();

    // printf "" | codex exec --skip-git-repo-check -C <cwd> -
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("-")
        .write_stdin("")
        .assert()
        .code(1)
        .stderr(contains("No prompt provided via stdin."));
}
