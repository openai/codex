use super::*;
use crate::protocol::v2::CommandAction;
use crate::protocol::v2::CommandExecutionSource;
use crate::protocol::v2::CommandExecutionStatus;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent as CoreAgentMessageContent;
use codex_protocol::items::AgentMessageItem as CoreAgentMessageItem;
use codex_protocol::items::PlanItem as CorePlanItem;
use codex_protocol::items::ReasoningItem as CoreReasoningItem;
use codex_protocol::items::TurnItem as CoreTurnItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
use codex_protocol::protocol::ExecOutputStream;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::PlanDeltaEvent;
use codex_protocol::protocol::ReasoningContentDeltaEvent;
use codex_protocol::protocol::ReasoningRawContentDeltaEvent;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::time::Duration;

const TURN_A: &str = "turn-a";
const TURN_B: &str = "turn-b";
const AGENT_ID: &str = "agent-1";
const PLAN_ID: &str = "plan-1";
const REASONING_ID: &str = "reasoning-1";
const COMMAND_ID: &str = "command-1";

#[test]
fn live_agent_message_state_is_cumulative() {
    let mut builder = started_builder(TURN_A);
    let started_item = core_agent(AGENT_ID, &[], Some(MessagePhase::Commentary));
    let mut expected = vec![agent_item(AGENT_ID, "", Some(MessagePhase::Commentary))];

    apply_single_item(&mut builder, item_started(TURN_A, started_item), &expected);
    expected[0] = agent_item(AGENT_ID, "hello ", Some(MessagePhase::Commentary));
    apply_single_item(
        &mut builder,
        agent_delta(TURN_A, AGENT_ID, "hello "),
        &expected,
    );
    apply_no_change(&mut builder, EventMsg::TokenCount(empty_token_count()));
    expected[0] = agent_item(AGENT_ID, "hello world", Some(MessagePhase::Commentary));
    apply_single_item(
        &mut builder,
        agent_delta(TURN_A, AGENT_ID, "world"),
        &expected,
    );

    let completed_item = core_agent(
        AGENT_ID,
        &["canonical ", "answer"],
        Some(MessagePhase::FinalAnswer),
    );
    expected[0] = agent_item(
        AGENT_ID,
        "canonical answer",
        Some(MessagePhase::FinalAnswer),
    );
    apply_single_item(
        &mut builder,
        item_completed(TURN_A, completed_item.clone()),
        &expected,
    );

    for legacy_event in completed_item.as_legacy_events(/*show_raw_agent_reasoning*/ true) {
        apply_no_change(&mut builder, legacy_event);
    }
}

#[test]
fn live_plan_state_is_cumulative_and_completion_is_authoritative() {
    let mut builder = started_builder(TURN_A);
    let mut expected = vec![plan_item(PLAN_ID, "")];

    apply_single_item(
        &mut builder,
        item_started(TURN_A, core_plan(PLAN_ID, "")),
        &expected,
    );
    expected[0] = plan_item(PLAN_ID, "draft ");
    apply_single_item(
        &mut builder,
        plan_delta(TURN_A, PLAN_ID, "draft "),
        &expected,
    );
    expected[0] = plan_item(PLAN_ID, "draft plan");
    apply_single_item(&mut builder, plan_delta(TURN_A, PLAN_ID, "plan"), &expected);

    expected[0] = plan_item(PLAN_ID, "canonical plan");
    apply_single_item(
        &mut builder,
        item_completed(TURN_A, core_plan(PLAN_ID, "canonical plan")),
        &expected,
    );
    apply_no_change(&mut builder, item_completed(TURN_A, core_plan(PLAN_ID, "")));
}

