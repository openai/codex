use std::sync::Arc;

use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_core::{
    ContentItem, LocalShellAction, LocalShellExecAction, LocalShellStatus, ModelClient,
    ModelProviderInfo, Prompt, ReasoningItemContent, ResponseItem, WireApi,
};
use core_test_support::load_default_config_for_test;
use futures::StreamExt;
use serde_json::Value;
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn network_disabled() -> bool {
    std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok()
}

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
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    config.show_raw_agent_reasoning = true;
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let config = Arc::new(config);

    let client = ModelClient::new(
        Arc::clone(&config),
        None,
        provider,
        effort,
        summary,
        Uuid::new_v4(),
    );

    let mut prompt = Prompt::default();
    prompt.input = input;

    let mut stream = client.stream(&prompt).await.expect("stream chat");
    while let Some(event) = stream.next().await {
        event.expect("stream event");
    }

    let requests = server.received_requests().await.expect("request made");
    requests[0].body_json().expect("valid json body")
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
    body["messages"].as_array().expect("messages array").clone()
}

fn first_assistant(messages: &[Value]) -> &Value {
    messages
        .iter()
        .find(|msg| msg["role"] == "assistant")
        .expect("assistant message present")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn omits_reasoning_when_none_present() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let body = run_request(vec![user_message("u1"), assistant_message("a1")]).await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);

    assert_eq!(assistant["content"], Value::String("a1".into()));
    assert!(assistant.get("reasoning").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attaches_reasoning_to_previous_assistant() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let body = run_request(vec![
        user_message("u1"),
        reasoning_item("rFunc"),
        function_call(),
    ])
    .await;
    let messages = messages_from(&body);
    let assistant = first_assistant(&messages);

    assert_eq!(assistant["reasoning"], Value::String("rFunc".into()));
    let tool_calls = assistant["tool_calls"].as_array().expect("tool call list");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["type"], Value::String("function".into()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attaches_reasoning_to_local_shell_call() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

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
