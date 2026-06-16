#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_includes_workspace_agents_md_in_request() -> anyhow::Result<()> {
    let test = test_codex_exec();
    std::fs::write(test.cwd_path().join("AGENTS.md"), "workspace instructions")?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .success();

    let user_messages = response_mock.single_request().message_input_texts("user");
    assert!(
        user_messages
            .iter()
            .any(|text| text.contains("workspace instructions")),
        "request should include workspace AGENTS.md instructions: {user_messages:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_prefers_workspace_agents_override_md() -> anyhow::Result<()> {
    let test = test_codex_exec();
    std::fs::write(test.cwd_path().join("AGENTS.md"), "base instructions")?;
    std::fs::write(
        test.cwd_path().join("AGENTS.override.md"),
        "override instructions",
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .success();

    let user_messages = response_mock.single_request().message_input_texts("user");
    assert!(
        user_messages
            .iter()
            .any(|text| text.contains("override instructions")),
        "request should include AGENTS.override.md instructions: {user_messages:?}"
    );
    assert!(
        user_messages
            .iter()
            .all(|text| !text.contains("base instructions")),
        "request should exclude shadowed AGENTS.md instructions: {user_messages:?}"
    );

    Ok(())
}
