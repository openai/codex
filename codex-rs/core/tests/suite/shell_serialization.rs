#![cfg(not(target_os = "windows"))]

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
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;

async fn submit_turn(test: &TestCodex, prompt: &str, sandbox_policy: SandboxPolicy) -> Result<()> {
    let session_model = test.session_configured.model.clone();

    test.codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;

    Ok(())
}

fn request_bodies(requests: &[wiremock::Request]) -> Result<Vec<Value>> {
    requests
        .iter()
        .map(|req| Ok(serde_json::from_slice::<Value>(&req.body)?))
        .collect()
}

fn find_function_call_output<'a>(bodies: &'a [Value], call_id: &str) -> Option<&'a Value> {
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                {
                    return Some(item);
                }
            }
        }
    }
    None
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_stays_json_without_freeform_apply_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = false;
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is a model family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-json";
    let args = json!({
        "command": ["/bin/echo", "shell json"],
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
        "run the json shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server
        .received_requests()
        .await
        .expect("recorded requests present");
    let bodies = request_bodies(&requests)?;
    let output_item = find_function_call_output(&bodies, call_id).expect("shell output present");
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output string");

    let parsed: Value = serde_json::from_str(output)?;
    assert_eq!(
        parsed
            .get("metadata")
            .and_then(|metadata| metadata.get("exit_code"))
            .and_then(Value::as_i64),
        Some(0),
        "expected zero exit code in unformatted JSON output",
    );
    let stdout = parsed
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        stdout.contains("shell json"),
        "expected stdout to include command output, got {stdout:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_is_structured_with_freeform_apply_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-structured";
    let args = json!({
        "command": ["/bin/echo", "freeform shell"],
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
        "run the structured shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server
        .received_requests()
        .await
        .expect("recorded requests present");
    let bodies = request_bodies(&requests)?;
    let output_item =
        find_function_call_output(&bodies, call_id).expect("structured output present");
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("structured output string");

    assert!(
        serde_json::from_str::<Value>(output).is_err(),
        "expected structured shell output to be plain text",
    );
    assert!(
        output.starts_with("Exit code: 0\n"),
        "expected exit code prefix, got {output:?}",
    );
    assert!(
        output.contains("\nOutput:\n"),
        "expected Output section, got {output:?}"
    );
    assert!(
        output.contains("freeform shell"),
        "expected stdout content, got {output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_reserializes_truncated_content() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("gpt-5-codex").expect("gpt-5 is a model family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-truncated";
    let args = json!({
        "command": ["/bin/sh", "-c", "seq 1 400"],
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
        "run the truncation shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = server
        .received_requests()
        .await
        .expect("recorded requests present");
    let bodies = request_bodies(&requests)?;
    let output_item =
        find_function_call_output(&bodies, call_id).expect("truncated output present");
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("truncated output string");

    assert!(
        serde_json::from_str::<Value>(output).is_err(),
        "expected truncated shell output to be plain text",
    );
    assert!(
        output.starts_with("Exit code: 0\n"),
        "expected exit code prefix, got {output:?}",
    );
    assert!(
        output.lines().any(|line| line == "Total output lines: 400"),
        "expected total output lines marker, got {output:?}",
    );
    assert!(
        output.contains("[... omitted"),
        "expected truncated marker, got {output:?}",
    );

    Ok(())
}
