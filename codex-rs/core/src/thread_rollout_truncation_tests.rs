use super::*;
use crate::session::tests::make_session_and_context;
use codex_protocol::AgentPath;
use codex_protocol::SegmentId;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::protocol::DEFAULT_ROLLOUT_REFERENCE_DEPTH;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::RolloutReferenceItem;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadRolledBackEvent;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn user_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
    }
}

fn assistant_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
    }
}

fn developer_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
    }
}

fn inter_agent_msg(text: &str, trigger_turn: bool) -> ResponseItem {
    let communication = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::try_from("/root/worker").expect("agent path"),
        Vec::new(),
        text.to_string(),
        trigger_turn,
    );
    communication.to_response_input_item().into()
}

async fn write_rollout(path: &std::path::Path, items: &[RolloutItem]) {
    let mut jsonl = String::new();
    for item in items {
        let line = RolloutLine {
            timestamp: "2026-04-30T00:00:00.000Z".to_string(),
            item: item.clone(),
        };
        jsonl.push_str(&serde_json::to_string(&line).expect("serialize rollout line"));
        jsonl.push('\n');
    }
    tokio::fs::write(path, jsonl).await.expect("write rollout");
}

fn session_meta_item(thread_id: ThreadId, segment_id: SegmentId) -> RolloutItem {
    RolloutItem::SessionMeta(SessionMetaLine {
        meta: SessionMeta {
            id: thread_id,
            segment_id: Some(segment_id),
            timestamp: "2026-04-30T00:00:00.000Z".to_string(),
            cwd: PathBuf::from("/tmp"),
            originator: "test".to_string(),
            cli_version: "0.0.0".to_string(),
            ..SessionMeta::default()
        },
        git: None,
    })
}

#[test]
fn truncates_rollout_from_start_before_nth_user_only() {
    let items = [
        user_msg("u1"),
        assistant_msg("a1"),
        assistant_msg("a2"),
        user_msg("u2"),
        assistant_msg("a3"),
        ResponseItem::Reasoning {
            id: "r1".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "s".to_string(),
            }],
            content: None,
            encrypted_content: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            call_id: "c1".to_string(),
            name: "tool".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
        },
        assistant_msg("a4"),
    ];

    let rollout: Vec<RolloutItem> = items
        .iter()
        .cloned()
        .map(RolloutItem::ResponseItem)
        .collect();

    let truncated =
        truncate_rollout_before_nth_user_message_from_start(&rollout, /*n_from_start*/ 1);
    let expected = vec![
        RolloutItem::ResponseItem(items[0].clone()),
        RolloutItem::ResponseItem(items[1].clone()),
        RolloutItem::ResponseItem(items[2].clone()),
    ];
    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );

    let truncated2 =
        truncate_rollout_before_nth_user_message_from_start(&rollout, /*n_from_start*/ 2);
    assert_eq!(
        serde_json::to_value(&truncated2).unwrap(),
        serde_json::to_value(&rollout).unwrap()
    );
}

#[test]
fn truncation_max_keeps_full_rollout() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(user_msg("u2")),
    ];

    let truncated = truncate_rollout_before_nth_user_message_from_start(&rollout, usize::MAX);

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&rollout).unwrap()
    );
}

#[test]
fn legacy_fork_reference_decodes_as_rollout_reference() {
    let item: RolloutItem = serde_json::from_value(serde_json::json!({
        "type": "fork_reference",
        "payload": {
            "rollout_path": "/tmp/legacy.jsonl",
            "nth_user_message": 3
        }
    }))
    .expect("deserialize legacy fork reference");

    let RolloutItem::RolloutReference(reference) = item else {
        panic!("legacy fork reference should decode as rollout reference");
    };
    assert_eq!(reference.rollout_path, PathBuf::from("/tmp/legacy.jsonl"));
    assert_eq!(reference.max_depth, DEFAULT_ROLLOUT_REFERENCE_DEPTH);
    assert_eq!(reference.nth_user_message, Some(3));

    let value = serde_json::to_value(RolloutItem::RolloutReference(reference))
        .expect("serialize rollout reference");
    assert_eq!(value["type"], "rollout_reference");
}

