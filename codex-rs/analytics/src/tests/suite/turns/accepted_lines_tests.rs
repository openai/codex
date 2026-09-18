//! Accepted-line aggregation, finalization, and event payload tests.

#[cfg(debug_assertions)]
use crate::client::AnalyticsEventsClient;
use crate::events::CodexAcceptedLineFingerprintsEventParams;
use crate::events::CodexAcceptedLineFingerprintsEventRequest;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
#[cfg(debug_assertions)]
use crate::facts::CustomAnalyticsFact;
#[cfg(debug_assertions)]
use crate::facts::TurnProfileFact;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::ingest_turn_prerequisites;
#[cfg(debug_assertions)]
use crate::tests::support::sample_initialize_fact;
#[cfg(debug_assertions)]
use crate::tests::support::sample_thread_start_response;
use crate::tests::support::sample_turn_completed_notification;
#[cfg(debug_assertions)]
use crate::tests::support::sample_turn_profile;
#[cfg(debug_assertions)]
use crate::tests::support::sample_turn_resolved_config;
#[cfg(debug_assertions)]
use crate::tests::support::sample_turn_start_request;
#[cfg(debug_assertions)]
use crate::tests::support::sample_turn_start_response;
#[cfg(debug_assertions)]
use crate::tests::support::sample_turn_started_notification;
#[cfg(debug_assertions)]
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::TurnDiffUpdatedNotification;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use pretty_assertions::assert_eq;
use serde_json::json;
#[cfg(debug_assertions)]
use std::time::SystemTime;

#[test]
fn accepted_line_fingerprints_event_serializes_expected_shape() {
    let event = TrackEventRequest::AcceptedLineFingerprints(Box::new(
        CodexAcceptedLineFingerprintsEventRequest {
            event_type: "codex_accepted_line_fingerprints",
            event_params: CodexAcceptedLineFingerprintsEventParams {
                event_type: "codex.accepted_line_fingerprints",
                turn_id: "turn-1".to_string(),
                thread_id: "thread-1".to_string(),
                product_surface: Some("codex".to_string()),
                model_slug: Some("gpt-5.1-codex".to_string()),
                completed_at: 1710000000,
                repo_hash: Some("repo-hash-1".to_string()),
                accepted_added_lines: 42,
                accepted_deleted_lines: 40,
                line_fingerprints: [],
            },
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize accepted line fingerprints event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_accepted_line_fingerprints",
            "event_params": {
                "event_type": "codex.accepted_line_fingerprints",
                "turn_id": "turn-1",
                "thread_id": "thread-1",
                "product_surface": "codex",
                "model_slug": "gpt-5.1-codex",
                "completed_at": 1710000000,
                "repo_hash": "repo-hash-1",
                "accepted_added_lines": 42,
                "accepted_deleted_lines": 40,
                "line_fingerprints": []
            }
        })
    );
}

#[tokio::test]
async fn reducer_emits_large_accepted_line_aggregates_without_fingerprints() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut events,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ true,
    )
    .await;
    events.clear();

    let mut diff = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -0,0 +1,20000 @@
