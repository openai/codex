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
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use serde_json::Value;

fn text_user_input(text: &str) -> Value {
    json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": text } ]
    })
}

fn find_input_index(input: &[Value], expected: &Value, label: &str) -> usize {
    input
        .iter()
        .position(|item| item == expected)
        .unwrap_or_else(|| panic!("expected {label} in input"))
}

fn find_function_call_output_index(input: &[Value], call_id: &str) -> usize {
    input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .unwrap_or_else(|| panic!("function_call_output {call_id} item not found in input"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_user_input_is_inserted_after_tool_output() -> anyhow::Result<()> {
    let call_id = "call-mid-turn-input";
    let first_text = "first message";
    let mid_text = "mid-turn input";

    let args = json!({
        "command": "sleep 2; echo finished",
        "timeout_ms": 10_000,
    })
    .to_string();
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-1"),
    ]);
    let follow_up_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);

    let server = start_mock_server().await;
    let first_mock = mount_sse_once(&server, first_response).await;
    let follow_up_mock = mount_sse_once(&server, follow_up_response).await;
    let test = test_codex().with_model("gpt-5.1").build(&server).await?;
    let codex = test.codex.clone();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_text.to_string(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| {
        matches!(ev, EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id)
    })
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: mid_text.to_string(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let first_request = first_mock.single_request();
    let first_input = first_request.input();
    assert_eq!(
        first_input,
        vec![text_user_input(first_text)],
        "expected first request input to match the initial message"
    );

    let follow_up = follow_up_mock.single_request();
    let input = follow_up.input();

    let first_message = text_user_input(first_text);
    let mid_message = text_user_input(mid_text);
    let first_index = find_input_index(&input, &first_message, "first user message");
    let mid_index = find_input_index(&input, &mid_message, "mid-turn user message");
    assert!(
        first_index < mid_index,
        "expected first message before mid-turn message"
    );

    let output_index = find_function_call_output_index(&input, call_id);
    assert!(
        output_index < mid_index,
        "expected tool output before mid-turn message"
    );

    let output = follow_up
        .function_call_output_text(call_id)
        .expect("missing function_call_output text");
    assert!(
        output.contains("finished"),
        "expected tool output to include \"finished\", got: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_user_turn_is_inserted_after_tool_output() -> anyhow::Result<()> {
    let call_id = "call-mid-turn-turn";
    let first_text = "first message";
    let mid_text = "mid-turn turn";

    let args = json!({
        "command": "sleep 2; echo finished",
        "timeout_ms": 10_000,
    })
    .to_string();
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-1"),
    ]);
    let follow_up_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);

    let server = start_mock_server().await;
    let first_mock = mount_sse_once(&server, first_response).await;
    let follow_up_mock = mount_sse_once(&server, follow_up_response).await;
    let test = test_codex().with_model("gpt-5.1").build(&server).await?;
    let codex = test.codex.clone();
    let cwd = test.cwd_path().to_path_buf();
    let model = test.session_configured.model.clone();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_text.to_string(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| {
        matches!(ev, EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id)
    })
    .await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: mid_text.to_string(),
            }],
            final_output_json_schema: None,
            cwd,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let first_request = first_mock.single_request();
    let first_input = first_request.input();
    assert_eq!(
        first_input,
        vec![text_user_input(first_text)],
        "expected first request input to match the initial message"
    );

    let follow_up = follow_up_mock.single_request();
    let input = follow_up.input();

    let first_message = text_user_input(first_text);
    let mid_message = text_user_input(mid_text);
    let first_index = find_input_index(&input, &first_message, "first user message");
    let mid_index = find_input_index(&input, &mid_message, "mid-turn user message");
    assert!(
        first_index < mid_index,
        "expected first message before mid-turn message"
    );

    let output_index = find_function_call_output_index(&input, call_id);
    assert!(
        output_index < mid_index,
        "expected tool output before mid-turn message"
    );

    let output = follow_up
        .function_call_output_text(call_id)
        .expect("missing function_call_output text");
    assert!(
        output.contains("finished"),
        "expected tool output to include \"finished\", got: {output}"
    );

    Ok(())
}
