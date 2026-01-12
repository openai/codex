use codex_core::features::Feature;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use std::sync::Arc;

fn long_running_exec_command_with_output() -> (String, Option<&'static str>) {
    if cfg!(windows) {
        (
            "Start-Sleep -Milliseconds 200; Write-Output 'partial output'; Start-Sleep -Seconds 60"
                .to_string(),
            Some("powershell"),
        )
    } else {
        (
            "sleep 0.2; printf 'partial output\\n'; sleep 60".to_string(),
            None,
        )
    }
}

fn long_running_shell_command_with_output() -> Vec<String> {
    if cfg!(windows) {
        vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Start-Sleep -Milliseconds 200; Write-Output 'partial output'; Start-Sleep -Seconds 60"
                .to_string(),
        ]
    } else {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 0.2; printf 'partial output\\n'; sleep 60".to_string(),
        ]
    }
}

fn long_running_local_shell_call_with_output() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "Start-Sleep -Milliseconds 200; Write-Output 'partial output'; Start-Sleep -Seconds 60",
        ]
    } else {
        vec![
            "/bin/sh",
            "-c",
            "sleep 0.2; printf 'partial output\\n'; sleep 60",
        ]
    }
}

fn saw_partial_output(call_id: &str, event: &EventMsg) -> bool {
    let EventMsg::ExecCommandOutputDelta(delta) = event else {
        return false;
    };
    delta.call_id == call_id && String::from_utf8_lossy(&delta.chunk).contains("partial output")
}

fn assert_aborted_output(output: &str) {
    let normalized_output = output.replace("\r\n", "\n").replace('\r', "\n");
    let normalized_output = normalized_output.trim_end_matches('\n');
    let expected_pattern = r"(?s)^Exit code: [0-9]+\nWall time: ([0-9]+(?:\.[0-9]+)?) seconds\nOutput:\npartial output\ncommand aborted by user$";
    let captures = assert_regex_match(expected_pattern, normalized_output);
    let secs: f32 = match captures.get(1) {
        Some(value) => match value.as_str().parse() {
            Ok(secs) => secs,
            Err(err) => panic!("failed to parse wall time seconds: {err}"),
        },
        None => panic!("aborted message with elapsed seconds"),
    };
    assert!(secs >= 0.0);
}

fn assert_unified_exec_aborted_output(output: &str) {
    let normalized_output = output.replace("\r\n", "\n").replace('\r', "\n");
    let normalized_output = normalized_output.trim_end_matches('\n');
    let expected_pattern = concat!(
        r#"(?s)^(?:Total output lines: \d+\n\n)?"#,
        r#"(?:Chunk ID: [^\n]+\n)?"#,
        r#"Wall time: (-?[0-9]+(?:\.[0-9]+)?) seconds\n"#,
        r#"(?:Process exited with code -?\d+\n)?"#,
        r#"(?:Process running with session ID -?\d+\n)?"#,
        r#"(?:Original token count: \d+\n)?"#,
        r#"Output:\npartial output\ncommand aborted by user$"#,
    );
    let captures = assert_regex_match(expected_pattern, normalized_output);
    let secs: f64 = match captures.get(1) {
        Some(value) => match value.as_str().parse() {
            Ok(secs) => secs,
            Err(err) => panic!("failed to parse unified exec wall time seconds: {err}"),
        },
        None => panic!("unified exec aborted message with elapsed seconds"),
    };
    assert!(secs >= 0.0);
}

/// Integration test: spawn a long‑running shell_command tool via a mocked Responses SSE
/// function call, then interrupt the session and expect TurnAborted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_long_running_tool_emits_turn_aborted() {
    let command = "sleep 60";

    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let body = sse(vec![
        ev_function_call("call_sleep", "shell_command", &args),
        ev_completed("done"),
    ]);

    let server = start_mock_server().await;
    mount_sse_once(&server, body).await;

    let codex = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap()
        .codex;

    // Kick off a turn that triggers the function call.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start sleep".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    // Wait until the exec begins to avoid a race, then interrupt.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    codex.submit(Op::Interrupt).await.unwrap();

    // Expect TurnAborted soon after.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;
}

/// After an interrupt we expect the next request to the model to include both
/// the original tool call and an `"aborted"` `function_call_output`. This test
/// exercises the follow-up flow: it sends another user turn, inspects the mock
/// responses server, and ensures the model receives the synthesized abort.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_tool_records_history_entries() {
    let (command, _shell) = long_running_exec_command_with_output();
    let call_id = "call-history";

    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-history"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-history"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start history recording".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;
    wait_for_event(&codex, |ev| saw_partial_output(call_id, ev)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert!(
        requests.len() == 2,
        "expected two calls to the responses API, got {}",
        requests.len()
    );

    assert!(
        response_mock.saw_function_call(call_id),
        "function call not recorded in responses payload"
    );
    let output = response_mock
        .function_call_output_text(call_id)
        .expect("missing function_call_output text");
    assert_aborted_output(&output);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_shell_records_partial_output() {
    let command = long_running_shell_command_with_output();
    let call_id = "call-shell";

    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-shell"),
        ev_function_call(call_id, "shell", &args),
        ev_completed("resp-shell"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-shell-followup"),
        ev_completed("resp-shell-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start shell".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;
    wait_for_event(&codex, |ev| saw_partial_output(call_id, ev)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output = response_mock
        .function_call_output_text(call_id)
        .expect("missing shell output");
    assert_aborted_output(&output);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_local_shell_records_partial_output() {
    let command = long_running_local_shell_call_with_output();
    let call_id = "call-local-shell";

    let first_body = sse(vec![
        ev_response_created("resp-local-shell"),
        ev_local_shell_call(call_id, "completed", command),
        ev_completed("resp-local-shell"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-local-shell-followup"),
        ev_completed("resp-local-shell-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start local shell".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;
    wait_for_event(&codex, |ev| saw_partial_output(call_id, ev)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output = response_mock
        .function_call_output_text(call_id)
        .expect("missing local shell output");
    assert_aborted_output(&output);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_unified_exec_records_partial_output() {
    let call_id = "call-unified-exec";
    let (cmd, shell) = long_running_exec_command_with_output();
    let args = if let Some(shell) = shell {
        json!({
            "cmd": cmd,
            "shell": shell,
            "yield_time_ms": 60_000,
        })
        .to_string()
    } else {
        json!({
            "cmd": cmd,
            "yield_time_ms": 60_000,
        })
        .to_string()
    };
    let first_body = sse(vec![
        ev_response_created("resp-unified-exec"),
        ev_function_call(call_id, "exec_command", &args),
        ev_completed("resp-unified-exec"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-unified-exec-followup"),
        ev_completed("resp-unified-exec-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .with_config(|config| {
            config.features.enable(Feature::UnifiedExec);
        })
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start unified exec".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;
    wait_for_event(&codex, |ev| saw_partial_output(call_id, ev)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output = response_mock
        .function_call_output_text(call_id)
        .expect("missing exec_command output");
    assert_unified_exec_aborted_output(&output);
}
