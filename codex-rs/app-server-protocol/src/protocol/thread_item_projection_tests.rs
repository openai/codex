use super::*;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_thread_store_protocol::StoredLifecycleProjectionState;
use codex_thread_store_protocol::StoredThreadHistoryProjectionState;
use codex_thread_store_protocol::StoredThreadItem;
use codex_thread_store_protocol::StoredThreadItemProjectionState;
use codex_thread_store_protocol::StoredTurn;
use codex_thread_store_protocol::StoredTurnItemsView;
use codex_thread_store_protocol::StoredTurnStatus;
use codex_thread_store_protocol::StoredTurnSummaryProjectionState;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::time::Duration;

#[test]
fn restores_thread_item_ordinal_for_open_item_upserts() {
    let mut observer = ThreadItemProjectionObserver::new();
    let begin_projection_source_events = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-a".into(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
        RolloutItem::EventMsg(EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "exec-1".into(),
            process_id: Some("pid-1".into()),
            turn_id: "turn-a".into(),
            started_at_ms: 0,
            command: vec!["echo".into(), "hello".into()],
            cwd: test_path_buf("/tmp").abs(),
            parsed_cmd: vec![ParsedCommand::Unknown {
                cmd: "echo hello".into(),
            }],
            source: ExecCommandSource::Agent,
            interaction_input: None,
        })),
    ];
    let begin_mutations = observer
        .observe_append(&[], &begin_projection_source_events)
        .thread_history_mutations;
    let stored_state = stored_projection_state(&observer, "turn-a");
    let mut resumed_observer =
        ThreadItemProjectionObserver::from_stored_state(&stored_state.thread_items)
            .expect("restore observer");
    let end_projection_source_events = vec![RolloutItem::EventMsg(EventMsg::ExecCommandEnd(
        ExecCommandEndEvent {
            call_id: "exec-1".into(),
            process_id: Some("pid-1".into()),
            turn_id: "turn-a".into(),
            completed_at_ms: 1,
            command: vec!["echo".into(), "hello".into()],
            cwd: test_path_buf("/tmp").abs(),
            parsed_cmd: vec![ParsedCommand::Unknown {
                cmd: "echo hello".into(),
            }],
            source: ExecCommandSource::Agent,
            interaction_input: None,
            stdout: "hello\n".into(),
            stderr: String::new(),
            aggregated_output: "hello\n".into(),
            exit_code: 0,
            duration: Duration::from_millis(1),
            formatted_output: "hello\n".into(),
            status: CoreExecCommandStatus::Completed,
        },
    ))];
    let end_mutations = resumed_observer
        .observe_append(&[], &end_projection_source_events)
        .thread_history_mutations;

    let begin_payload = only_thread_item_payload(begin_mutations);
    let end_payload = only_thread_item_payload(end_mutations);
    assert_eq!(begin_payload["itemOrdinal"], end_payload["itemOrdinal"]);
    assert_eq!(begin_payload["isOpen"], true);
    assert_eq!(end_payload["isOpen"], false);
}

#[test]
fn restores_current_reasoning_item_for_later_coalescing() {
    let mut observer = ThreadItemProjectionObserver::new();
    observer.observe_append(
        &[],
        &[
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                trace_id: None,
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::AgentReasoning(
                codex_protocol::protocol::AgentReasoningEvent {
                    text: "thinking".into(),
                },
            )),
        ],
    );
    let stored_state = stored_projection_state(&observer, "turn-a");
    let mut resumed_observer =
        ThreadItemProjectionObserver::from_stored_state(&stored_state.thread_items)
            .expect("restore observer");

    let mutations = resumed_observer
        .observe_append(
            &[],
            &[RolloutItem::EventMsg(EventMsg::AgentReasoning(
                codex_protocol::protocol::AgentReasoningEvent {
                    text: "more".into(),
                },
            ))],
        )
        .thread_history_mutations;

    let payload = only_thread_item_payload(mutations);
    assert_eq!(payload["itemKey"], "item-1");
    assert_eq!(payload["itemOrdinal"], 1);
    assert_eq!(
        payload["materializedThreadItem"]["summary"],
        serde_json::json!(["thinking", "more"])
    );
}

fn stored_projection_state(
    observer: &ThreadItemProjectionObserver,
    current_turn_id: &str,
) -> StoredThreadHistoryProjectionState {
    let checkpoint = observer.checkpoint();
    let turns = observer.materialized_turns();
    StoredThreadHistoryProjectionState {
        thread_items: StoredThreadItemProjectionState {
            turns: turns
                .iter()
                .map(|turn| stored_turn(turn, StoredTurnItemsView::NotLoaded))
                .collect(),
            items: turns
                .iter()
                .flat_map(|turn| {
                    turn.items.iter().map(|item| StoredThreadItem {
                        turn_id: turn.id.clone(),
                        item_key: item.id().to_string(),
                        item_ordinal: checkpoint
                            .open_items
                            .get(item.id())
                            .map_or(1, |item| item.item_ordinal),
                        is_open: checkpoint.open_items.contains_key(item.id()),
                        item: serde_json::to_value(item).expect("serialize item"),
                    })
                })
                .collect(),
            current_turn_id: Some(current_turn_id.to_string()),
            next_thread_item_ordinal: checkpoint.next_thread_item_ordinal,
            next_generated_thread_item_id_index: checkpoint.next_generated_thread_item_id_index,
        },
        turn_summaries: StoredTurnSummaryProjectionState {
            turns: turns
                .iter()
                .map(|turn| stored_turn(turn, StoredTurnItemsView::Summary))
                .collect(),
            current_turn_id: Some(current_turn_id.to_string()),
            next_generated_thread_item_id_index: checkpoint.next_generated_thread_item_id_index,
        },
        lifecycle: StoredLifecycleProjectionState {
            current_turn_id: Some(current_turn_id.to_string()),
        },
    }
}

fn stored_turn(turn: &Turn, items_view: StoredTurnItemsView) -> StoredTurn {
    StoredTurn {
        turn_id: turn.id.clone(),
        items: Vec::new(),
        items_view,
        status: StoredTurnStatus::InProgress,
        error: None,
        started_at: turn.started_at,
        completed_at: turn.completed_at,
        duration_ms: turn.duration_ms,
    }
}

fn only_thread_item_payload(mutations: Vec<ThreadHistoryMutation>) -> Value {
    let mut payloads = mutations.into_iter().filter_map(|mutation| match mutation {
        ThreadHistoryMutation::ThreadItem(mutation) => Some(mutation.payload),
        ThreadHistoryMutation::TurnSummary(_) | ThreadHistoryMutation::Lifecycle(_) => None,
    });
    let payload = payloads.next().expect("thread item mutation");
    assert!(payloads.next().is_none());
    payload
}
