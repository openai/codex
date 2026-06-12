//! Helpers for truncating rollouts based on "user turn" boundaries.
//!
//! In core, "user turns" are detected by scanning `ResponseItem::Message` items and
//! interpreting them via `event_mapping::parse_turn_item(...)`.

use crate::context_manager::is_user_turn_boundary;
use crate::event_mapping;
use crate::rollout::RolloutRecorder;
use crate::rollout::resolve_rollout_reference_rollout_path;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::RolloutItem;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

pub(crate) fn initial_history_has_prior_user_turns(conversation_history: &InitialHistory) -> bool {
    conversation_history.scan_rollout_items(rollout_item_is_user_turn_boundary)
}

fn rollout_item_is_user_turn_boundary(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(item) => is_user_turn_boundary(item),
        RolloutItem::InterAgentCommunication(_) => true,
        _ => false,
    }
}

/// Return the indices of user message boundaries in a rollout.
///
/// A user message boundary is a `RolloutItem::ResponseItem(ResponseItem::Message { .. })`
/// whose parsed turn item is `TurnItem::UserMessage`.
///
/// Rollouts can contain `ThreadRolledBack` markers. Those markers indicate that the
/// last N user turns were removed from the effective thread history; we apply them here so
/// indexing uses the post-rollback history rather than the raw stream.
pub(crate) fn user_message_positions_in_rollout(items: &[RolloutItem]) -> Vec<usize> {
    let mut user_positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            RolloutItem::ResponseItem(item @ ResponseItem::Message { .. })
                if matches!(
                    event_mapping::parse_turn_item(item),
                    Some(TurnItem::UserMessage(_))
                ) =>
            {
                user_positions.push(idx);
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                let num_turns = usize::try_from(rollback.num_turns).unwrap_or(usize::MAX);
                let new_len = user_positions.len().saturating_sub(num_turns);
                user_positions.truncate(new_len);
            }
            _ => {}
        }
    }
    user_positions
}

/// Return the indices of fork-turn boundaries in a rollout.
///
/// A fork-turn boundary is either:
/// - a real user message boundary, or
/// - an inter-agent communication whose `trigger_turn` is `true`, or
/// - a legacy assistant inter-agent envelope with the same flag.
///
/// Like `user_message_positions_in_rollout`, this applies `ThreadRolledBack` markers so indexing
/// reflects the effective post-rollback history. Rollback counts instruction turns, so a rollback
/// removes the stale suffix starting at the earliest rolled-back instruction-turn boundary instead
/// of simply truncating the mixed fork-boundary list.
pub(crate) fn fork_turn_positions_in_rollout(items: &[RolloutItem]) -> Vec<usize> {
    let mut rollback_turn_positions = Vec::new();
    let mut fork_turn_positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            RolloutItem::ResponseItem(item) => {
                if is_user_turn_boundary(item) {
                    rollback_turn_positions.push(idx);
                }
                if is_real_user_message_boundary(item) || is_trigger_turn_boundary(item) {
                    fork_turn_positions.push(idx);
                }
            }
            RolloutItem::InterAgentCommunication(communication) => {
                rollback_turn_positions.push(idx);
                if communication.trigger_turn {
                    fork_turn_positions.push(idx);
                }
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                let num_turns = usize::try_from(rollback.num_turns).unwrap_or(usize::MAX);
                if num_turns == 0 {
                    continue;
                }
                let Some(rollback_start_idx) = rollback_turn_positions
                    .len()
                    .checked_sub(num_turns)
                    .map(|rollback_start| rollback_turn_positions[rollback_start])
                    .or_else(|| rollback_turn_positions.first().copied())
                else {
                    continue;
                };
                let new_rollback_len = rollback_turn_positions.len().saturating_sub(num_turns);
                rollback_turn_positions.truncate(new_rollback_len);
                fork_turn_positions.retain(|position| *position < rollback_start_idx);
            }
            _ => {}
        }
    }
    fork_turn_positions
}

/// Return a prefix of `items` obtained by cutting strictly before the nth user message.
///
/// The boundary index is 0-based from the start of `items` (so `n_from_start = 0` returns
/// a prefix that excludes the first user message and everything after it).
///
/// If `n_from_start` is `usize::MAX`, this returns the full rollout (no truncation).
/// If fewer than or equal to `n_from_start` user messages exist, this returns the full
/// rollout unchanged.
pub(crate) fn truncate_rollout_before_nth_user_message_from_start(
    items: &[RolloutItem],
    n_from_start: usize,
) -> Vec<RolloutItem> {
    if n_from_start == usize::MAX {
        return items.to_vec();
    }

    let user_positions = user_message_positions_in_rollout(items);

    // If fewer than or equal to n user messages exist, keep the full rollout.
    if user_positions.len() <= n_from_start {
        return items.to_vec();
    }

    // Cut strictly before the nth user message (do not keep the nth itself).
    let cut_idx = user_positions[n_from_start];
    items[..cut_idx].to_vec()
}