#[tokio::test]
async fn materializes_prefix_truncated_rollout_reference_before_replay() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_path = temp
        .path()
        .join("rollout-2026-04-30T00-00-00-00000000-0000-0000-0000-000000000001.jsonl");
    let source_items = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(assistant_msg("a2")),
    ];
    write_rollout(&source_path, &source_items).await;

    let compact_reference = vec![
        RolloutItem::RolloutReference(RolloutReferenceItem {
            rollout_path: source_path.clone(),
            thread_id: None,
            rollout_timestamp: None,
            segment_id: None,
            max_depth: DEFAULT_ROLLOUT_REFERENCE_DEPTH,
            nth_user_message: Some(1),
            filter_fork_history: false,
            developer_message_filter_texts: None,
        }),
        RolloutItem::ResponseItem(user_msg("child request")),
    ];

    let materialized = materialize_rollout_items_for_replay(temp.path(), &compact_reference).await;

    let expected = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(user_msg("child request")),
    ];
    assert_eq!(
        serde_json::to_value(&materialized).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[tokio::test]
async fn materializes_prefix_truncated_rollout_reference_by_segment_id_after_source_rollover() {
    let temp = tempfile::tempdir().expect("tempdir");
    let thread_id = ThreadId::new();
    let old_segment_id = SegmentId::new();
    let new_segment_id = SegmentId::new();

    let old_active_path = temp
        .path()
        .join("sessions/2026/04/30")
        .join(format!("rollout-2026-04-30T00-00-00-{thread_id}.jsonl"));
    let old_archived_path = temp
        .path()
        .join("archived_sessions/2026/04/30")
        .join(format!("rollout-2026-04-30T00-00-00-{thread_id}.jsonl"));
    let new_active_path = temp
        .path()
        .join("sessions/2026/05/01")
        .join(format!("rollout-2026-05-01T00-00-00-{thread_id}.jsonl"));

    tokio::fs::create_dir_all(old_archived_path.parent().expect("archived parent"))
        .await
        .expect("create archived parent");
    tokio::fs::create_dir_all(new_active_path.parent().expect("active parent"))
        .await
        .expect("create active parent");

    write_rollout(
        &old_archived_path,
        &[
            session_meta_item(thread_id, old_segment_id),
            RolloutItem::ResponseItem(user_msg("old segment request")),
            RolloutItem::ResponseItem(assistant_msg("old segment answer")),
        ],
    )
    .await;
    write_rollout(
        &new_active_path,
        &[
            session_meta_item(thread_id, new_segment_id),
            RolloutItem::ResponseItem(user_msg("new segment request")),
            RolloutItem::ResponseItem(assistant_msg("new segment answer")),
        ],
    )
    .await;

    let compact_reference = vec![
        RolloutItem::RolloutReference(RolloutReferenceItem {
            rollout_path: old_active_path,
            thread_id: Some(thread_id),
            rollout_timestamp: None,
            segment_id: Some(old_segment_id),
            max_depth: DEFAULT_ROLLOUT_REFERENCE_DEPTH,
            nth_user_message: Some(usize::MAX),
            filter_fork_history: false,
            developer_message_filter_texts: None,
        }),
        RolloutItem::ResponseItem(user_msg("child request")),
    ];

    let materialized = materialize_rollout_items_for_replay(temp.path(), &compact_reference).await;
    let text = serde_json::to_string(&materialized).expect("serialize materialized rollout");

    assert!(text.contains("old segment request"));
    assert!(!text.contains("new segment request"));
    assert!(text.contains("child request"));
}

#[tokio::test]
async fn materializes_prefix_truncated_reference_before_bounded_rollout_references() {
    let temp = tempfile::tempdir().expect("tempdir");
    let thread_id = ThreadId::new();
    let old_segment_id = SegmentId::new();
    let current_segment_id = SegmentId::new();

    let old_path = temp.path().join("old.jsonl");
    let current_path = temp.path().join("current.jsonl");

    write_rollout(
        &old_path,
        &[
            session_meta_item(thread_id, old_segment_id),
            RolloutItem::ResponseItem(user_msg("u1")),
            RolloutItem::ResponseItem(assistant_msg("a1")),
        ],
    )
    .await;
    write_rollout(
        &current_path,
        &[
            session_meta_item(thread_id, current_segment_id),
            RolloutItem::RolloutReference(RolloutReferenceItem {
                rollout_path: old_path,
                thread_id: None,
                rollout_timestamp: None,
                segment_id: None,
                max_depth: 2,
                nth_user_message: None,
                filter_fork_history: false,
                developer_message_filter_texts: None,
            }),
            RolloutItem::ResponseItem(user_msg("u2")),
            RolloutItem::ResponseItem(assistant_msg("a2")),
            RolloutItem::ResponseItem(user_msg("u3")),
        ],
    )
    .await;

    let reference_items = vec![
        RolloutItem::RolloutReference(RolloutReferenceItem {
            rollout_path: current_path,
            thread_id: Some(thread_id),
            rollout_timestamp: None,
            segment_id: Some(current_segment_id),
            max_depth: DEFAULT_ROLLOUT_REFERENCE_DEPTH,
            nth_user_message: Some(2),
            filter_fork_history: false,
            developer_message_filter_texts: None,
        }),
        RolloutItem::ResponseItem(user_msg("child request")),
    ];

    let materialized = materialize_rollout_items_for_replay(temp.path(), &reference_items).await;
    let text = serde_json::to_string(&materialized).expect("serialize materialized rollout");

    assert!(text.contains("u1"));
    assert!(text.contains("u2"));
    assert!(!text.contains("u3"));
    assert!(text.contains("child request"));
}

#[tokio::test]
async fn materializes_rollout_reference_with_bounded_depth() {
    let temp = tempfile::tempdir().expect("tempdir");
    let thread_id = ThreadId::new();
    let oldest_segment_id = SegmentId::new();
    let old_segment_id = SegmentId::new();
    let middle_segment_id = SegmentId::new();

    let oldest_path = temp.path().join("oldest.jsonl");
    let old_path = temp.path().join("old.jsonl");
    let middle_path = temp.path().join("middle.jsonl");
    let current_path = temp.path().join("current.jsonl");

    write_rollout(
        &oldest_path,
        &[
            session_meta_item(thread_id, oldest_segment_id),
            RolloutItem::ResponseItem(user_msg("oldest segment request")),
        ],
    )
    .await;
    write_rollout(
        &old_path,
        &[
            session_meta_item(thread_id, old_segment_id),
            RolloutItem::RolloutReference(RolloutReferenceItem {
                rollout_path: oldest_path,
                thread_id: None,
                rollout_timestamp: None,
                segment_id: None,
                max_depth: 2,
                nth_user_message: None,
                filter_fork_history: false,
                developer_message_filter_texts: None,
            }),
            RolloutItem::ResponseItem(user_msg("old segment request")),
        ],
    )
    .await;
    write_rollout(
        &middle_path,
        &[
            session_meta_item(thread_id, middle_segment_id),
            RolloutItem::RolloutReference(RolloutReferenceItem {
                rollout_path: old_path,
                thread_id: None,
                rollout_timestamp: None,
                segment_id: None,
                max_depth: 2,
                nth_user_message: None,
                filter_fork_history: false,
                developer_message_filter_texts: None,
            }),
            RolloutItem::ResponseItem(user_msg("middle segment request")),
        ],
    )
    .await;

    let current_items = vec![
        RolloutItem::RolloutReference(RolloutReferenceItem {
            rollout_path: middle_path,
            thread_id: None,
            rollout_timestamp: None,
            segment_id: None,
            max_depth: 2,
            nth_user_message: None,
            filter_fork_history: false,
            developer_message_filter_texts: None,
        }),
        RolloutItem::ResponseItem(user_msg("current segment request")),
    ];
    write_rollout(&current_path, &current_items).await;
    let materialized = materialize_rollout_items_for_replay(temp.path(), &current_items).await;
    let text = serde_json::to_string(&materialized).expect("serialize materialized rollout");

    assert!(!text.contains("oldest segment request"));
    assert!(text.contains("old segment request"));
    assert!(text.contains("middle segment request"));
    assert!(text.contains("current segment request"));

    let fork_items = vec![RolloutItem::RolloutReference(RolloutReferenceItem {
        rollout_path: current_path,
        thread_id: None,
        rollout_timestamp: None,
        segment_id: None,
        max_depth: DEFAULT_ROLLOUT_REFERENCE_DEPTH,
        nth_user_message: Some(usize::MAX),
        filter_fork_history: false,
        developer_message_filter_texts: None,
    })];
    let fork_materialized = materialize_rollout_items_for_replay(temp.path(), &fork_items).await;
    assert_eq!(
        serde_json::to_value(&fork_materialized).unwrap(),
        serde_json::to_value(&materialized).unwrap()
    );
}

#[test]
fn truncates_rollout_from_start_applies_thread_rollback_markers() {
    let rollout_items = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(assistant_msg("a2")),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
            num_turns: 1,
        })),
        RolloutItem::ResponseItem(user_msg("u3")),
        RolloutItem::ResponseItem(assistant_msg("a3")),
        RolloutItem::ResponseItem(user_msg("u4")),
        RolloutItem::ResponseItem(assistant_msg("a4")),
    ];

    // Effective user history after applying rollback(1) is: u1, u3, u4.
    // So n_from_start=2 should cut before u4 (not u3).
    let truncated = truncate_rollout_before_nth_user_message_from_start(
        &rollout_items,
        /*n_from_start*/ 2,
    );
    let expected = rollout_items[..7].to_vec();
    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[tokio::test]
