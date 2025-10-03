#![cfg(not(target_os = "windows"))]

use std::collections::HashMap;

use anyhow::Result;
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
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use serde_json::Value;

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
                    let parsed: Value = serde_json::from_str(content)?;
                    outputs.insert(call_id.to_string(), parsed);
                }
            }
        }
    }
    Ok(outputs)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_reuses_session_via_stdin() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let first_call_id = "uexec-start";
    let first_args = serde_json::json!({
        "input": ["/bin/cat"],
        "timeout_ms": 200,
    });

    let second_call_id = "uexec-stdin";
    let second_args = serde_json::json!({
        "input": ["hello unified exec\n"],
        "session_id": "0",
        "timeout_ms": 500,
    });

    let responses = vec![
        sse(vec![
            serde_json::json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(
                first_call_id,
                "unified_exec",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            serde_json::json!({"type": "response.created", "response": {"id": "resp-2"}}),
            ev_function_call(
                second_call_id,
                "unified_exec",
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
            items: vec![InputItem::Text {
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

    let start_output = outputs
        .get(first_call_id)
        .expect("missing first unified_exec output");
    let session_id = start_output["session_id"].as_str().unwrap_or_default();
    assert!(
        !session_id.is_empty(),
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
        reuse_output["session_id"].as_str().unwrap_or_default(),
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
async fn unified_exec_timeout_and_followup_poll() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let first_call_id = "uexec-timeout";
    let first_args = serde_json::json!({
        "input": ["/bin/sh", "-c", "sleep 0.1; echo ready"],
        "timeout_ms": 10,
    });

    let second_call_id = "uexec-poll";
    let second_args = serde_json::json!({
        "input": Vec::<String>::new(),
        "session_id": "0",
        "timeout_ms": 800,
    });

    let responses = vec![
        sse(vec![
            serde_json::json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(
                first_call_id,
                "unified_exec",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            serde_json::json!({"type": "response.created", "response": {"id": "resp-2"}}),
            ev_function_call(
                second_call_id,
                "unified_exec",
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
            items: vec![InputItem::Text {
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
    assert_eq!(first_output["session_id"], "0");
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
