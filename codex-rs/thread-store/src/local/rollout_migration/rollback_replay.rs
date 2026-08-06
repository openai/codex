//! Mirrors the part of legacy cold resume that chooses the model-history checkpoint.
//!
//! Legacy rollback has two jobs. The forward history reducer decides which turns remain visible,
//! while cold resume scans backward to find the newest surviving compaction base. Migration needs
//! both answers because removing rollback markers must not change what the next resume sends to
//! the model.

use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;

use super::rollback;

enum ReplayRecord {
    Compacted {
        record_index: usize,
        has_replacement_history: bool,
    },
    Rollback(u32),
    TurnStarted(String),
    TurnComplete(String),
    TurnAborted(Option<String>),
    TurnContext(Option<String>),
    UserBoundary,
}

#[derive(Default)]
struct ReverseSegment {
    turn_id: Option<String>,
    counts_as_user_turn: bool,
    compaction_index: Option<usize>,
}

/// Rewrite needed when legacy reverse replay skips a compaction segment but still resumes from
/// the suffix after that checkpoint.
pub(super) struct ModelReplayPlan {
    pub(super) empty_replacement_history_compaction: Option<usize>,
}

pub(super) struct ModelReplayPlanner {
    records: Vec<ReplayRecord>,
}

impl ModelReplayPlanner {
    pub(super) fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, record_index: usize, item: &RolloutItem) {
        let record = match item {
            RolloutItem::Compacted(compacted) => ReplayRecord::Compacted {
                record_index,
                has_replacement_history: compacted.replacement_history.is_some(),
            },
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                ReplayRecord::Rollback(rollback.num_turns)
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                ReplayRecord::TurnStarted(event.turn_id.clone())
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                ReplayRecord::TurnComplete(event.turn_id.clone())
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                ReplayRecord::TurnAborted(event.turn_id.clone())
            }
            RolloutItem::TurnContext(context) => ReplayRecord::TurnContext(context.turn_id.clone()),
            RolloutItem::EventMsg(EventMsg::UserMessage(_))
            | RolloutItem::InterAgentCommunication(_) => ReplayRecord::UserBoundary,
            RolloutItem::ResponseItem(response) if rollback::counts_as_boundary(response) => {
                ReplayRecord::UserBoundary
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::EventMsg(_)
            | RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::WorldState(_) => return,
        };
        self.records.push(record);
    }

    pub(super) fn finish(self) -> ModelReplayPlan {
        let mut selected_compaction = None;
        let mut suffix_start = None;
        let mut pending_rollback_turns = 0usize;
        let mut active_segment: Option<ReverseSegment> = None;

        for record in self.records.iter().rev() {
            match record {
                ReplayRecord::Compacted {
                    record_index,
                    has_replacement_history,
                } => {
                    let segment = active_segment.get_or_insert_with(ReverseSegment::default);
                    if segment.compaction_index.is_none() && *has_replacement_history {
                        segment.compaction_index = Some(*record_index);
                        suffix_start = record_index.checked_add(1);
                    }
                }
                ReplayRecord::Rollback(num_turns) => {
                    pending_rollback_turns = pending_rollback_turns
                        .saturating_add(usize::try_from(*num_turns).unwrap_or(usize::MAX));
                }
                ReplayRecord::TurnComplete(turn_id) => {
                    let segment = active_segment.get_or_insert_with(ReverseSegment::default);
                    if segment.turn_id.is_none() {
                        segment.turn_id = Some(turn_id.clone());
                    }
                }
                ReplayRecord::TurnAborted(turn_id) => {
                    if let Some(segment) = active_segment.as_mut() {
                        if segment.turn_id.is_none() {
                            segment.turn_id = turn_id.clone();
                        }
                    } else if turn_id.is_some() {
                        active_segment = Some(ReverseSegment {
                            turn_id: turn_id.clone(),
                            ..Default::default()
                        });
                    }
                }
                ReplayRecord::TurnContext(turn_id) => {
                    let segment = active_segment.get_or_insert_with(ReverseSegment::default);
                    if segment.turn_id.is_none() {
                        segment.turn_id = turn_id.clone();
                    }
                }
                ReplayRecord::UserBoundary => {
                    active_segment
                        .get_or_insert_with(ReverseSegment::default)
                        .counts_as_user_turn = true;
                }
                ReplayRecord::TurnStarted(turn_id) => {
                    if active_segment.as_ref().is_some_and(|segment| {
                        turn_ids_are_compatible(segment.turn_id.as_deref(), Some(turn_id.as_str()))
                    }) && let Some(segment) = active_segment.take()
                    {
                        finalize_segment(
                            segment,
                            &mut selected_compaction,
                            &mut pending_rollback_turns,
                        );
                    }
                }
            }
        }

        if let Some(segment) = active_segment {
            finalize_segment(
                segment,
                &mut selected_compaction,
                &mut pending_rollback_turns,
            );
        }

        // If reverse replay saw a replacement-history checkpoint but rolled back its segment,
        // it resumes from that checkpoint's suffix without using the checkpoint history. Keep an
        // empty checkpoint in the paginated file so older SQLite-visible records do not leak back
        // into model context after the rollback markers are gone.
        let empty_replacement_history_compaction = selected_compaction
            .is_none()
            .then_some(suffix_start)
            .flatten()
            .and_then(|start| start.checked_sub(1));
        ModelReplayPlan {
            empty_replacement_history_compaction,
        }
    }
}

fn finalize_segment(
    segment: ReverseSegment,
    selected_compaction: &mut Option<usize>,
    pending_rollback_turns: &mut usize,
) {
    if *pending_rollback_turns > 0 {
        if segment.counts_as_user_turn {
            *pending_rollback_turns -= 1;
        }
        return;
    }
    if selected_compaction.is_none() {
        *selected_compaction = segment.compaction_index;
    }
}

fn turn_ids_are_compatible(active_turn_id: Option<&str>, item_turn_id: Option<&str>) -> bool {
    active_turn_id
        .is_none_or(|turn_id| item_turn_id.is_none_or(|item_turn_id| item_turn_id == turn_id))
}
