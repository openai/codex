use super::*;
use crate::context_manager::is_user_turn_boundary;

// Return value of `Session::reconstruct_history_from_rollout`, bundling the rebuilt history with
// the resume/fork hydration metadata derived from the same replay.
#[derive(Debug)]
pub(super) struct RolloutReconstruction {
    pub(super) history: Vec<ResponseItem>,
    pub(super) previous_turn_settings: Option<PreviousTurnSettings>,
    pub(super) reference_context_item: Option<TurnContextItem>,
    pub(super) window_generation: u64,
}

#[derive(Debug)]
pub(super) struct RolloutReconstructionPlan {
    base_replacement_history_index: Option<usize>,
    previous_turn_settings: Option<PreviousTurnSettings>,
    reference_context_item: Option<TurnContextItem>,
    window_generation: u64,
}

#[derive(Debug)]
pub(super) struct PreparedInitialHistory {
    history: InitialHistory,
    reconstruction_plan: Option<RolloutReconstructionPlan>,
}

impl PreparedInitialHistory {
    pub(super) fn new(history: InitialHistory) -> Self {
        let rollout_items = match &history {
            InitialHistory::Resumed(resumed_history) => Some(resumed_history.history.as_slice()),
            InitialHistory::Forked(rollout_items) => Some(rollout_items.as_slice()),
            InitialHistory::New | InitialHistory::Cleared => None,
        };
        let reconstruction_plan = rollout_items.map(Session::reconstruct_rollout_plan);
        Self {
            history,
            reconstruction_plan,
        }
    }

    pub(super) fn history(&self) -> &InitialHistory {
        &self.history
    }

    pub(super) fn window_generation(&self) -> u64 {
        self.reconstruction_plan
            .as_ref()
            .map_or(0, |plan| plan.window_generation)
    }

    pub(super) fn into_parts(self) -> (InitialHistory, Option<RolloutReconstructionPlan>) {
        (self.history, self.reconstruction_plan)
    }
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
    compaction_count: u64,
    pre_user_compaction_count: u64,
    previous_turn_settings: Option<PreviousTurnSettings>,
    reference_context_item: TurnReferenceContextItem,
    base_replacement_history_index: Option<usize>,
}

fn turn_ids_are_compatible(active_turn_id: Option<&str>, item_turn_id: Option<&str>) -> bool {
    active_turn_id
        .is_none_or(|turn_id| item_turn_id.is_none_or(|item_turn_id| item_turn_id == turn_id))
}

