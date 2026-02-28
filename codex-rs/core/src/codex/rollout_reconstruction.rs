use super::*;

// Return value of `Session::reconstruct_history_from_rollout`, bundling the rebuilt history with
// the resume/fork hydration metadata derived from the same replay.
#[derive(Debug)]
pub(super) struct RolloutReconstruction {
    pub(super) history: Vec<ResponseItem>,
    pub(super) previous_model: Option<String>,
    pub(super) reference_context_item: Option<TurnContextItem>,
}

#[derive(Clone, Debug)]
struct InMemoryReverseRolloutSource {
    rollout_items: Arc<[RolloutItem]>,
    next_older_index: usize,
}

impl InMemoryReverseRolloutSource {
    fn new(rollout_items: Vec<RolloutItem>) -> Self {
        let rollout_items = Arc::<[RolloutItem]>::from(rollout_items);
        let next_older_index = rollout_items.len();
        Self {
            rollout_items,
            next_older_index,
        }
    }

    fn pop_older(&mut self) -> Option<RolloutItem> {
        if self.next_older_index == 0 {
            return None;
        }

        self.next_older_index -= 1;
        Some(self.rollout_items[self.next_older_index].clone())
    }
}

#[derive(Clone, Debug)]
enum HistoryBase {
    StartOfFile,
    // The current history view starts from a replacement-history checkpoint. The checkpoint
    // rollout items are not materialized while this base is active, but they stay in memory so
    // future backtracking can roll before the compacted turn without restarting replay.
    Replacement {
        history: Vec<ResponseItem>,
        checkpoint_segment: Vec<RolloutItem>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct RolloutReconstructionState {
    source: InMemoryReverseRolloutSource,
    // Loaded rollout items older than the current `history_base`. They are not materialized in
    // the current history view, but stay in memory so later backtracking can move the visible
    // boundary farther back without re-reading from the newest rollout items again.
    loaded_prefix: Vec<RolloutItem>,
    history_base: HistoryBase,
    // Loaded rollout items newer than the current `history_base`, in rollout order.
    rollout_suffix: Vec<RolloutItem>,
    previous_model: Option<String>,
    reference_context_item: Option<TurnContextItem>,
}

impl RolloutReconstructionState {
    pub(super) fn new(rollout_items: Vec<RolloutItem>) -> Self {
        let mut reconstruction_state = Self {
            source: InMemoryReverseRolloutSource::new(rollout_items),
            loaded_prefix: Vec::new(),
            history_base: HistoryBase::StartOfFile,
            rollout_suffix: Vec::new(),
            previous_model: None,
            reference_context_item: None,
        };
        reconstruction_state.rebuild(0);
        reconstruction_state
    }

    pub(super) fn apply_backtracking(&mut self, additional_user_turns: u32) {
        self.rebuild(additional_user_turns);
    }

    fn rebuild(&mut self, additional_user_turns: u32) {
        // Re-canonicalize the loaded replay state from the currently loaded window plus any older
        // rollout items we still need from the source. Additional rollback is applied here
        // directly, so the durable state never stores a "pending rollback count".
        let mut loaded_rollout_items = self.loaded_prefix.clone();
        if let HistoryBase::Replacement {
            checkpoint_segment, ..
        } = &self.history_base
        {
            loaded_rollout_items.extend(checkpoint_segment.iter().cloned());
        }
        loaded_rollout_items.extend(self.rollout_suffix.iter().cloned());

        let mut new_loaded_prefix_rev = Vec::new();
        let mut new_rollout_suffix_rev = Vec::new();
        let mut new_history_base = None;
        let mut previous_model = None;
        let mut reference_context_item = TurnReferenceContextItem::NeverSet;
        let mut pending_rollback_turns =
            usize::try_from(additional_user_turns).unwrap_or(usize::MAX);
        let mut active_segment: Option<ActiveReplaySegment> = None;

        loop {
            let next_item = if let Some(item) = loaded_rollout_items.pop() {
                Some(item)
            } else {
                self.source.pop_older()
            };

            let Some(item) = next_item else {
                break;
            };

            match &item {
                RolloutItem::SessionMeta(_) => {}
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                    // Historical rollback markers are applied eagerly while rebuilding state and
                    // are not retained in the canonical loaded window.
                    pending_rollback_turns = pending_rollback_turns
                        .saturating_add(usize::try_from(rollback.num_turns).unwrap_or(usize::MAX));
                }
                RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                    if active_segment.as_ref().is_some_and(|active_segment| {
                        turn_ids_are_compatible(
                            active_segment.turn_id.as_deref(),
                            Some(event.turn_id.as_str()),
                        )
                    }) {
                        if let Some(mut active_segment) = active_segment.take() {
                            active_segment.rollout_items_rev.push(item);
                            finalize_active_segment(
                                active_segment,
                                &mut new_history_base,
                                &mut new_rollout_suffix_rev,
                                &mut new_loaded_prefix_rev,
                                &mut previous_model,
                                &mut reference_context_item,
                                &mut pending_rollback_turns,
                            );
                        }
                    } else if new_history_base.is_some() {
                        new_loaded_prefix_rev.push(item);
                    } else {
                        new_rollout_suffix_rev.push(item);
                    }
                }
                RolloutItem::ResponseItem(_)
                | RolloutItem::Compacted(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::EventMsg(_) => {
                    let active_segment =
                        active_segment.get_or_insert_with(ActiveReplaySegment::default);
                    active_segment.rollout_items_rev.push(item.clone());
                    match &item {
                        RolloutItem::Compacted(compacted) => {
                            if matches!(
                                active_segment.reference_context_item,
                                TurnReferenceContextItem::NeverSet
                            ) {
                                active_segment.reference_context_item =
                                    TurnReferenceContextItem::Cleared;
                            }
                            if active_segment.base_replacement_history.is_none()
                                && let Some(replacement_history) = &compacted.replacement_history
                            {
                                active_segment.base_replacement_history =
                                    Some(replacement_history.clone());
                            }
                        }
                        RolloutItem::TurnContext(ctx) => {
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
                        RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                            if active_segment.turn_id.is_none() {
                                active_segment.turn_id = Some(event.turn_id.clone());
                            }
                        }
                        RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                            if active_segment.turn_id.is_none()
                                && let Some(turn_id) = &event.turn_id
                            {
                                active_segment.turn_id = Some(turn_id.clone());
                            }
                        }
                        RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                            active_segment.counts_as_user_turn = true;
                        }
                        RolloutItem::ResponseItem(_) | RolloutItem::EventMsg(_) => {}
                        RolloutItem::SessionMeta(_) => {
                            unreachable!(
                                "session meta and rollback events are handled outside active segments"
                            )
                        }
                    }
                }
            }

            // Once the already-loaded window has been consumed, the newest surviving history
            // checkpoint and eager resume metadata are stable. At that point older unread source
            // items cannot change the current state, so we can stop without loading them.
            if loaded_rollout_items.is_empty()
                && active_segment.is_none()
                && pending_rollback_turns == 0
                && new_history_base.is_some()
                && previous_model.is_some()
                && !matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
            {
                break;
            }
        }

