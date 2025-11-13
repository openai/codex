#![cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::assert_regex_match;
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
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;

fn extract_output_text(item: &Value) -> Option<&str> {
    item.get("output").and_then(|value| match value {
        Value::String(text) => Some(text.as_str()),
        Value::Object(obj) => obj.get("content").and_then(Value::as_str),
        _ => None,
    })
}

#[derive(Debug)]
struct ParsedUnifiedExecOutput {
    chunk_id: Option<String>,
    wall_time_seconds: f64,
    session_id: Option<i32>,
    exit_code: Option<i32>,
    original_token_count: Option<usize>,
    output: String,
}

#[allow(clippy::expect_used)]
fn parse_unified_exec_output(raw: &str) -> Result<ParsedUnifiedExecOutput> {
    static OUTPUT_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = OUTPUT_REGEX.get_or_init(|| {
        Regex::new(concat!(
            r#"(?s)^(?:Total output lines: \d+\n\n)?"#,
            r#"(?:Chunk ID: (?P<chunk_id>[^\n]+)\n)?"#,
            r#"Wall time: (?P<wall_time>-?\d+(?:\.\d+)?) seconds\n"#,
            r#"(?:Process exited with code (?P<exit_code>-?\d+)\n)?"#,
            r#"(?:Process running with session ID (?P<session_id>-?\d+)\n)?"#,
            r#"(?:Original token count: (?P<original_token_count>\d+)\n)?"#,
            r#"Output:\n?(?P<output>.*)$"#,
        ))
        .expect("valid unified exec output regex")
    });

    let cleaned = raw.trim_matches('\r');
    let captures = regex
        .captures(cleaned)
        .ok_or_else(|| anyhow::anyhow!("missing Output section in unified exec output {raw}"))?;

    let chunk_id = captures
        .name("chunk_id")
        .map(|value| value.as_str().to_string());

    let wall_time_seconds = captures
        .name("wall_time")
        .expect("wall_time group present")
        .as_str()
        .parse::<f64>()
        .context("failed to parse wall time seconds")?;

    let exit_code = captures
        .name("exit_code")
        .map(|value| {
            value
                .as_str()
                .parse::<i32>()
                .context("failed to parse exit code from unified exec output")
        })
        .transpose()?;

    let session_id = captures
        .name("session_id")
        .map(|value| {
            value
                .as_str()
                .parse::<i32>()
                .context("failed to parse session id from unified exec output")
        })
        .transpose()?;

    let original_token_count = captures
        .name("original_token_count")
        .map(|value| {
            value
                .as_str()
                .parse::<usize>()
                .context("failed to parse original token count from unified exec output")
        })
        .transpose()?;

    let output = captures
        .name("output")
        .expect("output group present")
        .as_str()
        .to_string();

    Ok(ParsedUnifiedExecOutput {
        chunk_id,
        wall_time_seconds,
        session_id,
        exit_code,
        original_token_count,
        output,
    })
}