#[test]
fn live_reasoning_state_is_cumulative_by_index() {
    let mut builder = started_builder(TURN_A);
    let mut expected = vec![reasoning_item(REASONING_ID, &[], &[])];

    apply_single_item(
        &mut builder,
        item_started(TURN_A, core_reasoning(REASONING_ID, &[], &[])),
        &expected,
    );
    expected[0] = reasoning_item(REASONING_ID, &["think"], &[]);
    apply_single_item(
        &mut builder,
        reasoning_summary_delta(TURN_A, REASONING_ID, 0, "think"),
        &expected,
    );
    expected[0] = reasoning_item(REASONING_ID, &["think more"], &[]);
    apply_single_item(
        &mut builder,
        reasoning_summary_delta(TURN_A, REASONING_ID, 0, " more"),
        &expected,
    );
    expected[0] = reasoning_item(REASONING_ID, &["think more", "next"], &[]);
    apply_single_item(
        &mut builder,
        reasoning_summary_delta(TURN_A, REASONING_ID, 1, "next"),
        &expected,
    );
    expected[0] = reasoning_item(REASONING_ID, &["think more", "next"], &["raw"]);
    apply_single_item(
        &mut builder,
        reasoning_raw_delta(TURN_A, REASONING_ID, 0, "raw"),
        &expected,
    );
    expected[0] = reasoning_item(REASONING_ID, &["think more", "next"], &["raw detail"]);
    apply_single_item(
        &mut builder,
        reasoning_raw_delta(TURN_A, REASONING_ID, 0, " detail"),
        &expected,
    );

    let completed_item = core_reasoning(
        REASONING_ID,
        &["canonical summary", "second summary"],
        &["canonical raw", "second raw"],
    );
    expected[0] = reasoning_item(
        REASONING_ID,
        &["canonical summary", "second summary"],
        &["canonical raw", "second raw"],
    );
    apply_single_item(
        &mut builder,
        item_completed(TURN_A, completed_item.clone()),
        &expected,
    );

    let mut legacy_events = completed_item.as_legacy_events(/*show_raw_agent_reasoning*/ true);
    legacy_events.reverse();
    for legacy_event in legacy_events {
        apply_no_change(&mut builder, legacy_event);
    }
}

#[test]
fn live_command_state_is_cumulative_by_chunk() {
    let mut builder = started_builder(TURN_A);
    let mut expected = vec![running_command(/*aggregated_output*/ None)];

    apply_single_item(&mut builder, command_begin(TURN_A), &expected);
    expected[0] = running_command(Some("out "));
    apply_single_item(
        &mut builder,
        command_delta(ExecOutputStream::Stdout, b"out "),
        &expected,
    );
    expected[0] = running_command(Some("out warning "));
    apply_single_item(
        &mut builder,
        command_delta(ExecOutputStream::Stderr, b"warning "),
        &expected,
    );
    expected[0] = running_command(Some("out warning done"));
    apply_single_item(
        &mut builder,
        command_delta(ExecOutputStream::Stdout, b"done"),
        &expected,
    );

    expected[0] = completed_command("canonical output");
    apply_single_item(
        &mut builder,
        command_end(TURN_A, "canonical output"),
        &expected,
    );
}

#[test]
fn mismatched_live_events_are_noops() {
    let mut builder = started_builder(TURN_A);
    let expected = vec![
        agent_item(AGENT_ID, "", None),
        plan_item(PLAN_ID, ""),
        reasoning_item(REASONING_ID, &[], &[]),
        running_command(/*aggregated_output*/ None),
    ];
    for event in [
        item_started(TURN_A, core_agent(AGENT_ID, &[], None)),
        item_started(TURN_A, core_plan(PLAN_ID, "")),
        item_started(TURN_A, core_reasoning(REASONING_ID, &[], &[])),
        command_begin(TURN_A),
    ] {
        builder.handle_event(&event);
    }
    assert_active_turn(&builder, TURN_A, expected.clone(), TurnStatus::InProgress);

    for event in [
        agent_delta(TURN_B, AGENT_ID, "wrong turn"),
        plan_delta(TURN_A, "unknown-plan", "wrong item"),
        reasoning_summary_delta(TURN_A, AGENT_ID, 0, "wrong type"),
        reasoning_raw_delta(TURN_A, REASONING_ID, -1, "negative index"),
        EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
            call_id: "unknown-command".to_string(),
            stream: ExecOutputStream::Stdout,
            chunk: b"wrong item".to_vec(),
        }),
    ] {
        assert_eq!(
            apply_no_change_with_expected(&mut builder, event, expected.clone()),
            ThreadHistoryChangeSet::default()
        );
    }
}

