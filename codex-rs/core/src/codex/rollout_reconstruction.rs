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
    Latest(Box<TurnContextItem>),
}

#[derive(Debug, Default)]
struct ActiveReplaySegment {
    turn_id: Option<String>,
    counts_as_user_turn: bool,
    previous_model: Option<String>,
    reference_context_item: TurnReferenceContextItem,
    base_replacement_history: Option<Vec<ResponseItem>>,
}

fn turn_ids_are_compatible(active_turn_id: Option<&str>, item_turn_id: Option<&str>) -> bool {
    active_turn_id
        .is_none_or(|turn_id| item_turn_id.is_none_or(|item_turn_id| item_turn_id == turn_id))
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
        // are both known; then replay only the buffered surviving tail forward to preserve exact
        // history semantics.
        let mut base_replacement_history = None;
        let mut previous_model = None;
        let mut reference_context_item = TurnReferenceContextItem::NeverSet;
        // Rollback is "drop the newest N user turns". While scanning in reverse, that becomes
        // "skip the next N user-turn segments we finalize".
        let mut pending_rollback_turns = 0usize;
        // Rollout items that survive after the newest replacement-history checkpoint, stored in
        // reverse scan order so they can later be replayed forward on top of that checkpoint.
        let mut rollout_suffix_items_rev = Vec::new();
        // Reverse replay accumulates rollout items into the newest in-progress turn segment until
        // we hit its matching `TurnStarted`, at which point the segment can be finalized.
        let mut active_segment: Option<ActiveReplaySegment> = None;

        let finalize_active_segment =
            |active_segment: ActiveReplaySegment,
             base_replacement_history: &mut Option<Vec<ResponseItem>>,
             previous_model: &mut Option<String>,
             reference_context_item: &mut TurnReferenceContextItem,
             pending_rollback_turns: &mut usize| {
                // Thread rollback always targets the newest surviving user turns, so consume that
                // skip budget before letting this segment contribute metadata or a compaction base.
                if *pending_rollback_turns > 0 {
                    if active_segment.counts_as_user_turn {
                        *pending_rollback_turns -= 1;
                    }
                    return;
                }

                // A surviving replacement-history checkpoint is a complete history base. Once we
                // know the newest surviving one, older rollout items do not affect rebuilt history.
                if base_replacement_history.is_none()
                    && let Some(segment_base_replacement_history) =
                        active_segment.base_replacement_history
                {
                    *base_replacement_history = Some(segment_base_replacement_history);
                }

                // `previous_model` comes from the newest surviving user turn that established one.
                if previous_model.is_none() && active_segment.counts_as_user_turn {
                    *previous_model = active_segment.previous_model;
                }

                // `reference_context_item` comes from the newest surviving user turn baseline, or
                // from a surviving compaction that explicitly cleared that baseline.
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

        for item in rollout_items.iter().rev() {
            match item {
                RolloutItem::Compacted(compacted) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    // Looking backward, compaction clears any older baseline unless a newer
                    // `TurnContextItem` in this same segment has already re-established it.
                    if matches!(
                        active_segment.reference_context_item,
                        TurnReferenceContextItem::NeverSet
                    ) {
                        active_segment.reference_context_item = TurnReferenceContextItem::Cleared;
                    }
                    if active_segment.base_replacement_history.is_none()
                        && let Some(replacement_history) = &compacted.replacement_history
                    {
                        active_segment.base_replacement_history = Some(replacement_history.clone());
                    }
                }
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                    pending_rollback_turns = pending_rollback_turns
                        .saturating_add(usize::try_from(rollback.num_turns).unwrap_or(usize::MAX));
                }
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    // Reverse replay often sees `TurnComplete` before any turn-scoped metadata.
                    // Capture the turn id early so later `TurnContext` / abort items can match it.
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
                    // `TurnContextItem` can attach metadata to an existing segment, but only a
                    // real `UserMessage` event should make the segment count as a user turn.
                    if active_segment.turn_id.is_none() {
                        active_segment.turn_id = ctx.turn_id.clone();
                    }
                    if turn_ids_are_compatible(
                        active_segment.turn_id.as_deref(),
                        ctx.turn_id.as_deref(),
                    ) {
                        active_segment.previous_model = Some(ctx.model.clone());
                        if matches!(
                            active_segment.reference_context_item,
                            TurnReferenceContextItem::NeverSet
                        ) {
                            active_segment.reference_context_item =
                                TurnReferenceContextItem::Latest(Box::new(ctx.clone()));
                        }
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                    // `TurnStarted` is the oldest boundary of the active reverse segment.
                    if active_segment.as_ref().is_some_and(|active_segment| {
                        turn_ids_are_compatible(
                            active_segment.turn_id.as_deref(),
                            Some(event.turn_id.as_str()),
                        )
                    }) && let Some(active_segment) = active_segment.take()
                    {
                        finalize_active_segment(
                            active_segment,
                            &mut base_replacement_history,
                            &mut previous_model,
                            &mut reference_context_item,
                            &mut pending_rollback_turns,
                        );
                    }
                }
                RolloutItem::ResponseItem(_)
                | RolloutItem::EventMsg(_)
                | RolloutItem::SessionMeta(_) => {}
            }

            if base_replacement_history.is_some()
                && previous_model.is_some()
                && !matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
            {
                // At this point we have both eager resume metadata values and the replacement-
                // history base for the surviving tail, so older rollout items cannot affect this
                // result.
                break;
            }

            if base_replacement_history.is_none() {
                rollout_suffix_items_rev.push(item.clone());
            }
        }

        if let Some(active_segment) = active_segment.take() {
            finalize_active_segment(
                active_segment,
                &mut base_replacement_history,
                &mut previous_model,
                &mut reference_context_item,
                &mut pending_rollback_turns,
            );
        }

        let initial_context = self.build_initial_context(turn_context, None).await;
        let mut history = ContextManager::new();
        if let Some(base_replacement_history) = base_replacement_history {
            history.replace(base_replacement_history);
        }
        // Temporary eager bridge: rebuild exact history semantics from the buffered surviving tail
        // discovered by reverse replay. This keeps the history result stable while the control
        // flow moves toward the future lazy reverse loader design without depending on absolute
        // rollout indices.
        for item in rollout_suffix_items_rev.iter().rev() {
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

        let reference_context_item = match reference_context_item {
            TurnReferenceContextItem::NeverSet | TurnReferenceContextItem::Cleared => None,
            TurnReferenceContextItem::Latest(turn_reference_context_item) => {
                Some(*turn_reference_context_item)
            }
        };

        RolloutReconstruction {
            history: history.raw_items().to_vec(),
            previous_model,
            reference_context_item,
        }
    }
}
