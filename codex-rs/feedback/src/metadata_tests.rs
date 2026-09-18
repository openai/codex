//! Exercise dynamic tag collection, upload serialization, and bounded updates.

use super::*;
use crate::CodexFeedback;
use pretty_assertions::assert_eq;
use sentry::protocol::Envelope;
use sentry::protocol::EnvelopeItem;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[test]
fn dynamic_tags_survive_upload_alongside_static_fields() {
    let feedback = CodexFeedback::new();
    let _guard = tracing_subscriber::registry()
        .with(feedback.metadata_layer())
        .set_default();
    let mut expected: BTreeMap<_, _> = (0..200)
        .map(|index| {
            (
                format!("feature.test_{index}"),
                (index % 2 == 0).to_string(),
            )
        })
        .collect();
    expected.insert("model_context_window".to_string(), "872000".to_string());
    tracing::info!(
        target: FEEDBACK_TAGS_TARGET,
        model = "test-model",
        tags_json = ?tracing::field::display(serde_json::json!(expected)),
    );
    expected.insert("model".to_string(), "test-model".to_string());
    let snapshot = feedback.snapshot(/*session_id*/ None);
    assert_eq!(snapshot.tags, expected);
    let event = snapshot.feedback_event(
        "bug", /*reason*/ None, /*tags*/ None, /*session_source*/ None,
    );
    expected.extend([
        ("thread_id".to_string(), snapshot.thread_id),
        ("classification".to_string(), "bug".to_string()),
        (
            "cli_version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
    ]);
    assert_eq!(event.tags, expected);
    let mut envelope = Envelope::new();
    envelope.add_item(EnvelopeItem::Event(event));
    let (_, size) = crate::upload::encode_envelope(&envelope).unwrap();
    assert!(size < crate::MAX_EVENT_BYTES);
}

#[test]
fn bounded_dynamic_tags_still_update_existing_values() {
    let feedback = CodexFeedback::new();
    let _guard = tracing_subscriber::registry()
        .with(feedback.metadata_layer())
        .set_default();
    let mut expected: BTreeMap<_, _> = (0..MAX_FEEDBACK_TAGS)
        .map(|index| (format!("feature.test_{index}"), "false".to_string()))
        .collect();
    tracing::info!(target: FEEDBACK_TAGS_TARGET, tags_json = %serde_json::json!(expected));
    tracing::info!(
        target: FEEDBACK_TAGS_TARGET,
        tags_json = %serde_json::json!({
            "feature.overflow": "true",
            "feature.test_0": "true",
        }),
    );
    expected.insert("feature.test_0".to_string(), "true".to_string());
    assert_eq!(feedback.snapshot(/*session_id*/ None).tags, expected);
    tracing::info!(
        target: FEEDBACK_TAGS_TARGET,
        tags_json = %serde_json::json!({"feature.test_0": "false"}),
    );
    expected.insert("feature.test_0".to_string(), "false".to_string());
    assert_eq!(feedback.snapshot(/*session_id*/ None).tags, expected);
}
