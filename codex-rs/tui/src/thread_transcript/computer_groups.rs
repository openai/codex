//! Assemble computer activity across hidden reasoning and repair history-page group boundaries.

use super::tools::McpHistory;
use crate::history_cell::ComputerActivityCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::McpToolCallCell;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use std::sync::Arc;

pub(super) fn append(group: &mut ComputerActivityCell, call: McpHistory) {
    let cell = McpToolCallCell::new(call.id, call.invocation, /*animations_enabled*/ false);
    group.complete(cell, call.duration, call.result);
}

/// Rebuild only an adjacent computer group, using its persisted IDs to verify the turn boundary.
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn join_computer_groups(
    older: &Arc<dyn HistoryCell>,
    newer: &Arc<dyn HistoryCell>,
    turns: &[Turn],
) -> Option<Arc<dyn HistoryCell>> {
    let older = older.as_any().downcast_ref::<ComputerActivityCell>()?;
    let newer = newer.as_any().downcast_ref::<ComputerActivityCell>()?;
    let mut group = completed_group(adjacent_items(older, newer, turns)?)?;
    group.group.details = newer.group.details.clone();
    group
        .group
        .details
        .prepend(older.group.details.clone(), older.call_ids().count());
    Some(Arc::new(group))
}

/// Restore only the older half; the caller retains the live half and its pending calls/timers.
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn older_computer_group(
    older: &dyn HistoryCell,
    newer: &ComputerActivityCell,
    turns: &[Turn],
) -> Option<ComputerActivityCell> {
    let older = older.as_any().downcast_ref::<ComputerActivityCell>()?;
    let items = adjacent_items(older, newer, turns)?;
    let older_items = items
        .iter()
        .filter(|item| !matches!(item, ThreadItem::Reasoning { .. }))
        .take(older.call_ids().count())
        .cloned()
        .collect::<Vec<_>>();
    let mut group = completed_group(&older_items)?;
    group.group.details = older.group.details.clone();
    Some(group)
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
fn adjacent_items<'a>(
    older: &ComputerActivityCell,
    newer: &ComputerActivityCell,
    turns: &'a [Turn],
) -> Option<&'a [ThreadItem]> {
    let ids = older.call_ids().chain(newer.call_ids()).collect::<Vec<_>>();
    adjacent_activity_items(&ids, turns)
}

/// Validate source adjacency without mistaking invisible reasoning for an activity boundary.
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(super) fn adjacent_activity_items<'a>(
    ids: &[&str],
    turns: &'a [Turn],
) -> Option<&'a [ThreadItem]> {
    let first = *ids.first()?;
    let (turn, start) = turns.iter().find_map(|turn| {
        turn.items
            .iter()
            .position(|item| item.id() == first)
            .map(|start| (turn, start))
    })?;
    let mut matched = 0;
    for (offset, item) in turn.items[start..].iter().enumerate() {
        if item.id() != ids[matched] {
            if matches!(item, ThreadItem::Reasoning { .. }) {
                continue;
            }
            return None;
        }
        matched += 1;
        if matched == ids.len() {
            return Some(&turn.items[start..=start + offset]);
        }
    }
    None
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(super) fn completed_group(items: &[ThreadItem]) -> Option<ComputerActivityCell> {
    let mut group = None;
    for item in items {
        if matches!(item, ThreadItem::Reasoning { .. }) {
            continue;
        }
        let call = McpHistory::from_item(item.clone())?;
        if !call.invocation.is_computer_activity() {
            return None;
        }
        append(group.get_or_insert_default(), call);
    }
    group
}

#[cfg(test)]
#[path = "computer_groups_tests.rs"]
mod tests;