#[test]
fn nonmatching_agent_legacy_event_is_preserved() {
    let mut builder = started_builder(TURN_A);
    let completed_item = core_agent(AGENT_ID, &["same text"], Some(MessagePhase::FinalAnswer));
    let completed = agent_item(AGENT_ID, "same text", Some(MessagePhase::FinalAnswer));
    apply_single_item(
        &mut builder,
        item_completed(TURN_A, completed_item.clone()),
        std::slice::from_ref(&completed),
    );

    let commentary = agent_item("item-1", "same text", Some(MessagePhase::Commentary));
    assert_eq!(
        apply_with_expected(
            &mut builder,
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "same text".to_string(),
                phase: Some(MessagePhase::Commentary),
                memory_citation: None,
            }),
            vec![completed.clone(), commentary.clone()],
        ),
        single_item_change(TURN_A, commentary.clone())
    );
    assert_eq!(
        apply_no_change_with_expected(
            &mut builder,
            completed_item
                .as_legacy_events(/*show_raw_agent_reasoning*/ true)
                .into_iter()
                .next()
                .expect("agent completion should generate one legacy event"),
            vec![completed, commentary],
        ),
        ThreadHistoryChangeSet::default()
    );
}

#[test]
fn legacy_expectations_clear_at_turn_boundary() {
    let mut builder = started_builder(TURN_A);
    let completed_item = core_agent(AGENT_ID, &["repeated text"], None);
    builder.handle_event(&item_completed(TURN_A, completed_item));
    builder.handle_event(&turn_completed(TURN_A));
    builder.handle_event(&turn_started(TURN_B));

    let expected = vec![agent_item("item-1", "repeated text", None)];
    assert_eq!(
        apply_with_expected(
            &mut builder,
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "repeated text".to_string(),
                phase: None,
                memory_citation: None,
            }),
            expected.clone(),
        ),
        single_item_change(TURN_B, expected[0].clone())
    );
}

#[test]
fn interleaved_completions_consume_only_their_legacy_events() {
    let mut builder = started_builder(TURN_A);
    let first = core_agent("agent-1", &["first"], None);
    let second = core_agent("agent-2", &["second"], None);
    let expected = vec![
        agent_item("agent-1", "first", None),
        agent_item("agent-2", "second", None),
    ];
    builder.handle_event(&item_completed(TURN_A, first.clone()));
    builder.handle_event(&item_completed(TURN_A, second.clone()));
    assert_active_turn(&builder, TURN_A, expected.clone(), TurnStatus::InProgress);

    let second_legacy = second
        .as_legacy_events(/*show_raw_agent_reasoning*/ true)
        .into_iter()
        .next()
        .expect("agent completion should generate one legacy event");
    assert_eq!(
        apply_no_change_with_expected(&mut builder, second_legacy, expected.clone()),
        ThreadHistoryChangeSet::default()
    );
    assert_eq!(
        apply_no_change_with_expected(
            &mut builder,
            EventMsg::TokenCount(empty_token_count()),
            expected.clone(),
        ),
        ThreadHistoryChangeSet::default()
    );
    let first_legacy = first
        .as_legacy_events(/*show_raw_agent_reasoning*/ true)
        .into_iter()
        .next()
        .expect("agent completion should generate one legacy event");
    assert_eq!(
        apply_no_change_with_expected(&mut builder, first_legacy, expected),
        ThreadHistoryChangeSet::default()
    );
}