/// Return a suffix of `items` that keeps the last `n_from_end` fork turns.
///
/// If fewer than or equal to `n_from_end` fork turns exist, this keeps from the first fork-turn
/// boundary and still drops pre-turn startup context.
pub(crate) fn truncate_rollout_to_last_n_fork_turns(
    items: &[RolloutItem],
    n_from_end: usize,
) -> Vec<RolloutItem> {
    if n_from_end == 0 {
        return Vec::new();
    }

    let fork_turn_positions = fork_turn_positions_in_rollout(items);
    let Some(keep_idx) = fork_turn_positions
        .len()
        .checked_sub(n_from_end)
        .map(|position| fork_turn_positions[position])
        .or_else(|| fork_turn_positions.first().copied())
    else {
        return Vec::new();
    };
    items[keep_idx..].to_vec()
}

#[derive(Clone, Copy)]
enum RolloutMaterialization {
    ModelReplay,
    CompleteHistory,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum RolloutReferenceIdentity {
    Segment(codex_protocol::SegmentId),
    Path(PathBuf),
}

pub async fn materialize_rollout_items_for_model_replay(
    codex_home: &Path,
    rollout_items: &[RolloutItem],
) -> Vec<RolloutItem> {
    match materialize_rollout_items(
        codex_home,
        rollout_items,
        RolloutMaterialization::ModelReplay,
    )
    .await
    {
        Ok(items) => items,
        Err(err) => {
            warn!("failed to materialize rollout references for model replay: {err}");
            Vec::new()
        }
    }
}

pub async fn materialize_rollout_items_for_complete_history(
    codex_home: &Path,
    rollout_items: &[RolloutItem],
) -> io::Result<Vec<RolloutItem>> {
    materialize_rollout_items(
        codex_home,
        rollout_items,
        RolloutMaterialization::CompleteHistory,
    )
    .await
}

pub(crate) async fn materialize_initial_history_for_model_replay(
    codex_home: &Path,
    initial_history: InitialHistory,
) -> InitialHistory {
    match initial_history {
        InitialHistory::New => InitialHistory::New,
        InitialHistory::Cleared => InitialHistory::Cleared,
        InitialHistory::Resumed(mut resumed) => {
            resumed.history =
                materialize_rollout_items_for_model_replay(codex_home, &resumed.history).await;
            InitialHistory::Resumed(resumed)
        }
        InitialHistory::Forked(items) => InitialHistory::Forked(
            materialize_rollout_items_for_model_replay(codex_home, &items).await,
        ),
    }
}

async fn materialize_rollout_items(
    codex_home: &Path,
    rollout_items: &[RolloutItem],
    materialization: RolloutMaterialization,
) -> io::Result<Vec<RolloutItem>> {
    enum Work {
        Items {
            rollout_items: Vec<RolloutItem>,
            remaining_segment_depth: Option<usize>,
        },
        Item {
            item: Box<RolloutItem>,
            remaining_segment_depth: Option<usize>,
        },
        TruncateSuffix {
            start: usize,
            nth_user_message: usize,
        },
        LeaveReference(RolloutReferenceIdentity),
    }

    let mut materialized = Vec::new();
    let mut active_references = HashSet::new();
    let mut work = vec![Work::Items {
        rollout_items: rollout_items.to_vec(),
        remaining_segment_depth: None,
    }];
    while let Some(next) = work.pop() {
        match next {
            Work::Items {
                rollout_items,
                remaining_segment_depth,
            } => {
                for item in rollout_items.into_iter().rev() {
                    work.push(Work::Item {
                        item: Box::new(item),
                        remaining_segment_depth,
                    });
                }
            }
            Work::Item {
                item,
                remaining_segment_depth,
            } => {
                let reference = match *item {
                    RolloutItem::RolloutReference(reference) => reference,
                    item => {
                        materialized.push(item);
                        continue;
                    }
                };
                let has_prefix_truncation = reference.nth_user_message.is_some();
                let next_remaining_segment_depth = match materialization {
                    RolloutMaterialization::CompleteHistory => None,
                    RolloutMaterialization::ModelReplay if has_prefix_truncation => {
                        remaining_segment_depth
                    }
                    RolloutMaterialization::ModelReplay => {
                        let available_depth = remaining_segment_depth
                            .map_or(reference.max_depth, |remaining| {
                                remaining.min(reference.max_depth)
                            });
                        let Some(next_remaining_depth) = available_depth.checked_sub(1) else {
                            warn!("rollout reference materialization reached max depth");
                            continue;
                        };
                        Some(next_remaining_depth)
                    }
                };
                let resolved_path =
                    match resolve_rollout_reference_rollout_path(codex_home, &reference).await {
                        Ok(path) => path,
                        Err(err) => {
                            if matches!(materialization, RolloutMaterialization::CompleteHistory) {
                                return Err(io::Error::new(
                                    err.kind(),
                                    format!(
                                        "failed to resolve rollout reference {}: {err}",
                                        reference.rollout_path.display()
                                    ),
                                ));
                            }
                            warn!(
                                "failed to resolve rollout reference {}: {err}",
                                reference.rollout_path.display()
                            );
                            continue;
                        }
                    };
                let identity = reference
                    .segment_id
                    .map(RolloutReferenceIdentity::Segment)
                    .unwrap_or_else(|| RolloutReferenceIdentity::Path(resolved_path.clone()));
                if !active_references.insert(identity.clone()) {
                    if matches!(materialization, RolloutMaterialization::CompleteHistory) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "rollout reference cycle detected at {}",
                                resolved_path.display()
                            ),
                        ));
                    }
                    warn!(
                        "rollout reference cycle detected at {}",
                        resolved_path.display()
                    );
                    continue;
                }
                match RolloutRecorder::load_rollout_items(&resolved_path).await {
                    Ok((mut reference_items, _, _)) => {
                        if let Some(filter_texts) = reference
                            .compacted_replacement_history_filter_texts
                            .as_deref()
                        {
                            apply_compacted_replacement_history_filter(
                                &mut reference_items,
                                filter_texts,
                            );
                        }
                        work.push(Work::LeaveReference(identity));
                        if let Some(nth_user_message) = reference.nth_user_message {
                            work.push(Work::TruncateSuffix {
                                start: materialized.len(),
                                nth_user_message,
                            });
                        }
                        work.push(Work::Items {
                            rollout_items: reference_items,
                            remaining_segment_depth: next_remaining_segment_depth,
                        });
                    }
                    Err(err) => {
                        active_references.remove(&identity);
                        if matches!(materialization, RolloutMaterialization::CompleteHistory) {
                            return Err(io::Error::new(
                                err.kind(),
                                format!(
                                    "failed to load rollout reference {}: {err}",
                                    resolved_path.display()
                                ),
                            ));
                        }
                        warn!(
                            "failed to load rollout reference {}: {err}",
                            resolved_path.display()
                        );
                    }
                }
            }
            Work::TruncateSuffix {
                start,
                nth_user_message,
            } => {
                let suffix = truncate_rollout_before_nth_user_message_from_start(
                    &materialized[start..],
                    nth_user_message,
                );
                materialized.truncate(start);
                materialized.extend(suffix);
            }
            Work::LeaveReference(identity) => {
                active_references.remove(&identity);
            }
        }
    }
    Ok(materialized)
}

