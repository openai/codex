#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::str::contains;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exits_non_zero_when_required_mcp_server_fails_to_initialize() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let config_toml = r#"
        [mcp_servers.required_broken]
        command = "codex-definitely-not-a-real-binary"
        required = true
    "#;
    std::fs::write(test.home_path().join("config.toml"), config_toml)?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp_1"),
        responses::ev_assistant_message("msg_1", "hello"),
        responses::ev_completed("resp_1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--experimental-json")
        .arg("tell me something")
        .assert()
        .code(1)
        .stderr(contains(
            "required MCP servers failed to initialize: required_broken",
        ));

    Ok(())
}