#[test]
fn late_completion_updates_only_its_original_turn() {
    let mut builder = started_builder(TURN_A);
    builder.handle_event(&item_started(
        TURN_A,
        core_agent(AGENT_ID, &[], Some(MessagePhase::Commentary)),
    ));
    builder.handle_event(&turn_completed(TURN_A));
    builder.handle_event(&turn_started(TURN_B));

    let completed_item = core_agent(AGENT_ID, &["final"], Some(MessagePhase::FinalAnswer));
    let completed = agent_item(AGENT_ID, "final", Some(MessagePhase::FinalAnswer));
    assert_eq!(
        builder.handle_event_with_changes(&item_completed(TURN_A, completed_item.clone())),
        single_item_change(TURN_A, completed.clone())
    );
    assert_turn(&builder, TURN_A, vec![completed], TurnStatus::Completed);
    assert_active_turn(&builder, TURN_B, Vec::new(), TurnStatus::InProgress);

    for legacy_event in completed_item.as_legacy_events(/*show_raw_agent_reasoning*/ true) {
        assert_eq!(
            builder.handle_event_with_changes(&legacy_event),
            ThreadHistoryChangeSet::default()
        );
        assert_active_turn(&builder, TURN_B, Vec::new(), TurnStatus::InProgress);
    }
}

#[test]
fn batched_live_changes_coalesce_to_authoritative_completion() {
    let mut builder = started_builder(TURN_A);
    let completed_item = core_agent(AGENT_ID, &["canonical"], None);
    let mut rollout_items = vec![
        RolloutItem::EventMsg(item_started(TURN_A, core_agent(AGENT_ID, &[], None))),
        RolloutItem::EventMsg(agent_delta(TURN_A, AGENT_ID, "partial")),
        RolloutItem::EventMsg(item_completed(TURN_A, completed_item.clone())),
    ];
    rollout_items.extend(
        completed_item
            .as_legacy_events(/*show_raw_agent_reasoning*/ true)
            .into_iter()
            .map(RolloutItem::EventMsg),
    );
    let completed = agent_item(AGENT_ID, "canonical", None);

    assert_eq!(
        builder.handle_rollout_items_with_changes(&rollout_items),
        single_item_change(TURN_A, completed.clone())
    );
    assert_active_turn(&builder, TURN_A, vec![completed], TurnStatus::InProgress);
}

