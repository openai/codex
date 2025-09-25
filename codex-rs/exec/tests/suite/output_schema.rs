#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use assert_cmd::prelude::*;
use core_test_support::responses;
use serde_json::Value;
use std::process::Command;
use tempfile::TempDir;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_includes_output_schema_in_request() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let workspace = TempDir::new()?;

    let schema_contents = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    });
    let schema_path = workspace.path().join("schema.json");
    std::fs::write(&schema_path, serde_json::to_vec_pretty(&schema_contents)?)?;
    let expected_schema: Value = schema_contents;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp1"}
        }),
        responses::ev_assistant_message("m1", "fixture hello"),
        responses::ev_completed("resp1"),
    ]);
    responses::mount_sse_once(&server, any(), body).await;

    Command::cargo_bin("codex-exec")?
        .current_dir(workspace.path())
        .env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()))
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(workspace.path())
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