async fn ignores_session_prefix_messages_when_truncating_rollout_from_start() {
    let (session, turn_context) = make_session_and_context().await;
    let mut items = session.build_initial_context(&turn_context).await;
    items.push(user_msg("feature request"));
    items.push(assistant_msg("ack"));
    items.push(user_msg("second question"));
    items.push(assistant_msg("answer"));

    let rollout_items: Vec<RolloutItem> = items
        .iter()
        .cloned()
        .map(RolloutItem::ResponseItem)
        .collect();

    let truncated = truncate_rollout_before_nth_user_message_from_start(
        &rollout_items,
        /*n_from_start*/ 1,
    );
    let expected: Vec<RolloutItem> = vec![
        RolloutItem::ResponseItem(items[0].clone()),
        RolloutItem::ResponseItem(items[1].clone()),
        RolloutItem::ResponseItem(items[2].clone()),
        RolloutItem::ResponseItem(items[3].clone()),
    ];

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn truncates_rollout_to_last_n_fork_turns_counts_trigger_turn_messages() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "queued message",
            /*trigger_turn*/ false,
        )),
        RolloutItem::ResponseItem(assistant_msg("a2")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task",
            /*trigger_turn*/ true,
        )),
        RolloutItem::ResponseItem(assistant_msg("a3")),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(assistant_msg("a4")),
    ];

    let truncated = truncate_rollout_to_last_n_fork_turns(&rollout, /*n_from_end*/ 2);
    let expected = rollout[4..].to_vec();

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn truncates_rollout_to_last_n_fork_turns_drops_startup_prefix_even_when_under_limit() {
    let rollout = vec![
        RolloutItem::ResponseItem(developer_msg("startup developer context")),
        RolloutItem::ResponseItem(user_msg("current task")),
        RolloutItem::ResponseItem(assistant_msg("answer")),
    ];

    let truncated = truncate_rollout_to_last_n_fork_turns(&rollout, /*n_from_end*/ 2);
    let expected = rollout[1..].to_vec();

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn truncates_rollout_to_last_n_fork_turns_applies_thread_rollback_markers() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task",
            /*trigger_turn*/ true,
        )),
        RolloutItem::ResponseItem(assistant_msg("a2")),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
            num_turns: 1,
        })),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(assistant_msg("a3")),
    ];

    let truncated = truncate_rollout_to_last_n_fork_turns(&rollout, /*n_from_end*/ 2);

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&rollout).unwrap()
    );
}