#[test]
fn completed_agent_message_ignores_late_delta() {
    let mut builder = started_builder(TURN_A);
    let completed = agent_item(AGENT_ID, "final", None);
    builder.handle_event(&item_completed(
        TURN_A,
        core_agent(AGENT_ID, &["final"], None),
    ));

    assert_eq!(
        apply_no_change(&mut builder, agent_delta(TURN_A, AGENT_ID, " late")),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(&builder, TURN_A, vec![completed], TurnStatus::InProgress);
}

#[test]
fn completed_plan_ignores_late_delta() {
    let mut builder = started_builder(TURN_A);
    let completed = plan_item(PLAN_ID, "final");
    builder.handle_event(&item_completed(TURN_A, core_plan(PLAN_ID, "final")));

    assert_eq!(
        apply_no_change(&mut builder, plan_delta(TURN_A, PLAN_ID, " late")),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(&builder, TURN_A, vec![completed], TurnStatus::InProgress);
}

#[test]
fn completed_reasoning_ignores_late_deltas() {
    let mut builder = started_builder(TURN_A);
    let completed = reasoning_item(REASONING_ID, &["final summary"], &["final raw"]);
    builder.handle_event(&item_completed(
        TURN_A,
        core_reasoning(REASONING_ID, &["final summary"], &["final raw"]),
    ));

    assert_eq!(
        apply_no_change(
            &mut builder,
            reasoning_summary_delta(TURN_A, REASONING_ID, 0, " late")
        ),
        ThreadHistoryChangeSet::default()
    );
    assert_eq!(
        apply_no_change(
            &mut builder,
            reasoning_raw_delta(TURN_A, REASONING_ID, 0, " late")
        ),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(&builder, TURN_A, vec![completed], TurnStatus::InProgress);
}

#[test]
fn completed_command_ignores_late_begin_and_output() {
    let mut builder = started_builder(TURN_A);
    builder.handle_event(&command_end(TURN_A, "final"));

    apply_no_change(&mut builder, command_begin(TURN_A));
    assert_eq!(
        apply_no_change(
            &mut builder,
            command_delta(ExecOutputStream::Stdout, b" late")
        ),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(
        &builder,
        TURN_A,
        vec![completed_command("final")],
        TurnStatus::InProgress,
    );
}

#[test]
fn completed_command_ignores_duplicate_end() {
    let mut builder = started_builder(TURN_A);
    builder.handle_event(&command_end(TURN_A, "final"));

    assert_eq!(
        apply_no_change(&mut builder, command_end(TURN_A, "stale duplicate")),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(
        &builder,
        TURN_A,
        vec![completed_command("final")],
        TurnStatus::InProgress,
    );
}

#[test]
fn completed_item_ignores_late_start() {
    let mut builder = started_builder(TURN_A);
    let completed = agent_item(AGENT_ID, "final", None);
    builder.handle_event(&item_completed(
        TURN_A,
        core_agent(AGENT_ID, &["final"], None),
    ));

    assert_eq!(
        apply_no_change(
            &mut builder,
            item_started(TURN_A, core_agent(AGENT_ID, &[], None))
        ),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(&builder, TURN_A, vec![completed], TurnStatus::InProgress);
}

#[test]
fn command_output_preserves_utf8_across_every_chunk_boundary() {
    let text = "a😀éz";
    let bytes = text.as_bytes();

    for first_split in 0..=bytes.len() {
        for second_split in first_split..=bytes.len() {
            let mut builder = started_builder(TURN_A);
            builder.handle_event(&command_begin(TURN_A));
            for chunk in [
                &bytes[..first_split],
                &bytes[first_split..second_split],
                &bytes[second_split..],
            ] {
                builder.handle_event(&command_delta(ExecOutputStream::Stdout, chunk));
            }
            assert_active_turn(
                &builder,
                TURN_A,
                vec![running_command(Some(text))],
                TurnStatus::InProgress,
            );
        }
    }
}

#[test]
fn command_output_replaces_invalid_utf8_without_dropping_surrounding_bytes() {
    for (chunks, expected) in [
        (vec![b"a".to_vec(), vec![0xFF], b"b".to_vec()], "a�b"),
        (vec![vec![0xF0], vec![0x28, 0x8C], vec![0x28]], "�(�("),
    ] {
        let mut builder = started_builder(TURN_A);
        builder.handle_event(&command_begin(TURN_A));
        for chunk in chunks {
            builder.handle_event(&command_delta(ExecOutputStream::Stdout, &chunk));
        }
        assert_active_turn(
            &builder,
            TURN_A,
            vec![running_command(Some(expected))],
            TurnStatus::InProgress,
        );
    }
}

#[test]
fn completed_item_tracking_is_scoped_to_its_turn() {
    let mut builder = started_builder(TURN_A);
    builder.handle_event(&item_completed(
        TURN_A,
        core_agent(AGENT_ID, &["first"], None),
    ));
    builder.handle_event(&turn_completed(TURN_A));
    builder.handle_event(&turn_started(TURN_B));

    let second = agent_item(AGENT_ID, "second", None);
    builder.handle_event(&item_started(TURN_B, core_agent(AGENT_ID, &[], None)));
    assert_eq!(
        builder.handle_event_with_changes(&agent_delta(TURN_B, AGENT_ID, "second")),
        single_item_change(TURN_B, second.clone())
    );
    assert_turn(
        &builder,
        TURN_A,
        vec![agent_item(AGENT_ID, "first", None)],
        TurnStatus::Completed,
    );
    assert_active_turn(&builder, TURN_B, vec![second], TurnStatus::InProgress);
}

#[test]
fn late_command_output_updates_original_turn() {
    let mut builder = started_builder(TURN_A);
    builder.handle_event(&command_begin(TURN_A));
    builder.handle_event(&turn_completed(TURN_A));
    builder.handle_event(&turn_started(TURN_B));

    assert_eq!(
        builder.handle_event_with_changes(&command_delta(ExecOutputStream::Stdout, b"late")),
        single_item_change(TURN_A, running_command(Some("late")))
    );
    assert_turn(
        &builder,
        TURN_A,
        vec![running_command(Some("late"))],
        TurnStatus::Completed,
    );
    assert_active_turn(&builder, TURN_B, Vec::new(), TurnStatus::InProgress);
}

#[test]
fn reasoning_delta_rejects_sparse_index() {
    let mut builder = started_builder(TURN_A);
    let expected = vec![reasoning_item(REASONING_ID, &[], &[])];
    builder.handle_event(&item_started(
        TURN_A,
        core_reasoning(REASONING_ID, &[], &[]),
    ));

    assert_eq!(
        apply_no_change(
            &mut builder,
            reasoning_summary_delta(TURN_A, REASONING_ID, 2, "sparse")
        ),
        ThreadHistoryChangeSet::default()
    );
    assert_active_turn(&builder, TURN_A, expected, TurnStatus::InProgress);
}

fn started_builder(turn_id: &str) -> ThreadHistoryBuilder {
    let mut builder = ThreadHistoryBuilder::new();
    assert_eq!(
        builder.handle_event_with_changes(&turn_started(turn_id)),
        ThreadHistoryChangeSet {
            changed_items: Vec::new(),
            changed_turns: vec![ThreadHistoryTurnChange {
                turn_id: turn_id.to_string(),
                status: TurnStatus::InProgress,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            }],
            removed_turn_ids: Vec::new(),
        }
    );
    assert_active_turn(&builder, turn_id, Vec::new(), TurnStatus::InProgress);
    builder
}

fn apply_single_item(
    builder: &mut ThreadHistoryBuilder,
    event: EventMsg,
    expected_items: &[ThreadItem],
) {
    let expected_item = expected_items
        .first()
        .expect("event should leave one materialized item");
    assert_eq!(expected_items.len(), 1);
    let turn_id = builder.active_turn_snapshot().expect("active turn").id;
    assert_eq!(
        builder.handle_event_with_changes(&event),
        single_item_change(&turn_id, expected_item.clone())
    );
    assert_active_turn(
        builder,
        &turn_id,
        expected_items.to_vec(),
        TurnStatus::InProgress,
    );
}

fn apply_with_expected(
    builder: &mut ThreadHistoryBuilder,
    event: EventMsg,
    expected_items: Vec<ThreadItem>,
) -> ThreadHistoryChangeSet {
    let changes = builder.handle_event_with_changes(&event);
    let turn_id = builder.active_turn_snapshot().expect("active turn").id;
    assert_active_turn(builder, &turn_id, expected_items, TurnStatus::InProgress);
    changes
}

fn apply_no_change(builder: &mut ThreadHistoryBuilder, event: EventMsg) -> ThreadHistoryChangeSet {
    let expected_items = builder.active_turn_snapshot().expect("active turn").items;
    apply_no_change_with_expected(builder, event, expected_items)
}

fn apply_no_change_with_expected(
    builder: &mut ThreadHistoryBuilder,
    event: EventMsg,
    expected_items: Vec<ThreadItem>,
) -> ThreadHistoryChangeSet {
    let changes = apply_with_expected(builder, event, expected_items);
    assert_eq!(changes, ThreadHistoryChangeSet::default());
    changes
}

fn assert_active_turn(
    builder: &ThreadHistoryBuilder,
    turn_id: &str,
    items: Vec<ThreadItem>,
    status: TurnStatus,
) {
    assert_eq!(
        builder.active_turn_snapshot(),
        Some(turn(turn_id, items, status))
    );
}

fn assert_turn(
    builder: &ThreadHistoryBuilder,
    turn_id: &str,
    items: Vec<ThreadItem>,
    status: TurnStatus,
) {
    assert_eq!(
        builder.turn_snapshot(turn_id),
        Some(turn(turn_id, items, status))
    );
}

fn turn(turn_id: &str, items: Vec<ThreadItem>, status: TurnStatus) -> Turn {
    Turn {
        id: turn_id.to_string(),
        items,
        items_view: TurnItemsView::Full,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn single_item_change(turn_id: &str, item: ThreadItem) -> ThreadHistoryChangeSet {
    ThreadHistoryChangeSet {
        changed_items: vec![ThreadHistoryItemChange {
            turn_id: turn_id.to_string(),
            item,
        }],
        changed_turns: Vec::new(),
        removed_turn_ids: Vec::new(),
    }
}

fn turn_started(turn_id: &str) -> EventMsg {
    EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    })
}

fn turn_completed(turn_id: &str) -> EventMsg {
    EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: turn_id.to_string(),
        last_agent_message: None,
        completed_at: None,
        duration_ms: None,
        time_to_first_token_ms: None,
    })
}

fn item_started(turn_id: &str, item: CoreTurnItem) -> EventMsg {
    EventMsg::ItemStarted(ItemStartedEvent {
        thread_id: ThreadId::new(),
        turn_id: turn_id.to_string(),
        item,
        started_at_ms: 0,
    })
}

fn item_completed(turn_id: &str, item: CoreTurnItem) -> EventMsg {
    EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::new(),
        turn_id: turn_id.to_string(),
        item,
        completed_at_ms: 0,
    })
}

fn core_agent(id: &str, fragments: &[&str], phase: Option<MessagePhase>) -> CoreTurnItem {
    CoreTurnItem::AgentMessage(CoreAgentMessageItem {
        id: id.to_string(),
        content: fragments
            .iter()
            .map(|text| CoreAgentMessageContent::Text {
                text: (*text).to_string(),
            })
            .collect(),
        phase,
        memory_citation: None,
    })
}

fn core_plan(id: &str, text: &str) -> CoreTurnItem {
    CoreTurnItem::Plan(CorePlanItem {
        id: id.to_string(),
        text: text.to_string(),
    })
}

fn core_reasoning(id: &str, summary: &[&str], content: &[&str]) -> CoreTurnItem {
    CoreTurnItem::Reasoning(CoreReasoningItem {
        id: id.to_string(),
        summary_text: summary.iter().map(ToString::to_string).collect(),
        raw_content: content.iter().map(ToString::to_string).collect(),
    })
}

fn agent_delta(turn_id: &str, item_id: &str, delta: &str) -> EventMsg {
    EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
        thread_id: "thread-1".to_string(),
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
    })
}

