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
        // A `TurnContextItem` appears in rollout only for user turns that
        // emit model-visible context items (during normal context updates or when
        // mid-turn compaction reinjects full initial context). That means replay only
        // needs to answer:
        // - which surviving user turn last established the
        //   `reference_context_item` baseline for context diffing
        // - which `previous_model` that same turn carried
        // - whether any later compaction cleared that
        //   `reference_context_item` before resume
        //
        // We model that with:
        // - one active turn span while walking lifecycle events
        // - one finalized metadata segment per surviving user turn
        // - legacy `TurnContextItem` / `Compacted` entries contributing directly to that same
        //   segment stream when no matching turn span exists
        //
        // `ThreadRolledBack` updates both:
        // - history: drop user turns from reconstructed response items
        // - metadata segments: drop the same number of surviving user-turn segments
        //
        // This keeps history reconstruction and resume/fork hydration on the same replay.
        #[derive(Debug)]
        struct ActiveRolloutTurn {
            turn_id: String,
            saw_user_message: bool,
            previous_model: Option<String>,
            reference_context_item: Option<TurnContextItem>,
            cleared_reference_context_item: bool,
        }

        #[derive(Debug)]
        struct ReplayedUserTurn {
            previous_model: Option<String>,
            reference_context_item: Option<TurnContextItem>,
            cleared_reference_context_item: bool,
        }

        #[derive(Debug)]
        enum RolloutReplayMetaSegment {
            UserTurn(Box<ReplayedUserTurn>),
            // A later segment cleared any older `reference_context_item` without producing a new
            // surviving user-turn baseline. This can come from standalone compaction turns or
            // legacy/unmatched compaction replay.
            ReferenceContextCleared,
        }

        let mut history = ContextManager::new();
        let mut active_turn: Option<ActiveRolloutTurn> = None;
        let mut replayed_segments = Vec::new();
        let push_replayed_turn = |replayed_segments: &mut Vec<RolloutReplayMetaSegment>,
                                  active_turn: ActiveRolloutTurn| {
            if active_turn.saw_user_message {
                replayed_segments.push(RolloutReplayMetaSegment::UserTurn(Box::new(
                    ReplayedUserTurn {
                        previous_model: active_turn.previous_model,
                        reference_context_item: active_turn.reference_context_item,
                        cleared_reference_context_item: active_turn.cleared_reference_context_item,
                    },
                )));
            } else if active_turn.cleared_reference_context_item {
                replayed_segments.push(RolloutReplayMetaSegment::ReferenceContextCleared);
            }
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
                        active_turn.reference_context_item = None;
                        active_turn.cleared_reference_context_item = true;
                    } else {
                        replayed_segments.push(RolloutReplayMetaSegment::ReferenceContextCleared);
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
                        // dropped before we trim older finalized user-turn segments.
                        active_turn = None;
                        turns_to_drop -= 1;
                    }
                    if turns_to_drop > 0 {
                        let mut idx = replayed_segments.len();
                        while idx > 0 && turns_to_drop > 0 {
                            idx -= 1;
                            if let RolloutReplayMetaSegment::UserTurn(_) = &replayed_segments[idx] {
                                replayed_segments.remove(idx);
                                turns_to_drop -= 1;
                            }
                        }
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                    if let Some(active_turn) = active_turn.take() {
                        push_replayed_turn(&mut replayed_segments, active_turn);
                    }
                    active_turn = Some(ActiveRolloutTurn {
                        turn_id: event.turn_id.clone(),
                        saw_user_message: false,
                        previous_model: None,
                        reference_context_item: None,
                        cleared_reference_context_item: false,
                    });
                }
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                    if active_turn
                        .as_ref()
                        .is_some_and(|turn| turn.turn_id == event.turn_id)
                        && let Some(active_turn) = active_turn.take()
                    {
                        push_replayed_turn(&mut replayed_segments, active_turn);
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
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
                            if let Some(active_turn) = active_turn.take() {
                                push_replayed_turn(&mut replayed_segments, active_turn);
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
                        active_turn.previous_model = Some(ctx.model.clone());
                        active_turn.reference_context_item = Some(ctx.clone());
                        active_turn.cleared_reference_context_item = false;
                    } else {
                        replayed_segments.push(RolloutReplayMetaSegment::UserTurn(Box::new(
                            ReplayedUserTurn {
                                previous_model: Some(ctx.model.clone()),
                                reference_context_item: Some(ctx.clone()),
                                cleared_reference_context_item: false,
                            },
                        )));
                    }
                }
                _ => {}
            }
        }

        if let Some(active_turn) = active_turn.take() {
            push_replayed_turn(&mut replayed_segments, active_turn);
        }

        let mut previous_model = None;
        let mut reference_context_item = None;
        for segment in replayed_segments {
            match segment {
                RolloutReplayMetaSegment::ReferenceContextCleared => {
                    reference_context_item = None;
                }
                RolloutReplayMetaSegment::UserTurn(turn) => {
                    if let Some(turn_previous_model) = turn.previous_model {
                        previous_model = Some(turn_previous_model);
                    }
                    if turn.cleared_reference_context_item {
                        reference_context_item = None;
                    }
                    if let Some(turn_reference_context_item) = turn.reference_context_item {
                        reference_context_item = Some(turn_reference_context_item);
                    }
                }
            }
        }

        RolloutReconstruction {
            history: history.raw_items().to_vec(),
            previous_model,
            reference_context_item,
        }
    }
}
