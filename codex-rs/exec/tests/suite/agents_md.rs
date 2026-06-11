#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_surfaces_project_instruction_loading_warnings() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let project_agents_path = test.cwd_path().join("AGENTS.md");
    std::fs::write(&project_agents_path, b"project\xFFinstructions")?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .success()
        .stderr(contains("invalid UTF-8").and(contains(project_agents_path.display().to_string())));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_json_surfaces_project_instruction_loading_warnings() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let project_agents_path = test.cwd_path().join("AGENTS.md");
    std::fs::write(&project_agents_path, b"project\xFFinstructions")?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    let output = test
        .cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--json")
        .arg("tell me something")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let events = String::from_utf8(output)?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;

    assert!(
        events.iter().any(|event| {
            event["type"] == "item.completed"
                && event["item"]["type"] == "error"
                && event["item"]["message"].as_str().is_some_and(|message| {
                    message.contains("invalid UTF-8")
                        && message.contains(project_agents_path.display().to_string().as_str())
                })
        }),
        "expected a JSONL warning event; observed: {events:?}"
    );

    Ok(())
}
