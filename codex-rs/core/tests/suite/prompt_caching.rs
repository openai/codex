#![allow(clippy::unwrap_used)]

use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_core::protocol_config_types::ReasoningSummary;
use codex_core::shell::Shell;
use codex_core::shell::default_user_shell;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn text_user_input(text: String) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": text } ]
    })
}

fn default_env_context_str(cwd: &str, shell: &Shell) -> String {
    format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <approval_policy>on-request</approval_policy>
  <sandbox_mode>read-only</sandbox_mode>
  <network_access>restricted</network_access>
{}</environment_context>"#,
        cwd,
        match shell.name() {
            Some(name) => format!("  <shell>{name}</shell>\n"),
            None => String::new(),
        }
    )
}

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

fn assert_tool_names(body: &serde_json::Value, expected_names: &[&str]) {
    assert_eq!(
        body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        expected_names
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn codex_mini_latest_tools() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect two POSTs to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    config.include_apply_patch_tool = false;
    config.model = "codex-mini-latest".to_string();
    config.model_family = find_family_for_model("codex-mini-latest").unwrap();

    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let expected_instructions = [
        include_str!("../../prompt.md"),
        include_str!("../../../apply-patch/apply_patch_tool_instructions.md"),
    ]
    .join("\n");

    let body0 = requests[0].body_json::<serde_json::Value>().unwrap();
    assert_eq!(
        body0["instructions"],
        serde_json::json!(expected_instructions),
    );
    let body1 = requests[1].body_json::<serde_json::Value>().unwrap();
    assert_eq!(
        body1["instructions"],
        serde_json::json!(expected_instructions),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn prompt_tools_are_consistent_across_requests() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect two POSTs to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());
    config.include_apply_patch_tool = true;
    config.include_plan_tool = true;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let expected_instructions = config.model_family.base_instructions.clone();
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    // our internal implementation is responsible for keeping tools in sync
    // with the OpenAI schema, so we just verify the tool presence here
    let expected_tools_names: &[&str] = &[
        "shell",
        "update_plan",
        "apply_patch",
        "read_file",
        "view_image",
    ];
    let body0 = requests[0].body_json::<serde_json::Value>().unwrap();
    assert_eq!(
        body0["instructions"],
        serde_json::json!(expected_instructions),
    );
    assert_tool_names(&body0, expected_tools_names);

    let body1 = requests[1].body_json::<serde_json::Value>().unwrap();
    assert_eq!(
        body1["instructions"],
        serde_json::json!(expected_instructions),
    );
    assert_tool_names(&body1, expected_tools_names);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefixes_context_and_instructions_once_and_consistently_across_requests() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect two POSTs to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let shell = default_user_shell().await;

    let expected_env_text = format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <approval_policy>on-request</approval_policy>
  <sandbox_mode>read-only</sandbox_mode>
  <network_access>restricted</network_access>
{}</environment_context>"#,
        cwd.path().to_string_lossy(),
        match shell.name() {
            Some(name) => format!("  <shell>{name}</shell>\n"),
            None => String::new(),
        }
    );
    let expected_ui_text =
        "<user_instructions>\n\nbe consistent and helpful\n\n</user_instructions>";

    let expected_env_msg = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_env_text } ]
    });
    let expected_ui_msg = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_ui_text } ]
    });

    let expected_user_message_1 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": "hello 1" } ]
    });
    let body1 = requests[0].body_json::<serde_json::Value>().unwrap();
    assert_eq!(
        body1["input"],
        serde_json::json!([expected_ui_msg, expected_env_msg, expected_user_message_1])
    );

    let expected_user_message_2 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": "hello 2" } ]
    });
    let body2 = requests[1].body_json::<serde_json::Value>().unwrap();
    let expected_body2 = serde_json::json!(
        [
            body1["input"].as_array().unwrap().as_slice(),
            [expected_user_message_2].as_slice(),
        ]
        .concat()
    );
    assert_eq!(body2["input"], expected_body2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overrides_turn_context_but_keeps_cached_prefix_and_key_constant() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect two POSTs to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    // First turn
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let writable = TempDir::new().unwrap();
    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![writable.path().to_path_buf()],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }),
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: Some(ReasoningSummary::Detailed),
        })
        .await
        .unwrap();

    // Second turn after overrides
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Verify we issued exactly two requests, and the cached prefix stayed identical.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let body1 = requests[0].body_json::<serde_json::Value>().unwrap();
    let body2 = requests[1].body_json::<serde_json::Value>().unwrap();
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
    // After overriding the turn context, the environment context should be emitted again
    // reflecting the new approval policy and sandbox settings. Omit cwd because it did
    // not change.
    let expected_env_text_2 = format!(
        r#"<environment_context>
  <approval_policy>never</approval_policy>
  <sandbox_mode>workspace-write</sandbox_mode>
  <network_access>enabled</network_access>
  <writable_roots>
    <root>{}</root>
  </writable_roots>
</environment_context>"#,
        writable.path().to_string_lossy()
    );
    let expected_env_msg_2 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_env_text_2 } ]
    });
    let expected_body2 = serde_json::json!(
        [
            body1["input"].as_array().unwrap().as_slice(),
            [expected_env_msg_2, expected_user_message_2].as_slice(),
        ]
        .concat()
    );
    assert_eq!(body2["input"], expected_body2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_turn_overrides_keep_cached_prefix_and_key_constant() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect two POSTs to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    // First turn
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Second turn using per-turn overrides via UserTurn
    let new_cwd = TempDir::new().unwrap();
    let writable = TempDir::new().unwrap();
    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
            cwd: new_cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![writable.path().to_path_buf()],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            model: "o3".to_string(),
            effort: Some(ReasoningEffort::High),
            summary: ReasoningSummary::Detailed,
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Verify we issued exactly two requests, and the cached prefix stayed identical.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let body1 = requests[0].body_json::<serde_json::Value>().unwrap();
    let body2 = requests[1].body_json::<serde_json::Value>().unwrap();

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
    let expected_env_text_2 = format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <approval_policy>never</approval_policy>
  <sandbox_mode>workspace-write</sandbox_mode>
  <network_access>enabled</network_access>
  <writable_roots>
    <root>{}</root>
  </writable_roots>
</environment_context>"#,
        new_cwd.path().to_string_lossy(),
        writable.path().to_string_lossy(),
    );
    let expected_env_msg_2 = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_env_text_2 } ]
    });
    let expected_body2 = serde_json::json!(
        [
            body1["input"].as_array().unwrap().as_slice(),
            [expected_env_msg_2, expected_user_message_2].as_slice(),
        ]
        .concat()
    );
    assert_eq!(body2["input"], expected_body2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_user_turn_with_no_changes_does_not_send_environment_context() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let default_cwd = config.cwd.clone();
    let default_approval_policy = config.approval_policy;
    let default_sandbox_policy = config.sandbox_policy.clone();
    let default_model = config.model.clone();
    let default_effort = config.model_reasoning_effort;
    let default_summary = config.model_reasoning_summary;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy.clone(),
            model: default_model.clone(),
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy.clone(),
            model: default_model.clone(),
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let body1 = requests[0].body_json::<serde_json::Value>().unwrap();
    let body2 = requests[1].body_json::<serde_json::Value>().unwrap();

    let shell = default_user_shell().await;
    let expected_ui_text =
        "<user_instructions>\n\nbe consistent and helpful\n\n</user_instructions>";
    let expected_ui_msg = text_user_input(expected_ui_text.to_string());

    let expected_env_msg_1 = text_user_input(default_env_context_str(
        &cwd.path().to_string_lossy(),
        &shell,
    ));
    let expected_user_message_1 = text_user_input("hello 1".to_string());

    let expected_input_1 = serde_json::Value::Array(vec![
        expected_ui_msg.clone(),
        expected_env_msg_1.clone(),
        expected_user_message_1.clone(),
    ]);
    assert_eq!(body1["input"], expected_input_1);

    let expected_user_message_2 = text_user_input("hello 2".to_string());
    let expected_input_2 = serde_json::Value::Array(vec![
        expected_ui_msg,
        expected_env_msg_1,
        expected_user_message_1,
        expected_user_message_2,
    ]);
    assert_eq!(body2["input"], expected_input_2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_user_turn_with_changes_sends_environment_context() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let default_cwd = config.cwd.clone();
    let default_approval_policy = config.approval_policy;
    let default_sandbox_policy = config.sandbox_policy.clone();
    let default_model = config.model.clone();
    let default_effort = config.model_reasoning_effort;
    let default_summary = config.model_reasoning_summary;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config.clone())
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy.clone(),
            model: default_model,
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
            cwd: default_cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: "o3".to_string(),
            effort: Some(ReasoningEffort::High),
            summary: ReasoningSummary::Detailed,
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let body1 = requests[0].body_json::<serde_json::Value>().unwrap();
    let body2 = requests[1].body_json::<serde_json::Value>().unwrap();

    let shell = default_user_shell().await;
    let expected_ui_text =
        "<user_instructions>\n\nbe consistent and helpful\n\n</user_instructions>";
    let expected_ui_msg = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_ui_text } ]
    });
    let expected_env_text_1 = default_env_context_str(&default_cwd.to_string_lossy(), &shell);
    let expected_env_msg_1 = text_user_input(expected_env_text_1);
    let expected_user_message_1 = text_user_input("hello 1".to_string());
    let expected_input_1 = serde_json::Value::Array(vec![
        expected_ui_msg.clone(),
        expected_env_msg_1.clone(),
        expected_user_message_1.clone(),
    ]);
    assert_eq!(body1["input"], expected_input_1);

    let expected_env_msg_2 = text_user_input(format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <approval_policy>never</approval_policy>
  <sandbox_mode>danger-full-access</sandbox_mode>
  <network_access>enabled</network_access>
</environment_context>"#,
        default_cwd.to_string_lossy()
    ));
    let expected_user_message_2 = text_user_input("hello 2".to_string());
    let expected_input_2 = serde_json::Value::Array(vec![
        expected_ui_msg,
        expected_env_msg_1,
        expected_user_message_1,
        expected_env_msg_2,
        expected_user_message_2,
    ]);
    assert_eq!(body2["input"], expected_input_2);
}
