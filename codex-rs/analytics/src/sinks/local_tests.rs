use super::LOCAL_ANALYTICS_SCHEMA_VERSION;
use super::LocalAnalyticsRecord;
use super::LocalAnalyticsRecordType;
use super::append_local_analytics_record_best_effort;
use super::local_analytics_sink_for_path;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::TrackEventRequest;
use crate::facts::InvocationType;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

static NEXT_TEST_PATH_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn codex_analytics_record_extracts_generic_envelope_metadata() {
    let record = LocalAnalyticsRecord::from_codex_analytics_event(&sample_track_event())
        .expect("serialize local analytics event");

    assert_eq!(
        record,
        LocalAnalyticsRecord {
            schema_version: LOCAL_ANALYTICS_SCHEMA_VERSION,
            recorded_at_epoch_millis: record.recorded_at_epoch_millis,
            record_type: LocalAnalyticsRecordType::CodexAnalyticsEvent,
            session_id: None,
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            payload: json!({
                "event_type": "skill_invocation",
                "skill_id": "skill-1",
                "skill_name": "doc",
                "event_params": {
                    "product_client_id": null,
                    "skill_scope": null,
                    "plugin_id": null,
                    "repo_url": null,
                    "thread_id": "thread-1",
                    "turn_id": "turn-1",
                    "invoke_type": "explicit",
                    "model_slug": "gpt-5.1-codex"
                }
            }),
        }
    );
}

#[test]
fn process_global_sink_reuses_writer_for_same_path() {
    let path = test_sink_path("shared");
    let first = local_analytics_sink_for_path(path.clone()).expect("first sink");
    let second = local_analytics_sink_for_path(path).expect("second sink");

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn sink_appends_complete_jsonl_records() {
    let path = test_sink_path("records");
    let sink = local_analytics_sink_for_path(path.clone()).expect("sink");
    let first = LocalAnalyticsRecord::from_codex_analytics_event(&sample_track_event())
        .expect("first record");
    let mut second = first.clone();
    second.turn_id = Some("turn-2".to_string());

    append_local_analytics_record_best_effort(&sink, &first);
    append_local_analytics_record_best_effort(&sink, &second);

    let contents = fs::read_to_string(path).expect("read sink");
    let records = contents
        .lines()
        .map(|line| serde_json::from_str::<LocalAnalyticsRecord>(line).expect("record"))
        .collect::<Vec<_>>();
    assert_eq!(records, vec![first, second]);
}

#[test]
fn sink_initialization_failure_is_best_effort() {
    let path = test_sink_path("missing-parent")
        .join("missing")
        .join("events.jsonl");

    assert!(local_analytics_sink_for_path(path).is_none());
}

fn sample_track_event() -> TrackEventRequest {
    TrackEventRequest::SkillInvocation(SkillInvocationEventRequest {
        event_type: "skill_invocation",
        skill_id: "skill-1".to_string(),
        skill_name: "doc".to_string(),
        event_params: SkillInvocationEventParams {
            product_client_id: None,
            skill_scope: None,
            plugin_id: None,
            repo_url: None,
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            invoke_type: Some(InvocationType::Explicit),
            model_slug: Some("gpt-5.1-codex".to_string()),
        },
    })
}

fn test_sink_path(label: &str) -> PathBuf {
    let id = NEXT_TEST_PATH_ID.fetch_add(1, Ordering::Relaxed);
    let process_id = std::process::id();
    let dir = std::env::temp_dir().join(format!("codex-analytics-{process_id}-{label}-{id}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir.join("events.jsonl")
}
