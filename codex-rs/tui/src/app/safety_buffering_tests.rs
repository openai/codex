use super::*;
use codex_app_server_protocol::TurnItemsView;

fn turn(id: &str, status: TurnStatus) -> Turn {
    Turn {
        id: id.to_string(),
        is_forkable: true,
        items: Vec::new(),
        items_view: TurnItemsView::Full,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

#[test]
fn retry_forks_through_the_previous_terminal_turn() {
    let turns = vec![
        turn("turn-1", TurnStatus::Completed),
        turn("turn-2", TurnStatus::Interrupted),
    ];

    assert_eq!(
        safety_retry_fork_point(&turns, "turn-2").expect("fork point"),
        Some("turn-1".to_string())
    );
}

#[test]
fn retry_starts_fresh_when_the_interrupted_turn_is_first() {
    let turns = vec![turn("turn-1", TurnStatus::Interrupted)];

    assert_eq!(
        safety_retry_fork_point(&turns, "turn-1").expect("fork point"),
        None
    );
}

#[test]
fn retry_rejects_a_legacy_synthetic_predecessor() {
    let turns = vec![
        Turn {
            is_forkable: false,
            ..turn("legacy-turn", TurnStatus::Completed)
        },
        turn("turn-2", TurnStatus::Interrupted),
    ];

    let err = safety_retry_fork_point(&turns, "turn-2").expect_err("synthetic predecessor");

    assert_eq!(
        err.to_string(),
        "previous turn legacy-turn has no canonical fork boundary"
    );
}

#[test]
fn retry_rejects_a_stale_non_latest_turn() {
    let turns = vec![
        turn("turn-1", TurnStatus::Interrupted),
        turn("turn-2", TurnStatus::InProgress),
    ];

    assert!(safety_retry_fork_point(&turns, "turn-1").is_err());
}
