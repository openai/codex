use std::sync::Arc;

use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::mcp_protocol::ConversationId;
use core_test_support::load_default_config_for_test;
use futures::StreamExt;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn network_disabled() -> bool {
    std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok()
}

async fn run_stream(sse_body: &str) -> Vec<ResponseEvent> {
    let server = MockServer::start().await;

    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_body.to_string(), "text/event-stream");

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

    let codex_home = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => panic!("failed to create TempDir: {e}"),
    };
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
        ConversationId::new(),
    );

    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "hello".to_string(),
        }],
    }];

    let mut stream = match client.stream(&prompt).await {
        Ok(s) => s,
        Err(e) => panic!("stream chat failed: {e}"),
    };
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => events.push(ev),
            Err(e) => panic!("stream event error: {e}"),
        }
    }
    events
}

fn assert_message(item: &ResponseItem, expected: &str) {
    if let ResponseItem::Message { content, .. } = item {
        let text = content.iter().find_map(|part| match part {
            ContentItem::OutputText { text } | ContentItem::InputText { text } => Some(text),
            _ => None,
        });
        let Some(text) = text else {
            panic!("message missing text: {item:?}");
        };
        assert_eq!(text, expected);
    } else {
        panic!("expected message item, got: {item:?}");
    }
}

fn assert_reasoning(item: &ResponseItem, expected: &str) {
    if let ResponseItem::Reasoning {
        content: Some(parts),
        ..
    } = item
    {
        let mut combined = String::new();
        for part in parts {
            match part {
                codex_core::ReasoningItemContent::ReasoningText { text }
                | codex_core::ReasoningItemContent::Text { text } => combined.push_str(text),
            }
        }
        assert_eq!(combined, expected);
    } else {
        panic!("expected reasoning item, got: {item:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_text_without_reasoning() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{}}]}\n\n",
        "data: [DONE]\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 3, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::OutputTextDelta(text) => assert_eq!(text, "hi"),
        other => panic!("expected text delta, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::OutputItemDone(item) => assert_message(item, "hi"),
        other => panic!("expected terminal message, got {other:?}"),
    }

    assert!(matches!(events[2], ResponseEvent::Completed { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_from_string_delta() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":\"think1\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{} ,\"finish_reason\":\"stop\"}]}\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 5, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::ReasoningContentDelta(text) => assert_eq!(text, "think1"),
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::OutputTextDelta(text) => assert_eq!(text, "ok"),
        other => panic!("expected text delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "think1"),
        other => panic!("expected reasoning item, got {other:?}"),
    }

    match &events[3] {
        ResponseEvent::OutputItemDone(item) => assert_message(item, "ok"),
        other => panic!("expected message item, got {other:?}"),
    }

    assert!(matches!(events[4], ResponseEvent::Completed { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_from_object_delta() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":{\"text\":\"partA\"}}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"reasoning\":{\"content\":\"partB\"}}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"answer\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{} ,\"finish_reason\":\"stop\"}]}\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 6, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::ReasoningContentDelta(text) => assert_eq!(text, "partA"),
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::ReasoningContentDelta(text) => assert_eq!(text, "partB"),
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputTextDelta(text) => assert_eq!(text, "answer"),
        other => panic!("expected text delta, got {other:?}"),
    }

    match &events[3] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "partApartB"),
        other => panic!("expected reasoning item, got {other:?}"),
    }

    match &events[4] {
        ResponseEvent::OutputItemDone(item) => assert_message(item, "answer"),
        other => panic!("expected message item, got {other:?}"),
    }

    assert!(matches!(events[5], ResponseEvent::Completed { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_from_final_message() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let sse = "data: {\"choices\":[{\"message\":{\"reasoning\":\"final-cot\"},\"finish_reason\":\"stop\"}]}\n\n";

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 3, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::ReasoningContentDelta(text) => assert_eq!(text, "final-cot"),
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "final-cot"),
        other => panic!("expected reasoning item, got {other:?}"),
    }

    assert!(matches!(events[2], ResponseEvent::Completed { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_before_tool_call() {
    if network_disabled() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":\"pre-tool\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"run\",\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 4, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::ReasoningContentDelta(text) => assert_eq!(text, "pre-tool"),
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "pre-tool"),
        other => panic!("expected reasoning item, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputItemDone(ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        }) => {
            assert_eq!(name, "run");
            assert_eq!(arguments, "{}");
            assert_eq!(call_id, "call_1");
        }
        other => panic!("expected function call, got {other:?}"),
    }

    assert!(matches!(events[3], ResponseEvent::Completed { .. }));
}
