use super::*;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::time::Duration;

#[test]
fn reuses_thread_item_ordinal_for_open_item_upserts() {
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
    let checkpoint = observer.checkpoint().clone();
    let mut resumed_observer = ThreadItemProjectionObserver::from_checkpoint(checkpoint);
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

fn only_thread_item_payload(mutations: Vec<ThreadHistoryMutation>) -> Value {
    let mut payloads = mutations.into_iter().filter_map(|mutation| match mutation {
        ThreadHistoryMutation::ThreadItem(mutation) => Some(mutation.payload),
        ThreadHistoryMutation::TurnSummary(_) | ThreadHistoryMutation::Lifecycle(_) => None,
    });
    let payload = payloads.next().expect("thread item mutation");
    assert!(payloads.next().is_none());
    payload
}
