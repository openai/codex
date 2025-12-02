#![cfg(not(target_os = "windows"))]
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandSource;
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
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_event_with_timeout;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use tokio::time::Duration;

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
    process_id: Option<String>,
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
            r#"(?:Process running with session ID (?P<process_id>-?\d+)\n)?"#,
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

    let process_id = captures
        .name("process_id")
        .map(|value| value.as_str().to_string());

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
        process_id,
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
async fn unified_exec_intercepts_apply_patch_exec_command() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let harness = TestCodexHarness::with_builder(builder).await?;

    let patch =
        "*** Begin Patch\n*** Add File: uexec_apply.txt\n+hello from unified exec\n*** End Patch";
    let command = format!("apply_patch <<'EOF'\n{patch}\nEOF\n");
    let call_id = "uexec-apply-patch";
    let args = json!({
        "cmd": command,
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
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let test = harness.test();
    let codex = test.codex.clone();
    let cwd = test.cwd_path().to_path_buf();
    let session_model = test.session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "apply patch via unified exec".into(),
            }],
            final_output_json_schema: None,
            cwd,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let mut saw_patch_begin = false;
    let mut patch_end = None;
    let mut saw_exec_begin = false;
    let mut saw_exec_end = false;
    wait_for_event(&codex, |event| match event {
        EventMsg::PatchApplyBegin(begin) if begin.call_id == call_id => {
            saw_patch_begin = true;
            assert!(
                begin
                    .changes
                    .keys()
                    .any(|path| path.file_name() == Some(OsStr::new("uexec_apply.txt"))),
                "expected apply_patch changes to target uexec_apply.txt",
            );
            false
        }
        EventMsg::PatchApplyEnd(end) if end.call_id == call_id => {
            patch_end = Some(end.clone());
            false
        }
        EventMsg::ExecCommandBegin(event) if event.call_id == call_id => {
            saw_exec_begin = true;
            false
        }
        EventMsg::ExecCommandEnd(event) if event.call_id == call_id => {
            saw_exec_end = true;
            false
        }
        EventMsg::TaskComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(
        saw_patch_begin,
        "expected apply_patch to emit PatchApplyBegin"
    );
    let patch_end = patch_end.expect("expected apply_patch to emit PatchApplyEnd");
    assert!(
        patch_end.success,
        "expected apply_patch to finish successfully: stdout={:?} stderr={:?}",
        patch_end.stdout, patch_end.stderr,
    );
    assert!(
        !saw_exec_begin,
        "apply_patch should be intercepted before exec_command begin"
    );
    assert!(
        !saw_exec_end,
        "apply_patch should not emit exec_command end events"
    );

    let output = harness.function_call_stdout(call_id).await;
    assert!(
        output.contains("Success. Updated the following files:"),
        "expected apply_patch output, got: {output:?}"
    );
    assert!(
        output.contains("A uexec_apply.txt"),
        "expected apply_patch file summary, got: {output:?}"
    );
    assert_eq!(
        fs::read_to_string(harness.path("uexec_apply.txt"))?,
        "hello from unified exec\n"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_exec_command_begin_event() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5").with_config(|config| {
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
        vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "/bin/echo hello unified exec".to_string()
        ]
    );
    assert_eq!(begin_event.cwd, cwd.path());

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_resolves_relative_workdir() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5").with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let workdir_rel = std::path::PathBuf::from("uexec_relative_workdir");
    std::fs::create_dir_all(cwd.path().join(&workdir_rel))?;

    let call_id = "uexec-workdir-relative";
    let args = json!({
        "cmd": "pwd",
        "yield_time_ms": 250,
        "workdir": workdir_rel.to_string_lossy().to_string(),
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
                text: "run relative workdir test".into(),
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
        begin_event.cwd,
        cwd.path().join(workdir_rel),
        "exec_command cwd should resolve relative workdir against turn cwd",
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "flaky"]
async fn unified_exec_respects_workdir_override() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5").with_config(|config| {
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

    let begin_event = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandBegin(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(
        begin_event.cwd, workdir,
        "exec_command cwd should reflect the requested workdir override"
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

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
        "session_id": 1000,
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
        "session_id": 1000,
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
async fn unified_exec_emits_begin_for_write_stdin() -> Result<()> {
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

    let open_call_id = "uexec-open-for-begin";
    let open_args = json!({
        "cmd": "bash -i".to_string(),
        "yield_time_ms": 200,
    });

    let stdin_call_id = "uexec-stdin-begin";
    let stdin_args = json!({
        "chars": "echo hello",
        "session_id": 1000,
        "yield_time_ms": 400,
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
                text: "begin events for stdin".into(),
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
        EventMsg::ExecCommandBegin(ev) if ev.call_id == stdin_call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    assert_eq!(
        begin_event.command,
        vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "bash -i".to_string()
        ]
    );
    assert_eq!(
        begin_event.interaction_input,
        Some("echo hello".to_string())
    );
    assert_eq!(
        begin_event.source,
        ExecCommandSource::UnifiedExecInteraction
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_emits_begin_event_for_write_stdin_requests() -> Result<()> {
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
        "cmd": "bash -i".to_string(),
        "yield_time_ms": 250,
    });

    let poll_call_id = "uexec-poll-empty";
    let poll_args = json!({
        "chars": "",
        "session_id": 1000,
        "yield_time_ms": 150,
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
        2,
        "expected begin events for the startup command and the write_stdin call"
    );

    let open_event = begin_events
        .iter()
        .find(|ev| ev.call_id == open_call_id)
        .expect("missing exec_command begin");
    assert_eq!(
        open_event.command,
        vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "bash -i".to_string()
        ]
    );
    assert!(
        open_event.interaction_input.is_none(),
        "startup begin events should not include interaction input"
    );
    assert_eq!(open_event.source, ExecCommandSource::UnifiedExecStartup);

    let poll_event = begin_events
        .iter()
        .find(|ev| ev.call_id == poll_call_id)
        .expect("missing write_stdin begin");
    assert_eq!(
        poll_event.command,
        vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "bash -i".to_string()
        ]
    );
    assert!(
        poll_event.interaction_input.is_none(),
        "poll begin events should omit interaction input"
    );
    assert_eq!(poll_event.source, ExecCommandSource::UnifiedExecInteraction);

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
        "cmd": "printf 'token one token two token three token four token five token six token seven'",
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
        metadata.process_id.is_none(),
        "exec_command for a completed process should not include process_id"
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
async fn unified_exec_respects_early_exit_notifications() -> Result<()> {
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

    let call_id = "uexec-early-exit";
    let args = serde_json::json!({
        "cmd": "sleep 0.05",
        "yield_time_ms": 31415,
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
                text: "watch early exit timing".into(),
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
        .expect("missing early exit unified_exec output");

    assert!(
        output.process_id.is_none(),
        "short-lived process should not keep a session alive"
    );
    assert_eq!(
        output.exit_code,
        Some(0),
        "short-lived process should exit successfully"
    );

    let wall_time = output.wall_time_seconds;
    assert!(
        wall_time < 0.75,
        "wall_time should reflect early exit rather than the full yield time; got {wall_time}"
    );
    assert!(
        output.output.is_empty(),
        "sleep command should not emit output, got {:?}",
        output.output
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
        "session_id": 1000,
        "yield_time_ms": 500,
    });
    let exit_args = serde_json::json!({
        "chars": "\u{0004}",
        "session_id": 1000,
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
    let process_id = start_output
        .process_id
        .clone()
        .expect("expected process id from exec_command");
    assert!(
        process_id.len() > 3,
        "process_id should be at least 4 digits, got {process_id}"
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
        .process_id
        .clone()
        .expect("write_stdin should return process id while process is running");
    assert_eq!(
        echoed_session, process_id,
        "write_stdin should reuse existing process id"
    );
    assert!(
        send_output.exit_code.is_none(),
        "write_stdin should not include exit_code while process is running"
    );

    let exit_output = outputs
        .get(exit_call_id)
        .expect("missing exit metadata output");
    assert!(
        exit_output.process_id.is_none(),
        "process_id should be omitted once the process exits"
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
        "session_id": 1000,
        "yield_time_ms": 300,
    });

    let exit_call_id = "uexec-end-on-exit";
    let exit_args = serde_json::json!({
        "chars": "\u{0004}",
        "session_id": 1000,
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
        "session_id": 1000,
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
    let process_id = start_output.process_id.clone().unwrap_or_default();
    assert!(
        !process_id.is_empty(),
        "expected process id in first unified_exec response"
    );
    assert!(start_output.output.is_empty());

    let reuse_output = outputs
        .get(second_call_id)
        .expect("missing reused unified_exec output");
    assert_eq!(
        reuse_output.process_id.clone().unwrap_or_default(),
        process_id
    );
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

chunk = b'long content here to trigger truncation' * (1 << 10)
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
        "session_id": 1000,
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
    // This is a worst case scenario for the truncate logic.
    wait_for_event_with_timeout(
        &codex,
        |event| matches!(event, EventMsg::TaskComplete(_)),
        Duration::from_secs(10),
    )
    .await;

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
    let process_id = start_output.process_id.clone().unwrap_or_default();
    assert!(
        !process_id.is_empty(),
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
        "session_id": 1000,
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
    assert!(first_output.process_id.is_some());
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
import sys
sys.stdout.write("token token \n" * 5000)
PY
"#;

    let call_id = "uexec-large-output";
    let args = serde_json::json!({
        "cmd": script,
        "max_output_tokens": 100,
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

    let output_text = large_output.output.replace("\r\n", "\n");
    let truncated_pattern = r"(?s)^Total output lines: \d+\n\n(token token \n){5,}.*…\d+ tokens truncated….*(token token \n){5,}$";
    assert_regex_match(truncated_pattern, &output_text);

    let original_tokens = large_output
        .original_token_count
        .expect("missing original_token_count for large output summary");
    assert!(original_tokens > 0);

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn unified_exec_prunes_exited_sessions_first() -> Result<()> {
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

    const MAX_SESSIONS_FOR_TEST: i32 = 64;
    const FILLER_SESSIONS: i32 = MAX_SESSIONS_FOR_TEST - 1;

    let keep_call_id = "uexec-prune-keep";
    let keep_args = serde_json::json!({
        "cmd": "/bin/cat",
        "yield_time_ms": 250,
    });

    let prune_call_id = "uexec-prune-target";
    // Give the sleeper time to exit before the filler sessions trigger pruning.
    let prune_args = serde_json::json!({
        "cmd": "sleep 1",
        "yield_time_ms": 1_250,
    });

    let mut events = vec![ev_response_created("resp-prune-1")];
    events.push(ev_function_call(
        keep_call_id,
        "exec_command",
        &serde_json::to_string(&keep_args)?,
    ));
    events.push(ev_function_call(
        prune_call_id,
        "exec_command",
        &serde_json::to_string(&prune_args)?,
    ));

    for idx in 0..FILLER_SESSIONS {
        let filler_args = serde_json::json!({
            "cmd": format!("echo filler {idx}"),
            "yield_time_ms": 250,
        });
        let call_id = format!("uexec-prune-fill-{idx}");
        events.push(ev_function_call(
            &call_id,
            "exec_command",
            &serde_json::to_string(&filler_args)?,
        ));
    }

    let keep_write_call_id = "uexec-prune-keep-write";
    let keep_write_args = serde_json::json!({
        "chars": "still alive\n",
        "session_id": 1000,
        "yield_time_ms": 500,
    });
    events.push(ev_function_call(
        keep_write_call_id,
        "write_stdin",
        &serde_json::to_string(&keep_write_args)?,
    ));

    let probe_call_id = "uexec-prune-probe";
    let probe_args = serde_json::json!({
        "chars": "should fail\n",
        "session_id": 1001,
        "yield_time_ms": 500,
    });
    events.push(ev_function_call(
        probe_call_id,
        "write_stdin",
        &serde_json::to_string(&probe_args)?,
    ));

    events.push(ev_completed("resp-prune-1"));
    let first_response = sse(events);
    let completion_response = sse(vec![
        ev_response_created("resp-prune-2"),
        ev_assistant_message("msg-prune", "done"),
        ev_completed("resp-prune-2"),
    ]);
    let response_mock =
        mount_sse_sequence(&server, vec![first_response, completion_response]).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "fill session cache".into(),
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

    let requests = response_mock.requests();
    assert!(
        !requests.is_empty(),
        "expected at least one response request"
    );

    let keep_start = requests
        .iter()
        .find_map(|req| req.function_call_output_text(keep_call_id))
        .expect("missing initial keep session output");
    let keep_start_output = parse_unified_exec_output(&keep_start)?;
    assert!(keep_start_output.process_id.is_some());
    assert!(keep_start_output.exit_code.is_none());

    let prune_start = requests
        .iter()
        .find_map(|req| req.function_call_output_text(prune_call_id))
        .expect("missing initial prune session output");
    let prune_start_output = parse_unified_exec_output(&prune_start)?;
    assert!(prune_start_output.process_id.is_some());
    assert!(prune_start_output.exit_code.is_none());

    let keep_write = requests
        .iter()
        .find_map(|req| req.function_call_output_text(keep_write_call_id))
        .expect("missing keep write output");
    let keep_write_output = parse_unified_exec_output(&keep_write)?;
    assert!(keep_write_output.process_id.is_some());
    assert!(
        keep_write_output.output.contains("still alive"),
        "expected cat session to echo input, got {:?}",
        keep_write_output.output
    );

    let pruned_probe = requests
        .iter()
        .find_map(|req| req.function_call_output_text(probe_call_id))
        .expect("missing probe output");
    assert!(
        pruned_probe.contains("UnknownSessionId") || pruned_probe.contains("Unknown process id"),
        "expected probe to fail after pruning, got {pruned_probe:?}"
    );

    Ok(())
}