fn plan_delta(turn_id: &str, item_id: &str, delta: &str) -> EventMsg {
    EventMsg::PlanDelta(PlanDeltaEvent {
        thread_id: "thread-1".to_string(),
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
    })
}

fn reasoning_summary_delta(
    turn_id: &str,
    item_id: &str,
    summary_index: i64,
    delta: &str,
) -> EventMsg {
    EventMsg::ReasoningContentDelta(ReasoningContentDeltaEvent {
        thread_id: "thread-1".to_string(),
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
        summary_index,
    })
}

fn reasoning_raw_delta(turn_id: &str, item_id: &str, content_index: i64, delta: &str) -> EventMsg {
    EventMsg::ReasoningRawContentDelta(ReasoningRawContentDeltaEvent {
        thread_id: "thread-1".to_string(),
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
        content_index,
    })
}

fn command_begin(turn_id: &str) -> EventMsg {
    EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
        call_id: COMMAND_ID.to_string(),
        process_id: Some("process-1".to_string()),
        turn_id: turn_id.to_string(),
        started_at_ms: 0,
        command: vec!["echo".to_string(), "hello".to_string()],
        cwd: test_path_buf("/tmp").abs().into(),
        parsed_cmd: vec![ParsedCommand::Unknown {
            cmd: "echo hello".to_string(),
        }],
        source: ExecCommandSource::Agent,
        interaction_input: None,
    })
}

