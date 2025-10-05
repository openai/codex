#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use serde_json::json;
use wiremock::Request;

async fn submit_turn(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();

    test.codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    loop {
        let event = test.codex.next_event().await?;
        if matches!(event.msg, EventMsg::TaskComplete(_)) {
            break;
        }
    }

    Ok(())
}

fn request_bodies(requests: &[Request]) -> Result<Vec<Value>> {
    requests
        .iter()
        .map(|req| Ok(serde_json::from_slice::<Value>(&req.body)?))
        .collect()
}

fn collect_output_items<'a>(bodies: &'a [Value], ty: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some(ty) {
                    out.push(item);
                }
            }
        }
    }
    out
}

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_tool_unknown_returns_custom_output_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let call_id = "custom-unsupported";
    let tool_name = "unsupported_tool";

    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_custom_tool_call(call_id, tool_name, "\"payload\""),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    submit_turn(
        &test,
        "invoke custom tool",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    let bodies = request_bodies(&requests)?;
    let custom_items = collect_output_items(&bodies, "custom_tool_call_output");
    assert_eq!(custom_items.len(), 1, "expected single custom tool output");
    let item = custom_items[0];
    assert_eq!(item.get("call_id").and_then(Value::as_str), Some(call_id));

    let output = item
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected = format!("unsupported custom tool call: {tool_name}");
    assert_eq!(output, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_escalated_permissions_rejected_then_ok() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is a valid model");
    });
    let test = builder.build(&server).await?;

    let command = ["/bin/echo", "shell ok"];
    let call_id_blocked = "shell-blocked";
    let call_id_success = "shell-success";

    let first_args = json!({
        "command": command,
        "timeout_ms": 1_000,
        "with_escalated_permissions": true,
    });
    let second_args = json!({
        "command": command,
        "timeout_ms": 1_000,
    });

    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(
                call_id_blocked,
                "shell",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-2"}}),
            ev_function_call(
                call_id_success,
                "shell",
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

    submit_turn(
        &test,
        "run the shell command",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    let bodies = request_bodies(&requests)?;
    let function_outputs = collect_output_items(&bodies, "function_call_output");
    for item in &function_outputs {
        let call_id = item
            .get("call_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            call_id == call_id_blocked || call_id == call_id_success,
            "unexpected call id {call_id}"
        );
    }

    let policy = AskForApproval::Never;
    let expected_message = format!(
        "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}"
    );

    let blocked_outputs: Vec<&Value> = function_outputs
        .iter()
        .filter(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id_blocked))
        .copied()
        .collect();
    assert!(
        !blocked_outputs.is_empty(),
        "expected at least one rejection output for {call_id_blocked}"
    );
    for item in blocked_outputs {
        assert_eq!(
            item.get("output").and_then(Value::as_str),
            Some(expected_message.as_str()),
            "unexpected rejection message"
        );
    }

    let success_item = function_outputs
        .iter()
        .find(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id_success))
        .expect("success output present");
    let output_json: Value = serde_json::from_str(
        success_item
            .get("output")
            .and_then(Value::as_str)
            .expect("success output string"),
    )?;
    assert_eq!(
        output_json["metadata"]["exit_code"].as_i64(),
        Some(0),
        "expected exit code 0 after rerunning without escalation",
    );
    let stdout = output_json["output"].as_str().unwrap_or_default();
    assert!(
        stdout.contains("shell ok"),
        "expected stdout to include command output, got {stdout:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_shell_missing_ids_maps_to_function_output_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let local_shell_event = json!({
        "type": "response.output_item.done",
        "item": {
            "type": "local_shell_call",
            "status": "completed",
            "action": {
                "type": "exec",
                "command": ["/bin/echo", "hi"],
            }
        }
    });

    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            local_shell_event,
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    submit_turn(
        &test,
        "check shell output",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    let bodies = request_bodies(&requests)?;
    let function_outputs = collect_output_items(&bodies, "function_call_output");
    assert_eq!(
        function_outputs.len(),
        1,
        "expected a single function output"
    );
    let item = function_outputs[0];
    assert_eq!(item.get("call_id").and_then(Value::as_str), Some(""));
    assert_eq!(
        item.get("output").and_then(Value::as_str),
        Some("LocalShellCall without call_id or id"),
    );

    Ok(())
}

