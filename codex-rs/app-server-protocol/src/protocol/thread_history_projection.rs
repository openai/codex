//! Stateless projection from canonical paginated rollout records to thread-history changes.
//!
//! This module is only for the new paginated rollout format that persists canonical
//! `ItemCompleted(TurnItem)` records, not legacy event-only rollouts.

use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;

use crate::protocol::thread_history::ThreadHistoryChangeSet;
use crate::protocol::thread_history::ThreadHistoryItemChange;
use crate::protocol::thread_history::ThreadHistoryTurnChange;
use crate::protocol::v2::ThreadItem;
use crate::protocol::v2::TurnError;
use crate::protocol::v2::TurnStatus;

/// Project one durable rollout line without reconstructing earlier history.
///
/// Callers that replay a JSONL suffix should invoke it once per line, in ordinal order, so storage
/// can preserve the first and latest timestamps for repeated item snapshots independently.
pub fn project_rollout_line(line: &RolloutLine) -> ThreadHistoryChangeSet {
    match &line.item {
        RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => ThreadHistoryChangeSet {
            changed_turns: vec![ThreadHistoryTurnChange {
                turn_id: event.turn_id.clone(),
                status: TurnStatus::InProgress,
                error: None,
                started_at: event.started_at,
                completed_at: None,
                duration_ms: None,
            }],
            ..Default::default()
        },
        RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => ThreadHistoryChangeSet {
            changed_turns: vec![ThreadHistoryTurnChange {
                turn_id: event.turn_id.clone(),
                status: if event.error.is_some() {
                    TurnStatus::Failed
                } else {
                    TurnStatus::Completed
                },
                error: event.error.as_ref().map(|error| TurnError {
                    message: error.message.clone(),
                    codex_error_info: error.codex_error_info.clone().map(Into::into),
                    additional_details: None,
                }),
                started_at: event.started_at,
                completed_at: event.completed_at,
                duration_ms: event.duration_ms,
            }],
            ..Default::default()
        },
        RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
            let Some(turn_id) = event.turn_id.as_ref() else {
                return ThreadHistoryChangeSet::default();
            };
            ThreadHistoryChangeSet {
                changed_turns: vec![ThreadHistoryTurnChange {
                    turn_id: turn_id.clone(),
                    status: TurnStatus::Interrupted,
                    error: None,
                    started_at: event.started_at,
                    completed_at: event.completed_at,
                    duration_ms: event.duration_ms,
                }],
                ..Default::default()
            }
        }
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) => ThreadHistoryChangeSet {
            changed_items: vec![ThreadHistoryItemChange {
                turn_id: event.turn_id.clone(),
                item: ThreadItem::from(event.item.clone()),
            }],
            ..Default::default()
        },
        RolloutItem::SessionMeta(_)
        | RolloutItem::ResponseItem(_)
        | RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. }
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::WorldState(_)
        | RolloutItem::EventMsg(_) => ThreadHistoryChangeSet::default(),
    }
}

#[cfg(test)]
#[path = "thread_history_projection_tests.rs"]
mod tests;
