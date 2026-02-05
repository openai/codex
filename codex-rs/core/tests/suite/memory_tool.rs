#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_memory_tool_returns_persisted_thread_memory() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Sqlite);
        config.features.enable(Feature::MemoryTool);
    });
    let test = builder.build(&server).await?;

    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.session_id;
    let thread_id_string = thread_id.to_string();

    let mut thread_exists = false;
    // Wait for DB creation.
    for _ in 0..100 {
        if db.get_thread(thread_id).await?.is_some() {
            thread_exists = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(thread_exists, "thread should exist in state db");

    let trace_summary = "trace summary from sqlite";
    let memory_summary = "memory summary from sqlite";
    db.upsert_thread_memory(thread_id, trace_summary, memory_summary)
        .await?;

    let call_id = "memory-call-1";
    let arguments = json!({
        "memory_id": thread_id_string,
    })
    .to_string();
    let mocks =
        mount_function_call_agent_response(&server, call_id, &arguments, "get_memory").await;

    test.submit_turn("load the saved memory").await?;

    let initial_request = mocks.function_call.single_request().body_json();
    assert!(
        initial_request["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .any(|name| name == "get_memory"),
        "get_memory tool should be exposed when memory_tool feature is enabled"
    );

    let completion_request = mocks.completion.single_request();
    let (content_opt, success_opt) = completion_request
        .function_call_output_content_and_success(call_id)
        .expect("function_call_output should be present");
    let success = success_opt.unwrap_or(true);
    assert!(success, "expected successful get_memory tool call output");
    let content = content_opt.expect("function_call_output content should be present");
    let payload: Value = serde_json::from_str(&content)?;
    assert_eq!(
        payload,
        json!({
            "memory_id": thread_id_string,
            "trace_summary": trace_summary,
            "memory_summary": memory_summary,
        })
    );

    Ok(())
}
