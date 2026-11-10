//! Split older items at turn boundaries so each terminal row is rendered on the page that owns it.

use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use std::collections::HashMap;

pub(super) fn group_turn_items<'a>(
    items: &[ThreadItemPayload],
    turns: &'a [Turn],
) -> Vec<(std::ops::Range<usize>, Option<&'a Turn>)> {
    let offsets: HashMap<_, _> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id(), i + 1))
        .collect();
    let first = turns.iter().find_map(|turn| turn.items.first());
    // Itemless interruptions follow the preceding turn, or precede the first item in history.
    let mut end = first
        .filter(|item| offsets.get(item.id()) == Some(&1))
        .map(|_| 0);
    let mut start = 0;
    let mut groups = Vec::new();
    for turn in turns {
        if let Some(last) = turn.items.last() {
            end = offsets.get(last.id()).copied();
        } else if turn.status != TurnStatus::Interrupted
            || turn.items_view == TurnItemsView::NotLoaded
        {
            continue;
        }
        if let Some(end) = end {
            groups.push((start..end, Some(turn)));
            start = end;
        }
    }
    if start < items.len() {
        groups.push((start..items.len(), None));
    }
    groups
}

#[cfg(test)]
#[path = "history_completion_tests.rs"]
mod tests;
