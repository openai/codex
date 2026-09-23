//! Decides which legacy records remain visible after historical rollback.
//!
//! Legacy rollback removes logical instruction turns, not a physical suffix of the rollout file.
//! Most records happen to be ordered that way, but late completion events can target an older
//! surviving turn after a newer turn has started. This planner keeps compact per-record ownership
//! metadata for SQLite visibility, then combines it with `rollback_replay`'s cold-resume answer
//! before the writer makes its second streaming pass. Retained checkpoints are reduced when
//! rollback is encountered, never by reapplying old rollbacks to later checkpoints.

use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::protocol::UserMessageImageKind;
use codex_rollout::CompactedItem;
use codex_rollout::RetainedContextEntry;
use codex_rollout::RetainedInputSource;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;

use super::migration_error;
use super::rollback;
use super::rollback_replay::ModelReplayPlanner;
use crate::ThreadStoreResult;

#[derive(Clone)]
struct CompactionFrame {
    record_index: usize,
    boundary_depth: usize,
    owner: Option<usize>,
    item: CompactedItem,
}

#[derive(Clone)]
struct PendingUserResponse {
    boundary: usize,
    content: Vec<ContentItem>,
}

struct InstructionBoundary {
    record_index: usize,
    message_id: Option<ResponseItemId>,
    input_source: RetainedInputSource,
    alive: bool,
}

struct RetainedFactSource {
    record_index: usize,
    turn_id: String,
    acceptance_order: Option<u64>,
}

/// Compact plan keyed by parsed source-record index.
pub(super) struct RollbackPlan {
    record_boundaries: Vec<Option<usize>>,
    boundary_alive: Vec<bool>,
    compacted_items: HashMap<usize, CompactedItem>,
}

impl RollbackPlan {
    pub(super) fn record_count(&self) -> usize {
        self.record_boundaries.len()
    }

    pub(super) fn apply(
        &self,
        record_index: usize,
        mut line: RolloutLine,
    ) -> ThreadStoreResult<Option<RolloutLine>> {
        let boundary = self
            .record_boundaries
            .get(record_index)
            .ok_or_else(|| migration_error("rollback plan is shorter than source replay"))?;
        if matches!(
            &line.item,
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))
        ) {
            return Ok(None);
        }
        // A rolled-back turn can still own the empty checkpoint that keeps cold resume from
        // replaying older history.
        if let Some(compacted) = self.compacted_items.get(&record_index) {
            line.item = RolloutItem::Compacted(compacted.clone());
            return Ok(Some(line));
        }
        if boundary.is_some_and(|boundary| !self.boundary_alive[boundary]) {
            return Ok(None);
        }
        Ok(Some(line))
    }
}

/// Streaming builder for RollbackPlan.
pub(super) struct RollbackPlanner {
    record_boundaries: Vec<Option<usize>>,
    boundaries: Vec<InstructionBoundary>,
    boundary_stack: Vec<usize>,
    active_turn_id: Option<String>,
    pending_turn_records: Vec<usize>,
    pending_context_records: Vec<usize>,
    pending_user_response: Option<PendingUserResponse>,
    pending_delivery_boundary: Option<usize>,
    turn_boundaries: HashMap<String, usize>,
    call_boundaries: HashMap<(String, String), Option<usize>>,
    retained_fact_sources: Vec<RetainedFactSource>,
    compactions: Vec<CompactionFrame>,
    model_replay: ModelReplayPlanner,
}

impl RollbackPlanner {
    pub(super) fn new() -> Self {
        Self {
            record_boundaries: Vec::new(),
            boundaries: Vec::new(),
            boundary_stack: Vec::new(),
            active_turn_id: None,
            pending_turn_records: Vec::new(),
            pending_context_records: Vec::new(),
            pending_user_response: None,
            pending_delivery_boundary: None,
            turn_boundaries: HashMap::new(),
            call_boundaries: HashMap::new(),
            retained_fact_sources: Vec::new(),
            compactions: Vec::new(),
            model_replay: ModelReplayPlanner::new(),
        }
    }

