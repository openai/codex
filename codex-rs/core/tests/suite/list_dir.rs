#![cfg(not(target_os = "windows"))]

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
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disabled until we enable list_dir tool"]
async fn list_dir_tool_returns_entries() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let dir_path = cwd.path().join("sample_dir");
    std::fs::create_dir(&dir_path)?;
    std::fs::write(dir_path.join("alpha.txt"), "first file")?;
    std::fs::create_dir(dir_path.join("nested"))?;
    let dir_path = dir_path.to_string_lossy().to_string();

    let call_id = "list-dir-call";
    let arguments = serde_json::json!({
        "dir_path": dir_path,
        "offset": 1,
        "limit": 2,
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "list_dir", &arguments),
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
                text: "list directory contents".into(),
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

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().unwrap())
        .collect::<Vec<_>>();
    assert!(
        !request_bodies.is_empty(),
        "expected at least one request body"
    );

    let tool_output_item = request_bodies
        .iter()
        .find_map(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    })
                })
        })
        .unwrap_or_else(|| {
            panic!("function_call_output item not found in requests: {request_bodies:#?}")
        });

    assert_eq!(
        tool_output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let output_text = tool_output_item
        .get("output")
        .and_then(|value| match value {
            Value::String(text) => Some(text.as_str()),
            Value::Object(obj) => obj.get("content").and_then(Value::as_str),
            _ => None,
        })
        .expect("output text present");
    assert_eq!(output_text, "E1: [file] alpha.txt\nE2: [dir] nested");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disabled until we enable list_dir tool"]
async fn list_dir_tool_depth_one_omits_children() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let dir_path = cwd.path().join("depth_one");
    std::fs::create_dir(&dir_path)?;
    std::fs::write(dir_path.join("alpha.txt"), "alpha")?;
    std::fs::create_dir(dir_path.join("nested"))?;
    std::fs::write(dir_path.join("nested").join("beta.txt"), "beta")?;
    let dir_path = dir_path.to_string_lossy().to_string();

    let call_id = "list-dir-depth1";
    let arguments = serde_json::json!({
        "dir_path": dir_path,
        "offset": 1,
        "limit": 10,
        "depth": 1,
    })
    .to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "list_dir", &arguments),
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
                text: "list directory contents depth one".into(),
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

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().unwrap())
        .collect::<Vec<_>>();
    assert!(
        !request_bodies.is_empty(),
        "expected at least one request body"
    );

    let tool_output_item = request_bodies
        .iter()
        .find_map(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    })
                })
        })
        .unwrap_or_else(|| {
            panic!("function_call_output item not found in requests: {request_bodies:#?}")
        });

    assert_eq!(
        tool_output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let output_text = tool_output_item
        .get("output")
        .and_then(|value| match value {
            Value::String(text) => Some(text.as_str()),
            Value::Object(obj) => obj.get("content").and_then(Value::as_str),
            _ => None,
        })
        .expect("output text present");
    assert_eq!(output_text, "E1: [file] alpha.txt\nE2: [dir] nested");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disabled until we enable list_dir tool"]
async fn list_dir_tool_depth_two_includes_children_only() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let dir_path = cwd.path().join("depth_two");
    std::fs::create_dir(&dir_path)?;
    std::fs::write(dir_path.join("alpha.txt"), "alpha")?;
    let nested = dir_path.join("nested");
    std::fs::create_dir(&nested)?;
    std::fs::write(nested.join("beta.txt"), "beta")?;
    let deeper = nested.join("grand");
    std::fs::create_dir(&deeper)?;
    std::fs::write(deeper.join("gamma.txt"), "gamma")?;
    let dir_path_string = dir_path.to_string_lossy().to_string();

    let call_id = "list-dir-depth2";
    let arguments = serde_json::json!({
        "dir_path": dir_path_string,
        "offset": 1,
        "limit": 10,
        "depth": 2,
    })
    .to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "list_dir", &arguments),
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
                text: "list directory contents depth two".into(),
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

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().unwrap())
        .collect::<Vec<_>>();
    assert!(
        !request_bodies.is_empty(),
        "expected at least one request body"
    );

    let tool_output_item = request_bodies
        .iter()
        .find_map(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    })
                })
        })
        .unwrap_or_else(|| {
            panic!("function_call_output item not found in requests: {request_bodies:#?}")
        });

    assert_eq!(
        tool_output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let output_text = tool_output_item
        .get("output")
        .and_then(|value| match value {
            Value::String(text) => Some(text.as_str()),
            Value::Object(obj) => obj.get("content").and_then(Value::as_str),
            _ => None,
        })
        .expect("output text present");
    assert_eq!(
        output_text,
        "E1: [file] alpha.txt\nE2: [dir] nested\nE3: [file] nested/beta.txt\nE4: [dir] nested/grand"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disabled until we enable list_dir tool"]
async fn list_dir_tool_depth_three_includes_grandchildren() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let dir_path = cwd.path().join("depth_three");
    std::fs::create_dir(&dir_path)?;
    std::fs::write(dir_path.join("alpha.txt"), "alpha")?;
    let nested = dir_path.join("nested");
    std::fs::create_dir(&nested)?;
    std::fs::write(nested.join("beta.txt"), "beta")?;
    let deeper = nested.join("grand");
    std::fs::create_dir(&deeper)?;
    std::fs::write(deeper.join("gamma.txt"), "gamma")?;
    let dir_path_string = dir_path.to_string_lossy().to_string();

    let call_id = "list-dir-depth3";
    let arguments = serde_json::json!({
        "dir_path": dir_path_string,
        "offset": 1,
        "limit": 10,
        "depth": 3,
    })
    .to_string();

    let first_response = sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp-1"}
        }),
        ev_function_call(call_id, "list_dir", &arguments),
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
                text: "list directory contents depth three".into(),
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

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.expect("recorded requests");
    let request_bodies = requests
        .iter()
        .map(|req| req.body_json::<Value>().unwrap())
        .collect::<Vec<_>>();
    assert!(
        !request_bodies.is_empty(),
        "expected at least one request body"
    );

    let tool_output_item = request_bodies
        .iter()
        .find_map(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    })
                })
        })
        .unwrap_or_else(|| {
            panic!("function_call_output item not found in requests: {request_bodies:#?}")
        });

    assert_eq!(
        tool_output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let output_text = tool_output_item
        .get("output")
        .and_then(|value| match value {
            Value::String(text) => Some(text.as_str()),
            Value::Object(obj) => obj.get("content").and_then(Value::as_str),
            _ => None,
        })
        .expect("output text present");
    assert_eq!(
        output_text,
        "E1: [file] alpha.txt\nE2: [dir] nested\nE3: [file] nested/beta.txt\nE4: [dir] nested/grand\nE5: [file] nested/grand/gamma.txt"
    );

    Ok(())
}