fn collect_tool_outputs(bodies: &[Value]) -> Result<HashMap<String, ParsedUnifiedExecOutput>> {
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
                    let parsed = parse_unified_exec_output(content).with_context(|| {
                        format!("failed to parse unified exec output for {call_id}")
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
async fn unified_exec_respects_workdir_override() -> Result<()> {
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

    let workdir = cwd.path().join("uexec_workdir_test");
    std::fs::create_dir_all(&workdir)?;

    let call_id = "uexec-workdir";
    let args = json!({
        "cmd": "pwd",
        "yield_time_ms": 250,
        "workdir": workdir.to_string_lossy().to_string(),
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
                text: "run workdir test".into(),
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
    let output = outputs
        .get(call_id)
        .expect("missing exec_command workdir output");
    let output_text = output.output.trim();
    let output_canonical = std::fs::canonicalize(output_text)?;
    let expected_canonical = std::fs::canonicalize(&workdir)?;
    assert_eq!(
        output_canonical, expected_canonical,
        "pwd should reflect the requested workdir override"
    );

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
        "yield_time_ms": 1000,
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
        let event_msg = wait_for_event(&codex, |_| true).await;
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

    let chunk_id = metadata.chunk_id.as_ref().expect("missing chunk_id");
    assert_eq!(chunk_id.len(), 6, "chunk id should be 6 hex characters");
    assert!(
        chunk_id.chars().all(|c| c.is_ascii_hexdigit()),
        "chunk id should be hexadecimal: {chunk_id}"
    );

    let wall_time = metadata.wall_time_seconds;
    assert!(
        wall_time >= 0.0,
        "wall_time_seconds should be non-negative, got {wall_time}"
    );

    assert!(
        metadata.session_id.is_none(),
        "exec_command for a completed process should not include session_id"
    );

    let exit_code = metadata.exit_code.expect("expected exit_code");
    assert_eq!(exit_code, 0, "expected successful exit");

    let output_text = &metadata.output;
    assert!(
        output_text.contains("tokens truncated"),
        "expected truncation notice in output: {output_text:?}"
    );

    let original_tokens = metadata
        .original_token_count
        .expect("missing original_token_count") as usize;
    assert!(
        original_tokens > 6,
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
        .session_id
        .expect("expected session id from exec_command");
    assert!(
        session_id >= 0,
        "session_id should be non-negative, got {session_id}"
    );
    assert!(
        start_output.exit_code.is_none(),
        "initial exec_command should not include exit_code while session is running"
    );

    let send_output = outputs
        .get(send_call_id)
        .expect("missing write_stdin echo output");
    let echoed = send_output.output.as_str();
    assert!(
        echoed.contains("hello unified exec"),
        "expected echoed output from cat, got {echoed:?}"
    );
    let echoed_session = send_output
        .session_id
        .expect("write_stdin should return session id while process is running");
    assert_eq!(
        echoed_session, session_id,
        "write_stdin should reuse existing session id"
    );
    assert!(
        send_output.exit_code.is_none(),
        "write_stdin should not include exit_code while process is running"
    );

    let exit_output = outputs
        .get(exit_call_id)
        .expect("missing exit metadata output");
    assert!(
        exit_output.session_id.is_none(),
        "session_id should be omitted once the process exits"
    );
    let exit_code = exit_output
        .exit_code
        .expect("expected exit_code after sending EOF");
    assert_eq!(exit_code, 0, "cat should exit cleanly after EOF");

    let exit_chunk = exit_output
        .chunk_id
        .as_ref()
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
    let session_id = start_output.session_id.unwrap_or_default();
    assert!(
        session_id >= 0,
        "expected session id in first unified_exec response"
    );
    assert!(start_output.output.is_empty());

    let reuse_output = outputs
        .get(second_call_id)
        .expect("missing reused unified_exec output");
    assert_eq!(reuse_output.session_id.unwrap_or_default(), session_id);
    let echoed = reuse_output.output.as_str();
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
    let session_id = start_output.session_id.unwrap_or_default();
    assert!(
        session_id >= 0,
        "expected session id from initial unified_exec response"
    );

    let poll_output = outputs
        .get(second_call_id)
        .expect("missing poll unified_exec output");
    let poll_text = poll_output.output.as_str();
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
    assert_eq!(first_output.session_id, Some(0));
    assert!(first_output.output.is_empty());

    let poll_output = outputs.get(second_call_id).expect("missing poll output");
    let output_text = poll_output.output.as_str();
    assert!(
        output_text.contains("ready"),
        "expected ready output, got {output_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Skipped on arm because the ctor logic to handle arg0 doesn't work on ARM
#[cfg(not(target_arch = "arm"))]
async fn unified_exec_formats_large_output_summary() -> Result<()> {
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

    let script = r#"python3 - <<'PY'
for i in range(300):
    print(f"line-{i}")
PY
"#;

    let call_id = "uexec-large-output";
    let args = serde_json::json!({
        "cmd": script,
        "yield_time_ms": 500,
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
                text: "summarize large output".into(),
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
    let large_output = outputs.get(call_id).expect("missing large output summary");

    assert_regex_match(
        concat!(
            r"(?s)",
            r"line-0.*?",
            r"\[\.{3} omitted \d+ of \d+ lines \.{3}\].*?",
            r"line-299",
        ),
        &large_output.output,
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_runs_under_sandbox() -> Result<()> {
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

    let call_id = "uexec";
    let args = serde_json::json!({
        "cmd": "echo 'hello'",
        "yield_time_ms": 500,
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
                text: "summarize large output".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            // Important!
            sandbox_policy: SandboxPolicy::ReadOnly,
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
    let output = outputs.get(call_id).expect("missing output");

    assert_regex_match("hello[\r\n]+", &output.output);

    Ok(())
}