    pub(super) fn observe(&mut self, line: &RolloutLine) -> ThreadStoreResult<()> {
        let index = self.record_boundaries.len();
        self.model_replay.observe(index, &line.item);
        self.record_boundaries
            .push(self.boundary_stack.last().copied());
        let paired_user_boundary = match (&self.pending_user_response, &line.item) {
            (Some(pending), RolloutItem::EventMsg(EventMsg::UserMessage(event)))
                if user_response_matches_event(&pending.content, event) =>
            {
                Some(pending.boundary)
            }
            _ => None,
        };
        let paired_delivery_boundary = match (&self.pending_delivery_boundary, &line.item) {
            (Some(boundary), RolloutItem::ResponseItem(response))
                if matches!(&response.item, ResponseItem::AgentMessage { .. }) =>
            {
                Some(*boundary)
            }
            _ => None,
        };
        self.pending_user_response = None;
        self.pending_delivery_boundary = None;

        match &line.item {
            RolloutItem::SessionMeta(_) => self.record_boundaries[index] = None,
            RolloutItem::ResponseItem(response) => {
                if matches!(&response.item, ResponseItem::Message { role, .. } if role == "assistant")
                    || matches!(&response.item, ResponseItem::FunctionCall { .. })
                {
                    self.retained_fact_sources.push(RetainedFactSource {
                        record_index: index,
                        turn_id: response.turn_id().unwrap_or_default().to_owned(),
                        acceptance_order: response
                            .metadata
                            .as_ref()
                            .filter(|metadata| !metadata.inherited_user_message)
                            .and_then(|metadata| metadata.user_input_order),
                    });
                }
                if let Some(boundary) = paired_delivery_boundary {
                    self.record_boundaries[index] = Some(boundary);
                    self.boundaries[boundary].message_id = response.id().cloned();
                } else if rollback::counts_as_boundary(&response.item) {
                    let boundary = self.start_boundary(index);
                    self.boundaries[boundary].message_id = response.id().cloned();
                    self.boundaries[boundary].input_source = response.metadata.as_ref().into();
                    if let ResponseItem::Message { role, content, .. } = &response.item
                        && role == "user"
                    {
                        self.pending_user_response = Some(PendingUserResponse {
                            boundary,
                            content: content.clone(),
                        });
                    }
                } else if rollback::is_pre_turn_context_update(&response.item) {
                    // Until another user boundary arrives, this is trailing context for the
                    // previous turn. Keep that fallback owner so rollback drops it when there is
                    // no later turn to attach it to.
                    self.pending_context_records.push(index);
                }
                if let ResponseItem::FunctionCall { call_id, .. } = &response.item
                    && let Some(turn_id) = response.turn_id().or(self.active_turn_id.as_deref())
                {
                    self.call_boundaries.insert(
                        (turn_id.to_owned(), call_id.clone()),
                        self.record_boundaries[index],
                    );
                }
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                self.apply_rollback(rollback.num_turns)?;
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                self.active_turn_id = Some(event.turn_id.clone());
                self.pending_turn_records.clear();
                self.pending_turn_records.push(index);
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                self.assign_targeted_record(index, Some(event.turn_id.as_str()));
                if self.active_turn_id.as_deref() == Some(event.turn_id.as_str()) {
                    self.active_turn_id = None;
                    self.pending_turn_records.clear();
                }
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => {
                self.assign_targeted_record(index, event.turn_id.as_deref());
                if event
                    .turn_id
                    .as_deref()
                    .is_some_and(|turn_id| self.active_turn_id.as_deref() == Some(turn_id))
                {
                    self.active_turn_id = None;
                    self.pending_turn_records.clear();
                }
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(_)) => {
                let boundary = paired_user_boundary.unwrap_or_else(|| self.start_boundary(index));
                self.record_boundaries[index] = Some(boundary);
            }
            RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) => {
                self.assign_targeted_record(index, Some(event.turn_id.as_str()));
            }
            RolloutItem::EventMsg(event) => {
                self.assign_targeted_record(index, explicit_event_turn_id(event));
            }
            RolloutItem::InterAgentCommunication(_) => {
                self.start_boundary(index);
            }
            RolloutItem::InterAgentCommunicationMetadata { .. } => {
                let boundary = self.start_boundary(index);
                self.pending_delivery_boundary = Some(boundary);
            }
            RolloutItem::Compacted(item) => {
                let owner = self
                    .active_turn_id
                    .as_deref()
                    .and_then(|turn_id| self.turn_boundaries.get(turn_id).copied());
                self.record_boundaries[index] = owner;
                self.compactions.push(CompactionFrame {
                    record_index: index,
                    boundary_depth: self.boundary_stack.len(),
                    owner,
                    item: item.clone(),
                });
            }
            RolloutItem::TurnContext(_) => {
                if self.active_turn_id.is_some()
                    && self
                        .active_turn_id
                        .as_deref()
                        .is_none_or(|turn_id| !self.turn_boundaries.contains_key(turn_id))
                {
                    self.pending_turn_records.push(index);
                }
            }
            RolloutItem::TokenUsageRecord(record) => {
                self.assign_targeted_record(index, Some(record.turn_id.as_str()));
            }
            RolloutItem::WorldState(_) | RolloutItem::RealtimeItem(_) => {}
            RolloutItem::RetainedContext(codex_rollout::RetainedContextEvent::VerifiedAnswer {
                answer,
                acceptance_order,
            }) => {
                let source = (answer.turn_id.clone(), answer.call_id.clone());
                // A late answer still belongs to its call's instruction boundary, even
                // if the same running turn has since received another user steer.
                if let Some(boundary) = self.call_boundaries.get(&source) {
                    self.record_boundaries[index] = *boundary;
                } else {
                    self.assign_targeted_record(index, Some(&answer.turn_id));
                }
                self.call_boundaries
                    .insert(source, self.record_boundaries[index]);
                self.retained_fact_sources.push(RetainedFactSource {
                    record_index: index,
                    turn_id: answer.turn_id.clone(),
                    acceptance_order: *acceptance_order,
                });
            }
            RolloutItem::SecurityRiskScore(_) => self.record_boundaries[index] = None,
        }

        Ok(())
    }