fn finalize_active_segment(
    active_segment: ActiveReplaySegment,
    base_replacement_history_index: &mut Option<usize>,
    previous_turn_settings: &mut Option<PreviousTurnSettings>,
    reference_context_item: &mut TurnReferenceContextItem,
    window_generation: &mut u64,
    pending_rollback_turns: &mut usize,
) {
    // Thread rollback drops the newest surviving real user-message boundaries. In replay, that
    // means skipping the next finalized segments that contain a non-contextual
    // `EventMsg::UserMessage`.
    if *pending_rollback_turns > 0 {
        if active_segment.counts_as_user_turn {
            *window_generation =
                window_generation.saturating_add(active_segment.pre_user_compaction_count);
            *pending_rollback_turns -= 1;
        }
        return;
    }

    *window_generation = window_generation.saturating_add(active_segment.compaction_count);

    // A surviving replacement-history checkpoint is a complete history base. Once we
    // know the newest surviving one, older rollout items do not affect rebuilt history.
    if base_replacement_history_index.is_none()
        && let Some(segment_base_replacement_history_index) =
            active_segment.base_replacement_history_index
    {
        *base_replacement_history_index = Some(segment_base_replacement_history_index);
    }

    // `previous_turn_settings` come from the newest surviving user turn that established them.
    if previous_turn_settings.is_none() && active_segment.counts_as_user_turn {
        *previous_turn_settings = active_segment.previous_turn_settings;
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
}

impl Session {
    fn reconstruct_rollout_plan(rollout_items: &[RolloutItem]) -> RolloutReconstructionPlan {
        // Replay metadata should already match the shape of the future lazy reverse loader, even
        // while history materialization still uses an eager bridge. Scan newest-to-oldest once to
        // derive the surviving history base, hydration metadata, and window lineage together.
        let mut base_replacement_history_index = None;
        let mut previous_turn_settings = None;
        let mut reference_context_item = TurnReferenceContextItem::NeverSet;
        let mut window_generation = 0u64;
        // Rollback is "drop the newest N user turns". While scanning in reverse, that becomes
        // "skip the next N user-turn segments we finalize".
        let mut pending_rollback_turns = 0usize;
        // Reverse replay accumulates rollout items into the newest in-progress turn segment until
        // we hit its matching `TurnStarted`, at which point the segment can be finalized.
        let mut active_segment: Option<ActiveReplaySegment> = None;

        for (index, item) in rollout_items.iter().enumerate().rev() {
            match item {
                RolloutItem::Compacted(compacted) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    active_segment.compaction_count =
                        active_segment.compaction_count.saturating_add(1);
                    // A compaction seen after the user boundary in reverse replay occurred before
                    // the user input, so it survives rollback of that user turn.
                    if active_segment.counts_as_user_turn {
                        active_segment.pre_user_compaction_count =
                            active_segment.pre_user_compaction_count.saturating_add(1);
                    }
                    // Looking backward, compaction clears any older baseline unless a newer
                    // `TurnContextItem` in this same segment has already re-established it.
                    if matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
                        && matches!(
                            active_segment.reference_context_item,
                            TurnReferenceContextItem::NeverSet
                        )
                    {
                        active_segment.reference_context_item = TurnReferenceContextItem::Cleared;
                    }
                    if base_replacement_history_index.is_none()
                        && active_segment.base_replacement_history_index.is_none()
                        && compacted.replacement_history.is_some()
                    {
                        active_segment.base_replacement_history_index = Some(index);
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
                    if (previous_turn_settings.is_none()
                        || matches!(reference_context_item, TurnReferenceContextItem::NeverSet))
                        && turn_ids_are_compatible(
                            active_segment.turn_id.as_deref(),
                            ctx.turn_id.as_deref(),
                        )
                    {
                        if previous_turn_settings.is_none() {
                            active_segment.previous_turn_settings = Some(PreviousTurnSettings {
                                model: ctx.model.clone(),
                                realtime_active: ctx.realtime_active,
                            });
                        }
                        if matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
                            && matches!(
                                active_segment.reference_context_item,
                                TurnReferenceContextItem::NeverSet
                            )
                        {
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
                            &mut base_replacement_history_index,
                            &mut previous_turn_settings,
                            &mut reference_context_item,
                            &mut window_generation,
                            &mut pending_rollback_turns,
                        );
                    }
                }
                RolloutItem::ResponseItem(response_item) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    active_segment.counts_as_user_turn |= is_user_turn_boundary(response_item);
                }
                RolloutItem::EventMsg(_) | RolloutItem::SessionMeta(_) => {}
            }
        }

        if let Some(active_segment) = active_segment.take() {
            finalize_active_segment(
                active_segment,
                &mut base_replacement_history_index,
                &mut previous_turn_settings,
                &mut reference_context_item,
                &mut window_generation,
                &mut pending_rollback_turns,
            );
        }

        let reference_context_item = match reference_context_item {
            TurnReferenceContextItem::NeverSet | TurnReferenceContextItem::Cleared => None,
            TurnReferenceContextItem::Latest(turn_reference_context_item) => {
                Some(*turn_reference_context_item)
            }
        };

        RolloutReconstructionPlan {
            base_replacement_history_index,
            previous_turn_settings,
            reference_context_item,
            window_generation,
        }
    }

    pub(super) fn materialize_rollout_reconstruction(
        truncation_policy: TruncationPolicy,
        rollout_items: &[RolloutItem],
        plan: RolloutReconstructionPlan,
    ) -> RolloutReconstruction {
        let RolloutReconstructionPlan {
            base_replacement_history_index,
            previous_turn_settings,
            reference_context_item,
            window_generation,
        } = plan;
        let mut history = ContextManager::new();
        if let Some(index) = base_replacement_history_index {
            let Some(RolloutItem::Compacted(compacted)) = rollout_items.get(index) else {
                unreachable!("rollout reconstruction base must be a compaction");
            };
            let Some(replacement_history) = &compacted.replacement_history else {
                unreachable!("rollout reconstruction base must have replacement history");
            };
            history.replace(replacement_history.clone());
        }
        let rollout_suffix_start_index =
            base_replacement_history_index.map_or(0, |index| index + 1);
        let rollout_suffix = &rollout_items[rollout_suffix_start_index..];

        let mut saw_legacy_compaction_without_replacement_history = false;
        // Materialize exact history semantics from the replay-derived suffix. The eventual lazy
        // design should keep this same replay shape, but drive it from a resumable reverse source
        // instead of an eagerly loaded `&[RolloutItem]`.
        for item in rollout_suffix {
            match item {
                RolloutItem::ResponseItem(response_item) => {
                    history.record_items(std::iter::once(response_item), truncation_policy);
                }
                RolloutItem::Compacted(compacted) => {
                    if let Some(replacement_history) = &compacted.replacement_history {
                        history.replace(replacement_history.clone());
                    } else {
                        saw_legacy_compaction_without_replacement_history = true;
                        // Legacy rollouts without `replacement_history` should rebuild the
                        // historical TurnContext at the correct insertion point from persisted
                        // `TurnContextItem`s. These are rare enough that we currently just clear
                        // `reference_context_item`, reinject canonical context at the end of the
                        // resumed conversation, and accept the temporary out-of-distribution
                        // prompt shape.
                        // TODO(ccunningham): if we drop support for None replacement_history compaction items,
                        // we can get rid of this second loop entirely and just build `history` directly in the first loop.
                        let user_messages = collect_user_messages(history.raw_items());
                        let rebuilt = compact::build_compacted_history(
                            Vec::new(),
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

        let reference_context_item = if saw_legacy_compaction_without_replacement_history {
            None
        } else {
            reference_context_item
        };

        RolloutReconstruction {
            history: history.raw_items().to_vec(),
            previous_turn_settings,
            reference_context_item,
            window_generation,
        }
    }

    pub(super) async fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> RolloutReconstruction {
        let plan = Self::reconstruct_rollout_plan(rollout_items);
        Self::materialize_rollout_reconstruction(
            turn_context.truncation_policy,
            rollout_items,
            plan,
        )
    }
}
