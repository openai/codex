use super::*;

// Return value of `Session::reconstruct_history_from_rollout`, bundling the rebuilt history with
// the resume/fork hydration metadata derived from the same replay.
#[derive(Debug)]
pub(super) struct RolloutReconstruction {
    pub(super) history: Vec<ResponseItem>,
    pub(super) previous_model: Option<String>,
    pub(super) reference_context_item: Option<TurnContextItem>,
}

#[derive(Debug, Default)]
enum TurnReferenceContextItem {
    /// No `TurnContextItem` has been seen for this replay span yet.
    ///
    /// This differs from `Cleared`: `NeverSet` means there is no evidence this turn ever
    /// established a baseline, while `Cleared` means a baseline existed and a later compaction
    /// invalidated it. Only the latter must emit an explicit clearing segment for resume/fork
    /// hydration.
    #[default]
    NeverSet,
    /// A previously established baseline was invalidated by later compaction.
    Cleared,
    /// The latest baseline established by this replay span.
    LatestSet(Box<TurnContextItem>),
}

#[derive(Debug, Default)]
struct ActiveReplaySegment {
    turn_id: Option<String>,
    counts_as_user_turn: bool,
    previous_model: Option<String>,
    reference_context_item: TurnReferenceContextItem,
    replacement_history_index: Option<usize>,
}

impl Session {
    pub(super) async fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> RolloutReconstruction {
        // Replay metadata should already match the shape of the future lazy reverse loader, even
        // while history materialization still uses an eager bridge. Scan newest-to-oldest,
        // stopping once a surviving replacement-history checkpoint and the required resume metadata
        // are both known; then replay only that suffix forward to preserve exact history semantics.
        let mut replay_start_index = None;
        let mut previous_model = None;
        let mut reference_context_item = TurnReferenceContextItem::NeverSet;
        let mut pending_rollback_turns = 0usize;
        let mut active_segment: Option<ActiveReplaySegment> = None;

        let finalize_active_segment =
            |active_segment: ActiveReplaySegment,
             replay_start_index: &mut Option<usize>,
             previous_model: &mut Option<String>,
             reference_context_item: &mut TurnReferenceContextItem,
             pending_rollback_turns: &mut usize| {
                if *pending_rollback_turns > 0 {
                    if active_segment.counts_as_user_turn {
                        *pending_rollback_turns -= 1;
                    }
                    return;
                }

                if replay_start_index.is_none()
                    && let Some(replacement_history_index) =
                        active_segment.replacement_history_index
                {
                    *replay_start_index = Some(replacement_history_index);
                }

                if previous_model.is_none() && active_segment.counts_as_user_turn {
                    *previous_model = active_segment.previous_model;
                }

                if matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
                    && (active_segment.counts_as_user_turn
                        || matches!(
                            active_segment.reference_context_item,
                            TurnReferenceContextItem::Cleared
                        ))
                {
                    *reference_context_item = active_segment.reference_context_item;
                }
            };

        for (index, item) in rollout_items.iter().enumerate().rev() {
            match item {
                RolloutItem::Compacted(compacted) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    if matches!(
                        active_segment.reference_context_item,
                        TurnReferenceContextItem::NeverSet | TurnReferenceContextItem::Cleared
                    ) {
                        active_segment.reference_context_item = TurnReferenceContextItem::Cleared;
                    }
                    if active_segment.replacement_history_index.is_none()
                        && compacted.replacement_history.is_some()
                    {
                        active_segment.replacement_history_index = Some(index);
                    }
                }
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                    pending_rollback_turns = pending_rollback_turns
                        .saturating_add(usize::try_from(rollback.num_turns).unwrap_or(usize::MAX));
                }
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                    if active_segment.as_ref().is_some_and(|active_segment| {
                        active_segment
                            .turn_id
                            .as_deref()
                            .is_none_or(|turn_id| turn_id == event.turn_id)
                    }) && let Some(active_segment) = active_segment.take()
                    {
                        finalize_active_segment(
                            active_segment,
                            &mut replay_start_index,
                            &mut previous_model,
                            &mut reference_context_item,
                            &mut pending_rollback_turns,
                        );
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    if active_segment.turn_id.is_none() {
                        active_segment.turn_id = Some(event.turn_id.clone());
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                    if let Some(active_segment) = active_segment.as_mut() {
                        if active_segment.turn_id.is_none()
                            && let Some(turn_id) = &event.turn_id
                        {
                            active_segment.turn_id = Some(turn_id.clone());
                        }
                    } else if let Some(turn_id) = &event.turn_id {
                        active_segment = Some(ActiveReplaySegment {
                            turn_id: Some(turn_id.clone()),
                            ..Default::default()
                        });
                    }
                }
                RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    active_segment.counts_as_user_turn = true;
                }
                RolloutItem::TurnContext(ctx) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    if active_segment.turn_id.is_none() {
                        active_segment.turn_id = ctx.turn_id.clone();
                        active_segment.counts_as_user_turn = true;
                    }
                    if active_segment.turn_id.as_deref().is_none_or(|turn_id| {
                        ctx.turn_id
                            .as_deref()
                            .is_none_or(|ctx_turn_id| ctx_turn_id == turn_id)
                    }) {
                        active_segment.previous_model = Some(ctx.model.clone());
                        if matches!(
                            active_segment.reference_context_item,
                            TurnReferenceContextItem::NeverSet
                        ) {
                            active_segment.reference_context_item =
                                TurnReferenceContextItem::LatestSet(Box::new(ctx.clone()));
                        }
                    }
                }
                RolloutItem::ResponseItem(_)
                | RolloutItem::EventMsg(_)
                | RolloutItem::SessionMeta(_) => {}
            }

            if replay_start_index.is_some()
                && previous_model.is_some()
                && !matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
            {
                break;
            }
        }

        if let Some(active_segment) = active_segment.take() {
            finalize_active_segment(
                active_segment,
                &mut replay_start_index,
                &mut previous_model,
                &mut reference_context_item,
                &mut pending_rollback_turns,
            );
        }

        let initial_context = self.build_initial_context(turn_context, None).await;
        let mut history = ContextManager::new();
        for item in &rollout_items[replay_start_index.unwrap_or(0)..] {
            match item {
                RolloutItem::ResponseItem(response_item) => {
                    history.record_items(
                        std::iter::once(response_item),
                        turn_context.truncation_policy,
                    );
                }
                RolloutItem::Compacted(compacted) => {
                    if let Some(replacement_history) = &compacted.replacement_history {
                        history.replace(replacement_history.clone());
                    } else {
                        let user_messages = collect_user_messages(history.raw_items());
                        let rebuilt = compact::build_compacted_history(
                            initial_context.clone(),
                            &user_messages,
                            &compacted.message,
                        );
                        history.replace(rebuilt);
                    }
                }
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                    history.drop_last_n_user_turns(rollback.num_turns);
                }
                RolloutItem::EventMsg(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::SessionMeta(_) => {}
            }
        }

        RolloutReconstruction {
            history: history.raw_items().to_vec(),
            previous_model,
            reference_context_item: match reference_context_item {
                TurnReferenceContextItem::NeverSet | TurnReferenceContextItem::Cleared => None,
                TurnReferenceContextItem::LatestSet(turn_reference_context_item) => {
                    Some(*turn_reference_context_item)
                }
            },
        }
    }
}
