use super::*;

#[derive(Debug)]
pub(super) struct RolloutReconstruction {
    pub(super) history: Vec<ResponseItem>,
    pub(super) previous_model: Option<String>,
    pub(super) reference_context_item: Option<TurnContextItem>,
}

#[derive(Debug)]
struct HistoryCheckpoint {
    prefix_len: usize,
    replacement_history: Option<Vec<ResponseItem>>,
    message: String,
}

#[derive(Debug, Default)]
struct ReverseHistoryCollector {
    rollback_user_turns_to_skip: usize,
    kept_items_rev: Vec<ResponseItem>,
    pending_items_rev: Vec<ResponseItem>,
    pending_keep_start: usize,
}

impl ReverseHistoryCollector {
    fn record_response_item(&mut self, item: &ResponseItem) {
        if self.rollback_user_turns_to_skip == 0 {
            self.kept_items_rev.push(item.clone());
            return;
        }

        self.pending_items_rev.push(item.clone());
        if matches!(item, ResponseItem::Message { role, .. } if role == "user") {
            self.rollback_user_turns_to_skip -= 1;
            self.pending_keep_start = self.pending_items_rev.len();
            if self.rollback_user_turns_to_skip == 0 {
                self.pending_items_rev.clear();
                self.pending_keep_start = 0;
            }
        }
    }

    fn record_rollback(&mut self, num_turns: u32) {
        self.rollback_user_turns_to_skip = self
            .rollback_user_turns_to_skip
            .saturating_add(usize::try_from(num_turns).unwrap_or(usize::MAX));
    }

    fn finish_with_base_history(
        mut self,
        base_history: Vec<ResponseItem>,
        truncation_policy: TruncationPolicy,
    ) -> Vec<ResponseItem> {
        for item in base_history.iter().rev() {
            self.record_response_item(item);
        }

        let mut surviving_items = if self.rollback_user_turns_to_skip > 0 {
            self.pending_items_rev[self.pending_keep_start..]
                .iter()
                .rev()
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        surviving_items.extend(self.kept_items_rev.into_iter().rev());

        let mut history = ContextManager::new();
        history.record_items(surviving_items.iter(), truncation_policy);
        history.raw_items().to_vec()
    }
}

#[derive(Debug, Default)]
struct ReverseMetadataTurn {
    turn_id: Option<String>,
    saw_user_message: bool,
    previous_model: Option<String>,
    reference_context_item: Option<TurnContextItem>,
    reference_context_item_cleared: bool,
}

#[derive(Debug, Default)]
struct ReverseMetadataState {
    rollback_user_turns_to_skip: usize,
    active_turn: Option<ReverseMetadataTurn>,
    previous_model: Option<String>,
    reference_context_item: Option<TurnContextItem>,
    reference_context_item_cleared: bool,
}

impl ReverseMetadataState {
    fn record_rollback(&mut self, num_turns: u32) {
        self.rollback_user_turns_to_skip = self
            .rollback_user_turns_to_skip
            .saturating_add(usize::try_from(num_turns).unwrap_or(usize::MAX));
    }

    fn record_turn_end(&mut self, turn_id: Option<&str>) {
        match (&mut self.active_turn, turn_id) {
            (None, Some(turn_id)) => {
                self.active_turn = Some(ReverseMetadataTurn {
                    turn_id: Some(turn_id.to_string()),
                    ..Default::default()
                });
            }
            (None, None) => {
                self.active_turn = Some(ReverseMetadataTurn::default());
            }
            (Some(active_turn), Some(turn_id)) if active_turn.turn_id.is_none() => {
                active_turn.turn_id = Some(turn_id.to_string());
            }
            (Some(active_turn), Some(turn_id))
                if active_turn.turn_id.as_deref() == Some(turn_id) => {}
            (Some(_), Some(_)) => {
                // Ignore unmatched end markers for some other turn; they should not consume the
                // newer turn we are currently walking backwards through.
            }
            (Some(_), None) => {}
        }
    }

