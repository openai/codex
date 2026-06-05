use super::*;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn ignores_item_only_events_between_turn_summary_updates() {
    let mut observer = TurnSummaryProjectionObserver::new();
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
    ];
    let mutations = observer.observe_append(&persisted_rollout_items, &[]);

    assert_eq!(
        turn_summary_payloads(mutations),
        vec![
            json!({
                "turnId": "turn-a",
                "mutation": {
                    "status": "inProgress",
                    "startedAt": 10,
                    "completedAt": null,
                    "durationMs": null,
                },
            }),
            json!({
                "turnId": "turn-a",
                "mutation": {
                    "status": "completed",
                    "startedAt": 10,
                    "completedAt": 11,
                    "durationMs": 1_000,
                },
            }),
        ]
    );
}

fn turn_summary_payloads(mutations: Vec<ThreadHistoryMutation>) -> Vec<serde_json::Value> {
    mutations
        .into_iter()
        .map(|mutation| match mutation {
            ThreadHistoryMutation::TurnSummary(mutation) => mutation.payload,
            ThreadHistoryMutation::ThreadItem(_) | ThreadHistoryMutation::Lifecycle(_) => {
                panic!("turn summary observer emitted non-turn-summary mutation")
            }
        })
        .collect()
}