    pub(super) fn finish(self) -> RollbackPlan {
        let RollbackPlanner {
            record_boundaries,
            boundaries,
            compactions,
            model_replay,
            ..
        } = self;
        let replay_anchor = model_replay.finish().empty_replacement_history_compaction;
        let boundary_alive = boundaries
            .into_iter()
            .map(|boundary| boundary.alive)
            .collect::<Vec<_>>();
        let compacted_items = compactions
            .into_iter()
            .filter_map(|mut frame| {
                if Some(frame.record_index) == replay_anchor {
                    frame.item.replacement_history = Some(Vec::new());
                    frame.item.mcp_resource_origins = None;
                    return Some((frame.record_index, frame.item));
                }
                frame
                    .owner
                    .is_none_or(|boundary| boundary_alive[boundary])
                    .then_some((frame.record_index, frame.item))
            })
            .collect::<HashMap<_, _>>();
        RollbackPlan {
            record_boundaries,
            boundary_alive,
            compacted_items,
        }
    }

    fn start_boundary(&mut self, index: usize) -> usize {
        let boundary = self.boundaries.len();
        self.boundaries.push(InstructionBoundary {
            record_index: index,
            message_id: None,
            input_source: RetainedInputSource::Local(None),
            alive: true,
        });
        let had_prior_boundary = !self.boundary_stack.is_empty();
        if had_prior_boundary {
            for pending_index in self.pending_context_records.drain(..) {
                self.record_boundaries[pending_index] = Some(boundary);
            }
        } else {
            self.pending_context_records.clear();
        }
        for pending_index in self.pending_turn_records.drain(..) {
            self.record_boundaries[pending_index] = Some(boundary);
        }
        self.record_boundaries[index] = Some(boundary);
        self.boundary_stack.push(boundary);
        self.bind_active_turn(boundary);
        boundary
    }

    fn bind_active_turn(&mut self, boundary: usize) {
        if let Some(turn_id) = self.active_turn_id.as_ref() {
            self.turn_boundaries.insert(turn_id.clone(), boundary);
        }
    }

    fn assign_targeted_record(&mut self, index: usize, turn_id: Option<&str>) {
        if let Some(boundary) = turn_id.and_then(|turn_id| self.turn_boundaries.get(turn_id)) {
            self.record_boundaries[index] = Some(*boundary);
        } else if self.active_turn_id.is_some()
            && self
                .active_turn_id
                .as_deref()
                .is_none_or(|turn_id| !self.turn_boundaries.contains_key(turn_id))
        {
            self.pending_turn_records.push(index);
        }
    }

