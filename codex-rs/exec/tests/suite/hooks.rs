#![cfg(not(target_os = "windows"))]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_hook_trust_bypass_runs_session_start_hook() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let marker_path = test.home_path().join("session-start-ran");
    let command = format!("touch {}", marker_path.display());
    std::fs::write(
        test.home_path().join("hooks.json"),
        serde_json::to_vec_pretty(&json!({
            "hooks": {
                "SessionStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": command,
                    }],
                }],
            },
        }))?,
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "done"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("--dangerously-bypass-hook-trust")
        .arg("run the session start hook")
        .assert()
        .success();

    assert!(marker_path.exists(), "session start hook did not run");
    Ok(())
}