    fn record_turn_start(&mut self, turn_id: &str) {
        if self
            .active_turn
            .as_ref()
            .is_some_and(|turn| turn.turn_id.as_deref().is_none_or(|id| id == turn_id))
            && let Some(turn) = self.active_turn.take()
        {
            self.finalize_turn(turn);
        }
    }

    fn ensure_active_turn(&mut self) -> &mut ReverseMetadataTurn {
        self.active_turn
            .get_or_insert_with(ReverseMetadataTurn::default)
    }

    fn record_user_message(&mut self) {
        self.ensure_active_turn().saw_user_message = true;
    }

    fn record_turn_context(&mut self, ctx: &TurnContextItem) {
        if self.active_turn.is_none() {
            let turn = self.ensure_active_turn();
            turn.saw_user_message = true;
            turn.previous_model = Some(ctx.model.clone());
            turn.reference_context_item = Some(ctx.clone());
            turn.reference_context_item_cleared = false;
            return;
        }

        if self.active_turn.as_ref().is_some_and(|turn| {
            turn.turn_id.as_deref().is_none_or(|turn_id| {
                ctx.turn_id
                    .as_deref()
                    .is_none_or(|ctx_turn_id| ctx_turn_id == turn_id)
            })
        }) {
            let turn = self.ensure_active_turn();
            if turn.turn_id.is_none() {
                turn.saw_user_message = true;
            }
            if turn.previous_model.is_none() {
                turn.previous_model = Some(ctx.model.clone());
            }
            if !turn.reference_context_item_cleared && turn.reference_context_item.is_none() {
                turn.reference_context_item = Some(ctx.clone());
            }
            return;
        }

        self.finalize_turn(ReverseMetadataTurn {
            turn_id: ctx.turn_id.clone(),
            saw_user_message: true,
            previous_model: Some(ctx.model.clone()),
            reference_context_item: Some(ctx.clone()),
            reference_context_item_cleared: false,
        });
    }

    fn record_compaction(&mut self) {
        let turn = self.ensure_active_turn();
        if turn.reference_context_item.is_none() {
            turn.reference_context_item_cleared = true;
        }
    }

    fn finalize_turn(&mut self, turn: ReverseMetadataTurn) {
        if turn.saw_user_message {
            if self.rollback_user_turns_to_skip > 0 {
                self.rollback_user_turns_to_skip -= 1;
                return;
            }

            if turn.reference_context_item_cleared {
                self.reference_context_item = None;
                self.reference_context_item_cleared = true;
            }

            if self.previous_model.is_none()
                && let Some(previous_model) = turn.previous_model
            {
                self.previous_model = Some(previous_model);
                if !self.reference_context_item_cleared {
                    self.reference_context_item = turn.reference_context_item;
                }
            }
        } else if turn.reference_context_item_cleared {
            self.reference_context_item = None;
            self.reference_context_item_cleared = true;
        }
    }

    fn finish(mut self) -> (Option<String>, Option<TurnContextItem>) {
        if let Some(turn) = self.active_turn.take() {
            self.finalize_turn(turn);
        }
        (self.previous_model, self.reference_context_item)
    }

