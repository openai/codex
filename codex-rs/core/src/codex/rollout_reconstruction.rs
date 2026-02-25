use super::*;

#[derive(Debug)]
pub(super) struct RolloutReconstruction {
    pub(super) history: Vec<ResponseItem>,
    pub(super) previous_model: Option<String>,
    pub(super) reference_context_item: Option<TurnContextItem>,
}

impl Session {
    pub(super) async fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> RolloutReconstruction {
        // Replay rollout items once and compute two things in lockstep:
        //   1) reconstructed conversation history (via `ContextManager`)
        //   2) resume/fork hydration metadata (`previous_model` and
        //      `reference_context_item`)
        //
        // The metadata part needs rollback-aware accounting over "turn spans" and
        // compaction placement:
        // - `ActiveRolloutTurn` tracks the in-progress turn span while we walk forward
        //   through lifecycle events (`TurnStarted` ... `TurnComplete`/`TurnAborted`).
        // - `ReplayedRolloutTurn` is the finalized per-turn metadata we keep after a
        //   turn ends (whether it had a user message, a `TurnContextItem`, and whether
        //   any compaction in that span happened before or after the first
        //   `TurnContextItem` for that turn).
        // - `RolloutReplayMetaSegment` stores the finalized sequence we later
        //   rollback-adjust and reverse-scan to find the last surviving regular turn
        //   context. Replaced/trailing incomplete turns are finalized as ordinary
        //   `Turn(...)` segments.
        //
        // Explicit replay rule:
        // - compaction before the first `TurnContextItem` in a turn span is treated as
        //   preturn compaction for that turn and invalidates
        //   `reference_context_item` on resume
        // - compaction after the first `TurnContextItem` in the same turn span is
        //   treated as mid-turn compaction and does not invalidate that turn's own
        //   `reference_context_item`
        // - compaction outside any matched turn span is treated conservatively as
        //   preturn-equivalent for baseline hydration (invalidate older baseline)
        //
        // `ThreadRolledBack` updates both:
        // - history: drop user turns from reconstructed response items
        // - metadata segments: remove finalized turn spans that consumed those user turns
        //
        // This keeps resume/fork baseline hydration consistent with the same replay
        // logic used to rebuild history, instead of maintaining a second bespoke scan.
        #[derive(Debug)]
        struct ActiveRolloutTurn {
            turn_id: String,
            saw_user_message: bool,
            turn_context_item: Option<TurnContextItem>,
            has_preturn_compaction: bool,
            has_midturn_compaction: bool,
        }

        #[derive(Debug)]
        struct ReplayedRolloutTurn {
            saw_user_message: bool,
            turn_context_item: Option<TurnContextItem>,
            has_preturn_compaction: bool,
            has_midturn_compaction: bool,
        }

        #[derive(Debug)]
        enum RolloutReplayMetaSegment {
            Turn(Box<ReplayedRolloutTurn>),
            // Unexpected for modern rollouts, where compaction should occur inside
            // a matched turn span (`TurnStarted` ... `TurnComplete`/`TurnAborted`).
            //
            // We keep this as a minimal fallback for legacy/incomplete lifecycle
            // data: treat as "compaction happened after older baseline" and prefer
            // conservative baseline invalidation over complex reconstruction.
            CompactionOutsideTurn,
        }

        let mut history = ContextManager::new();
        let mut saw_turn_lifecycle_event = false;
        let mut active_turn: Option<ActiveRolloutTurn> = None;
        let mut replayed_segments = Vec::new();
        let push_replayed_turn = |replayed_segments: &mut Vec<RolloutReplayMetaSegment>,
                                  active_turn: ActiveRolloutTurn| {
            replayed_segments.push(RolloutReplayMetaSegment::Turn(Box::new(
                ReplayedRolloutTurn {
                    saw_user_message: active_turn.saw_user_message,
                    turn_context_item: active_turn.turn_context_item,
                    has_preturn_compaction: active_turn.has_preturn_compaction,
                    has_midturn_compaction: active_turn.has_midturn_compaction,
                },
            )));
        };

        for item in rollout_items {
            match item {
                RolloutItem::ResponseItem(response_item) => {
                    history.record_items(
                        std::iter::once(response_item),
                        turn_context.truncation_policy,
                    );
                }
                RolloutItem::Compacted(compacted) => {
                    if let Some(replacement) = &compacted.replacement_history {
                        history.replace(replacement.clone());
                    } else {
                        let user_messages = collect_user_messages(history.raw_items());
                        let rebuilt = compact::build_compacted_history(
                            self.build_initial_context(turn_context).await,
                            &user_messages,
                            &compacted.message,
                        );
                        history.replace(rebuilt);
                    }
                    if let Some(active_turn) = active_turn.as_mut() {
                        if active_turn.turn_context_item.is_none() {
                            active_turn.has_preturn_compaction = true;
                        } else {
                            active_turn.has_midturn_compaction = true;
                        }
                    } else {
                        replayed_segments.push(RolloutReplayMetaSegment::CompactionOutsideTurn);
                    }
                }
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                    history.drop_last_n_user_turns(rollback.num_turns);
                    let mut turns_to_drop =
                        usize::try_from(rollback.num_turns).unwrap_or(usize::MAX);
                    if turns_to_drop > 0
                        && active_turn
                            .as_ref()
                            .is_some_and(|turn| turn.saw_user_message)
                    {
                        // Match `drop_last_n_user_turns`: an unfinished active turn that has
                        // already emitted a user message is the newest user turn and should be
                        // dropped before we trim older finalized turn spans.
                        active_turn = None;
                        turns_to_drop -= 1;
                    }
                    if turns_to_drop > 0 {
                        let mut idx = replayed_segments.len();
                        while idx > 0 && turns_to_drop > 0 {
                            idx -= 1;
                            if let RolloutReplayMetaSegment::Turn(turn) = &replayed_segments[idx]
                                && turn.saw_user_message
                            {
                                replayed_segments.remove(idx);
                                turns_to_drop -= 1;
                            }
                        }
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                    saw_turn_lifecycle_event = true;
                    if let Some(active_turn) = active_turn.take() {
                        // Treat a replaced incomplete turn as ended at the point the next turn
                        // starts so replay preserves any `TurnContextItem` it already emitted.
                        push_replayed_turn(&mut replayed_segments, active_turn);
                    }
                    active_turn = Some(ActiveRolloutTurn {
                        turn_id: event.turn_id.clone(),
                        saw_user_message: false,
                        turn_context_item: None,
                        has_preturn_compaction: false,
                        has_midturn_compaction: false,
                    });
                }
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                    saw_turn_lifecycle_event = true;
                    if active_turn
                        .as_ref()
                        .is_some_and(|turn| turn.turn_id == event.turn_id)
                        && let Some(active_turn) = active_turn.take()
                    {
                        push_replayed_turn(&mut replayed_segments, active_turn);
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                    saw_turn_lifecycle_event = true;
                    match event.turn_id.as_deref() {
                        Some(aborted_turn_id)
                            if active_turn
                                .as_ref()
                                .is_some_and(|turn| turn.turn_id == aborted_turn_id) =>
                        {
                            if let Some(active_turn) = active_turn.take() {
                                push_replayed_turn(&mut replayed_segments, active_turn);
                            }
                        }
                        Some(_) => {
                            // Ignore aborts for some other turn and keep the current active turn
                            // alive so later `TurnContext`/`TurnComplete` events still apply.
                        }
                        None => {
                            if let Some(active_turn) = active_turn.take()
                                && (active_turn.has_preturn_compaction
                                    || active_turn.has_midturn_compaction)
                            {
                                // Legacy/incomplete lifecycle events may omit `turn_id` on
                                // abort. Keep fallback handling minimal: drop this ambiguous
                                // turn span and preserve only a conservative "outside-turn
                                // compaction" marker.
                                replayed_segments
                                    .push(RolloutReplayMetaSegment::CompactionOutsideTurn);
                            }
                        }
                    }
                }
                RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                    if let Some(active_turn) = active_turn.as_mut() {
                        active_turn.saw_user_message = true;
                    }
                }
                RolloutItem::TurnContext(ctx) => {
                    if let Some(active_turn) = active_turn.as_mut()
                        && ctx
                            .turn_id
                            .as_deref()
                            .is_none_or(|turn_id| turn_id == active_turn.turn_id)
                    {
                        // Keep the latest `TurnContextItem` in rollout order for the turn.
                        active_turn.turn_context_item = Some(ctx.clone());
                    }
                }
                _ => {}
            }
        }

