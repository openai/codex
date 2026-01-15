#![allow(clippy::unwrap_used)]

use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use codex_core::features::Feature;
use codex_core::models_manager::model_info::BASE_INSTRUCTIONS;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningSummary;
use codex_core::shell::Shell;
use codex_core::shell::default_user_shell;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use tempfile::TempDir;

fn text_user_input(text: String) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": text } ]
    })
}

fn default_env_context_str(cwd: &str, shell: &Shell) -> String {
    let shell_name = shell.name();
    format!(
        r#"<environment_context>
  <cwd>{cwd}</cwd>
  <shell>{shell_name}</shell>
</environment_context>"#
    )
}

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
}

fn assert_tool_names(body: &serde_json::Value, expected_names: &[&str]) {
    assert_eq!(
        body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                t.get("name")
                    .and_then(|value| value.as_str())
                    .or_else(|| t.get("type").and_then(|value| value.as_str()))
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>(),
        expected_names
    );
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn prompt_tools_are_consistent_across_requests() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let TestCodex {
        codex,
        config,
        thread_manager,
        ..
    } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
            config.model = Some("gpt-5.1-codex-max".to_string());
            // Keep tool expectations stable when the default web_search mode changes.
            config.web_search_mode = WebSearchMode::Cached;
        })
        .build(&server)
        .await?;
    let base_instructions = thread_manager
        .get_models_manager()
        .get_model_info(
            config
                .model
                .as_deref()
                .expect("test config should have a model"),
            &config,
        )
        .await
        .base_instructions;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let expected_tools_names = vec![
        "shell_command",
        "list_mcp_resources",
        "list_mcp_resource_templates",
        "read_mcp_resource",
        "update_plan",
        "apply_patch",
        "web_search",
        "view_image",
    ];
    let body0 = req1.single_request().body_json();

    let expected_instructions = if expected_tools_names.contains(&"apply_patch") {
        base_instructions
    } else {
        [base_instructions, APPLY_PATCH_TOOL_INSTRUCTIONS.to_string()].join("\n")
    };

    assert_eq!(
        body0["instructions"],
        serde_json::json!(expected_instructions),
    );
    assert_tool_names(&body0, &expected_tools_names);

    let body1 = req2.single_request().body_json();
    assert_eq!(
        body1["instructions"],
        serde_json::json!(expected_instructions),
    );
    assert_tool_names(&body1, &expected_tools_names);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_mini_latest_tools() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
            config.features.disable(Feature::ApplyPatchFreeform);
            config.model = Some("codex-mini-latest".to_string());
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let expected_instructions = [BASE_INSTRUCTIONS, APPLY_PATCH_TOOL_INSTRUCTIONS].join("\n");

    let body0 = req1.single_request().body_json();
    let instructions0 = body0["instructions"]
        .as_str()
        .expect("instructions should be a string");
    assert_eq!(
        normalize_newlines(instructions0),
        normalize_newlines(&expected_instructions)
    );

    let body1 = req2.single_request().body_json();
    let instructions1 = body1["instructions"]
        .as_str()
        .expect("instructions should be a string");
    assert_eq!(
        normalize_newlines(instructions1),
        normalize_newlines(&expected_instructions)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefixes_context_and_instructions_once_and_consistently_across_requests()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let TestCodex { codex, config, .. } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let input1 = body1["input"].as_array().expect("input array");
    assert_eq!(
        input1.len(),
        4,
        "expected permissions + cached prefix + env + user msg"
    );

    let ui_text = input1[1]["content"][0]["text"]
        .as_str()
        .expect("ui message text");
    assert!(
        ui_text.contains("be consistent and helpful"),
        "expected user instructions in UI message: {ui_text}"
    );

    let shell = default_user_shell();
    let cwd_str = config.cwd.to_string_lossy();
    let expected_env_text = default_env_context_str(&cwd_str, &shell);
    assert_eq!(
        input1[2],
        text_user_input(expected_env_text),
        "expected environment context after UI message"
    );
    assert_eq!(input1[3], text_user_input("hello 1".to_string()));

    let body2 = req2.single_request().body_json();
    let input2 = body2["input"].as_array().expect("input array");
    assert_eq!(
        &input2[..input1.len()],
        input1.as_slice(),
        "expected cached prefix to be reused"
    );
    assert_eq!(input2[input1.len()], text_user_input("hello 2".to_string()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overrides_turn_context_but_keeps_cached_prefix_and_key_constant() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
        })
        .build(&server)
        .await?;

    // First turn
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let writable = TempDir::new().unwrap();
    let new_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![writable.path().try_into().unwrap()],
        network_access: true,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(new_policy.clone()),
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: Some(ReasoningSummary::Detailed),
        })
        .await?;

    // Second turn after overrides
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let body2 = req2.single_request().body_json();
    // prompt_cache_key should remain constant across overrides
    assert_eq!(
        body1["prompt_cache_key"], body2["prompt_cache_key"],
        "prompt_cache_key should not change across overrides"
    );

    // The entire prefix from the first request should be identical and reused
    // as the prefix of the second request, ensuring cache hit potential.
    let expected_user_message_2 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": "hello 2" } ]
    });
    let expected_permissions_msg = body1["input"][0].clone();
    // After overriding the turn context, emit a new permissions message.
    let body1_input = body1["input"].as_array().expect("input array");
    let expected_permissions_msg_2 = body2["input"][body1_input.len()].clone();
    assert_ne!(
        expected_permissions_msg_2, expected_permissions_msg,
        "expected updated permissions message after override"
    );
    let mut expected_body2 = body1["input"].as_array().expect("input array").to_vec();
    expected_body2.push(expected_permissions_msg_2);
    expected_body2.push(expected_user_message_2);
    assert_eq!(body2["input"], serde_json::Value::Array(expected_body2));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_before_first_turn_emits_environment_context() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
        })
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first message".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body = req.single_request().body_json();
    let input = body["input"]
        .as_array()
        .expect("input array must be present");
    assert!(
        !input.is_empty(),
        "expected at least environment context and user message"
    );

    let env_texts: Vec<&str> = input
        .iter()
        .filter_map(|msg| {
            msg["content"]
                .as_array()
                .and_then(|content| content.first())
                .and_then(|item| item["text"].as_str())
        })
        .filter(|text| text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG))
        .collect();
    assert!(
        !env_texts.is_empty(),
        "expected environment context to be emitted: {env_texts:?}"
    );

    let env_count = input
        .iter()
        .filter(|msg| {
            msg["content"]
                .as_array()
                .and_then(|content| {
                    content.iter().find(|item| {
                        item["type"].as_str() == Some("input_text")
                            && item["text"]
                                .as_str()
                                .map(|text| text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG))
                                .unwrap_or(false)
                    })
                })
                .is_some()
        })
        .count();
    assert!(
        env_count >= 1,
        "environment context should appear at least once, found {env_count}"
    );

    let permissions_texts: Vec<&str> = input
        .iter()
        .filter_map(|msg| {
            let role = msg["role"].as_str()?;
            if role != "developer" {
                return None;
            }
            msg["content"]
                .as_array()
                .and_then(|content| content.first())
                .and_then(|item| item["text"].as_str())
        })
        .collect();
    assert!(
        permissions_texts
            .iter()
            .any(|text| text.contains("`approval_policy` is `never`")),
        "permissions message should reflect overridden approval policy: {permissions_texts:?}"
    );

    let user_texts: Vec<&str> = input
        .iter()
        .filter_map(|msg| {
            msg["content"]
                .as_array()
                .and_then(|content| content.first())
                .and_then(|item| item["text"].as_str())
        })
        .collect();
    assert!(
        user_texts.contains(&"first message"),
        "expected user message text, got {user_texts:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_turn_overrides_keep_cached_prefix_and_key_constant() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
        })
        .build(&server)
        .await?;

    // First turn
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Second turn using per-turn overrides via UserTurn
    let new_cwd = TempDir::new().unwrap();
    let writable = TempDir::new().unwrap();
    let new_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![AbsolutePathBuf::try_from(writable.path()).unwrap()],
        network_access: true,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            cwd: new_cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: new_policy.clone(),
            model: "o3".to_string(),
            effort: Some(ReasoningEffort::High),
            summary: ReasoningSummary::Detailed,
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let body2 = req2.single_request().body_json();

    // prompt_cache_key should remain constant across per-turn overrides
    assert_eq!(
        body1["prompt_cache_key"], body2["prompt_cache_key"],
        "prompt_cache_key should not change across per-turn overrides"
    );

    // The entire prefix from the first request should be identical and reused
    // as the prefix of the second request.
    let expected_user_message_2 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": "hello 2" } ]
    });
    let shell = default_user_shell();

    let expected_env_text_2 = format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <shell>{}</shell>
