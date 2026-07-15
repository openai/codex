use super::*;
use codex_app_server_protocol::TurnItemsView;

fn turn(id: &str, status: TurnStatus) -> Turn {
    Turn {
        id: id.to_string(),
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
fn retry_rejects_a_stale_or_in_progress_turn() {
    let stale = vec![
        turn("turn-1", TurnStatus::Interrupted),
        turn("turn-2", TurnStatus::InProgress),
    ];
    let in_progress = vec![turn("turn-1", TurnStatus::InProgress)];
    let previous_in_progress = vec![
        turn("turn-1", TurnStatus::InProgress),
        turn("turn-2", TurnStatus::Interrupted),
    ];

    assert!(safety_retry_fork_point(&stale, "turn-1").is_err());
    assert!(safety_retry_fork_point(&in_progress, "turn-1").is_err());
    assert!(safety_retry_fork_point(&in_progress, "missing").is_err());
    assert!(safety_retry_fork_point(&previous_in_progress, "turn-2").is_err());
}