    fn apply_rollback(&mut self, num_turns: u32) -> ThreadStoreResult<()> {
        let count = usize::try_from(num_turns).unwrap_or(usize::MAX);
        if count == 0 {
            return Ok(());
        }
        let depth_before = self.boundary_stack.len();
        let mut removed_boundaries = HashSet::new();
        let mut first_removed_boundary = None;
        for _ in 0..count {
            let Some(boundary) = self.boundary_stack.pop() else {
                break;
            };
            self.boundaries[boundary].alive = false;
            removed_boundaries.insert(boundary);
            first_removed_boundary = Some(boundary);
        }
        if let Some(boundary) = first_removed_boundary {
            let source = &self.boundaries[boundary];
            let acceptance_order = source.input_source.acceptance_order();
            if let Some(order) = acceptance_order {
                // An answer may have been persisted before the queued instruction
                // accepted ahead of it. Both are removed at that acceptance boundary.
                for fact in &self.retained_fact_sources {
                    if fact
                        .acceptance_order
                        .is_some_and(|accepted| accepted >= order)
                    {
                        self.record_boundaries[fact.record_index] = Some(boundary);
                    }
                }
            }
            let removed_turns = self
                .turn_boundaries
                .iter()
                .filter(|(_, boundary)| removed_boundaries.contains(*boundary))
                .map(|(turn_id, _)| turn_id.as_str())
                .chain(
                    self.retained_fact_sources
                        .iter()
                        .filter(|fact| {
                            self.record_boundaries[fact.record_index]
                                .is_some_and(|boundary| removed_boundaries.contains(&boundary))
                        })
                        .map(|fact| fact.turn_id.as_str()),
                )
                .collect::<Vec<_>>();
            // Accepted answers can precede a queued instruction in the rollout,
            // including in checkpoints. Legacy evidence uses the recorded boundary.
            // Later checkpoints have not been observed yet and already reflect this rollback.
            for frame in self.compactions.iter_mut().rev().take_while(|frame| {
                acceptance_order.is_some() || frame.record_index >= source.record_index
            }) {
                if let Some(context) = &mut frame.item.retained_context {
                    if acceptance_order.is_some()
                        || context
                            .ordered_entries()
                            .any(|(_, entry)| matches!(entry, RetainedContextEntry::UserMessage(_)))
                    {
                        context.rollback(
                            &removed_turns,
                            source.message_id.as_ref().map(ResponseItemId::as_str),
                            source.input_source,
                        );
                    } else {
                        // Checkpoints written without instruction retention keep the legacy
                        // source-call boundary, including answers before a same-turn steer.
                        context.retain_answers(|answer| {
                            self.call_boundaries
                                .get(&(answer.turn_id.clone(), answer.call_id.clone()))
                                .map_or_else(
                                    || !removed_turns.contains(&answer.turn_id.as_str()),
                                    |boundary| {
                                        boundary
                                            .is_none_or(|boundary| self.boundaries[boundary].alive)
                                    },
                                )
                        });
                    }
                }
            }
        }
        let compaction_index = self.compactions.iter().rposition(|frame| {
            frame
                .owner
                .is_none_or(|boundary| self.boundaries[boundary].alive)
        });
        if let Some(compaction_index) = compaction_index {
            let frame = &mut self.compactions[compaction_index];
            let post_compaction_turns = depth_before.saturating_sub(frame.boundary_depth);
            let remaining = count.saturating_sub(post_compaction_turns);
            if remaining > 0 {
                frame.item.mcp_resource_origins = None;
                let replacement_history =
                    frame.item.replacement_history.as_mut().ok_or_else(|| {
                        migration_error(
                            "legacy rollback crosses a compaction without replacement history",
                        )
                    })?;
                rollback::drop_last_n_user_turns(
                    replacement_history,
                    u32::try_from(remaining).unwrap_or(u32::MAX),
                );
            }
        }
        self.active_turn_id = None;
        self.pending_turn_records.clear();
        self.pending_context_records.clear();
        self.pending_user_response = None;
        self.pending_delivery_boundary = None;
        Ok(())
    }
}

fn explicit_event_turn_id(event: &EventMsg) -> Option<&str> {
    match event {
        EventMsg::ExecCommandEnd(event) => Some(event.turn_id.as_str()),
        EventMsg::PatchApplyEnd(event) => Some(event.turn_id.as_str()),
        EventMsg::DynamicToolCallResponse(event) => Some(event.turn_id.as_str()),
        EventMsg::EnteredReviewMode(event) => event.turn_id.as_deref(),
        EventMsg::ExitedReviewMode(event) => event.turn_id.as_deref(),
        _ => None,
    }
    .filter(|turn_id| !turn_id.is_empty())
}

fn user_response_matches_event(content: &[ContentItem], event: &UserMessageEvent) -> bool {
    let mut text = String::new();
    let mut images = Vec::new();
    let mut file_ids = Vec::new();
    let mut image_order = Vec::new();
    let mut audio = Vec::new();
    for item in content {
        match item {
            ContentItem::InputText { text: item_text } => text.push_str(item_text),
            ContentItem::InputImage { image, .. } => match image {
                ImageReference::Inline { image_url } => {
                    image_order.push(UserMessageImageKind::Inline);
                    images.push(image_url.as_str());
                }
                ImageReference::File { file_id } => {
                    image_order.push(UserMessageImageKind::File);
                    file_ids.push(file_id.as_str());
                }
            },
            ContentItem::InputAudio { audio_url } => audio.push(audio_url.as_str()),
            ContentItem::OutputText { .. } => return false,
        }
    }
    text == event.message
        && (!event.has_complete_image_order() || image_order == event.image_order)
        && images
            == event
                .images
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        && file_ids
            == event
                .file_ids
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        && audio
            == event
                .audio
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        && event.local_images.is_empty()
        && event.local_audio.is_empty()
}
