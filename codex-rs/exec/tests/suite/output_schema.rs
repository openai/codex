#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::Value;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_includes_output_schema_in_request() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let schema_contents = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    });
    let schema_path = test.cwd_path().join("schema.json");
    std::fs::write(&schema_path, serde_json::to_vec_pretty(&schema_contents)?)?;
    let expected_schema: Value = schema_contents;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp1"),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    responses::mount_sse_once_match(&server, any(), body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        // keep using -C in the test to exercise the flag as well
        .arg("-C")
        .arg(test.cwd_path())
        .arg("--output-schema")
        .arg(&schema_path)
        .arg("-m")
        .arg("gpt-5")
        .arg("tell me a joke")
        .assert()
        .success();

    let requests = server
        .received_requests()
        .await
        .expect("failed to capture requests");
    assert_eq!(requests.len(), 1, "expected exactly one request");
    let payload: Value = serde_json::from_slice(&requests[0].body)?;
    let text = payload.get("text").expect("request missing text field");
    let format = text
        .get("format")
        .expect("request missing text.format field");
    assert_eq!(
        format,
        &serde_json::json!({
            "name": "codex_output_schema",
            "type": "json_schema",
            "strict": true,
            "schema": expected_schema,
        })
    );

    Ok(())
}