"
    .to_string();
    for index in 0..20_000 {
        diff.push_str(&format!("+let value_{index} = {index};\n"));
    }

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::TurnDiffUpdated(
                TurnDiffUpdatedNotification {
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                    diff,
                },
            ))),
            &mut events,
        )
        .await;
    assert!(events.is_empty());

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut events,
        )
        .await;

    let accepted_line_events = events
        .iter()
        .filter_map(|event| match event {
            TrackEventRequest::AcceptedLineFingerprints(event) => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(accepted_line_events.len(), 1);
    let event = accepted_line_events[0];
    assert_eq!(event.event_params.turn_id, "turn-2");
    assert_eq!(event.event_params.thread_id, "thread-2");
    assert_eq!(event.event_params.accepted_added_lines, 20_000);
    assert_eq!(event.event_params.accepted_deleted_lines, 0);
    assert!(event.event_params.line_fingerprints.is_empty());
    assert!(serde_json::to_vec(event).expect("serialize event").len() < 2_100_000);
}

#[tokio::test]
async fn reducer_emits_accepted_line_fingerprints_once_from_latest_turn_diff_on_completion() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut events,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ true,
    )
    .await;
    events.clear();

    for line in ["let old_value = 1;", "let latest_value = 2;"] {
        let diff = format!(
            "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -0,0 +1 @@
+{line}
"
        );
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(ServerNotification::TurnDiffUpdated(
                    TurnDiffUpdatedNotification {
                        thread_id: "thread-2".to_string(),
                        turn_id: "turn-2".to_string(),
                        diff,
                    },
                ))),
                &mut events,
            )
            .await;
    }
    assert!(events.is_empty());

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut events,
        )
        .await;

    let accepted_line_events = events
        .iter()
        .filter_map(|event| match event {
            TrackEventRequest::AcceptedLineFingerprints(event) => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(accepted_line_events.len(), 1);
    let event = accepted_line_events[0];
    assert_eq!(event.event_params.accepted_added_lines, 1);
    assert!(event.event_params.line_fingerprints.is_empty());
}

#[tokio::test]
#[cfg(debug_assertions)]
async fn analytics_flush_delivers_completed_turn_with_file_diff() {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let capture_path = std::env::temp_dir().join(format!(
        "codex-analytics-turn-flush-{}-{nonce}.jsonl",
        std::process::id()
    ));
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let client = AnalyticsEventsClient::new_for_capture_file(auth_manager, capture_path.clone());

    for fact in [
        sample_initialize_fact(/*connection_id*/ 7),
        AnalyticsFact::ClientResponse {
            connection_id: 7,
            request_id: RequestId::Integer(1),
            response: Box::new(sample_thread_start_response(
                "thread-2", /*ephemeral*/ false, "gpt-5",
            )),
            thread_originator: None,
        },
        AnalyticsFact::ClientRequest {
            connection_id: 7,
            request_id: RequestId::Integer(3),
            request: Box::new(sample_turn_start_request("thread-2", /*request_id*/ 3)),
        },
        AnalyticsFact::ClientResponse {
            connection_id: 7,
            request_id: RequestId::Integer(3),
            response: Box::new(sample_turn_start_response("turn-2")),
            thread_originator: None,
        },
        AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
            sample_turn_resolved_config("thread-2", "turn-2"),
        ))),
        AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
            "thread-2", "turn-2",
        ))),
        AnalyticsFact::Custom(CustomAnalyticsFact::TurnProfile(Box::new(
            TurnProfileFact {
                turn_id: "turn-2".to_string(),
                profile: sample_turn_profile(),
            },
        ))),
        AnalyticsFact::Notification(Box::new(ServerNotification::TurnDiffUpdated(
            TurnDiffUpdatedNotification {
                thread_id: "thread-2".to_string(),
                turn_id: "turn-2".to_string(),
                diff: "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -0,0 +1 @@
+let value = 1;
"
                .to_string(),
            },
        ))),
        AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
            "thread-2",
            "turn-2",
            AppServerTurnStatus::Completed,
            /*codex_error_info*/ None,
        ))),
    ] {
        client.record_fact(fact);
    }

    client.flush().await;

    let contents = std::fs::read_to_string(&capture_path).expect("read captured analytics events");
    let event_types = contents
        .lines()
        .flat_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .expect("parse captured analytics events")["events"]
                .as_array()
                .expect("captured events should be an array")
                .iter()
                .map(|event| {
                    event["event_type"]
                        .as_str()
                        .expect("captured event type should be a string")
                        .to_string()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(event_types.iter().any(|event| event == "codex_turn_event"));
    assert!(
        event_types
            .iter()
            .any(|event| event == "codex_accepted_line_fingerprints")
    );

    std::fs::remove_file(capture_path).expect("remove analytics capture file");
}
