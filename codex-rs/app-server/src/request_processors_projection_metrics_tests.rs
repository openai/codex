use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::AgentReasoningRawContentEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;

use super::build_api_turns_from_rollout_items;
use super::is_completed_user_assistant_turn;

#[test]
fn cold_projection_coalesces_repeated_rollout_updates() {
    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "question".to_string(),
            ..Default::default()
        })),
        RolloutItem::EventMsg(EventMsg::AgentReasoningRawContent(
            AgentReasoningRawContentEvent {
                text: "first".to_string(),
            },
        )),
        RolloutItem::EventMsg(EventMsg::AgentReasoningRawContent(
            AgentReasoningRawContentEvent {
                text: "second".to_string(),
            },
        )),
        RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
            message: "answer".to_string(),
            phase: None,
            memory_citation: None,
        })),
        RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("answer".to_string()),
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        })),
    ];
    let turns = build_api_turns_from_rollout_items(&rollout_items);

    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].items.len(), 3);
    assert!(is_completed_user_assistant_turn(&turns[0]));
}
