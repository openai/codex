//! App mention and usage events.

use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexAppUsedMetadata;
use crate::events::TrackEventRequest;
use crate::events::codex_app_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CustomAnalyticsFact;
use crate::facts::ElicitationType;
use crate::facts::InvocationType;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::test_tracking_context;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn app_mentioned_event_serializes_expected_shape() {
    let tracking = test_tracking_context("thread-1", "turn-1");
    let event = TrackEventRequest::AppMentioned(CodexAppMentionedEventRequest {
        event_type: "codex_app_mentioned",
        event_params: codex_app_metadata(
            &tracking,
            AppInvocation {
                connector_id: Some("calendar".to_string()),
                app_name: Some("Calendar".to_string()),
                invocation_type: Some(InvocationType::Explicit),
            },
        ),
    });

    let payload = serde_json::to_value(&event).expect("serialize app mentioned event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_app_mentioned",
            "event_params": {
                "connector_id": "calendar",
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "app_name": "Calendar",
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
                "invoke_type": "explicit",
                "model_slug": "gpt-5"
            }
        })
    );
}

#[test]
fn app_used_event_serializes_expected_shape() {
    let tracking = test_tracking_context("thread-2", "turn-2");
    let event = TrackEventRequest::AppUsed(CodexAppUsedEventRequest {
        event_type: "codex_app_used",
        event_params: CodexAppUsedMetadata {
            app: codex_app_metadata(
                &tracking,
                AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Google Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
            ),
            voice_session_id: None,
            elicitation_type: None,
        },
    });

    let payload = serde_json::to_value(&event).expect("serialize app used event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_app_used",
            "event_params": {
                "connector_id": "drive",
                "thread_id": "thread-2",
                "turn_id": "turn-2",
                "app_name": "Google Drive",
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
                "invoke_type": "implicit",
                "model_slug": "gpt-5",
                "voice_session_id": null,
                "elicitation_type": null
            }
        })
    );
}

#[tokio::test]
async fn reducer_ingests_app_facts() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = test_tracking_context("thread-1", "turn-1");

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppMentioned(AppMentionedInput {
                tracking: tracking.clone(),
                mentions: vec![AppInvocation {
                    connector_id: Some("calendar".to_string()),
                    app_name: Some("Calendar".to_string()),
                    invocation_type: Some(InvocationType::Explicit),
                }],
            })),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput {
                tracking,
                app: AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
                elicitation_type: Some(ElicitationType::AuthOrLink),
            })),
            &mut events,
        )
        .await;

    let [
        TrackEventRequest::AppMentioned(mention),
        TrackEventRequest::AppUsed(usage),
    ] = events.as_slice()
    else {
        panic!("expected app mention and usage events");
    };
    assert_eq!(
        [
            (
                mention.event_type,
                mention.event_params.product_client_id.as_deref()
            ),
            (
                usage.event_type,
                usage.event_params.app.product_client_id.as_deref()
            ),
        ],
        [
            ("codex_app_mentioned", Some(TEST_PRODUCT_CLIENT_ID)),
            ("codex_app_used", Some(TEST_PRODUCT_CLIENT_ID)),
        ]
    );
    assert_eq!(
        usage.event_params.elicitation_type,
        Some(ElicitationType::AuthOrLink)
    );
}
