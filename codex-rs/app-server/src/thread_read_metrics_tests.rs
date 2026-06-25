use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;

use super::ThreadReadCounts;
use super::count_completed_turns;

fn turn(status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
    Turn {
        id: "turn".to_string(),
        items,
        items_view: TurnItemsView::Full,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

#[test]
fn counts_items_in_completed_user_assistant_turns() {
    let user_message = || ThreadItem::UserMessage {
        id: "user".to_string(),
        client_id: None,
        content: Vec::new(),
    };
    let agent_message = |phase| ThreadItem::AgentMessage {
        id: "agent".to_string(),
        text: "answer".to_string(),
        phase,
        memory_citation: None,
    };
    let turns = vec![
        turn(
            TurnStatus::Completed,
            vec![user_message(), agent_message(None)],
        ),
        turn(
            TurnStatus::Completed,
            vec![
                user_message(),
                agent_message(Some(MessagePhase::Commentary)),
            ],
        ),
        turn(
            TurnStatus::InProgress,
            vec![user_message(), agent_message(None)],
        ),
    ];

    assert_eq!(
        count_completed_turns(&turns),
        ThreadReadCounts {
            completed_turns: 1,
            completed_turn_items: 2,
        }
    );
}
