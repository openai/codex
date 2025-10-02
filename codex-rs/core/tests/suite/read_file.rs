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
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_file_tool_returns_requested_lines() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let file_path = cwd.path().join("sample.txt");
    std::fs::write(&file_path, "first\nsecond\nthird\nfourth\n")?;
    let file_path = file_path.to_string_lossy().to_string();

    let call_id = "read-file-call";
    let arguments = serde_json::json!({
        "file_path": file_path,
        "offset": 2,
        "limit": 2,
    })
    .to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "read_file", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please inspect sample.txt".into(),
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

    let requests = server.received_requests().await.expect("recorded requests");
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().unwrap())
        .collect::<Vec<_>>();
    assert!(
        !request_bodies.is_empty(),
        "expected at least one request body"
    );

    let tool_output_item = request_bodies
        .iter()
        .find_map(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    })
                })
        })
        .unwrap_or_else(|| {
            panic!("function_call_output item not found in requests: {request_bodies:#?}")
        });

    assert_eq!(
        tool_output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let output_text = tool_output_item
        .get("output")
        .and_then(|value| match value {
            Value::String(text) => Some(text.as_str()),
            Value::Object(obj) => obj.get("content").and_then(Value::as_str),
            _ => None,
        })
        .expect("output text present");
    assert_eq!(output_text, "L2: second\nL3: third");

    Ok(())
}
