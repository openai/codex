#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::WarningEvent;
use codex_core::status::set_test_idle_timeout;
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
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_warning_when_stream_is_idle_and_status_is_degraded() {
    let status_server = start_mock_server().await;
    let status_path = "/proxy/status.openai.com";

    Mock::given(method("GET"))
        .and(path(status_path))
        .respond_with(status_payload())
        .mount(&status_server)
        .await;

    set_test_status_widget_url(format!("{}{}", status_server.uri(), status_path));
    set_test_idle_timeout(Duration::from_millis(300));

    let responses_server = start_mock_server().await;
    let stalled_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "finally"),
        ev_completed("resp-1"),
    ]);

    let _responses_mock = mount_sse_once_with_delay(
        &responses_server,
        stalled_response,
        Duration::from_millis(400),
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

    let status_requests = status_server
        .received_requests()
        .await
        .expect("status server running");
    assert!(
        !status_requests.is_empty(),
        "status widget was not queried before idle warning"
    );

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;
}

fn status_payload() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_json(serde_json::json!({
            "summary": {
                "components": [
                    {"id": "cmp-1", "name": "Codex", "status_page_id": "page-1"}
                ],
                "affected_components": [
                    {"component_id": "cmp-1", "status": "major_outage"}
                ]
            }
        }))
}
