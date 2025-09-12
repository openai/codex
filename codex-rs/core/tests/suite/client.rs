use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::WireApi;
use codex_core::built_in_model_providers;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::wait_for_event;
use serde_json::json;
use std::io::Write;
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
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
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
                "cli_version": "test_version"
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
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    // Configure Codex to resume from our file
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.experimental_resume = Some(session_path.clone());
    // Also configure user instructions to ensure they are NOT delivered on resume.
    config.user_instructions = Some("be nice".to_string());

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let NewConversation {
        conversation: codex,
        session_configured,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

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
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();
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
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
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
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
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
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);

    config.base_instructions = Some("test instructions".to_string());
    config.model_provider = model_provider;

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
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(
        request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("test instructions")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chatgpt_auth_sends_correct_request() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    let mut config = load_default_config_for_test(&codex_home);
    let include_reasoning = config.model_reasoning_effort.is_some();
    config.model_provider = model_provider;
    let conversation_manager = ConversationManager::with_auth(create_dummy_codex_auth());
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
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
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
    if include_reasoning {
        assert_eq!(
            request_body["include"][0].as_str().unwrap(),
            "reasoning.encrypted_content"
        );
    } else {
        assert!(
            request_body["include"]
                .as_array()
                .is_none_or(|items| items.is_empty())
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefers_apikey_when_config_prefers_apikey_even_with_chatgpt_tokens() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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

    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;

    let auth_manager = match CodexAuth::from_codex_home(codex_home.path()) {
        Ok(Some(auth)) => codex_core::AuthManager::from_auth_for_testing(auth),
        Ok(None) => panic!("No CodexAuth found in codex_home"),
        Err(e) => panic!("Failed to load CodexAuth: {e}"),
    };
    let conversation_manager = ConversationManager::new(auth_manager);
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_user_instructions_message_in_request() {
    let server = MockServer::start().await;

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

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.user_instructions = Some("be nice".to_string());

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
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(
        !request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("be nice")
    );
    assert_message_role(&request_body["input"][0], "user");
    assert_message_starts_with(&request_body["input"][0], "<user_instructions>");
    assert_message_ends_with(&request_body["input"][0], "</user_instructions>");
    assert_message_role(&request_body["input"][1], "user");
    assert_message_starts_with(&request_body["input"][1], "<environment_context>");
    assert_message_ends_with(&request_body["input"][1], "</environment_context>");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn azure_overrides_assign_properties_used_for_responses_url() {
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
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_auth(create_dummy_codex_auth());
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_var_overrides_loaded_auth() {
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
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = provider;

    let conversation_manager = ConversationManager::with_auth(create_dummy_codex_auth());
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
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
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server that will receive three sequential requests and return the same SSE stream
    // each time: a few deltas, then a final assistant message, then completed.
    let server = MockServer::start().await;

    // Build a small SSE stream with deltas and a final assistant message.
    // We emit the same body for all 3 turns; ids vary but are unused by assertions.
    let sse_raw = r##"[
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
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
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
            items: vec![InputItem::Text { text: "U1".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 2: user sends U2; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U2".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Turn 3: user sends U3; wait for completion.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: "U3".into() }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Inspect the three captured requests.
    let requests = server.received_requests().await.unwrap();
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