#[test]
fn fork_turn_positions_ignore_zero_turn_rollback_markers() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task",
            /*trigger_turn*/ true,
        )),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
            num_turns: 0,
        })),
        RolloutItem::ResponseItem(user_msg("u2")),
    ];

    assert_eq!(fork_turn_positions_in_rollout(&rollout), vec![0, 1, 3]);
}

#[test]
fn truncates_rollout_to_last_n_fork_turns_discards_trigger_boundaries_in_rolled_back_suffix() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task",
            /*trigger_turn*/ true,
        )),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
            num_turns: 1,
        })),
        RolloutItem::ResponseItem(user_msg("u3")),
        RolloutItem::ResponseItem(assistant_msg("a2")),
    ];

    let truncated = truncate_rollout_to_last_n_fork_turns(&rollout, /*n_from_end*/ 2);

    let expected = rollout[1..].to_vec();

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn truncates_rollout_to_last_n_fork_turns_discards_rolled_back_assistant_instruction_turns() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task 1",
            /*trigger_turn*/ true,
        )),
        RolloutItem::ResponseItem(assistant_msg("a2")),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
            num_turns: 1,
        })),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task 2",
            /*trigger_turn*/ true,
        )),
        RolloutItem::ResponseItem(assistant_msg("a3")),
    ];

    let truncated = truncate_rollout_to_last_n_fork_turns(&rollout, /*n_from_end*/ 1);
    let expected = rollout[5..].to_vec();

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn truncates_rollout_to_last_n_fork_turns_keeps_full_rollout_when_n_is_large() {
    let rollout = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(inter_agent_msg(
            "triggered task",
            /*trigger_turn*/ true,
        )),
        RolloutItem::ResponseItem(assistant_msg("a2")),
    ];

    let truncated = truncate_rollout_to_last_n_fork_turns(&rollout, /*n_from_end*/ 10);

    assert_eq!(
        serde_json::to_value(&truncated).unwrap(),
        serde_json::to_value(&rollout).unwrap()
    );
}
