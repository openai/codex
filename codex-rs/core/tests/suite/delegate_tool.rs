#![cfg(not(target_os = "windows"))]

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use wiremock::matchers::any;

/// The model returns a single `delegate_agent` call that carries a `batch` array.
/// We expect the tool handler to execute each entry sequentially and return a
/// single aggregated response containing both runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delegate_tool_handles_batch_requests() -> anyhow::Result<()> {
    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex()
        .with_config(|config| {
            config.include_delegate_tool = true;
        })
        .build(&server)
        .await?;

    let call_id = "delegate-batch-call";
    let batch_args = json!({
        "batch": [
            {"agent_id": "alpha", "prompt": "first"},
            {"agent_id": "bravo", "prompt": "second"}
        ]
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "delegate_agent", &batch_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "batch done"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please delegate to two agents".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = second_mock.single_request();
    let output_item = request.function_call_output(call_id);
    dbg!(&output_item);
    let content = output_item
        .get("output")
        .and_then(|value| match value {
            serde_json::Value::String(text) => Some(text.as_str()),
            serde_json::Value::Object(obj) => obj.get("content").and_then(|v| v.as_str()),
            _ => None,
        })
        .expect("batch response text");

    let parsed: serde_json::Value = serde_json::from_str(content)?;
    assert_eq!(parsed["status"], "ok");
    let runs = parsed["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["agent_id"], "alpha");
    assert_eq!(runs[1]["agent_id"], "bravo");
    assert!(
        runs.iter()
            .all(|run| run.get("run_id").and_then(|v| v.as_str()).is_some())
    );

    Ok(())
}
