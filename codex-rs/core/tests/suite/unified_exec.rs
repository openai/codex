#![cfg(not(target_os = "windows"))]

use std::collections::HashMap;

use anyhow::Result;
use codex_core::features::Feature;
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
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_event_with_timeout;
use serde_json::Value;
use serde_json::json;

fn extract_output_text(item: &Value) -> Option<&str> {
    item.get("output").and_then(|value| match value {
        Value::String(text) => Some(text.as_str()),
        Value::Object(obj) => obj.get("content").and_then(Value::as_str),
        _ => None,
    })
}

fn collect_tool_outputs(bodies: &[Value]) -> Result<HashMap<String, Value>> {
    let mut outputs = HashMap::new();
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) != Some("function_call_output") {
                    continue;
                }
                if let Some(call_id) = item.get("call_id").and_then(Value::as_str) {
                    let content = extract_output_text(item)
                        .ok_or_else(|| anyhow::anyhow!("missing tool output content"))?;
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let parsed: Value = serde_json::from_str(trimmed).map_err(|err| {
                        anyhow::anyhow!("failed to parse tool output content {trimmed:?}: {err}")
                    })?;
                    outputs.insert(call_id.to_string(), parsed);
                }
            }
        }
    }
    Ok(outputs)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_exec_command_begin_event() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "uexec-begin-event";
    let args = json!({
        "cmd": "/bin/echo hello unified exec".to_string(),
        "yield_time_ms": 250,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "finished"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "emit begin event".into(),
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

    let begin_event = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandBegin(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(
        begin_event.command,
        vec!["/bin/echo hello unified exec".to_string()]
    );
    assert_eq!(begin_event.cwd, cwd.path());

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_exec_command_end_event() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "uexec-end-event";
    let args = json!({
        "cmd": "/bin/echo END-EVENT".to_string(),
        "yield_time_ms": 250,
    });
    let poll_call_id = "uexec-end-event-poll";
    let poll_args = json!({
        "chars": "",
        "session_id": 0,
        "yield_time_ms": 250,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                poll_call_id,
                "write_stdin",
                &serde_json::to_string(&poll_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-1", "finished"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "emit end event".into(),
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

    let end_event = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    assert_eq!(end_event.exit_code, 0);
    assert!(
        end_event.aggregated_output.contains("END-EVENT"),
        "expected aggregated output to contain marker"
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_output_delta_for_exec_command() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "uexec-delta-1";
    let args = json!({
        "cmd": "printf 'HELLO-UEXEC'",
        "yield_time_ms": 250,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "finished"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "emit delta".into(),
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

    let delta = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandOutputDelta(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    let text = String::from_utf8_lossy(&delta.chunk).to_string();
    assert!(
        text.contains("HELLO-UEXEC"),
        "delta chunk missing expected text: {text:?}"
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_output_delta_for_write_stdin() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let open_call_id = "uexec-open";
    let open_args = json!({
        "cmd": "/bin/bash -i",
        "yield_time_ms": 200,
    });

    let stdin_call_id = "uexec-stdin-delta";
    let stdin_args = json!({
        "chars": "echo WSTDIN-MARK\\n",
        "session_id": 0,
        "yield_time_ms": 800,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                open_call_id,
                "exec_command",
                &serde_json::to_string(&open_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                stdin_call_id,
                "write_stdin",
                &serde_json::to_string(&stdin_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "stdin delta".into(),
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

    // Expect a delta event corresponding to the write_stdin call.
    let delta = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandOutputDelta(ev) if ev.call_id == open_call_id => {
            let text = String::from_utf8_lossy(&ev.chunk);
            if text.contains("WSTDIN-MARK") {
                Some(ev.clone())
            } else {
                None
            }
        }
        _ => None,
    })
    .await;

    let text = String::from_utf8_lossy(&delta.chunk).to_string();
    assert!(
        text.contains("WSTDIN-MARK"),
        "stdin delta chunk missing expected text: {text:?}"
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_skips_begin_event_for_empty_input() -> Result<()> {
    use tokio::time::Duration;

    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let open_call_id = "uexec-open-session";
    let open_args = json!({
        "cmd": "/bin/sh -c echo ready".to_string(),
        "yield_time_ms": 250,
    });

    let poll_call_id = "uexec-poll-empty";
    let poll_args = json!({
        "input": Vec::<String>::new(),
        "session_id": "0",
        "timeout_ms": 150,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                open_call_id,
                "exec_command",
                &serde_json::to_string(&open_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                poll_call_id,
                "write_stdin",
                &serde_json::to_string(&poll_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-1", "complete"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "check poll event behavior".into(),
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

    let mut begin_events = Vec::new();
    loop {
        let event_msg = wait_for_event_with_timeout(&codex, |_| true, Duration::from_secs(2)).await;
        match event_msg {
            EventMsg::ExecCommandBegin(event) => begin_events.push(event),
            EventMsg::TaskComplete(_) => break,
            _ => {}
        }
    }

    assert_eq!(
        begin_events.len(),
        1,
        "expected only the initial command to emit begin event"
    );
    assert_eq!(begin_events[0].call_id, open_call_id);
    assert_eq!(begin_events[0].command[0], "/bin/sh -c echo ready");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_reports_chunk_and_exit_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "uexec-metadata";
    let args = serde_json::json!({
        "cmd": "printf 'abcdefghijklmnopqrstuvwxyz'",
        "yield_time_ms": 500,
        "max_output_tokens": 6,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "run metadata test".into(),
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

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let outputs = collect_tool_outputs(&bodies)?;
    let metadata = outputs
        .get(call_id)
        .expect("missing exec_command metadata output");

    let chunk_id = metadata
        .get("chunk_id")
        .and_then(Value::as_str)
        .expect("missing chunk_id");
    assert_eq!(chunk_id.len(), 6, "chunk id should be 6 hex characters");
    assert!(
        chunk_id.chars().all(|c| c.is_ascii_hexdigit()),
        "chunk id should be hexadecimal: {chunk_id}"
    );

    let wall_time = metadata
        .get("wall_time_seconds")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    assert!(
        wall_time >= 0.0,
        "wall_time_seconds should be non-negative, got {wall_time}"
    );

    assert!(
        metadata.get("session_id").is_none(),
        "exec_command for a completed process should not include session_id"
    );

    let exit_code = metadata
        .get("exit_code")
        .and_then(Value::as_i64)
        .expect("expected exit_code");
    assert_eq!(exit_code, 0, "expected successful exit");

    let output_text = metadata
        .get("output")
        .and_then(Value::as_str)
        .expect("missing output text");
    assert!(
        output_text.contains("tokens truncated"),
        "expected truncation notice in output: {output_text:?}"
    );

    let original_tokens = metadata
        .get("original_token_count")
        .and_then(Value::as_u64)
        .expect("missing original_token_count");
    assert!(
        original_tokens as usize > 6,
        "original token count should exceed max_output_tokens"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn write_stdin_returns_exit_metadata_and_clears_session() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let start_call_id = "uexec-cat-start";
    let send_call_id = "uexec-cat-send";
    let exit_call_id = "uexec-cat-exit";

    let start_args = serde_json::json!({
        "cmd": "/bin/cat",
        "yield_time_ms": 500,
    });
    let send_args = serde_json::json!({
        "chars": "hello unified exec\n",
        "session_id": 0,
        "yield_time_ms": 500,
    });
    let exit_args = serde_json::json!({
        "chars": "\u{0004}",
        "session_id": 0,
        "yield_time_ms": 500,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                start_call_id,
                "exec_command",
                &serde_json::to_string(&start_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                send_call_id,
                "write_stdin",
                &serde_json::to_string(&send_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_function_call(
                exit_call_id,
                "write_stdin",
                &serde_json::to_string(&exit_args)?,
            ),
            ev_completed("resp-3"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "all done"),
            ev_completed("resp-4"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "test write_stdin exit behavior".into(),
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

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let outputs = collect_tool_outputs(&bodies)?;

    let start_output = outputs
        .get(start_call_id)
        .expect("missing start output for exec_command");
    let session_id = start_output
        .get("session_id")
        .and_then(Value::as_i64)
        .expect("expected session id from exec_command");
    assert!(
        session_id >= 0,
        "session_id should be non-negative, got {session_id}"
    );
    assert!(
        start_output.get("exit_code").is_none(),
        "initial exec_command should not include exit_code while session is running"
    );

    let send_output = outputs
        .get(send_call_id)
        .expect("missing write_stdin echo output");
    let echoed = send_output
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        echoed.contains("hello unified exec"),
        "expected echoed output from cat, got {echoed:?}"
    );
    let echoed_session = send_output
        .get("session_id")
        .and_then(Value::as_i64)
        .expect("write_stdin should return session id while process is running");
    assert_eq!(
        echoed_session, session_id,
        "write_stdin should reuse existing session id"
    );
    assert!(
        send_output.get("exit_code").is_none(),
        "write_stdin should not include exit_code while process is running"
    );

    let exit_output = outputs
        .get(exit_call_id)
        .expect("missing exit metadata output");
    assert!(
        exit_output.get("session_id").is_none(),
        "session_id should be omitted once the process exits"
    );
    let exit_code = exit_output
        .get("exit_code")
        .and_then(Value::as_i64)
        .expect("expected exit_code after sending EOF");
    assert_eq!(exit_code, 0, "cat should exit cleanly after EOF");

    let exit_chunk = exit_output
        .get("chunk_id")
        .and_then(Value::as_str)
        .expect("missing chunk id for exit output");
    assert!(
        exit_chunk.chars().all(|c| c.is_ascii_hexdigit()),
        "chunk id should be hexadecimal: {exit_chunk}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_end_event_when_session_dies_via_stdin() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let start_call_id = "uexec-end-on-exit-start";
    let start_args = serde_json::json!({
        "cmd": "/bin/cat",
        "yield_time_ms": 200,
    });

    let echo_call_id = "uexec-end-on-exit-echo";
    let echo_args = serde_json::json!({
        "chars": "bye-END\n",
        "session_id": 0,
        "yield_time_ms": 300,
    });

    let exit_call_id = "uexec-end-on-exit";
    let exit_args = serde_json::json!({
        "chars": "\u{0004}",
        "session_id": 0,
        "yield_time_ms": 500,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                start_call_id,
                "exec_command",
                &serde_json::to_string(&start_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                echo_call_id,
                "write_stdin",
                &serde_json::to_string(&echo_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_function_call(
                exit_call_id,
                "write_stdin",
                &serde_json::to_string(&exit_args)?,
            ),
            ev_completed("resp-3"),
        ]),
        sse(vec![
            ev_response_created("resp-4"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-4"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "end on exit".into(),
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

    // We expect the ExecCommandEnd event to match the initial exec_command call_id.
    let end_event = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == start_call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    assert_eq!(end_event.exit_code, 0);

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_reuses_session_via_stdin() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let first_call_id = "uexec-start";
    let first_args = serde_json::json!({
        "cmd": "/bin/cat",
        "yield_time_ms": 200,
    });

    let second_call_id = "uexec-stdin";
    let second_args = serde_json::json!({
        "chars": "hello unified exec\n",
        "session_id": 0,
        "yield_time_ms": 500,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_call_id,
                "exec_command",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                second_call_id,
                "write_stdin",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "all done"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "run unified exec".into(),
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

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let outputs = collect_tool_outputs(&bodies)?;

    let start_output = outputs
        .get(first_call_id)
        .expect("missing first unified_exec output");
    let session_id = start_output["session_id"].as_i64().unwrap_or_default();
    assert!(
        session_id >= 0,
        "expected session id in first unified_exec response"
    );
    assert!(
        start_output["output"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
    );

    let reuse_output = outputs
        .get(second_call_id)
        .expect("missing reused unified_exec output");
    assert_eq!(
        reuse_output["session_id"].as_i64().unwrap_or_default(),
        session_id
    );
    let echoed = reuse_output["output"].as_str().unwrap_or_default();
    assert!(
        echoed.contains("hello unified exec"),
        "expected echoed output, got {echoed:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_streams_after_lagged_output() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let script = r#"python3 - <<'PY'
import sys
import time

chunk = b'x' * (1 << 20)
for _ in range(4):
    sys.stdout.buffer.write(chunk)
    sys.stdout.flush()

time.sleep(0.2)
for _ in range(5):
    sys.stdout.write("TAIL-MARKER\n")
    sys.stdout.flush()
    time.sleep(0.05)

time.sleep(0.2)
PY
"#;

    let first_call_id = "uexec-lag-start";
    let first_args = serde_json::json!({
        "cmd": script,
        "yield_time_ms": 25,
    });

    let second_call_id = "uexec-lag-poll";
    let second_args = serde_json::json!({
        "chars": "",
        "session_id": 0,
        "yield_time_ms": 2_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_call_id,
                "exec_command",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                second_call_id,
                "write_stdin",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "lag handled"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "exercise lag handling".into(),
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

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let outputs = collect_tool_outputs(&bodies)?;

    let start_output = outputs
        .get(first_call_id)
        .expect("missing initial unified_exec output");
    let session_id = start_output["session_id"].as_i64().unwrap_or_default();
    assert!(
        session_id >= 0,
        "expected session id from initial unified_exec response"
    );

    let poll_output = outputs
        .get(second_call_id)
        .expect("missing poll unified_exec output");
    let poll_text = poll_output["output"].as_str().unwrap_or_default();
    assert!(
        poll_text.contains("TAIL-MARKER"),
        "expected poll output to contain tail marker, got {poll_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_timeout_and_followup_poll() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let first_call_id = "uexec-timeout";
    let first_args = serde_json::json!({
        "cmd": "sleep 0.5; echo ready",
        "yield_time_ms": 10,
    });

    let second_call_id = "uexec-poll";
    let second_args = serde_json::json!({
        "chars": "",
        "session_id": 0,
        "yield_time_ms": 800,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_call_id,
                "exec_command",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                second_call_id,
                "write_stdin",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "check timeout".into(),
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

    loop {
        let event = codex.next_event().await.expect("event");
        if matches!(event.msg, EventMsg::TaskComplete(_)) {
            break;
        }
    }

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let outputs = collect_tool_outputs(&bodies)?;

    let first_output = outputs.get(first_call_id).expect("missing timeout output");
    assert_eq!(first_output["session_id"], 0);
    assert!(
        first_output["output"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
    );

    let poll_output = outputs.get(second_call_id).expect("missing poll output");
    let output_text = poll_output["output"].as_str().unwrap_or_default();
    assert!(
        output_text.contains("ready"),
        "expected ready output, got {output_text:?}"
    );

    Ok(())
}
