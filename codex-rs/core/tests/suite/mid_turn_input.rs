#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[derive(Clone, Copy, Debug)]
enum MidTurnOp {
    UserInput,
    UserTurn,
}

fn message_contains_text(item: &Value, text: &str) -> bool {
    item.get("type").and_then(Value::as_str) == Some("message")
        && item.get("role").and_then(Value::as_str) == Some("user")
        && item
            .get("content")
            .and_then(Value::as_array)
            .map(|content| {
                content.iter().any(|span| {
                    span.get("type").and_then(Value::as_str) == Some("input_text")
                        && span.get("text").and_then(Value::as_str) == Some(text)
                })
            })
            .unwrap_or(false)
}

async fn run_mid_turn_injection_test(mid_turn_op: MidTurnOp) -> Result<()> {
    let harness = TestCodexHarness::new().await?;
    let test = harness.test();
    let codex = test.codex.clone();
    let session_model = test.session_configured.model.clone();
    let cwd = test.cwd_path().to_path_buf();

    let call_id = "shell-mid-turn";
    let first_message = "first message";
    let mid_turn_message = "mid-turn message";
    let workdir = cwd.to_string_lossy().to_string();

    let args = json!({
        "command": ["bash", "-lc", "sleep 2; echo finished"],
        "workdir": workdir,
        "timeout_ms": 10_000,
    });

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
        ev_completed("resp-1"),
    ]);
    let second_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "follow up"),
        ev_completed("resp-2"),
    ]);

    mount_sse_once(harness.server(), first_response).await;
    let request_log = mount_sse_once(harness.server(), second_response).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: first_message.to_string(),
            }],
            final_output_json_schema: None,
            cwd: cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let _ = wait_for_event_match(&codex, |event| match event {
        EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    match mid_turn_op {
        MidTurnOp::UserInput => {
            codex
                .submit(Op::UserInput {
                    items: vec![UserInput::Text {
                        text: mid_turn_message.to_string(),
                    }],
                    final_output_json_schema: None,
                })
                .await?;
        }
        MidTurnOp::UserTurn => {
            codex
                .submit(Op::UserTurn {
                    items: vec![UserInput::Text {
                        text: mid_turn_message.to_string(),
                    }],
                    final_output_json_schema: None,
                    cwd: cwd.clone(),
                    approval_policy: AskForApproval::Never,
                    sandbox_policy: SandboxPolicy::DangerFullAccess,
                    model: session_model,
                    effort: None,
                    summary: ReasoningSummary::Auto,
                })
                .await?;
        }
    }

    let end_event = wait_for_event_match(&codex, |event| match event {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;
    assert_eq!(end_event.exit_code, 0);
    assert!(
        end_event.stdout.contains("finished"),
        "expected stdout to include finished: {}",
        end_event.stdout
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let request = request_log.single_request();
    let user_messages = request.message_input_texts("user");
    assert_eq!(
        user_messages,
        vec![first_message.to_string(), mid_turn_message.to_string()]
    );

    let input = request.input();
    let tool_index = input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("expected function_call_output in request");
    let mid_turn_index = input
        .iter()
        .position(|item| message_contains_text(item, mid_turn_message))
        .expect("expected mid-turn user message in request");
    assert!(
        tool_index < mid_turn_index,
        "expected tool output before mid-turn input"
    );

    let tool_output = request
        .function_call_output_text(call_id)
        .expect("expected function_call_output output text");
    assert!(
        tool_output.contains("finished"),
        "expected tool output to include finished: {tool_output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_input_inserts_user_input_after_tool_output() -> Result<()> {
    run_mid_turn_injection_test(MidTurnOp::UserInput).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_input_inserts_user_turn_after_tool_output() -> Result<()> {
    run_mid_turn_injection_test(MidTurnOp::UserTurn).await
}