        if let Some(active_segment) = active_segment.take() {
            finalize_active_segment(
                active_segment,
                &mut new_history_base,
                &mut new_rollout_suffix_rev,
                &mut new_loaded_prefix_rev,
                &mut previous_model,
                &mut reference_context_item,
                &mut pending_rollback_turns,
            );
        }

        let history_base = new_history_base.unwrap_or(HistoryBase::StartOfFile);
        let reference_context_item = match reference_context_item {
            TurnReferenceContextItem::NeverSet | TurnReferenceContextItem::Cleared => None,
            TurnReferenceContextItem::Latest(turn_reference_context_item) => {
                Some(*turn_reference_context_item)
            }
        };

        self.loaded_prefix = new_loaded_prefix_rev.into_iter().rev().collect();
        self.history_base = history_base;
        self.rollout_suffix = new_rollout_suffix_rev.into_iter().rev().collect();
        self.previous_model = previous_model;
        self.reference_context_item = reference_context_item;
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
    previous_model: Option<String>,
    reference_context_item: TurnReferenceContextItem,
    base_replacement_history: Option<Vec<ResponseItem>>,
    rollout_items_rev: Vec<RolloutItem>,
}

fn turn_ids_are_compatible(active_turn_id: Option<&str>, item_turn_id: Option<&str>) -> bool {
    active_turn_id
        .is_none_or(|turn_id| item_turn_id.is_none_or(|item_turn_id| item_turn_id == turn_id))
}

