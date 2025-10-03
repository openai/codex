#![cfg(not(target_os = "windows"))]

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::plan_tool::StepStatus;
use core_test_support::responses;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use serde_json::json;
use wiremock::matchers::any;

fn function_call_output(body: &Value) -> Option<&Value> {
    body.get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
            })
        })
}

fn extract_output_text(item: &Value) -> Option<&str> {
    item.get("output").and_then(|value| match value {
        Value::String(text) => Some(text.as_str()),
        Value::Object(obj) => obj.get("content").and_then(Value::as_str),
        _ => None,
    })
}

fn find_request_with_function_call_output(requests: &[Value]) -> Option<&Value> {
    requests
        .iter()
        .find(|body| function_call_output(body).is_some())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_tool_executes_command_and_streams_output() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "shell-tool-call";
    let command = vec!["/bin/echo", "tool harness"];
    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_local_shell_call(call_id, "completed", command),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "all done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please run the shell command".into(),
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

    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    let output_text = extract_output_text(output_item).expect("output text present");
    let exec_output: Value = serde_json::from_str(output_text)?;
    assert_eq!(exec_output["metadata"]["exit_code"], 0);
    let stdout = exec_output["output"].as_str().expect("stdout field");
    assert!(
        stdout.contains("tool harness"),
        "expected stdout to contain command output, got {stdout:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_plan_tool_emits_plan_update_event() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.include_plan_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "plan-tool-call";
    let plan_args = json!({
        "explanation": "Tool harness check",
        "plan": [
            {"step": "Inspect workspace", "status": "in_progress"},
            {"step": "Report results", "status": "pending"},
        ],
    })
    .to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "update_plan", &plan_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "plan acknowledged"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please update the plan".into(),
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

    let mut saw_plan_update = false;

    loop {
        let event = codex.next_event().await.expect("event");
        match event.msg {
            EventMsg::PlanUpdate(update) => {
                saw_plan_update = true;
                assert_eq!(update.explanation.as_deref(), Some("Tool harness check"));
                assert_eq!(update.plan.len(), 2);
                assert_eq!(update.plan[0].step, "Inspect workspace");
                assert!(matches!(update.plan[0].status, StepStatus::InProgress));
                assert_eq!(update.plan[1].step, "Report results");
                assert!(matches!(update.plan[1].status, StepStatus::Pending));
            }
            EventMsg::TaskComplete(_) => break,
            _ => {}
        }
    }

    assert!(saw_plan_update, "expected PlanUpdate event");

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );
    let output_text = extract_output_text(output_item).expect("output text present");
    assert_eq!(output_text, "Plan updated");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_plan_tool_rejects_malformed_payload() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.include_plan_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "plan-tool-invalid";
    let invalid_args = json!({
        "explanation": "Missing plan data"
    })
    .to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "update_plan", &invalid_args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "malformed plan payload"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please update the plan".into(),
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

    let mut saw_plan_update = false;

    loop {
        let event = codex.next_event().await.expect("event");
        match event.msg {
            EventMsg::PlanUpdate(_) => saw_plan_update = true,
            EventMsg::TaskComplete(_) => break,
            _ => {}
        }
    }

    assert!(
        !saw_plan_update,
        "did not expect PlanUpdate event for malformed payload"
    );

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );
    let output_text = extract_output_text(output_item).expect("output text present");
    assert!(
        output_text.contains("failed to parse function arguments"),
        "expected parse error message in output text, got {output_text:?}"
    );
    if let Some(success_flag) = output_item
        .get("output")
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("success"))
        .and_then(serde_json::Value::as_bool)
    {
        assert!(
            !success_flag,
            "expected tool output to mark success=false for malformed payload"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_tool_executes_and_emits_patch_events() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "apply-patch-call";
    let patch_content = r#"*** Begin Patch
*** Add File: notes.txt
+Tool harness apply patch
*** End Patch"#;

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_apply_patch_function_call(call_id, patch_content),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "patch complete"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please apply a patch".into(),
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

    let mut saw_patch_begin = false;
    let mut patch_end_success = None;

    loop {
        let event = codex.next_event().await.expect("event");
        match event.msg {
            EventMsg::PatchApplyBegin(begin) => {
                saw_patch_begin = true;
                assert_eq!(begin.call_id, call_id);
            }
            EventMsg::PatchApplyEnd(end) => {
                assert_eq!(end.call_id, call_id);
                patch_end_success = Some(end.success);
            }
            EventMsg::TaskComplete(_) => break,
            _ => {}
        }
    }

    assert!(saw_patch_begin, "expected PatchApplyBegin event");
    let patch_end_success =
        patch_end_success.expect("expected PatchApplyEnd event to capture success flag");

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(!requests.is_empty(), "expected at least one POST request");

    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );
    let output_text = extract_output_text(output_item).expect("output text present");

    if let Ok(exec_output) = serde_json::from_str::<Value>(output_text) {
        let exit_code = exec_output["metadata"]["exit_code"]
            .as_i64()
            .expect("exit_code present");
        let summary = exec_output["output"].as_str().expect("output field");
        assert_eq!(
            exit_code, 0,
            "expected apply_patch exit_code=0, got {exit_code}, summary: {summary:?}"
        );
        assert!(
            patch_end_success,
            "expected PatchApplyEnd success flag, summary: {summary:?}"
        );
        assert!(
            summary.contains("Success."),
            "expected apply_patch summary to note success, got {summary:?}"
        );

        let patched_path = cwd.path().join("notes.txt");
        let contents = std::fs::read_to_string(&patched_path)
            .unwrap_or_else(|e| panic!("failed reading {}: {e}", patched_path.display()));
        assert_eq!(contents, "Tool harness apply patch\n");
    } else {
        assert!(
            output_text.contains("codex-run-as-apply-patch"),
            "expected apply_patch failure message to mention codex-run-as-apply-patch, got {output_text:?}"
        );
        assert!(
            !patch_end_success,
            "expected PatchApplyEnd to report success=false when apply_patch invocation fails"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_reports_parse_diagnostics() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "apply-patch-parse-error";
    let patch_content = r"*** Begin Patch
*** Update File: broken.txt
*** End Patch";

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_apply_patch_function_call(call_id, patch_content),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "failed"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please apply a patch".into(),
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

    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );
    let output_text = extract_output_text(output_item).expect("output text present");

    assert!(
        output_text.contains("apply_patch verification failed"),
        "expected apply_patch verification failure message, got {output_text:?}"
    );
    assert!(
        output_text.contains("invalid hunk"),
        "expected parse diagnostics in output text, got {output_text:?}"
    );

    if let Some(success_flag) = output_item
        .get("output")
        .and_then(|value| value.as_object())
        .and_then(|obj| obj.get("success"))
        .and_then(serde_json::Value::as_bool)
    {
        assert!(
            !success_flag,
            "expected tool output to mark success=false for parse failures"
        );
    }

    Ok(())
}