    fn resolved_previous_model(&self) -> bool {
        self.previous_model.is_some()
    }
}

#[derive(Debug)]
struct TailScan {
    history_collector: ReverseHistoryCollector,
    history_checkpoint: Option<HistoryCheckpoint>,
    previous_model: Option<String>,
    reference_context_item: Option<TurnContextItem>,
}

fn scan_rollout_tail(rollout_items: &[RolloutItem]) -> TailScan {
    let mut history_collector = ReverseHistoryCollector::default();
    let mut history_checkpoint = None;
    let mut metadata = ReverseMetadataState::default();

    for (index, item) in rollout_items.iter().enumerate().rev() {
        match item {
            RolloutItem::ResponseItem(response_item) => {
                if history_checkpoint.is_none() {
                    history_collector.record_response_item(response_item);
                }
            }
            RolloutItem::Compacted(compacted) => {
                if history_checkpoint.is_none() {
                    history_checkpoint = Some(HistoryCheckpoint {
                        prefix_len: index,
                        replacement_history: compacted.replacement_history.clone(),
                        message: compacted.message.clone(),
                    });
                }
                metadata.record_compaction();
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                if history_checkpoint.is_none() {
                    history_collector.record_rollback(rollback.num_turns);
                }
                metadata.record_rollback(rollback.num_turns);
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                metadata.record_turn_end(Some(&event.turn_id));
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                metadata.record_turn_end(event.turn_id.as_deref());
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                metadata.record_turn_start(&event.turn_id);
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                metadata.record_user_message();
            }
            RolloutItem::TurnContext(ctx) => {
                metadata.record_turn_context(ctx);
            }
            _ => {}
        }

        if history_checkpoint.is_some() && metadata.resolved_previous_model() {
            break;
        }
    }

    let (previous_model, reference_context_item) = metadata.finish();
    TailScan {
        history_collector,
        history_checkpoint,
        previous_model,
        reference_context_item,
    }
}

fn reconstruct_history_from_tail_scan(
    initial_context: &[ResponseItem],
    truncation_policy: TruncationPolicy,
    rollout_items: &[RolloutItem],
    tail_scan: TailScan,
) -> Vec<ResponseItem> {
    let base_history = match tail_scan.history_checkpoint {
        Some(HistoryCheckpoint {
            prefix_len: _,
            replacement_history: Some(replacement_history),
            ..
        }) => replacement_history,
        Some(HistoryCheckpoint {
            prefix_len,
            replacement_history: None,
            message,
        }) => {
            let history_before = reconstruct_history_from_rollout_items(
                initial_context,
                truncation_policy,
                &rollout_items[..prefix_len],
            );
            let user_messages = collect_user_messages(&history_before);
            compact::build_compacted_history(initial_context.to_vec(), &user_messages, &message)
        }
        None => Vec::new(),
    };

    tail_scan
        .history_collector
        .finish_with_base_history(base_history, truncation_policy)
}

fn reconstruct_history_from_rollout_items(
    initial_context: &[ResponseItem],
    truncation_policy: TruncationPolicy,
    rollout_items: &[RolloutItem],
) -> Vec<ResponseItem> {
    let tail_scan = scan_rollout_tail(rollout_items);
    reconstruct_history_from_tail_scan(initial_context, truncation_policy, rollout_items, tail_scan)
}

impl Session {
    pub(super) async fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> RolloutReconstruction {
        // Read the rollout from the tail inward.
        //
        // The reverse scan does two things at once:
        // - resolve resume metadata from the newest surviving user turn after applying
        //   `ThreadRolledBack` as a simple "skip N user turns" counter
        // - capture the raw history suffix after the newest `Compacted` checkpoint
        //
        // Once that tail scan finds a `Compacted` item, older raw `ResponseItem`s no longer need
        // to be read directly. `replacement_history: Some(...)` already contains the full base
        // snapshot at that point, while `replacement_history: None` rebuilds that snapshot by
        // recursively reconstructing the rollout prefix before the compaction and passing its user
        // messages into `build_compacted_history`.
        //
        // This keeps replay aligned with the eventual reverse-file reader we want to build: the
        // tail scan identifies the newest surviving baseline information and the newest history
        // checkpoint, then recursive prefix rebuild handles only the compacted prefix when needed.
        let initial_context = self.build_initial_context(turn_context).await;
        let tail_scan = scan_rollout_tail(rollout_items);
        let previous_model = tail_scan.previous_model.clone();
        let reference_context_item = tail_scan.reference_context_item.clone();
        let history = reconstruct_history_from_tail_scan(
            &initial_context,
            turn_context.truncation_policy,
            rollout_items,
            tail_scan,
        );

        RolloutReconstruction {
            history,
            previous_model,
            reference_context_item,
        }
    }
}