fn command_delta(stream: ExecOutputStream, chunk: &[u8]) -> EventMsg {
    EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
        call_id: COMMAND_ID.to_string(),
        stream,
        chunk: chunk.to_vec(),
    })
}

fn command_end(turn_id: &str, aggregated_output: &str) -> EventMsg {
    EventMsg::ExecCommandEnd(ExecCommandEndEvent {
        call_id: COMMAND_ID.to_string(),
        process_id: Some("process-1".to_string()),
        turn_id: turn_id.to_string(),
        completed_at_ms: 0,
        command: vec!["echo".to_string(), "hello".to_string()],
        cwd: test_path_buf("/tmp").abs().into(),
        parsed_cmd: vec![ParsedCommand::Unknown {
            cmd: "echo hello".to_string(),
        }],
        source: ExecCommandSource::Agent,
        interaction_input: None,
        stdout: aggregated_output.to_string(),
        stderr: String::new(),
        aggregated_output: aggregated_output.to_string(),
        exit_code: 0,
        duration: Duration::from_millis(5),
        formatted_output: aggregated_output.to_string(),
        status: CoreExecCommandStatus::Completed,
    })
}

fn agent_item(id: &str, text: &str, phase: Option<MessagePhase>) -> ThreadItem {
    ThreadItem::AgentMessage {
        id: id.to_string(),
        text: text.to_string(),
        phase,
        memory_citation: None,
    }
}

