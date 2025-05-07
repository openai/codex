//! Verifies that the agent retries when the SSE stream terminates before
//! delivering a `response.completed` event.

use std::time::Duration;

use codex_core::Codex;
use codex_core::config::Config;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::Submission;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn sse_incomplete() -> String {
    // Only a single line; missing the completed event.
    "event: response.output_item.done\n\n".to_string()
}

fn sse_completed(id: &str) -> String {
    format!(
        "event: response.completed\n\
data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\",\"output\":[]}}}}\n\n\n",
        id
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retries_on_early_close() {
    let server = MockServer::start().await;

    struct SeqResponder;
    impl Respond for SeqResponder {
        fn respond(&self, _: &Request) -> ResponseTemplate {
            use std::sync::atomic::AtomicUsize;
            use std::sync::atomic::Ordering;
            static CALLS: AtomicUsize = AtomicUsize::new(0);
            let n = CALLS.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(sse_incomplete(), "text/event-stream")
            } else {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(sse_completed("resp_ok"), "text/event-stream")
            }
        }
    }

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(SeqResponder {})
        .expect(2)
        .mount(&server)
        .await;

    // Environment
    //
    // As of Rust 2024 `std::env::set_var` has been made `unsafe` because
    // mutating the process environment is inherently racy when other threads
    // are running.  We therefore have to wrap every call in an explicit
    // `unsafe` block.  These are limited to the test-setup section so the
    // scope is very small and clearly delineated.

    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test-key");
        std::env::set_var("OPENAI_API_BASE", server.uri());
        std::env::set_var("OPENAI_REQUEST_MAX_RETRIES", "0");
        std::env::set_var("OPENAI_STREAM_MAX_RETRIES", "1");
        std::env::set_var("OPENAI_STREAM_IDLE_TIMEOUT_MS", "2000");
    }

    let codex = Codex::spawn(std::sync::Arc::new(tokio::sync::Notify::new())).unwrap();

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
    let _ = codex.next_event().await.unwrap();

    codex
        .submit(Submission {
            id: "task".into(),
            op: Op::UserInput {
                items: vec![InputItem::Text {
                    text: "hello".into(),
                }],
            },
        })
        .await
        .unwrap();

    // Wait until TaskComplete (should succeed after retry).
    loop {
        let ev = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .unwrap()
            .unwrap();
        if matches!(ev.msg, codex_core::protocol::EventMsg::TaskComplete) {
            break;
        }
    }
}