fn apply_compacted_replacement_history_filter(
    rollout_items: &mut [RolloutItem],
    filter_texts: &[String],
) {
    for item in rollout_items {
        match item {
            RolloutItem::Compacted(compacted) => {
                if let Some(replacement_history) = compacted.replacement_history.as_mut() {
                    replacement_history.retain(|response_item| {
                        !matches_filtered_developer_message(response_item, filter_texts)
                    });
                }
            }
            RolloutItem::RolloutReference(reference) => {
                let nested_filter_texts = reference
                    .compacted_replacement_history_filter_texts
                    .get_or_insert_default();
                for filter_text in filter_texts {
                    if !nested_filter_texts.contains(filter_text) {
                        nested_filter_texts.push(filter_text.clone());
                    }
                }
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::InterAgentCommunication(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::EventMsg(_) => {}
        }
    }
}

fn matches_filtered_developer_message(item: &ResponseItem, filter_texts: &[String]) -> bool {
    let ResponseItem::Message { role, content, .. } = item else {
        return false;
    };
    if role != "developer" {
        return false;
    }
    let [ContentItem::InputText { text }] = content.as_slice() else {
        return false;
    };

    filter_texts.iter().any(|filter_text| filter_text == text)
}

fn is_real_user_message_boundary(item: &ResponseItem) -> bool {
    matches!(
        event_mapping::parse_turn_item(item),
        Some(TurnItem::UserMessage(_))
    )
}

fn is_trigger_turn_boundary(item: &ResponseItem) -> bool {
    let ResponseItem::Message { role, content, .. } = item else {
        return false;
    };

    role == "assistant"
        && InterAgentCommunication::from_message_content(content)
            .is_some_and(|communication| communication.trigger_turn)
}

#[cfg(test)]
#[path = "thread_rollout_truncation_tests.rs"]
mod tests;
