//! Pagination boundaries preserve grouped item rendering and attach completion metadata once.

use super::*;
use codex_app_server_protocol::TurnItemsView;
use pretty_assertions::assert_eq;

fn turn(id: &str, status: TurnStatus, item_ids: &[&str]) -> Turn {
    Turn {
        id: id.to_string(),
        items: item_ids
            .iter()
            .map(|id| ThreadItem::UserMessage {
                id: id.to_string(),
                client_id: None,
                content: Vec::new(),
            })
            .collect(),
        items_view: TurnItemsView::Summary,
        status,
        error: None,
        started_at: None,
        completed_at: Some(1_700_000_000),
        duration_ms: Some(125_000),
    }
}

#[test]
fn split_turn_only_completes_on_the_page_with_its_last_item() {
    let turns = vec![turn(
        "turn",
        TurnStatus::Completed,
        &["first", "second", "last"],
    )];
    assert_eq!(
        group_completed_turn_items(turns[0].items[1..].to_vec(), &turns),
        vec![(turns[0].items[1..].to_vec(), Some(&turns[0]))],
    );
    assert_eq!(
        group_completed_turn_items(turns[0].items[..1].to_vec(), &turns),
        vec![(turns[0].items[..1].to_vec(), None)],
    );
}

#[test]
fn multiple_turns_keep_item_groups_and_completion_order() {
    let turns = vec![
        turn("first", TurnStatus::Completed, &["a", "b"]),
        turn("second", TurnStatus::Completed, &["c", "d"]),
    ];
    let items = turns.iter().flat_map(|turn| turn.items.clone()).collect();
    assert_eq!(
        group_completed_turn_items(items, &turns),
        vec![
            (turns[0].items.clone(), Some(&turns[0])),
            (turns[1].items.clone(), Some(&turns[1])),
        ],
    );
}

#[test]
fn unsuccessful_and_running_turns_separate_groups_without_completion_metadata() {
    let turns = vec![
        turn("failed", TurnStatus::Failed, &["a"]),
        turn("interrupted", TurnStatus::Interrupted, &["b"]),
        turn("running", TurnStatus::InProgress, &["c"]),
    ];
    let items = turns
        .iter()
        .flat_map(|turn| turn.items.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        group_completed_turn_items(items, &turns),
        turns
            .iter()
            .map(|turn| (turn.items.clone(), None))
            .collect::<Vec<_>>(),
    );
}