fn finalize_active_segment(
    active_segment: ActiveReplaySegment,
    history_base: &mut Option<HistoryBase>,
    rollout_suffix_rev: &mut Vec<RolloutItem>,
    loaded_prefix_rev: &mut Vec<RolloutItem>,
    previous_model: &mut Option<String>,
    reference_context_item: &mut TurnReferenceContextItem,
    pending_rollback_turns: &mut usize,
) {
    // Thread rollback drops the newest surviving real user-message boundaries. In replay, that
    // means skipping the next finalized segments that contain a non-contextual
    // `EventMsg::UserMessage`.
    if *pending_rollback_turns > 0 {
        if active_segment.counts_as_user_turn {
            *pending_rollback_turns -= 1;
        }
        return;
    }

    let ActiveReplaySegment {
        counts_as_user_turn,
        previous_model: segment_previous_model,
        reference_context_item: segment_reference_context_item,
        base_replacement_history,
        rollout_items_rev,
        ..
    } = active_segment;

    // `previous_model` comes from the newest surviving user turn that established one.
    if previous_model.is_none() && counts_as_user_turn {
        *previous_model = segment_previous_model;
    }

    // `reference_context_item` comes from the newest surviving user turn baseline, or
    // from a surviving compaction that explicitly cleared that baseline.
    if matches!(reference_context_item, TurnReferenceContextItem::NeverSet)
        && (counts_as_user_turn
            || matches!(
                segment_reference_context_item,
                TurnReferenceContextItem::Cleared
            ))
    {
        *reference_context_item = segment_reference_context_item;
    }

    if history_base.is_none()
        && let Some(replacement_history) = base_replacement_history
    {
        *history_base = Some(HistoryBase::Replacement {
            history: replacement_history,
            checkpoint_segment: rollout_items_rev.into_iter().rev().collect(),
        });
        return;
    }

    if history_base.is_some() {
        loaded_prefix_rev.extend(rollout_items_rev);
    } else {
        rollout_suffix_rev.extend(rollout_items_rev);
    }
}

impl Session {
    pub(super) async fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> RolloutReconstruction {
        let reconstruction_state = RolloutReconstructionState::new(rollout_items.to_vec());
        self.reconstruct_history_from_rollout_state(turn_context, &reconstruction_state)
            .await
    }

    pub(super) async fn reconstruct_history_from_rollout_state(
        &self,
        turn_context: &TurnContext,
        reconstruction_state: &RolloutReconstructionState,
    ) -> RolloutReconstruction {
        let mut history = ContextManager::new();
        let mut saw_legacy_compaction_without_replacement_history = false;

        match &reconstruction_state.history_base {
            HistoryBase::StartOfFile => {}
            HistoryBase::Replacement {
                history: replacement_history,
                ..
            } => {
                history.replace(replacement_history.clone());
            }
        }

        // Materialize the current history view from the replay-derived base plus the loaded raw
        // rollout suffix. The future lazy reader should keep this same semantic split, even when
        // the loaded items come from a resumable reverse source instead of an eager in-memory
        // source.
        for item in &reconstruction_state.rollout_suffix {
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
                        saw_legacy_compaction_without_replacement_history = true;
                        // Legacy rollouts without `replacement_history` should rebuild the
                        // historical TurnContext at the correct insertion point from persisted
                        // `TurnContextItem`s. These are rare enough that we currently just clear
                        // `reference_context_item`, reinject canonical context at the end of the
                        // resumed conversation, and accept the temporary out-of-distribution
                        // prompt shape.
                        // If we eventually drop support for None replacement_history compaction
                        // items, we can remove this legacy branch and build `history` directly in
                        // the first replay loop.
                        let user_messages = collect_user_messages(history.raw_items());
                        let rebuilt = compact::build_compacted_history(
                            Vec::new(),
                            &user_messages,
                            &compacted.message,
                        );
                        history.replace(rebuilt);
                    }
                }
                RolloutItem::EventMsg(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::SessionMeta(_) => {}
            }
        }

        let reference_context_item = if saw_legacy_compaction_without_replacement_history {
            None
        } else {
            reconstruction_state.reference_context_item.clone()
        };

        RolloutReconstruction {
            history: history.raw_items().to_vec(),
            previous_model: reconstruction_state.previous_model.clone(),
            reference_context_item,
        }
    }
}
