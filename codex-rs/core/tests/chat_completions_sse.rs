use assert_matches::assert_matches;
use codex_core::AuthManager;
use std::sync::Arc;
use tracing_test::traced_test;

use codex_core::CodexAuth;
use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::openai_models::models_manager::ModelsManager;
use codex_otel::otel_manager::OtelManager;
use codex_protocol::ConversationId;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::protocol::SessionSource;
use core_test_support::load_default_config_for_test;
use core_test_support::skip_if_no_network;
use futures::StreamExt;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn run_stream(sse_body: &str) -> Vec<ResponseEvent> {
    run_stream_with_bytes(sse_body.as_bytes()).await
}

async fn run_stream_with_bytes(sse_body: &[u8]) -> Vec<ResponseEvent> {
    let server = MockServer::start().await;

    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_bytes(sse_body.to_vec());

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

    let conversation_id = ConversationId::new();
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let auth_mode = auth_manager.get_auth_mode();
    let model = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = ModelsManager::construct_model_family_offline(model.as_str(), &config);
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_family.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        auth_mode,
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
            // We still collect the error to exercise telemetry and complete the task.
            Err(_e) => break,
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
                ReasoningItemContent::ReasoningText { text }
                | ReasoningItemContent::Text { text } => combined.push_str(text),
            }
        }
        assert_eq!(combined, expected);
    } else {
        panic!("expected reasoning item, got: {item:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_text_without_reasoning() {
    skip_if_no_network!();

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{}}]}\n\n",
        "data: [DONE]\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 4, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::OutputItemAdded(ResponseItem::Message { .. }) => {}
        other => panic!("expected initial assistant item, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::OutputTextDelta(text) => assert_eq!(text, "hi"),
        other => panic!("expected text delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputItemDone(item) => assert_message(item, "hi"),
        other => panic!("expected terminal message, got {other:?}"),
    }

    assert_matches!(events[3], ResponseEvent::Completed { .. });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_from_string_delta() {
    skip_if_no_network!();

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":\"think1\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{} ,\"finish_reason\":\"stop\"}]}\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 7, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::OutputItemAdded(ResponseItem::Reasoning { .. }) => {}
        other => panic!("expected initial reasoning item, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => {
            assert_eq!(delta, "think1");
            assert_eq!(content_index, &0);
        }
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputItemAdded(ResponseItem::Message { .. }) => {}
        other => panic!("expected initial message item, got {other:?}"),
    }

    match &events[3] {
        ResponseEvent::OutputTextDelta(text) => assert_eq!(text, "ok"),
        other => panic!("expected text delta, got {other:?}"),
    }

    match &events[4] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "think1"),
        other => panic!("expected terminal reasoning, got {other:?}"),
    }

    match &events[5] {
        ResponseEvent::OutputItemDone(item) => assert_message(item, "ok"),
        other => panic!("expected terminal message, got {other:?}"),
    }

    assert_matches!(events[6], ResponseEvent::Completed { .. });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_from_object_delta() {
    skip_if_no_network!();

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":{\"text\":\"partA\"}}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"reasoning\":{\"content\":\"partB\"}}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"answer\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{} ,\"finish_reason\":\"stop\"}]}\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 8, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::OutputItemAdded(ResponseItem::Reasoning { .. }) => {}
        other => panic!("expected initial reasoning item, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => {
            assert_eq!(delta, "partA");
            assert_eq!(content_index, &0);
        }
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => {
            assert_eq!(delta, "partB");
            assert_eq!(content_index, &1);
        }
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[3] {
        ResponseEvent::OutputItemAdded(ResponseItem::Message { .. }) => {}
        other => panic!("expected initial message item, got {other:?}"),
    }

    match &events[4] {
        ResponseEvent::OutputTextDelta(text) => assert_eq!(text, "answer"),
        other => panic!("expected text delta, got {other:?}"),
    }

    match &events[5] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "partApartB"),
        other => panic!("expected terminal reasoning, got {other:?}"),
    }

    match &events[6] {
        ResponseEvent::OutputItemDone(item) => assert_message(item, "answer"),
        other => panic!("expected terminal message, got {other:?}"),
    }

    assert_matches!(events[7], ResponseEvent::Completed { .. });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_from_final_message() {
    skip_if_no_network!();

    let sse = "data: {\"choices\":[{\"message\":{\"reasoning\":\"final-cot\"},\"finish_reason\":\"stop\"}]}\n\n";

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 4, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::OutputItemAdded(ResponseItem::Reasoning { .. }) => {}
        other => panic!("expected initial reasoning item, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => {
            assert_eq!(delta, "final-cot");
            assert_eq!(content_index, &0);
        }
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "final-cot"),
        other => panic!("expected reasoning item, got {other:?}"),
    }

    assert_matches!(events[3], ResponseEvent::Completed { .. });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_reasoning_before_tool_call() {
    skip_if_no_network!();

    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":\"pre-tool\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"run\",\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
    );

    let events = run_stream(sse).await;
    assert_eq!(events.len(), 5, "unexpected events: {events:?}");

    match &events[0] {
        ResponseEvent::OutputItemAdded(ResponseItem::Reasoning { .. }) => {}
        other => panic!("expected initial reasoning item, got {other:?}"),
    }

    match &events[1] {
        ResponseEvent::ReasoningContentDelta {
            delta,
            content_index,
        } => {
            assert_eq!(delta, "pre-tool");
            assert_eq!(content_index, &0);
        }
        other => panic!("expected reasoning delta, got {other:?}"),
    }

    match &events[2] {
        ResponseEvent::OutputItemDone(item) => assert_reasoning(item, "pre-tool"),
        other => panic!("expected reasoning item, got {other:?}"),
    }

    match &events[3] {
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

    assert_matches!(events[4], ResponseEvent::Completed { .. });
}

#[tokio::test]
#[traced_test]
async fn chat_sse_emits_failed_on_parse_error() {
    skip_if_no_network!();

    let sse_body = concat!("data: not-json\n\n", "data: [DONE]\n\n");

    let _ = run_stream(sse_body).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.api_request") && line.contains("http.response.status_code=200")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("cannot find codex.api_request event".to_string()))
    });

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("error.message")
                    && line.contains("expected ident at line 1 column 2")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("cannot find SSE event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn chat_sse_done_chunk_emits_event() {
    skip_if_no_network!();

    let sse_body = "data: [DONE]\n\n";

    let _ = run_stream(sse_body).await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| line.contains("codex.sse_event") && line.contains("event.kind=message"))
            .map(|_| Ok(()))
            .unwrap_or(Err("cannot find SSE event".to_string()))
    });
}

#[tokio::test]
#[traced_test]
async fn chat_sse_emits_error_on_invalid_utf8() {
    skip_if_no_network!();

    let _ = run_stream_with_bytes(b"data: \x80\x80\n\n").await;

    logs_assert(|lines: &[&str]| {
        lines
            .iter()
            .find(|line| {
                line.contains("codex.sse_event")
                    && line.contains("error.message")
                    && line.contains("UTF8 error: invalid utf-8 sequence of 1 bytes from index 0")
            })
            .map(|_| Ok(()))
            .unwrap_or(Err("cannot find SSE event".to_string()))
    });
}
