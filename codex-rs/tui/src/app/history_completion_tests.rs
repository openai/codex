//! Pagination boundaries preserve grouped item rendering and attach completion metadata once.

use super::*;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnItemsView;
use pretty_assertions::assert_eq;

fn payloads(items: &[codex_app_server_protocol::ThreadItem]) -> Vec<ThreadItemPayload> {
    items.iter().map(|entry| entry.payload.clone()).collect()
}

fn turn(id: &str, status: TurnStatus, item_ids: &[&str]) -> Turn {
    Turn {
        id: id.to_string(),
        root_turn_id: None,
        items: item_ids
            .iter()
            .map(|id| ThreadItemPayload::UserMessage {
                id: id.to_string(),
                client_id: None,
                content: Vec::new(),
            })
            .map(ThreadItem::untimed)
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
        group_completed_turn_items(payloads(&turns[0].items[1..]), &turns),
        vec![(payloads(&turns[0].items[1..]), Some(&turns[0]))],
    );
    assert_eq!(
        group_completed_turn_items(payloads(&turns[0].items[..1]), &turns),
        vec![(payloads(&turns[0].items[..1]), None)],
    );
}

#[test]
fn multiple_turns_keep_item_groups_and_completion_order() {
    let turns = vec![
        turn("first", TurnStatus::Completed, &["a", "b"]),
        turn("second", TurnStatus::Completed, &["c", "d"]),
    ];
    let items = turns
        .iter()
        .flat_map(Turn::item_payloads)
        .cloned()
        .collect();
    assert_eq!(
        group_completed_turn_items(items, &turns),
        vec![
            (payloads(&turns[0].items), Some(&turns[0])),
            (payloads(&turns[1].items), Some(&turns[1])),
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
        .flat_map(Turn::item_payloads)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        group_completed_turn_items(items, &turns),
        turns
            .iter()
            .map(|turn| (payloads(&turn.items), None))
            .collect::<Vec<_>>(),
    );
}