async fn collect_tools(use_unified_exec: bool) -> Result<Vec<String>> {
    let server = start_mock_server().await;

    let responses = vec![sse(vec![
        json!({"type": "response.created", "response": {"id": "resp-1"}}),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ])];
    mount_sse_sequence(&server, responses).await;

    let mut builder = test_codex().with_config(move |config| {
        config.use_experimental_unified_exec_tool = use_unified_exec;
    });
    let test = builder.build(&server).await?;

    submit_turn(
        &test,
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(
        requests.len(),
        1,
        "expected a single request for tools collection"
    );
    let bodies = request_bodies(&requests)?;
    let first_body = bodies.first().expect("request body present");
    Ok(tool_names(first_body))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_spec_toggle_end_to_end() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tools_disabled = collect_tools(false).await?;
    assert!(
        !tools_disabled.iter().any(|name| name == "unified_exec"),
        "tools list should not include unified_exec when disabled: {tools_disabled:?}"
    );

    let tools_enabled = collect_tools(true).await?;
    assert!(
        tools_enabled.iter().any(|name| name == "unified_exec"),
        "tools list should include unified_exec when enabled: {tools_enabled:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_timeout_includes_timeout_prefix_and_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is a valid model");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-timeout";
    let timeout_ms = 50u64;
    let args = json!({
        "command": ["/bin/sh", "-c", "yes line | head -n 400; sleep 1"],
        "timeout_ms": timeout_ms,
    });

    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    submit_turn(
        &test,
        "run a long command",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    let bodies = request_bodies(&requests)?;
    let function_outputs = collect_output_items(&bodies, "function_call_output");
    let timeout_item = function_outputs
        .iter()
        .find(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id))
        .expect("timeout output present");

    let output_str = timeout_item
        .get("output")
        .and_then(Value::as_str)
        .expect("timeout output string");

    // The exec path can report a timeout in two ways depending on timing:
    // 1) Structured JSON with exit_code 124 and a timeout prefix (preferred), or
    // 2) A plain error string if the child is observed as killed by a signal first.
    if let Ok(output_json) = serde_json::from_str::<Value>(output_str) {
        assert_eq!(
            output_json["metadata"]["exit_code"].as_i64(),
            Some(124),
            "expected timeout exit code 124",
        );

        let stdout = output_json["output"].as_str().unwrap_or_default();
        assert!(
            stdout.contains("command timed out after "),
            "expected timeout prefix, got {stdout:?}"
        );
        let third_line = stdout.lines().nth(2).unwrap_or_default();
        let duration_ms = third_line
            .strip_prefix("command timed out after ")
            .and_then(|line| line.strip_suffix(" milliseconds"))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default();
        assert!(
            duration_ms >= timeout_ms,
            "expected duration >= configured timeout, got {duration_ms} (timeout {timeout_ms})"
        );
    } else {
        // Fallback: accept the signal classification path to deflake the test.
        assert!(
            output_str.contains("execution error"),
            "unexpected non-JSON output: {output_str:?}"
        );
        assert!(
            output_str.contains("Signal(") || output_str.to_lowercase().contains("signal"),
            "expected signal classification in error output, got {output_str:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_sandbox_denied_truncates_error_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let call_id = "shell-denied";
    let long_line = "this is a long stderr line that should trigger truncation 0123456789abcdefghijklmnopqrstuvwxyz";
    let script = format!(
        "for i in $(seq 1 500); do >&2 echo '{long_line}'; done; cat <<'EOF' > denied.txt\ncontent\nEOF",
    );
    let args = json!({
        "command": ["/bin/sh", "-c", script],
        "timeout_ms": 1_000,
    });

    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    submit_turn(
        &test,
        "attempt to write in read-only sandbox",
        AskForApproval::Never,
        SandboxPolicy::ReadOnly,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    let bodies = request_bodies(&requests)?;
    let function_outputs = collect_output_items(&bodies, "function_call_output");
    let denied_item = function_outputs
        .iter()
        .find(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id))
        .expect("denied output present");

    let output = denied_item
        .get("output")
        .and_then(Value::as_str)
        .expect("denied output string");

    assert!(
        output.contains("failed in sandbox: "),
        "expected sandbox error prefix, got {output:?}"
    );
    assert!(
        output.contains("[... omitted"),
        "expected truncated marker, got {output:?}"
    );
    assert!(
        output.contains(long_line),
        "expected truncated stderr sample, got {output:?}"
    );
    // Linux distributions may surface sandbox write failures as different errno messages
    // depending on the underlying mechanism (e.g., EPERM, EACCES, or EROFS). Accept a
    // small set of common variants to keep this cross-platform.
    let denial_markers = [
        "Operation not permitted", // EPERM
        "Permission denied",       // EACCES
        "Read-only file system",   // EROFS
    ];
    assert!(
        denial_markers.iter().any(|m| output.contains(m)),
        "expected sandbox denial message, got {output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_spawn_failure_truncates_exec_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|cfg| {
        cfg.sandbox_policy = SandboxPolicy::DangerFullAccess;
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-spawn-failure";
    let bogus_component = "missing-bin-".repeat(700);
    let bogus_exe = test
        .cwd
        .path()
        .join(bogus_component)
        .to_string_lossy()
        .into_owned();

    let args = json!({
        "command": [bogus_exe],
        "timeout_ms": 1_000,
    });

    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    submit_turn(
        &test,
        "spawn a missing binary",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server.received_requests().await.expect("recorded requests");
    let bodies = request_bodies(&requests)?;
    let function_outputs = collect_output_items(&bodies, "function_call_output");
    let failure_item = function_outputs
        .iter()
        .find(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id))
        .expect("spawn failure output present");

    let output = failure_item
        .get("output")
        .and_then(Value::as_str)
        .expect("spawn failure output string");

    assert!(
        output.contains("execution error:"),
        "expected execution error prefix, got {output:?}"
    );
    assert!(output.len() <= 10 * 1024);

    Ok(())
}
