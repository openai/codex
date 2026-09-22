//! Tests for cumulative MCP attribution restoration and checkpoint acknowledgements.

use super::*;
use codex_history::CodexHarnessMetadata;
use codex_history::CompactedItem;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

fn source(tool_name: &str, first_turn_id: &str) -> McpAttributionSource {
    McpAttributionSource {
        connector_id: None,
        plugin_id: None,
        server_name: "example".to_string(),
        tool_name: tool_name.to_string(),
        first_turn_id: first_turn_id.to_string(),
    }
}

fn envelope(attribution: Option<McpAttribution>) -> ResponseItemEnvelope {
    ResponseItemEnvelope {
        item: ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: Vec::new(),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: attribution.map(|mcp_attribution| CodexHarnessMetadata {
            mcp_attribution: Some(mcp_attribution),
            ..Default::default()
        }),
    }
}

#[test]
fn records_the_first_turn_for_each_unique_source() {
    let recorder = McpAttributionRecorder::default();
    recorder.record(source("search", "turn_1"));
    recorder.record(source("search", "turn_2"));
    recorder.record(source("fetch", "turn_2"));

    assert_eq!(
        recorder.snapshot(),
        McpAttribution {
            status: McpAttributionStatus::Complete,
            sources: vec![source("search", "turn_1"), source("fetch", "turn_2")],
        }
    );
}

#[test]
fn restores_cumulative_item_and_compaction_checkpoints() {
    let initial = McpAttribution {
        status: McpAttributionStatus::Complete,
        sources: vec![source("search", "turn_1")],
    };
    let cumulative = McpAttribution {
        status: McpAttributionStatus::Complete,
        sources: vec![source("search", "turn_1"), source("fetch", "turn_2")],
    };
    let history = InitialHistory::Forked(vec![
        RolloutItem::ResponseItem(envelope(Some(initial))),
        RolloutItem::Compacted(CompactedItem {
            message: "summary".to_string(),
            replacement_history: Some(vec![envelope(Some(cumulative.clone()))]),
            guardian_history: None,
            retained_context: None,
            mcp_resource_origins: None,
            window_number: None,
            first_window_id: None,
            previous_window_id: None,
            window_id: None,
            compaction_response_id: None,
            latest_token_usage_record: None,
            resume_metadata: None,
        }),
    ]);

    assert_eq!(McpAttributionRecorder::new(&history).snapshot(), cumulative);
}

#[test]
fn legacy_or_conflicting_history_is_not_complete() {
    let legacy = InitialHistory::Forked(vec![RolloutItem::ResponseItem(envelope(
        /*attribution*/ None,
    ))]);
    assert_eq!(
        McpAttributionRecorder::new(&legacy).snapshot(),
        McpAttribution {
            status: McpAttributionStatus::AttributionError,
            sources: Vec::new(),
        }
    );

    let history = InitialHistory::Forked(
        ["turn_1", "turn_2"]
            .map(|turn_id| {
                RolloutItem::ResponseItem(envelope(Some(McpAttribution {
                    status: McpAttributionStatus::Complete,
                    sources: vec![source("search", turn_id)],
                })))
            })
            .to_vec(),
    );
    assert_eq!(
        McpAttributionRecorder::new(&history).snapshot(),
        McpAttribution {
            status: McpAttributionStatus::AttributionError,
            sources: vec![source("search", "turn_1")],
        }
    );
}

#[test]
fn acknowledging_an_older_checkpoint_does_not_clear_newer_changes() {
    let recorder = McpAttributionRecorder::default();
    let (_, initial_revision) = recorder
        .checkpoint(/*force*/ false)
        .expect("initial checkpoint");
    recorder.record(source("search", "turn_1"));
    recorder.mark_persisted(initial_revision);

    let (_, latest_revision) = recorder
        .checkpoint(/*force*/ false)
        .expect("dirty checkpoint");
    recorder.mark_persisted(latest_revision);
    assert_eq!(recorder.checkpoint(/*force*/ false), None);
    assert!(recorder.checkpoint(/*force*/ true).is_some());
}
