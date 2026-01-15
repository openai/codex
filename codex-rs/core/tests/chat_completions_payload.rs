#![allow(clippy::expect_used)]

use std::sync::Arc;

use codex_app_server_protocol::AuthMode;
use codex_core::ContentItem;
use codex_core::LocalShellAction;
use codex_core::LocalShellExecAction;
use codex_core::LocalShellStatus;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::models_manager::manager::ModelsManager;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::protocol::SessionSource;
use core_test_support::load_default_config_for_test;
use core_test_support::skip_if_no_network;
use futures::StreamExt;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn run_request(input: Vec<ResponseItem>) -> Value {
    let server = MockServer::start().await;

    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            "data: {\"choices\":[{\"delta\":{}}]}\n\ndata: [DONE]\n\n",
            "text/event-stream",
        );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(template)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
    };

    let codex_home = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => panic!("failed to create TempDir: {e}"),
    };
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    config.show_raw_agent_reasoning = true;
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let config = Arc::new(config);

    let conversation_id = ThreadId::new();
    let model = ModelsManager::get_model_offline(config.model.as_deref());
    let model_info = ModelsManager::construct_model_info_offline(model.as_str(), &config);
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        Some(AuthMode::ApiKey),
        false,
        "test".to_string(),
        SessionSource::Exec,
    );

    let mut client_session = ModelClient::new(
        Arc::clone(&config),
        None,
        model_info,
        otel_manager,
        provider,
        effort,
        summary,
        conversation_id,
        SessionSource::Exec,
    )
    .new_session();

    let mut prompt = Prompt::default();
    prompt.input = input;

    let mut stream = match client_session.stream(&prompt).await {
        Ok(s) => s,
        Err(e) => panic!("stream chat failed: {e}"),
    };
    while let Some(event) = stream.next().await {
        if let Err(e) = event {
            panic!("stream event error: {e}");
        }
    }

    let all_requests = server.received_requests().await.expect("received requests");
    let requests: Vec<_> = all_requests
        .iter()
        .filter(|req| req.method == "POST" && req.url.path().ends_with("/chat/completions"))
        .collect();
    let request = requests
        .first()
        .unwrap_or_else(|| panic!("expected POST request to /chat/completions"));
    match request.body_json() {
        Ok(v) => v,
        Err(e) => panic!("invalid json body: {e}"),
    }
}

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
    }
}

fn assistant_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
    }
}

fn reasoning_item(text: &str) -> ResponseItem {
    ResponseItem::Reasoning {
        id: String::new(),
        summary: Vec::new(),
        content: Some(vec![ReasoningItemContent::ReasoningText {
            text: text.to_string(),
        }]),
        encrypted_content: None,
    }
}

fn function_call() -> ResponseItem {
    ResponseItem::FunctionCall {
        id: None,
        name: "f".to_string(),
        arguments: "{}".to_string(),
        call_id: "c1".to_string(),
    }
}

fn local_shell_call() -> ResponseItem {
    ResponseItem::LocalShellCall {
        id: Some("id1".to_string()),
        call_id: None,
        status: LocalShellStatus::InProgress,
        action: LocalShellAction::Exec(LocalShellExecAction {
            command: vec!["echo".to_string()],
            timeout_ms: Some(1_000),
            working_directory: None,
            env: None,
            user: None,
        }),
    }
}

fn messages_from(body: &Value) -> Vec<Value> {
    match body["messages"].as_array() {
        Some(arr) => arr.clone(),
        None => panic!("messages array missing"),
    }
}

fn first_assistant(messages: &[Value]) -> &Value {
    match messages.iter().find(|msg| msg["role"] == "assistant") {
        Some(v) => v,
        None => panic!("assistant message not present"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn omits_reasoning_when_none_present() {
    skip_if_no_network!();

    let body = run_request(vec![user_message("u1"), assistant_message("a1")]).await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);

    assert_eq!(assistant["content"], Value::String("a1".into()));
    assert!(assistant.get("reasoning").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attaches_reasoning_to_previous_assistant() {
    skip_if_no_network!();

    let body = run_request(vec![
        user_message("u1"),
        assistant_message("a1"),
        reasoning_item("rA"),
    ])
    .await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);

    assert_eq!(assistant["content"], Value::String("a1".into()));
    assert_eq!(assistant["reasoning"], Value::String("rA".into()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attaches_reasoning_to_function_call_anchor() {
    skip_if_no_network!();

    let body = run_request(vec![
        user_message("u1"),
        reasoning_item("rFunc"),
        function_call(),
    ])
    .await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);

    assert_eq!(assistant["reasoning"], Value::String("rFunc".into()));
    let tool_calls = match assistant["tool_calls"].as_array() {
        Some(arr) => arr,
        None => panic!("tool call list missing"),
    };
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["type"], Value::String("function".into()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attaches_reasoning_to_local_shell_call() {
    skip_if_no_network!();

    let body = run_request(vec![
        user_message("u1"),
        reasoning_item("rShell"),
        local_shell_call(),
    ])
    .await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);

    assert_eq!(assistant["reasoning"], Value::String("rShell".into()));
    assert_eq!(
        assistant["tool_calls"][0]["type"],
        Value::String("local_shell_call".into())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drops_reasoning_when_last_role_is_user() {
    skip_if_no_network!();

    let body = run_request(vec![
        assistant_message("aPrev"),
        reasoning_item("rHist"),
        user_message("uNew"),
    ])
    .await;
    let messages = messages_from(&body);
    assert!(messages.iter().all(|msg| msg.get("reasoning").is_none()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ignores_reasoning_before_last_user() {
    skip_if_no_network!();

    let body = run_request(vec![
        user_message("u1"),
        assistant_message("a1"),
        user_message("u2"),
        reasoning_item("rAfterU1"),
    ])
    .await;
    let messages = messages_from(&body);
    assert!(messages.iter().all(|msg| msg.get("reasoning").is_none()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skips_empty_reasoning_segments() {
    skip_if_no_network!();

    let body = run_request(vec![
        user_message("u1"),
        assistant_message("a1"),
        reasoning_item(""),
        reasoning_item("   "),
    ])
    .await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);
    assert!(assistant.get("reasoning").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suppresses_duplicate_assistant_messages() {
    skip_if_no_network!();

    let body = run_request(vec![assistant_message("dup"), assistant_message("dup")]).await;
    let messages = messages_from(&body);
    let assistant_messages: Vec<_> = messages
        .iter()
        .filter(|msg| msg["role"] == "assistant")
        .collect();
    assert_eq!(assistant_messages.len(), 1);
    assert_eq!(
        assistant_messages[0]["content"],
        Value::String("dup".into())
    );
}
