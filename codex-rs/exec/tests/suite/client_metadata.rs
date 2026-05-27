#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::str::contains;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_passes_responsesapi_client_metadata_to_turn_header() -> anyhow::Result<()> {
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
        .arg("--responsesapi-client-metadata")
        .arg("usage_source=chronicle")
        .arg("--responsesapi-client-metadata")
        .arg("feature=memory_summary")
        .arg("tell me something")
        .assert()
        .success();

    let request = response_mock.single_request();
    let header = request
        .header("x-codex-turn-metadata")
        .expect("request missing x-codex-turn-metadata header");
    let metadata: Value = serde_json::from_str(&header)?;

    assert_eq!(metadata["usage_source"].as_str(), Some("chronicle"));
    assert_eq!(metadata["feature"].as_str(), Some("memory_summary"));
    assert!(metadata.get("turn_id").is_some());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_rejects_responsesapi_client_metadata() -> anyhow::Result<()> {
    let test = test_codex_exec();

    test.cmd()
        .arg("review")
        .arg("--uncommitted")
        .arg("--responsesapi-client-metadata")
        .arg("usage_source=chronicle")
        .assert()
        .failure()
        .stderr(contains(
            "--responsesapi-client-metadata is only supported for exec user turns",
        ));

    Ok(())
}