fn plan_item(id: &str, text: &str) -> ThreadItem {
    ThreadItem::Plan {
        id: id.to_string(),
        text: text.to_string(),
    }
}

fn reasoning_item(id: &str, summary: &[&str], content: &[&str]) -> ThreadItem {
    ThreadItem::Reasoning {
        id: id.to_string(),
        summary: summary.iter().map(ToString::to_string).collect(),
        content: content.iter().map(ToString::to_string).collect(),
    }
}

fn running_command(aggregated_output: Option<&str>) -> ThreadItem {
    ThreadItem::CommandExecution {
        id: COMMAND_ID.to_string(),
        command: "echo hello".to_string(),
        cwd: test_path_buf("/tmp").abs().into(),
        process_id: Some("process-1".to_string()),
        source: CommandExecutionSource::Agent,
        status: CommandExecutionStatus::InProgress,
        command_actions: vec![CommandAction::Unknown {
            command: "echo hello".to_string(),
        }],
        aggregated_output: aggregated_output.map(ToString::to_string),
        exit_code: None,
        duration_ms: None,
    }
}

fn completed_command(aggregated_output: &str) -> ThreadItem {
    ThreadItem::CommandExecution {
        id: COMMAND_ID.to_string(),
        command: "echo hello".to_string(),
        cwd: test_path_buf("/tmp").abs().into(),
        process_id: Some("process-1".to_string()),
        source: CommandExecutionSource::Agent,
        status: CommandExecutionStatus::Completed,
        command_actions: vec![CommandAction::Unknown {
            command: "echo hello".to_string(),
        }],
        aggregated_output: Some(aggregated_output.to_string()),
        exit_code: Some(0),
        duration_ms: Some(5),
    }
}

fn empty_token_count() -> TokenCountEvent {
    TokenCountEvent {
        info: None,
        rate_limits: None,
    }
}