        if let Some(active_turn) = active_turn.take() {
            // Treat a trailing incomplete turn as ended at EOF so replay preserves any
            // `TurnContextItem` it already emitted before the rollout was truncated.
            push_replayed_turn(&mut replayed_segments, active_turn);
        }

        let (previous_model, reference_context_item) = if saw_turn_lifecycle_event {
            let mut compaction_cleared_reference_context_item = false;
            let mut previous_regular_turn_context_item = None;

            for segment in replayed_segments.iter().rev() {
                match segment {
                    RolloutReplayMetaSegment::CompactionOutsideTurn => {
                        compaction_cleared_reference_context_item = true;
                    }
                    RolloutReplayMetaSegment::Turn(turn) => {
                        if let Some(turn_context_item) = &turn.turn_context_item {
                            if turn.has_preturn_compaction {
                                compaction_cleared_reference_context_item = true;
                            }
                            previous_regular_turn_context_item = Some(turn_context_item.clone());
                            break;
                        }
                        if turn.has_preturn_compaction || turn.has_midturn_compaction {
                            // This later surviving turn compacted (for example via `/compact` or
                            // auto-compaction) but did not persist a replacement TurnContextItem,
                            // so conservatively invalidate any older baseline we might select.
                            compaction_cleared_reference_context_item = true;
                        }
                    }
                }
            }

            let previous_model = previous_regular_turn_context_item
                .as_ref()
                .map(|ctx| ctx.model.clone());
            let reference_context_item = if compaction_cleared_reference_context_item {
                // Keep the baseline empty when compaction may have stripped the referenced
                // context diffs so the first resumed regular turn fully reinjects context.
                None
            } else {
                previous_regular_turn_context_item
            };
            (previous_model, reference_context_item)
        } else {
            // Legacy/minimal fallback (no lifecycle events): use the last persisted
            // `TurnContextItem` in rollout order and conservatively null baseline when a
            // later `Compacted` item exists.
            let mut legacy_last_turn_context_item: Option<TurnContextItem> = None;
            let mut legacy_saw_compaction_after_last_turn_context = false;
            for item in rollout_items.iter().rev() {
                match item {
                    RolloutItem::Compacted(_) => {
                        legacy_saw_compaction_after_last_turn_context = true;
                    }
                    RolloutItem::TurnContext(ctx) => {
                        legacy_last_turn_context_item = Some(ctx.clone());
                        break;
                    }
                    _ => {}
                }
            }

            let previous_model = legacy_last_turn_context_item
                .as_ref()
                .map(|ctx| ctx.model.clone());
            let reference_context_item = if legacy_saw_compaction_after_last_turn_context {
                None
            } else {
                legacy_last_turn_context_item
            };
            (previous_model, reference_context_item)
        };

        RolloutReconstruction {
            history: history.raw_items().to_vec(),
            previous_model,
            reference_context_item,
        }
    }
}
