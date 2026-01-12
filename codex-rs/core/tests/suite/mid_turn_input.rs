#![cfg(not(target_os = "windows"))]

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
enum MidTurnOp {
    UserInput,
    UserTurn,
}

fn call_output(req: &ResponsesRequest, call_id: &str) -> String {
    let raw = req.function_call_output(call_id);
    assert_eq!(
        raw.get("call_id").and_then(Value::as_str),
        Some(call_id),
        "mismatched call_id in function_call_output"
    );
    let (content_opt, _success) = match req.function_call_output_content_and_success(call_id) {
        Some(values) => values,
        None => panic!("function_call_output present"),
    };
    match content_opt {
        Some(content) => content,
        None => panic!("function_call_output content present"),
    }
}

async fn run_mid_turn_injection(op_kind: MidTurnOp) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5");
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "shell-tool-call";
    let command = vec!["/bin/sh", "-c", "sleep 2; echo finished"];
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_local_shell_call(call_id, "completed", command),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "follow up"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "first message".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::ExecCommandBegin(_))).await;

    let mid_turn_text = match op_kind {
        MidTurnOp::UserInput => "mid-turn input",
        MidTurnOp::UserTurn => "mid-turn turn",
    };

    match op_kind {
        MidTurnOp::UserInput => {
            codex
                .submit(Op::UserInput {
                    items: vec![UserInput::Text {
                        text: mid_turn_text.to_string(),
                    }],
                    final_output_json_schema: None,
                })
                .await?;
        }
        MidTurnOp::UserTurn => {
            codex
                .submit(Op::UserTurn {
                    items: vec![UserInput::Text {
                        text: mid_turn_text.to_string(),
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
        }
    }

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let req = second_mock.single_request();
    let output_text = call_output(&req, call_id);
    let exec_output: Value = serde_json::from_str(&output_text)?;
    let stdout = exec_output["output"].as_str().expect("stdout field");
    assert_eq!(stdout.trim(), "finished");

    let user_messages = req.message_input_texts("user");
    assert_eq!(
        user_messages,
        vec!["first message".to_string(), mid_turn_text.to_string()]
    );

    let input = req.input();
    let call_idx = input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("function_call_output item");
    let user_idx = input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("message")
                && item.get("role").and_then(Value::as_str) == Some("user")
                && item
                    .get("content")
                    .and_then(Value::as_array)
                    .map(|content| {
                        content.iter().any(|span| {
                            span.get("type").and_then(Value::as_str) == Some("input_text")
                                && span.get("text").and_then(Value::as_str) == Some(mid_turn_text)
                        })
                    })
                    .unwrap_or(false)
        })
        .expect("mid-turn user message");
    assert!(call_idx < user_idx, "expected tool output before mid-turn input");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_user_input_is_injected_after_tool_call() -> anyhow::Result<()> {
    run_mid_turn_injection(MidTurnOp::UserInput).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_user_turn_is_injected_after_tool_call() -> anyhow::Result<()> {
    run_mid_turn_injection(MidTurnOp::UserTurn).await
}