</environment_context>"#,
        new_cwd.path().display(),
        shell.name()
    );
    let expected_env_msg_2 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_env_text_2 } ]
    });
    let expected_permissions_msg = body1["input"][0].clone();
    let body1_input = body1["input"].as_array().expect("input array");
    let expected_permissions_msg_2 = body2["input"][body1_input.len() + 1].clone();
    assert_ne!(
        expected_permissions_msg_2, expected_permissions_msg,
        "expected updated permissions message after per-turn override"
    );
    let mut expected_body2 = body1_input.to_vec();
    expected_body2.push(expected_env_msg_2);
    expected_body2.push(expected_permissions_msg_2);
    expected_body2.push(expected_user_message_2);
    assert_eq!(body2["input"], serde_json::Value::Array(expected_body2));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_user_turn_with_no_changes_does_not_send_environment_context() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
        })
        .build(&server)
        .await?;

    let default_cwd = config.cwd.clone();
    let default_approval_policy = config.approval_policy.value();
    let default_sandbox_policy = config.sandbox_policy.get();
    let default_model = session_configured.model;
    let default_effort = config.model_reasoning_effort;
    let default_summary = config.model_reasoning_summary;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy.clone(),
            model: default_model.clone(),
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy.clone(),
            model: default_model.clone(),
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let body2 = req2.single_request().body_json();

    let expected_permissions_msg = body1["input"][0].clone();
    let expected_ui_msg = body1["input"][1].clone();

    let shell = default_user_shell();
    let default_cwd_lossy = default_cwd.to_string_lossy();

    let expected_env_msg_1 = text_user_input(default_env_context_str(&default_cwd_lossy, &shell));
    let expected_user_message_1 = text_user_input("hello 1".to_string());

    let expected_input_1 = serde_json::Value::Array(vec![
        expected_permissions_msg.clone(),
        expected_ui_msg.clone(),
        expected_env_msg_1.clone(),
        expected_user_message_1.clone(),
    ]);
    assert_eq!(body1["input"], expected_input_1);

    let expected_user_message_2 = text_user_input("hello 2".to_string());
    let expected_input_2 = serde_json::Value::Array(vec![
        expected_permissions_msg,
        expected_ui_msg,
        expected_env_msg_1,
        expected_user_message_1,
        expected_user_message_2,
    ]);
    assert_eq!(body2["input"], expected_input_2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_user_turn_with_changes_sends_environment_context() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    let server = start_mock_server().await;

    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;
    let TestCodex {
        codex,
        config,
        session_configured,
        ..
    } = test_codex()
        .with_config(|config| {
            config.user_instructions = Some("be consistent and helpful".to_string());
        })
        .build(&server)
        .await?;

    let default_cwd = config.cwd.clone();
    let default_approval_policy = config.approval_policy.value();
    let default_sandbox_policy = config.sandbox_policy.get();
    let default_model = session_configured.model;
    let default_effort = config.model_reasoning_effort;
    let default_summary = config.model_reasoning_summary;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy.clone(),
            model: default_model,
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: "o3".to_string(),
            effort: Some(ReasoningEffort::High),
            summary: ReasoningSummary::Detailed,
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let body2 = req2.single_request().body_json();

    let expected_permissions_msg = body1["input"][0].clone();
    let expected_ui_msg = body1["input"][1].clone();

    let shell = default_user_shell();
    let expected_env_text_1 = default_env_context_str(&default_cwd.to_string_lossy(), &shell);
    let expected_env_msg_1 = text_user_input(expected_env_text_1);
    let expected_user_message_1 = text_user_input("hello 1".to_string());
    let expected_input_1 = serde_json::Value::Array(vec![
        expected_permissions_msg.clone(),
        expected_ui_msg.clone(),
        expected_env_msg_1.clone(),
        expected_user_message_1.clone(),
    ]);
    assert_eq!(body1["input"], expected_input_1);

    let body1_input = body1["input"].as_array().expect("input array");
    let expected_permissions_msg_2 = body2["input"][body1_input.len()].clone();
    assert_ne!(
        expected_permissions_msg_2, expected_permissions_msg,
        "expected updated permissions message after policy change"
    );
    let expected_user_message_2 = text_user_input("hello 2".to_string());
    let expected_input_2 = serde_json::Value::Array(vec![
        expected_permissions_msg,
        expected_ui_msg,
        expected_env_msg_1,
        expected_user_message_1,
        expected_permissions_msg_2,
        expected_user_message_2,
    ]);
    assert_eq!(body2["input"], expected_input_2);

    Ok(())
}
