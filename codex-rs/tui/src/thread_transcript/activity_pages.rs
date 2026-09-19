//! Attach transparent page-boundary details to a freshly rebuilt activity group.

use crate::exec_cell::ExecCell;
use crate::history_cell::ComputerActivityCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::ReasoningSummaryCell;
use std::sync::Arc;

#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn is_hidden_activity_detail(cell: &Arc<dyn HistoryCell>) -> bool {
    cell.as_any()
        .downcast_ref::<ReasoningSummaryCell>()
        .is_some_and(ReasoningSummaryCell::is_transcript_only)
}

/// Fold standalone reasoning using exact persisted identities from one turn.
///
/// An older page can arrive after the final reasoning page even with no newer group or live tail.
/// Rebuild only known completed calls and share their immutable details; never infer adjacency
/// from identical reasoning text or attach a detail from the next turn.
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn fold_trailing_activity_details(
    older: &Arc<dyn HistoryCell>,
    details: &[Arc<dyn HistoryCell>],
    turns: &[codex_app_server_protocol::Turn],
) -> Option<Arc<dyn HistoryCell>> {
    let (mut ids, retained) =
        if let Some(group) = older.as_any().downcast_ref::<ComputerActivityCell>() {
            (group.call_ids().collect::<Vec<_>>(), &group.group.details)
        } else {
            let group = older.as_any().downcast_ref::<ExecCell>()?;
            if !group.is_exploring_cell() || group.should_flush() {
                return None;
            }
            (
                group
                    .iter_calls()
                    .map(|call| call.call_id.as_str())
                    .collect::<Vec<_>>(),
                &group.group.details,
            )
        };
    let count = ids.len();
    for detail in details {
        let detail = detail.as_any().downcast_ref::<ReasoningSummaryCell>()?;
        if !detail.is_transcript_only() {
            return None;
        }
        ids.push(detail.source_item_id()?);
    }
    let items = super::computer_groups::adjacent_activity_items(&ids, turns)?;
    let mut retained = retained.clone();
    for detail in details {
        retained.push(count, Arc::clone(detail));
    }
    if older.as_any().is::<ComputerActivityCell>() {
        let mut group = super::computer_groups::completed_group(items)?;
        group.group.details = retained;
        Some(Arc::new(group))
    } else {
        let mut group = super::exploration_groups::completed_group(items)?;
        group.group.details = retained;
        Some(Arc::new(group))
    }
}
