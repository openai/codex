#![cfg(not(target_os = "windows"))]

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
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

fn find_image_message(body: &Value) -> Option<&Value> {
    body.get("input")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("message")
                    && item
                        .get("content")
                        .and_then(Value::as_array)
                        .map(|content| {
                            content.iter().any(|span| {
                                span.get("type").and_then(Value::as_str) == Some("input_image")
                            })
                        })
                        .unwrap_or(false)
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
async fn view_image_tool_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "assets/example.png";
    let abs_path = cwd.path().join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image_bytes = b"fake_png_bytes".to_vec();
    std::fs::write(&abs_path, &image_bytes)?;

    let call_id = "view-image-call";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please add the screenshot".into(),
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

    let mut tool_event = None;
    loop {
        let event = codex.next_event().await.expect("event");
        match event.msg {
            EventMsg::ViewImageToolCall(ev) => tool_event = Some(ev),
            EventMsg::TaskComplete(_) => break,
            _ => {}
        }
    }

    let tool_event = tool_event.expect("view image tool event emitted");
    assert_eq!(tool_event.call_id, call_id);
    assert_eq!(tool_event.path, abs_path);

    let requests = server.received_requests().await.expect("recorded requests");
    assert!(
        requests.len() >= 2,
        "expected at least two POST requests, got {}",
        requests.len()
    );
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    let output_text = extract_output_text(output_item).expect("output text present");
    assert_eq!(output_text, "attached local image path");

    let image_message = find_image_message(body_with_tool_output)
        .expect("pending input image message not included in request");
    let image_url = image_message
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content.iter().find_map(|span| {
                if span.get("type").and_then(Value::as_str) == Some("input_image") {
                    span.get("image_url").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .expect("image_url present");

    let expected_image_url = format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(&image_bytes)
    );
    assert_eq!(image_url, expected_image_url);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_path_is_directory() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "assets";
    let abs_path = cwd.path().join(rel_path);
    std::fs::create_dir_all(&abs_path)?;

    let call_id = "view-image-directory";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please attach the folder".into(),
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
    assert!(
        requests.len() >= 2,
        "expected at least two POST requests, got {}",
        requests.len()
    );
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    let output_text = extract_output_text(output_item).expect("output text present");
    let expected_message = format!("image path `{}` is not a file", abs_path.display());
    assert_eq!(output_text, expected_message);

    assert!(
        find_image_message(body_with_tool_output).is_none(),
        "directory path should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_file_missing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "missing/example.png";
    let abs_path = cwd.path().join(rel_path);

    let call_id = "view-image-missing";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(&server, any(), second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please attach the missing image".into(),
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
    assert!(
        requests.len() >= 2,
        "expected at least two POST requests, got {}",
        requests.len()
    );
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect::<Vec<_>>();

    let body_with_tool_output = find_request_with_function_call_output(&request_bodies)
        .expect("function_call_output item not found in requests");
    let output_item = function_call_output(body_with_tool_output).expect("tool output item");
    let output_text = extract_output_text(output_item).expect("output text present");
    let expected_prefix = format!("unable to locate image at `{}`:", abs_path.display());
    assert!(
        output_text.starts_with(&expected_prefix),
        "expected error to start with `{expected_prefix}` but got `{output_text}`"
    );

    assert!(
        find_image_message(body_with_tool_output).is_none(),
        "missing file should not produce an input_image message"
    );

    Ok(())
}
