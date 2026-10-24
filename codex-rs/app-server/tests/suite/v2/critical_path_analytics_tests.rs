//! Verify Core capture reaches the existing analytics endpoint on terminal turns.

use super::analytics::mount_analytics_capture;
use super::analytics::wait_for_matching_analytics_event;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[derive(Clone, Copy)]
enum Finish {
    Complete,
    Interrupt,
}

#[derive(Clone, Copy)]
enum Capture {
    Default,
    Enabled,
    Disabled,
}

#[test_case::test_case(Finish::Complete, Capture::Default; "complete_default")]
#[test_case::test_case(Finish::Complete, Capture::Enabled; "complete_enabled")]
#[test_case::test_case(Finish::Complete, Capture::Disabled; "complete_disabled")]
#[test_case::test_case(Finish::Interrupt, Capture::Default; "interrupt_default")]
#[test_case::test_case(Finish::Interrupt, Capture::Enabled; "interrupt_enabled")]
#[test_case::test_case(Finish::Interrupt, Capture::Disabled; "interrupt_disabled")]
#[tokio::test]
async fn critical_path_reaches_turn_event(finish: Finish, capture: Capture) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response-1"),
        responses::ev_assistant_message("answer", "Done"),
        responses::ev_completed("response-1"),
    ]);
    let mut response = ResponseTemplate::new(200).set_body_raw(body, "text/event-stream");
    if matches!(finish, Finish::Interrupt) {
        response = response.set_delay(Duration::from_secs(60));
    }
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(response)
        .mount(&server)
        .await;
    let codex_home = TempDir::new()?;
    let mut features = BTreeMap::from([(Feature::RuntimeMetrics, true)]);
    match capture {
        Capture::Default => {}
        Capture::Enabled => {
            features.insert(Feature::CriticalPathTiming, true);
        }
        Capture::Disabled => {
            features.insert(Feature::CriticalPathTiming, false);
        }
    }
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!("chatgpt_base_url = {:?}", server.uri()))
        .with_provider_config("supports_websockets = false")
        .with_features(&features)
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
    let params = TurnStartParams {
        thread_id: thread.id.clone(),
        input: vec![UserInput::Text {
            text: "Hello".into(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    let (turn_id, expected_status) = match finish {
        Finish::Complete => {
            let completed = app.start_turn_and_wait_for_completion(params).await?;
            (completed.turn.id, "completed")
        }
        Finish::Interrupt => {
            let started: TurnStartResponse = app
                .request(|request_id| ClientRequest::TurnStart { request_id, params })
                .await?;
            // Wait for sampling to begin before interrupting, without a timing-dependent sleep.
            timeout(Duration::from_secs(10), async {
                loop {
                    if server
                        .received_requests()
                        .await
                        .unwrap_or_default()
                        .iter()
                        .any(|r| r.url.path().ends_with("/responses"))
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await?;
            app.interrupt_turn_and_wait_for_aborted(
                thread.id.clone(),
                started.turn.id.clone(),
                Duration::from_secs(10),
            )
            .await?;
            (started.turn.id, "interrupted")
        }
    };
    let event = wait_for_matching_analytics_event(&server, Duration::from_secs(30), |event| {
        event["event_type"] == "codex_turn_event" && event["event_params"]["turn_id"] == turn_id
    })
    .await?;
    let p = &event["event_params"];
    assert_eq!(
        json!([p["thread_id"], p["status"]]),
        json!([thread.id, expected_status])
    );
    let fields = [
        "critical_path_duration_ms",
        "critical_path_inference_ms",
        "critical_path_auto_review_ms",
        "critical_path_user_wait_ms",
        "critical_path_tools_ms",
        "critical_path_overhead_ms",
        "critical_path_unknown_ms",
    ];
    if matches!(capture, Capture::Enabled) {
        assert_eq!(p["critical_path_status"], "complete");
        assert_eq!(p[fields[0]], p["duration_ms"]);
        let total = p[fields[0]]
            .as_u64()
            .expect("complete capture has a numeric total");
        let sum: u64 = fields[1..]
            .iter()
            .map(|key| {
                p[key]
                    .as_u64()
                    .expect("complete capture has numeric categories")
            })
            .sum();
        assert!(sum <= total && total - sum <= 5);
        assert_eq!(
            json!([
                p["critical_path_user_wait_ms"],
                p["critical_path_tools_ms"],
                p["critical_path_auto_review_ms"],
                p["critical_path_unknown_ms"],
            ]),
            json!([0, 0, 0, 0])
        );
    } else {
        assert_eq!(p["critical_path_status"], "disabled");
        assert_eq!(
            fields.map(|key| p.get(key)),
            [Some(&serde_json::Value::Null); 7]
        );
    }
    Ok(())
}
