//! Apply live exploration compatibility to replay and repair groups split by history pages.

use super::tools::CommandHistory;
use crate::exec_cell::ExecCell;
use crate::history_cell::HistoryCell;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use std::sync::Arc;

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn join_exploration_groups(
    older: &Arc<dyn HistoryCell>,
    newer: &Arc<dyn HistoryCell>,
    turns: &[Turn],
) -> Option<Arc<dyn HistoryCell>> {
    let older = older.as_any().downcast_ref::<ExecCell>()?;
    let newer = newer.as_any().downcast_ref::<ExecCell>()?;
    let items = adjacent_items(older, newer, turns)?;
    let mut group = completed_group(items)?;
    group.group.details = newer.group.details.clone();
    group
        .group
        .details
        .prepend(older.group.details.clone(), older.group.calls.len());
    Some(Arc::new(group))
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn older_exploration_group(
    older: &dyn HistoryCell,
    newer: &ExecCell,
    turns: &[Turn],
) -> Option<ExecCell> {
    let older = older.as_any().downcast_ref::<ExecCell>()?;
    let items = adjacent_items(older, newer, turns)?;
    let older_items = items
        .iter()
        .filter(|item| !matches!(item, ThreadItem::Reasoning { .. }))
        .take(older.group.calls.len())
        .cloned()
        .collect::<Vec<_>>();
    let mut group = completed_group(&older_items)?;
    group.group.details = older.group.details.clone();
    Some(group)
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
fn adjacent_items<'a>(
    older: &ExecCell,
    newer: &ExecCell,
    turns: &'a [Turn],
) -> Option<&'a [ThreadItem]> {
    if !older.is_exploring_cell() || older.should_flush() || !newer.is_exploring_cell() {
        return None;
    }
    let ids = older
        .iter_calls()
        .chain(newer.iter_calls())
        .map(|call| call.call_id.as_str())
        .collect::<Vec<_>>();
    super::computer_groups::adjacent_activity_items(&ids, turns)
}

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(super) fn completed_group(items: &[ThreadItem]) -> Option<ExecCell> {
    let mut group: Option<ExecCell> = None;
    for item in items {
        if matches!(item, ThreadItem::Reasoning { .. }) {
            continue;
        }
        let call = CommandHistory::from_item(item.clone())?.into_cell();
        if !call.is_exploring_cell() {
            return None;
        }
        if let Some(group) = &mut group {
            group.append_completed(call).ok()?;
        } else {
            group = Some(call);
        }
    }
    group
}
