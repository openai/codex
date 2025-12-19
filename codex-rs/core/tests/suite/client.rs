use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::ContentItem;
use codex_core::ConversationManager;
use codex_core::LocalShellAction;
use codex_core::LocalShellExecAction;
use codex_core::LocalShellStatus;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::built_in_model_providers;
use codex_core::error::CodexErr;
use codex_core::features::Feature;
use codex_core::openai_models::models_manager::ModelsManager;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SessionSource;
use codex_otel::otel_manager::OtelManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::WebSearchAction;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::get_responses_requests;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::sse_failed;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use dunce::canonicalize as normalize_path;
use futures::StreamExt;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[expect(clippy::unwrap_used)]
fn assert_message_role(request_body: &serde_json::Value, role: &str) {
    assert_eq!(request_body["role"].as_str().unwrap(), role);
}

#[expect(clippy::expect_used)]
fn assert_message_equals(request_body: &serde_json::Value, text: &str) {
    let content = request_body["content"][0]["text"]
        .as_str()
        .expect("invalid message content");

    assert_eq!(
        content, text,
        "expected message content '{content}' to equal '{text}'"
    );
}

#[expect(clippy::expect_used)]
fn assert_message_starts_with(request_body: &serde_json::Value, text: &str) {
    let content = request_body["content"][0]["text"]
        .as_str()
        .expect("invalid message content");

    assert!(
        content.starts_with(text),
        "expected message content '{content}' to start with '{text}'"
    );
}

#[expect(clippy::expect_used)]
fn assert_message_ends_with(request_body: &serde_json::Value, text: &str) {
    let content = request_body["content"][0]["text"]
        .as_str()
        .expect("invalid message content");

    assert!(
        content.ends_with(text),
        "expected message content '{content}' to end with '{text}'"
    );
}

