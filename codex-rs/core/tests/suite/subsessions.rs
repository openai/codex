use anyhow::Result;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use tokio::test;
use wiremock::matchers::any;
use wiremock::matchers::body_string_contains;

#[allow(clippy::expect_used)]
async fn collect_tool_names(model: &str) -> Result<Vec<String>> {
    let server = start_mock_server().await;
    let model_owned = model.to_string();
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = test_codex()
        .with_config(move |config| {
            config.model = model_owned.clone();
            config.model_family =
                find_family_for_model(&model_owned).expect("model family available for test");
        })
        .build(&server)
        .await?;

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), response).await;

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "ping".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
    let requests = server.received_requests().await.expect("recorded requests");
    let first_body = requests
        .first()
        .ok_or_else(|| anyhow::anyhow!("expected at least one request"))?
        .body_json::<Value>()?;
    let tool_names = first_body
        .get("tools")
        .and_then(|tools| tools.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(tool_names)
}

#[test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_models_expose_subsession_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let tools = collect_tool_names("codex-mini-latest").await?;
    assert!(
        tools.contains(&"create_session".to_string())
            && tools.contains(&"wait_session".to_string())
            && tools.contains(&"cancel_session".to_string()),
        "expected subsession tool trio in {tools:?}"
    );
    Ok(())
}

#[test(flavor = "multi_thread", worker_threads = 2)]
async fn test_models_expose_subsession_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let tools = collect_tool_names("test-gpt-5-codex").await?;
    assert!(
        tools.contains(&"create_session".to_string())
            && tools.contains(&"wait_session".to_string())
            && tools.contains(&"cancel_session".to_string()),
        "expected subsession tool trio in {tools:?}"
    );
    Ok(())
}

#[test(flavor = "multi_thread", worker_threads = 2)]
async fn gpt5_codex_models_do_not_expose_subsession_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let tools = collect_tool_names("gpt-5-codex").await?;
    assert!(
        !tools.contains(&"create_session".to_string()),
        "unexpected subsession tools in {tools:?}"
    );
    Ok(())
}

#[test(flavor = "multi_thread", worker_threads = 2)]
async fn subsession_can_apply_patch_to_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        // Ensure subsession tools are exposed and apply_patch is available in child.
        config.model = "test-gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("test-gpt-5-codex").expect("model family available for test");
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    // Parent turn 1: ask to spawn a subsession to create a file.
    // The parent model will call create_session with a prompt instructing the child.
    let parent_first = sse(vec![
        ev_response_created("resp-parent-1"),
        responses::ev_function_call(
            "create-session-1",
            "create_session",
            &serde_json::json!({
                "session_type": "default",
                "prompt": "Create a file named subsession.txt with the exact contents 'Hello from subsession'",
            })
            .to_string(),
        ),
        ev_completed("resp-parent-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), parent_first).await;

    // Child turn 1: upon spawn, the child will call apply_patch to create the file.
    // Match on the subsession instructions to route this to the child conversation.
    let child_first = sse(vec![
        ev_response_created("resp-child-1"),
        responses::ev_apply_patch_function_call(
            "apply-patch-child-1",
            r#"*** Begin Patch
*** Add File: subsession.txt
+Hello from subsession
*** End Patch"#,
        ),
        ev_completed("resp-child-1"),
    ]);
    responses::mount_sse_once_match(
        &server,
        body_string_contains("You are a compact subsession assistant"),
        child_first,
    )
    .await;

    // Parent follow-up: after the tool result is returned, the parent may send a
    // subsequent request. Provide a simple assistant message to close the turn.
    let parent_second = sse(vec![
        ev_assistant_message("msg-parent-2", "subsession started"),
        ev_completed("resp-parent-2"),
    ]);
    responses::mount_sse_once_match(
        &server,
        body_string_contains("\"function_call_output\""),
        parent_second,
    )
    .await;

    // Child follow-up: after apply_patch executes, the child continues and then finishes.
    let child_second = sse(vec![
        ev_assistant_message("msg-child-2", "done"),
        ev_completed("resp-child-2"),
    ]);
    responses::mount_sse_once_match(
        &server,
        body_string_contains("You are a compact subsession assistant"),
        child_second,
    )
    .await;

    // Kick off the parent turn which should spawn the subsession.
    let session_model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "please spawn a subsession to create a file".into(),
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

    // Capture the child session id from the background event.
    let mut child_id: Option<String> = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::BackgroundEvent(ev) => {
            if let Some((_, after)) = ev.message.split_once("spawned child session ")
                && let Some((id, _)) = after.split_once(' ')
            {
                child_id = Some(id.to_string());
                return true;
            }
            false
        }
        _ => false,
    })
    .await;

    // Wait for the child to complete and emit its final background event.
    // This makes the file write deterministic before we assert.
    let _ = wait_for_event(&codex, |event| match event {
        EventMsg::BackgroundEvent(ev) => {
            if let Some(id) = child_id.as_deref() {
                return ev
                    .message
                    .contains(&format!("child session {id} completed"));
            }
            false
        }
        _ => false,
    })
    .await;

    // Debug: inspect recorded requests to confirm routing during failures.
    if let Some(requests) = server.received_requests().await {
        for (i, req) in requests.iter().enumerate() {
            if let Ok(body) = req.body_json::<Value>() {
                let instr = body
                    .get("instructions")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let has_apply = body
                    .get("tools")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter().any(|t| {
                            t.get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(Value::as_str)
                                == Some("apply_patch")
                                || t.get("name").and_then(Value::as_str) == Some("apply_patch")
                        })
                    })
                    .unwrap_or(false);
                let has_fn_output = body
                    .get("input")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter().any(|it| {
                            it.get("type").and_then(Value::as_str) == Some("function_call_output")
                        })
                    })
                    .unwrap_or(false);
                eprintln!(
                    "req#{i}: instr_has_subsession_prompt={} tools_include_apply_patch={} has_fn_output={}",
                    instr.contains("compact subsession assistant"),
                    has_apply,
                    has_fn_output,
                );
            }
        }
    }

    // Verify the file created by the subsession exists with the expected contents.
    let created_path = cwd.path().join("subsession.txt");
    let contents = std::fs::read_to_string(&created_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", created_path.display()));
    assert_eq!(contents, "Hello from subsession\n");

    Ok(())
}
