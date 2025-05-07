use std::time::Duration;

use codex_core::Codex;
use codex_core::config::Config;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::Submission;
use serde_json::Value;
use tokio::time::timeout;
use wiremock::Match;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Matcher asserting that JSON body has NO `previous_response_id` field.
struct NoPrevId;

impl Match for NoPrevId {
    fn matches(&self, req: &Request) -> bool {
        serde_json::from_slice::<Value>(&req.body)
            .map(|v| v.get("previous_response_id").is_none())
            .unwrap_or(false)
    }
}

/// Matcher asserting that JSON body HAS a `previous_response_id` field.
struct HasPrevId;

impl Match for HasPrevId {
    fn matches(&self, req: &Request) -> bool {
        serde_json::from_slice::<Value>(&req.body)
            .map(|v| v.get("previous_response_id").is_some())
            .unwrap_or(false)
    }
}

/// Build minimal SSE stream with completed marker.
fn sse_completed(id: &str) -> String {
    format!(
        "event: response.completed\n\
data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\",\"output\":[]}}}}\n\n\n",
        id
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keeps_previous_response_id_between_tasks() {
    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(NoPrevId)
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    // Second request – MUST include `previous_response_id`.
    let second = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp2"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(HasPrevId)
        .respond_with(second)
        .expect(1)
        .mount(&server)
        .await;

    // Environment
    // Update environment – `set_var` is `unsafe` starting with the 2024
    // edition so we group the calls into a single `unsafe { … }` block.
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test-key");
        std::env::set_var("OPENAI_API_BASE", server.uri());
        std::env::set_var("OPENAI_REQUEST_MAX_RETRIES", "0");
        std::env::set_var("OPENAI_STREAM_MAX_RETRIES", "0");
    }

    let codex = Codex::spawn(std::sync::Arc::new(tokio::sync::Notify::new())).unwrap();

    // Init session
    let config = Config::load_default_config_for_test();
    codex
        .submit(Submission {
            id: "init".into(),
            op: Op::ConfigureSession {
                model: config.model,
                instructions: None,
                approval_policy: config.approval_policy,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                disable_response_storage: false,
                notify: None,
                cwd: std::env::current_dir().unwrap(),
            },
        })
        .await
        .unwrap();
    // drain init event
    let _ = codex.next_event().await.unwrap();

    // Task 1 – triggers first request (no previous_response_id)
    codex
        .submit(Submission {
            id: "task1".into(),
            op: Op::UserInput {
                items: vec![InputItem::Text {
                    text: "hello".into(),
                }],
            },
        })
        .await
        .unwrap();

    // Wait for TaskComplete
    loop {
        let ev = timeout(Duration::from_secs(1), codex.next_event())
            .await
            .unwrap()
            .unwrap();
        if matches!(ev.msg, codex_core::protocol::EventMsg::TaskComplete) {
            break;
        }
    }

    // Task 2 – should include `previous_response_id` (triggers second request)
    codex
        .submit(Submission {
            id: "task2".into(),
            op: Op::UserInput {
                items: vec![InputItem::Text {
                    text: "again".into(),
                }],
            },
        })
        .await
        .unwrap();

    // Wait for TaskComplete or error
    loop {
        let ev = timeout(Duration::from_secs(1), codex.next_event())
            .await
            .unwrap()
            .unwrap();
        match ev.msg {
            codex_core::protocol::EventMsg::TaskComplete => break,
            codex_core::protocol::EventMsg::Error { message } => {
                panic!("unexpected error: {message}")
            }
            _ => (),
        }
    }
}