/// Writes an `auth.json` into the provided `codex_home` with the specified parameters.
/// Returns the fake JWT string written to `tokens.id_token`.
#[expect(clippy::unwrap_used)]
fn write_auth_json(
    codex_home: &TempDir,
    openai_api_key: Option<&str>,
    chatgpt_plan_type: &str,
    access_token: &str,
    account_id: Option<&str>,
) -> String {
    use base64::Engine as _;

    let header = json!({ "alg": "none", "typ": "JWT" });
    let payload = json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": chatgpt_plan_type,
            "chatgpt_account_id": account_id.unwrap_or("acc-123")
        }
    });

    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let header_b64 = b64(&serde_json::to_vec(&header).unwrap());
    let payload_b64 = b64(&serde_json::to_vec(&payload).unwrap());
    let signature_b64 = b64(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

    let mut tokens = json!({
        "id_token": fake_jwt,
        "access_token": access_token,
        "refresh_token": "refresh-test",
    });
    if let Some(acc) = account_id {
        tokens["account_id"] = json!(acc);
    }

    let auth_json = json!({
        "OPENAI_API_KEY": openai_api_key,
        "tokens": tokens,
        // RFC3339 datetime; value doesn't matter for these tests
        "last_refresh": chrono::Utc::now(),
    });

    std::fs::write(
        codex_home.path().join("auth.json"),
        serde_json::to_string_pretty(&auth_json).unwrap(),
    )
    .unwrap();

    fake_jwt
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_includes_initial_messages_and_sends_prior_items() {
    skip_if_no_network!();

    // Create a fake rollout session file with prior user + system + assistant messages.
    let tmpdir = TempDir::new().unwrap();
    let session_path = tmpdir.path().join("resume-session.jsonl");
    let mut f = std::fs::File::create(&session_path).unwrap();
    let convo_id = Uuid::new_v4();
    writeln!(
        f,
        "{}",
        json!({
            "timestamp": "2024-01-01T00:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "id": convo_id,
                "timestamp": "2024-01-01T00:00:00Z",
                "instructions": "be nice",
                "cwd": ".",
                "originator": "test_originator",
                "cli_version": "test_version",
                "model_provider": "test-provider"
            }
        })
    )
    .unwrap();

    // Prior item: user message (should be delivered)
    let prior_user = codex_protocol::models::ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![codex_protocol::models::ContentItem::InputText {
            text: "resumed user message".to_string(),
        }],
    };
    let prior_user_json = serde_json::to_value(&prior_user).unwrap();
    writeln!(
        f,
        "{}",
        json!({
            "timestamp": "2024-01-01T00:00:01.000Z",
            "type": "response_item",
            "payload": prior_user_json
        })
    )
    .unwrap();

    // Prior item: system message (excluded from API history)
    let prior_system = codex_protocol::models::ResponseItem::Message {
        id: None,
        role: "system".to_string(),
        content: vec![codex_protocol::models::ContentItem::OutputText {
            text: "resumed system instruction".to_string(),
        }],
    };
    let prior_system_json = serde_json::to_value(&prior_system).unwrap();
    writeln!(
        f,
        "{}",
        json!({
            "timestamp": "2024-01-01T00:00:02.000Z",
            "type": "response_item",
            "payload": prior_system_json
        })
    )
    .unwrap();

    // Prior item: assistant message
    let prior_item = codex_protocol::models::ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![codex_protocol::models::ContentItem::OutputText {
            text: "resumed assistant message".to_string(),
        }],
    };
    let prior_item_json = serde_json::to_value(&prior_item).unwrap();
    writeln!(
        f,
        "{}",
        json!({
            "timestamp": "2024-01-01T00:00:03.000Z",
            "type": "response_item",
            "payload": prior_item_json
        })
    )
    .unwrap();
    drop(f);

    // Mock server that will receive the resumed request
    let server = MockServer::start().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    // Configure Codex to resume from our file
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;
    // Also configure user instructions to ensure they are NOT delivered on resume.
    config.user_instructions = Some("be nice".to_string());

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let NewConversation {
        conversation: codex,
        session_configured,
        ..
    } = conversation_manager
        .resume_conversation_from_rollout(config, session_path.clone(), auth_manager)
        .await
        .expect("resume conversation");

    // 1) Assert initial_messages only includes existing EventMsg entries; response items are not converted
    let initial_msgs = session_configured
        .initial_messages
        .clone()
        .expect("expected initial messages option for resumed session");
    let initial_json = serde_json::to_value(&initial_msgs).unwrap();
    let expected_initial_json = json!([]);
    assert_eq!(initial_json, expected_initial_json);

    // 2) Submit new input; the request body must include the prior item followed by the new user input.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();
    let expected_input = json!([
        {
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": "resumed user message" }]
        },
        {
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": "resumed assistant message" }]
        },
        {
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": "hello" }]
        }
    ]);
    assert_eq!(request_body["input"], expected_input);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_conversation_id_and_model_headers_in_request() {
    skip_if_no_network!();

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let NewConversation {
        conversation: codex,
        conversation_id,
        session_configured: _,
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let requests = get_responses_requests(&server).await;
    let request = requests
        .first()
        .expect("expected POST request to /responses");
    let request_conversation_id = request.headers.get("conversation_id").unwrap();
    let request_authorization = request.headers.get("authorization").unwrap();
    let request_originator = request.headers.get("originator").unwrap();

    assert_eq!(
        request_conversation_id.to_str().unwrap(),
        conversation_id.to_string()
    );
    assert_eq!(request_originator.to_str().unwrap(), "codex_cli_rs");
    assert_eq!(
        request_authorization.to_str().unwrap(),
        "Bearer Test API Key"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_base_instructions_override_in_request() {
    skip_if_no_network!();
    // Mock server
    let server = MockServer::start().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;

    config.base_instructions = Some("test instructions".to_string());
    config.model_provider = model_provider;

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert!(
        request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("test instructions")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chatgpt_auth_sends_correct_request() {
    skip_if_no_network!();

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/api/codex/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/api/codex", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;
    let conversation_manager = ConversationManager::with_models_provider_and_home(
        create_dummy_codex_auth(),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let NewConversation {
        conversation: codex,
        conversation_id,
        session_configured: _,
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let requests = get_responses_requests(&server).await;
    let request = requests
        .first()
        .expect("expected POST request to /responses");
    let request_conversation_id = request.headers.get("conversation_id").unwrap();
    let request_authorization = request.headers.get("authorization").unwrap();
    let request_originator = request.headers.get("originator").unwrap();
    let request_chatgpt_account_id = request.headers.get("chatgpt-account-id").unwrap();
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert_eq!(
        request_conversation_id.to_str().unwrap(),
        conversation_id.to_string()
    );
    assert_eq!(request_originator.to_str().unwrap(), "codex_cli_rs");
    assert_eq!(
        request_authorization.to_str().unwrap(),
        "Bearer Access Token"
    );
    assert_eq!(request_chatgpt_account_id.to_str().unwrap(), "account_id");
    assert!(request_body["stream"].as_bool().unwrap());
    assert_eq!(
        request_body["include"][0].as_str().unwrap(),
        "reasoning.encrypted_content"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefers_apikey_when_config_prefers_apikey_even_with_chatgpt_tokens() {
    skip_if_no_network!();

    // Mock server
    let server = MockServer::start().await;

    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect API key header, no ChatGPT account header required.
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header_regex("Authorization", r"Bearer sk-test-key"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    // Write auth.json that contains both API key and ChatGPT tokens for a plan that should prefer ChatGPT,
    // but config will force API key preference.
    let _jwt = write_auth_json(
        &codex_home,
        Some("sk-test-key"),
        "pro",
        "Access-123",
        Some("acc-123"),
    );

    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;

    let auth_manager =
        match CodexAuth::from_auth_storage(codex_home.path(), AuthCredentialsStoreMode::File) {
            Ok(Some(auth)) => codex_core::AuthManager::from_auth_for_testing(auth),
            Ok(None) => panic!("No CodexAuth found in codex_home"),
            Err(e) => panic!("Failed to load CodexAuth: {e}"),
        };
    let conversation_manager = ConversationManager::new(auth_manager, SessionSource::Exec);
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_user_instructions_message_in_request() {
    skip_if_no_network!();
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;
    config.user_instructions = Some("be nice".to_string());

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert!(
        !request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("be nice")
    );
    assert_message_role(&request_body["input"][0], "user");
    assert_message_starts_with(&request_body["input"][0], "# AGENTS.md instructions for ");
    assert_message_ends_with(&request_body["input"][0], "</INSTRUCTIONS>");
    let ui_text = request_body["input"][0]["content"][0]["text"]
        .as_str()
        .expect("invalid message content");
    assert!(ui_text.contains("<INSTRUCTIONS>"));
    assert!(ui_text.contains("be nice"));
    assert_message_role(&request_body["input"][1], "user");
    assert_message_starts_with(&request_body["input"][1], "<environment_context>");
    assert_message_ends_with(&request_body["input"][1], "</environment_context>");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skills_append_to_instructions() {
    skip_if_no_network!();
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let skill_dir = codex_home.path().join("skills/demo");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo\ndescription: build charts\n---\n\n# body\n",
    )
    .expect("write skill");

    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;
    config.cwd = codex_home.path().to_path_buf();
    config.features.enable(Feature::Skills);

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert_message_role(&request_body["input"][0], "user");
    let instructions_text = request_body["input"][0]["content"][0]["text"]
        .as_str()
        .expect("instructions text");
    assert!(
        instructions_text.contains("## Skills"),
        "expected skills section present"
    );
    assert!(
        instructions_text.contains("demo: build charts"),
        "expected skill summary"
    );
    let expected_path = normalize_path(skill_dir.join("SKILL.md")).unwrap();
    let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
    assert!(
        instructions_text.contains(&expected_path_str),
        "expected path {expected_path_str} in instructions"
    );
    let _codex_home_guard = codex_home;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_configured_effort_in_request() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex()
        .with_model("gpt-5.1-codex")
        .with_config(|config| {
            config.model_reasoning_effort = Some(ReasoningEffort::Medium);
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert_eq!(
        request_body
            .get("reasoning")
            .and_then(|t| t.get("effort"))
            .and_then(|v| v.as_str()),
        Some("medium")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_no_effort_in_request() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex()
        .with_model("gpt-5.1-codex")
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert_eq!(
        request_body
            .get("reasoning")
            .and_then(|t| t.get("effort"))
            .and_then(|v| v.as_str()),
        None
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_default_reasoning_effort_in_request_when_defined_by_model_family()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex().with_model("gpt-5.1").build(&server).await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert_eq!(
        request_body
            .get("reasoning")
            .and_then(|t| t.get("effort"))
            .and_then(|v| v.as_str()),
        Some("medium")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_reasoning_summary_is_sent() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.model_reasoning_summary = ReasoningSummary::Concise;
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    pretty_assertions::assert_eq!(
        request_body
            .get("reasoning")
            .and_then(|reasoning| reasoning.get("summary"))
            .and_then(|value| value.as_str()),
        Some("concise")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_summary_is_omitted_when_disabled() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.model_reasoning_summary = ReasoningSummary::None;
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    pretty_assertions::assert_eq!(
        request_body
            .get("reasoning")
            .and_then(|reasoning| reasoning.get("summary")),
        None
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_default_verbosity_in_request() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex().with_model("gpt-5.1").build(&server).await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert_eq!(
        request_body
            .get("text")
            .and_then(|t| t.get("verbosity"))
            .and_then(|v| v.as_str()),
        Some("low")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_verbosity_not_sent_for_models_without_support() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex()
        .with_model("gpt-5.1-codex")
        .with_config(|config| {
            config.model_verbosity = Some(Verbosity::High);
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert!(
        request_body
            .get("text")
            .and_then(|t| t.get("verbosity"))
            .is_none()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_verbosity_is_sent() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;
    let TestCodex { codex, .. } = test_codex()
        .with_model("gpt-5.1")
        .with_config(|config| {
            config.model_verbosity = Some(Verbosity::High);
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert_eq!(
        request_body
            .get("text")
            .and_then(|t| t.get("verbosity"))
            .and_then(|v| v.as_str()),
        Some("high")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_developer_instructions_message_in_request() {
    skip_if_no_network!();
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;
    config.user_instructions = Some("be nice".to_string());
    config.developer_instructions = Some("be useful".to_string());

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();

    assert!(
        !request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("be nice")
    );
    assert_message_role(&request_body["input"][0], "developer");
    assert_message_equals(&request_body["input"][0], "be useful");
    assert_message_role(&request_body["input"][1], "user");
    assert_message_starts_with(&request_body["input"][1], "# AGENTS.md instructions for ");
    assert_message_ends_with(&request_body["input"][1], "</INSTRUCTIONS>");
    let ui_text = request_body["input"][1]["content"][0]["text"]
        .as_str()
        .expect("invalid message content");
    assert!(ui_text.contains("<INSTRUCTIONS>"));
    assert!(ui_text.contains("be nice"));
    assert_message_role(&request_body["input"][2], "user");
    assert_message_starts_with(&request_body["input"][2], "<environment_context>");
    assert_message_ends_with(&request_body["input"][2], "</environment_context>");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_responses_request_includes_store_and_reasoning_ids() {
    skip_if_no_network!();

    let server = MockServer::start().await;

    let sse_body = concat!(
        "data: {\"type\":\"response.created\",\"response\":{}}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\"}}\n\n",
    );

    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_body, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/openai/responses"))
        .respond_with(template)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "azure".into(),
        base_url: Some(format!("{}/openai", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let model = ModelsManager::get_model_offline(config.model.as_deref());
    config.model = Some(model.clone());
    let config = Arc::new(config);
    let model_family = ModelsManager::construct_model_family_offline(model.as_str(), &config);
    let conversation_id = ConversationId::new();
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_family.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        auth_manager.get_auth_mode(),
        false,
        "test".to_string(),
        SessionSource::Exec,
    );

    let client = ModelClient::new(
        Arc::clone(&config),
        None,
        model_family,
        otel_manager,
        provider,
        effort,
        summary,
        conversation_id,
        SessionSource::Exec,
    );

    let mut prompt = Prompt::default();
    prompt.input.push(ResponseItem::Reasoning {
        id: "reasoning-id".into(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "summary".into(),
        }],
        content: Some(vec![ReasoningItemContent::ReasoningText {
            text: "content".into(),
        }]),
        encrypted_content: None,
    });
    prompt.input.push(ResponseItem::Message {
        id: Some("message-id".into()),
        role: "assistant".into(),
        content: vec![ContentItem::OutputText {
            text: "message".into(),
        }],
    });
    prompt.input.push(ResponseItem::WebSearchCall {
        id: Some("web-search-id".into()),
        status: Some("completed".into()),
        action: WebSearchAction::Search {
            query: Some("weather".into()),
        },
    });
    prompt.input.push(ResponseItem::FunctionCall {
        id: Some("function-id".into()),
        name: "do_thing".into(),
        arguments: "{}".into(),
        call_id: "function-call-id".into(),
    });
    prompt.input.push(ResponseItem::LocalShellCall {
        id: Some("local-shell-id".into()),
        call_id: Some("local-shell-call-id".into()),
        status: LocalShellStatus::Completed,
        action: LocalShellAction::Exec(LocalShellExecAction {
            command: vec!["echo".into(), "hello".into()],
            timeout_ms: None,
            working_directory: None,
            env: None,
            user: None,
        }),
    });
    prompt.input.push(ResponseItem::CustomToolCall {
        id: Some("custom-tool-id".into()),
        status: Some("completed".into()),
        call_id: "custom-tool-call-id".into(),
        name: "custom_tool".into(),
        input: "{}".into(),
    });

    let mut stream = client
        .stream(&prompt)
        .await
        .expect("responses stream to start");

    while let Some(event) = stream.next().await {
        if let Ok(ResponseEvent::Completed { .. }) = event {
            break;
        }
    }

    let requests = get_responses_requests(&server).await;
    assert_eq!(requests.len(), 1, "expected a single POST request");
    let body: serde_json::Value = requests[0]
        .body_json()
        .expect("request body to be valid JSON");

    assert_eq!(body["store"], serde_json::Value::Bool(true));
    assert_eq!(body["stream"], serde_json::Value::Bool(true));
    assert_eq!(body["input"].as_array().map(Vec::len), Some(6));
    assert_eq!(body["input"][0]["id"].as_str(), Some("reasoning-id"));
    assert_eq!(body["input"][1]["id"].as_str(), Some("message-id"));
    assert_eq!(body["input"][2]["id"].as_str(), Some("web-search-id"));
    assert_eq!(body["input"][3]["id"].as_str(), Some("function-id"));
    assert_eq!(body["input"][4]["id"].as_str(), Some("local-shell-id"));
    assert_eq!(body["input"][5]["id"].as_str(), Some("custom-tool-id"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_count_includes_rate_limits_snapshot() {
    skip_if_no_network!();
    let server = MockServer::start().await;

    let sse_body = sse(vec![ev_completed_with_tokens("resp_rate", 123)]);

    let response = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .insert_header("x-codex-primary-used-percent", "12.5")
        .insert_header("x-codex-secondary-used-percent", "40.0")
        .insert_header("x-codex-primary-window-minutes", "10")
        .insert_header("x-codex-secondary-window-minutes", "60")
        .insert_header("x-codex-primary-reset-at", "1704069000")
        .insert_header("x-codex-secondary-reset-at", "1704074400")
        .set_body_raw(sse_body, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(response)
        .expect(1)
        .mount(&server)
        .await;

    let mut provider = built_in_model_providers()["openai"].clone();
    provider.base_url = Some(format!("{}/v1", server.uri()));

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home).await;
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("test"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    let first_token_event =
        wait_for_event(&codex, |msg| matches!(msg, EventMsg::TokenCount(_))).await;
    let rate_limit_only = match first_token_event {
        EventMsg::TokenCount(ev) => ev,
        _ => unreachable!(),
    };

    let rate_limit_json = serde_json::to_value(&rate_limit_only).unwrap();
    pretty_assertions::assert_eq!(
        rate_limit_json,
        json!({
            "info": null,
            "rate_limits": {
                "primary": {
                    "used_percent": 12.5,
                    "window_minutes": 10,
                    "resets_at": 1704069000
                },
                "secondary": {
                    "used_percent": 40.0,
                    "window_minutes": 60,
                    "resets_at": 1704074400
                },
                "credits": null,
                "plan_type": null
            }
        })
    );

    let token_event = wait_for_event(
        &codex,
        |msg| matches!(msg, EventMsg::TokenCount(ev) if ev.info.is_some()),
    )
    .await;
    let final_payload = match token_event {
        EventMsg::TokenCount(ev) => ev,
        _ => unreachable!(),
    };
    // Assert full JSON for the final token count event (usage + rate limits)
    let final_json = serde_json::to_value(&final_payload).unwrap();
    pretty_assertions::assert_eq!(
        final_json,
        json!({
            "info": {
                "total_token_usage": {
                    "input_tokens": 123,
                    "cached_input_tokens": 0,
                    "output_tokens": 0,
                    "reasoning_output_tokens": 0,
                    "total_tokens": 123
                },
                "last_token_usage": {
                    "input_tokens": 123,
                    "cached_input_tokens": 0,
                    "output_tokens": 0,
                    "reasoning_output_tokens": 0,
                    "total_tokens": 123
                },
                // Default model is gpt-5.1-codex-max in tests → 95% usable context window
                "model_context_window": 258400
            },
            "rate_limits": {
                "primary": {
                    "used_percent": 12.5,
                    "window_minutes": 10,
                    "resets_at": 1704069000
                },
                "secondary": {
                    "used_percent": 40.0,
                    "window_minutes": 60,
                    "resets_at": 1704074400
                },
                "credits": null,
                "plan_type": null
            }
        })
    );
    let usage = final_payload
        .info
        .expect("token usage info should be recorded after completion");
    assert_eq!(usage.total_token_usage.total_tokens, 123);
    let final_snapshot = final_payload
        .rate_limits
        .expect("latest rate limit snapshot should be retained");
    assert_eq!(
        final_snapshot
            .primary
            .as_ref()
            .map(|window| window.used_percent),
        Some(12.5)
    );
    assert_eq!(
        final_snapshot
            .primary
            .as_ref()
            .and_then(|window| window.resets_at),
        Some(1704069000)
    );

    wait_for_event(&codex, |msg| matches!(msg, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn usage_limit_error_emits_rate_limit_event() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let response = ResponseTemplate::new(429)
        .insert_header("x-codex-primary-used-percent", "100.0")
        .insert_header("x-codex-secondary-used-percent", "87.5")
        .insert_header("x-codex-primary-over-secondary-limit-percent", "95.0")
        .insert_header("x-codex-primary-window-minutes", "15")
        .insert_header("x-codex-secondary-window-minutes", "60")
        .set_body_json(json!({
            "error": {
                "type": "usage_limit_reached",
                "message": "limit reached",
                "resets_at": 1704067242,
                "plan_type": "pro"
            }
        }));

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(response)
        .expect(1)
        .mount(&server)
        .await;

    let mut builder = test_codex();
    let codex_fixture = builder.build(&server).await?;
    let codex = codex_fixture.codex.clone();

    let expected_limits = json!({
        "primary": {
            "used_percent": 100.0,
            "window_minutes": 15,
            "resets_at": null
        },
        "secondary": {
            "used_percent": 87.5,
            "window_minutes": 60,
            "resets_at": null
        },
        "credits": null,
        "plan_type": null
    });

    let submission_id = codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .expect("submission should succeed while emitting usage limit error events");

    let token_event = wait_for_event(&codex, |msg| matches!(msg, EventMsg::TokenCount(_))).await;
    let EventMsg::TokenCount(event) = token_event else {
        unreachable!();
    };

    let event_json = serde_json::to_value(&event).expect("serialize token count event");
    pretty_assertions::assert_eq!(
        event_json,
        json!({
            "info": null,
            "rate_limits": expected_limits
        })
    );

    let error_event = wait_for_event(&codex, |msg| matches!(msg, EventMsg::Error(_))).await;
    let EventMsg::Error(error_event) = error_event else {
        unreachable!();
    };
    assert!(
        error_event.message.to_lowercase().contains("usage limit"),
        "unexpected error message for submission {submission_id}: {}",
        error_event.message
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn context_window_error_sets_total_tokens_to_model_window() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    const EFFECTIVE_CONTEXT_WINDOW: i64 = (272_000 * 95) / 100;

    mount_sse_once_match(
        &server,
        body_string_contains("trigger context window"),
        sse_failed(
            "resp_context_window",
            "context_length_exceeded",
            "Your input exceeds the context window of this model. Please adjust your input and try again.",
        ),
    )
    .await;

    mount_sse_once_match(
        &server,
        body_string_contains("seed turn"),
        sse_completed("resp_seed"),
    )
    .await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.model = Some("gpt-5.1".to_string());
            config.model_context_window = Some(272_000);
        })
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "seed turn".into(),
            }],
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "trigger context window".into(),
            }],
        })
        .await?;

    let token_event = wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::TokenCount(payload)
                if payload.info.as_ref().is_some_and(|info| {
                    info.model_context_window == Some(info.total_token_usage.total_tokens)
                        && info.total_token_usage.total_tokens > 0
                })
        )
    })
    .await;

    let EventMsg::TokenCount(token_payload) = token_event else {
        unreachable!("wait_for_event returned unexpected event");
    };

    let info = token_payload
        .info
        .expect("token usage info present when context window is exceeded");

    assert_eq!(info.model_context_window, Some(EFFECTIVE_CONTEXT_WINDOW));
    assert_eq!(
        info.total_token_usage.total_tokens,
        EFFECTIVE_CONTEXT_WINDOW
    );

    let error_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Error(_))).await;
    let expected_context_window_message = CodexErr::ContextWindowExceeded.to_string();
    assert!(
        matches!(
            error_event,
            EventMsg::Error(ref err) if err.message == expected_context_window_message
        ),
        "expected context window error; got {error_event:?}"
    );

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_overrides_assign_properties_used_for_responses_url() {
    skip_if_no_network!();
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect POST to /openai/responses with api-version query param
    Mock::given(method("POST"))
        .and(path("/openai/responses"))
        .and(query_param("api-version", "2025-04-01-preview"))
        .and(header_regex("Custom-Header", "Value"))
        .and(header_regex(
            "Authorization",
            format!(
                "Bearer {}",
                std::env::var(existing_env_var_with_random_value).unwrap()
            )
            .as_str(),
        ))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "custom".to_string(),
        base_url: Some(format!("{}/openai", server.uri())),
        // Reuse the existing environment variable to avoid using unsafe code
        env_key: Some(existing_env_var_with_random_value.to_string()),
        experimental_bearer_token: None,
        query_params: Some(std::collections::HashMap::from([(
            "api-version".to_string(),
            "2025-04-01-preview".to_string(),
        )])),
        env_key_instructions: None,
        wire_api: WireApi::Responses,
        http_headers: Some(std::collections::HashMap::from([(
            "Custom-Header".to_string(),
            "Value".to_string(),
        )])),
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        create_dummy_codex_auth(),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_var_overrides_loaded_auth() {
    skip_if_no_network!();
    let existing_env_var_with_random_value = if cfg!(windows) { "USERNAME" } else { "USER" };

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    // Expect POST to /openai/responses with api-version query param
    Mock::given(method("POST"))
        .and(path("/openai/responses"))
        .and(query_param("api-version", "2025-04-01-preview"))
        .and(header_regex("Custom-Header", "Value"))
        .and(header_regex(
            "Authorization",
            format!(
                "Bearer {}",
                std::env::var(existing_env_var_with_random_value).unwrap()
            )
            .as_str(),
        ))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "custom".to_string(),
        base_url: Some(format!("{}/openai", server.uri())),
        // Reuse the existing environment variable to avoid using unsafe code
        env_key: Some(existing_env_var_with_random_value.to_string()),
        query_params: Some(std::collections::HashMap::from([(
            "api-version".to_string(),
            "2025-04-01-preview".to_string(),
        )])),
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        http_headers: Some(std::collections::HashMap::from([(
            "Custom-Header".to_string(),
            "Value".to_string(),
        )])),
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        create_dummy_codex_auth(),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

fn create_dummy_codex_auth() -> CodexAuth {
    CodexAuth::create_dummy_chatgpt_auth_for_testing()
}

/// Scenario:
/// - Turn 1: user sends U1; model streams deltas then a final assistant message A.
/// - Turn 2: user sends U2; model streams a delta then the same final assistant message A.
/// - Turn 3: user sends U3; model responds (same SSE again, not important).
///
/// We assert that the `input` sent on each turn contains the expected conversation history
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_dedupes_streamed_and_final_messages_across_turns() {
    // Skip under Codex sandbox network restrictions (mirrors other tests).
    skip_if_no_network!();

    // Mock server that will receive three sequential requests and return the same SSE stream
    // each time: a few deltas, then a final assistant message, then completed.
    let server = MockServer::start().await;

    // Build a small SSE stream with deltas and a final assistant message.
    // We emit the same body for all 3 turns; ids vary but are unused by assertions.
    let sse_raw = r##"[
        {"type":"response.output_item.added", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":""}]
        }},
        {"type":"response.output_text.delta", "delta":"Hey "},
        {"type":"response.output_text.delta", "delta":"there"},
        {"type":"response.output_text.delta", "delta":"!\n"},
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":"Hey there!\n"}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"##;
    let sse1 = core_test_support::load_sse_fixture_with_id_from_str(sse_raw, "resp1");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse1.clone(), "text/event-stream"),
        )
        .expect(3) // respond identically to the three sequential turns
        .mount(&server)
        .await;

    // Configure provider to point to mock server (Responses API) and use API key auth.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session with isolated codex home.
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = model_provider;

    let conversation_manager = ConversationManager::with_models_provider_and_home(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    // Turn 1: user sends U1; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: "U1".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 2: user sends U2; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: "U2".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 3: user sends U3; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: "U3".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Inspect the three captured requests.
    let requests = get_responses_requests(&server).await;
    assert_eq!(requests.len(), 3, "expected 3 requests (one per turn)");

    // Replace full-array compare with tail-only raw JSON compare using a single hard-coded value.
    let r3_tail_expected = json!([
        {
            "type": "message",
            "role": "user",
            "content": [{"type":"input_text","text":"U1"}]
        },
        {
            "type": "message",
            "role": "assistant",
            "content": [{"type":"output_text","text":"Hey there!\n"}]
        },
        {
            "type": "message",
            "role": "user",
            "content": [{"type":"input_text","text":"U2"}]
        },
        {
            "type": "message",
            "role": "assistant",
            "content": [{"type":"output_text","text":"Hey there!\n"}]
        },
        {
            "type": "message",
            "role": "user",
            "content": [{"type":"input_text","text":"U3"}]
        }
    ]);

    let r3_input_array = requests[2]
        .body_json::<serde_json::Value>()
        .unwrap()
        .get("input")
        .and_then(|v| v.as_array())
        .cloned()
        .expect("r3 missing input array");
    // skipping earlier context and developer messages
    let tail_len = r3_tail_expected.as_array().unwrap().len();
    let actual_tail = &r3_input_array[r3_input_array.len() - tail_len..];
    assert_eq!(
        serde_json::Value::Array(actual_tail.to_vec()),
        r3_tail_expected,
        "request 3 tail mismatch",
    );
}
