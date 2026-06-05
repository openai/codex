use super::*;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn emits_lifecycle_mutations_from_lifecycle_events_only() {
    let mut observer = LifecycleProjectionObserver::new();
    let persisted_rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-a".into(),
            trace_id: None,
            started_at: Some(10),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: "hello".into(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
            ..Default::default()
        })),
        RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-a".into(),
            last_agent_message: None,
            completed_at: Some(11),
            duration_ms: Some(1_000),
            time_to_first_token_ms: None,
        })),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
            num_turns: 1,
        })),
    ];
    let mutations = observer.observe_append(&persisted_rollout_items, &[]);

    assert_eq!(
        lifecycle_payloads(mutations),
        vec![
            json!({
                "eventType": "turn.started",
                "turnId": "turn-a",
                "payload": {
                    "startedAt": 10,
                },
            }),
            json!({
                "eventType": "turn.completed",
                "turnId": "turn-a",
                "payload": {
                    "completedAt": 11,
                    "durationMs": 1_000,
                },
            }),
            json!({
                "eventType": "thread.rolled_back",
                "turnId": null,
                "payload": {
                    "numTurns": 1,
                },
            }),
        ]
    );
}

#[test]
fn emits_turn_failed_for_turn_affecting_errors() {
    let mut observer = LifecycleProjectionObserver::new();
    let persisted_rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-a".into(),
            trace_id: None,
            started_at: Some(10),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
        RolloutItem::EventMsg(EventMsg::Error(ErrorEvent {
            message: "boom".into(),
            codex_error_info: None,
        })),
    ];
    let mutations = observer.observe_append(&persisted_rollout_items, &[]);

    assert_eq!(
        lifecycle_payloads(mutations),
        vec![
            json!({
                "eventType": "turn.started",
                "turnId": "turn-a",
                "payload": {
                    "startedAt": 10,
                },
            }),
            json!({
                "eventType": "turn.failed",
                "turnId": "turn-a",
                "payload": {
                    "message": "boom",
                    "codexErrorInfo": null,
                },
            }),
        ]
    );
}

fn lifecycle_payloads(mutations: Vec<ThreadHistoryMutation>) -> Vec<serde_json::Value> {
    mutations
        .into_iter()
        .map(|mutation| match mutation {
            ThreadHistoryMutation::Lifecycle(mutation) => mutation.payload,
            ThreadHistoryMutation::ThreadItem(_) | ThreadHistoryMutation::TurnSummary(_) => {
                panic!("lifecycle observer emitted non-lifecycle mutation")
            }
        })
        .collect()
}
