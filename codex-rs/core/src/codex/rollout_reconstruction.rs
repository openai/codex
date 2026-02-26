use super::*;

// Return value of `Session::reconstruct_history_from_rollout`, bundling the rebuilt history with
// the resume/fork hydration metadata derived from the same replay.
#[derive(Debug)]
pub(super) struct RolloutReconstruction {
    pub(super) history: Vec<ResponseItem>,
    pub(super) previous_model: Option<String>,
    pub(super) reference_context_item: Option<TurnContextItem>,
}

#[derive(Debug)]
struct ReplayedUserTurn {
    previous_model: Option<String>,
    reference_context_item: Option<TurnContextItem>,
    reference_context_item_cleared: bool,
}

#[derive(Debug)]
enum ReplayMetadataSegment {
    UserTurn(Box<ReplayedUserTurn>),
    // Unexpected for modern rollouts, where compaction should normally happen inside a user turn.
    // Keep this as a minimal legacy/incomplete-rollout fallback so later resume conservatively
    // clears the baseline until another `TurnContextItem` re-establishes it.
    ReferenceContextCleared,
}

#[derive(Debug, Default)]
struct ActiveUserTurn {
    turn_id: Option<String>,
    saw_user_message: bool,
    previous_model: Option<String>,
    reference_context_item: Option<TurnContextItem>,
    reference_context_item_cleared: bool,
}

impl Session {
    pub(super) async fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> RolloutReconstruction {
        // Keep rollout replay eager and forward-only for now.
        //
        // This unifies mainline history replay with eager resume metadata hydration in one place,
        // without yet introducing the lazy reverse-loading machinery we want later. History is
        // rebuilt exactly as rollout recorded it, while metadata keeps only enough per-turn state
        // to answer:
        // - which surviving user turn last provided `previous_model`
        // - which surviving `TurnContextItem` baseline, if any, remains after later compaction
        let initial_context = self.build_initial_context(turn_context).await;
        let mut history = ContextManager::new();
        let mut replayed_segments = Vec::new();
        let mut active_turn: Option<ActiveUserTurn> = None;

        let finalize_active_turn =
            |replayed_segments: &mut Vec<ReplayMetadataSegment>, active_turn: ActiveUserTurn| {
                if active_turn.saw_user_message {
                    replayed_segments.push(ReplayMetadataSegment::UserTurn(Box::new(
                        ReplayedUserTurn {
                            previous_model: active_turn.previous_model,
                            reference_context_item: active_turn.reference_context_item,
                            reference_context_item_cleared: active_turn
                                .reference_context_item_cleared,
                        },
                    )));
                } else if active_turn.reference_context_item_cleared {
                    replayed_segments.push(ReplayMetadataSegment::ReferenceContextCleared);
                }
            };

        let drop_last_n_user_turns = |replayed_segments: &mut Vec<ReplayMetadataSegment>,
                                      num_turns: u32| {
            let mut remaining = usize::try_from(num_turns).unwrap_or(usize::MAX);
            while remaining > 0 {
                let Some(index) = replayed_segments
                    .iter()
                    .rposition(|segment| matches!(segment, ReplayMetadataSegment::UserTurn(_)))
                else {
                    break;
                };
                replayed_segments.remove(index);
                remaining -= 1;
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

                    if let Some(active_turn) = active_turn.as_mut() {
                        active_turn.reference_context_item = None;
                        active_turn.reference_context_item_cleared = true;
                    } else {
                        replayed_segments.push(ReplayMetadataSegment::ReferenceContextCleared);
                    }
                }
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                    history.drop_last_n_user_turns(rollback.num_turns);

                    let mut remaining = usize::try_from(rollback.num_turns).unwrap_or(usize::MAX);
                    if remaining > 0
                        && active_turn
                            .as_ref()
                            .is_some_and(|active_turn| active_turn.saw_user_message)
                    {
                        active_turn = None;
                        remaining -= 1;
                    }
                    if remaining > 0 {
                        drop_last_n_user_turns(
                            &mut replayed_segments,
                            u32::try_from(remaining).unwrap_or(u32::MAX),
                        );
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                    if let Some(active_turn) = active_turn.take() {
                        finalize_active_turn(&mut replayed_segments, active_turn);
                    }
                    active_turn = Some(ActiveUserTurn {
                        turn_id: Some(event.turn_id.clone()),
                        ..Default::default()
                    });
                }
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                    if active_turn.as_ref().is_some_and(|active_turn| {
                        active_turn
                            .turn_id
                            .as_deref()
                            .is_none_or(|turn_id| turn_id == event.turn_id)
                    }) && let Some(active_turn) = active_turn.take()
                    {
                        finalize_active_turn(&mut replayed_segments, active_turn);
                    }
                }
                RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                    match event.turn_id.as_deref() {
                        Some(turn_id)
                            if active_turn.as_ref().is_some_and(|active_turn| {
                                active_turn
                                    .turn_id
                                    .as_deref()
                                    .is_none_or(|active_turn_id| active_turn_id == turn_id)
                            }) =>
                        {
                            if let Some(active_turn) = active_turn.take() {
                                finalize_active_turn(&mut replayed_segments, active_turn);
                            }
                        }
                        Some(_) => {}
                        None => {
                            if let Some(active_turn) = active_turn.take() {
                                finalize_active_turn(&mut replayed_segments, active_turn);
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
                    if let Some(active_turn) = active_turn.as_mut() {
                        if active_turn.turn_id.as_deref().is_none_or(|turn_id| {
                            ctx.turn_id
                                .as_deref()
                                .is_none_or(|ctx_turn_id| ctx_turn_id == turn_id)
                        }) {
                            if active_turn.previous_model.is_none() {
                                active_turn.previous_model = Some(ctx.model.clone());
                            }
                            active_turn.reference_context_item = Some(ctx.clone());
                            active_turn.reference_context_item_cleared = false;
                        }
                    } else {
                        replayed_segments.push(ReplayMetadataSegment::UserTurn(Box::new(
                            ReplayedUserTurn {
                                previous_model: Some(ctx.model.clone()),
                                reference_context_item: Some(ctx.clone()),
                                reference_context_item_cleared: false,
                            },
                        )));
                    }
                }
                _ => {}
            }
        }

        if let Some(active_turn) = active_turn.take() {
            finalize_active_turn(&mut replayed_segments, active_turn);
        }

        let mut previous_model = None;
        let mut reference_context_item = None;
        for segment in replayed_segments {
            match segment {
                ReplayMetadataSegment::UserTurn(turn) => {
                    if let Some(turn_previous_model) = turn.previous_model {
                        previous_model = Some(turn_previous_model);
                    }
                    if turn.reference_context_item_cleared {
                        reference_context_item = None;
                    }
                    if let Some(turn_reference_context_item) = turn.reference_context_item {
                        reference_context_item = Some(turn_reference_context_item);
                    }
                }
                ReplayMetadataSegment::ReferenceContextCleared => {
                    reference_context_item = None;
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
