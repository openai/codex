#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::WarningEvent;
use codex_core::status::set_test_status_widget_url;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_with_delay;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_warning_when_status_is_degraded_at_turn_start() {
    let status_server = start_mock_server().await;
    let status_path = "/proxy/status.openai.com";

    Mock::given(method("GET"))
        .and(path(status_path))
        .respond_with(status_payload("major_outage"))
        .mount(&status_server)
        .await;

    set_test_status_widget_url(format!("{}{}", status_server.uri(), status_path));
    let responses_server = start_mock_server().await;
    let stalled_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "finally"),
        ev_completed("resp-1"),
    ]);

    let _responses_mock = mount_sse_once_with_delay(
        &responses_server,
        stalled_response,
        Duration::from_millis(10),
    )
    .await;

    let test_codex = test_codex().build(&responses_server).await.unwrap();
    let codex = test_codex.codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: "hi".into() }],
        })
        .await
        .unwrap();

    let warning = wait_for_event(&codex, |event| matches!(event, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning else {
        panic!("expected warning event");
    };
    assert_eq!(
        message,
        "Codex is experiencing a major outage. If a response stalls, try again later. You can follow incident updates at status.openai.com.",
        "unexpected warning message"
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn warns_once_per_status_change_only_when_unhealthy() {
    let status_server = start_mock_server().await;
    let status_path = "/proxy/status.openai.com";

    let responder = SequenceResponder::new(vec!["major_outage", "partial_outage"]);

    Mock::given(method("GET"))
        .and(path(status_path))
        .respond_with(responder)
        .mount(&status_server)
        .await;

    set_test_status_widget_url(format!("{}{}", status_server.uri(), status_path));
    let responses_server = start_mock_server().await;
    let stalled_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "finally"),
        ev_completed("resp-1"),
    ]);

    let _responses_mock = mount_sse_once_with_delay(
        &responses_server,
        stalled_response,
        Duration::from_millis(2000),
    )
    .await;

    let test_codex = test_codex().build(&responses_server).await.unwrap();
    let codex = test_codex.codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: "hi".into() }],
        })
        .await
        .unwrap();

    let first_warning = wait_for_event(&codex, |event| matches!(event, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = first_warning else {
        panic!("expected warning event");
    };
    assert_eq!(
        message,
        "Codex is experiencing a major outage. If a response stalls, try again later. You can follow incident updates at status.openai.com.",
    );
    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "second".into(),
            }],
        })
        .await
        .unwrap();

    let mut task_completes = 0usize;
    let mut warnings = Vec::new();
    while task_completes < 2 {
        let event = codex.next_event().await.expect("event");
        match event.msg {
            EventMsg::Warning(WarningEvent { message }) => warnings.push(message),
            EventMsg::TaskComplete(_) => task_completes += 1,
            _ => {}
        }
    }

    assert!(
        !warnings.is_empty(),
        "expected at least one warning for non-operational status"
    );
    assert_eq!(
        warnings[0],
        "Codex is experiencing a major outage. If a response stalls, try again later. You can follow incident updates at status.openai.com.",
    );
    if warnings.len() > 1 {
        assert_eq!(
            warnings[1],
            "Codex is experiencing a partial outage. If a response stalls, try again later. You can follow incident updates at status.openai.com.",
        );
    }
}

fn status_payload(status: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_json(serde_json::json!({
            "summary": {
                "components": [
                    {"id": "cmp-1", "name": "Codex", "status_page_id": "page-1"}
                ],
                "affected_components": [
                    {"component_id": "cmp-1", "status": status}
                ]
            }
        }))
}

#[derive(Clone)]
struct SequenceResponder {
    statuses: Vec<&'static str>,
    calls: Arc<AtomicUsize>,
}

impl SequenceResponder {
    fn new(statuses: Vec<&'static str>) -> Self {
        Self {
            statuses,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Respond for SequenceResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let idx = usize::try_from(call).unwrap_or(0);
        let status = self
            .statuses
            .get(idx)
            .copied()
            .or_else(|| self.statuses.last().copied())
            .unwrap_or("operational");
        status_payload(status)
    }
}
