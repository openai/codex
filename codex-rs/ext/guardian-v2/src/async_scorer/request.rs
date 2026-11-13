//! Admits classifier requests before moving committed history, without advancing the cursor.
//! The complete immutable prefix, setup and new evidence share the classifier input limit.
//! Instruction and sync-review delivery metadata stay with history. Previously delivered
//! evidence is omitted before budgeting. Elastic allocation selects only the new evidence,
//! after accounting for setup, checkpoint and the complete immutable prefix.

use codex_context_fragments::ContextualUserFragment;
use codex_core::context::GuardianBudgetOmission;
use codex_guardian_context::RequestBudget;
use codex_guardian_context::ReservedElasticItemIds;
use codex_guardian_context::TranscriptCursor;
use codex_guardian_context::TruncationObservation;
use codex_guardian_context::estimate_input_tokens;
use codex_guardian_reviewer::ConversationState;
use codex_history::ResponseItemEnvelope;
use std::borrow::Cow;

use super::sampler::INPUT_TOKEN_MARGIN;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplingRequest;
use super::transcript::CollectedTranscript;

impl LunaSamplingRequest {
    /// Takes committed history only after admission; rejection leaves the checkpoint intact.
    pub(super) fn prepare_retained(
        &self,
        evidence: &CollectedTranscript,
        conversation: &mut ConversationState<Vec<ResponseItemEnvelope>>,
        max_input_tokens: usize,
    ) -> Option<PreparedRequest> {
        let cursor = conversation.cursor();
        if conversation.snapshot().is_some() != cursor.is_some() {
            return None;
        }
        let (_, next_cursor) = evidence.select(cursor);
        if cursor.is_some_and(|cursor| {
            cursor.parent_history_version != next_cursor.parent_history_version
                || cursor.transcript_entry_count > next_cursor.transcript_entry_count
        }) {
            return None;
        }
        let history = conversation
            .snapshot()
            .map(|snapshot| snapshot.history().as_slice())
            .unwrap_or_default();
        let setup = if conversation.snapshot().is_none() {
            LunaSampler::prepare_setup(self)
        } else {
            Vec::new()
        };
        let (mut context, _) = evidence.compose(cursor, history).ok()?;
        if evidence.elastic.is_none() {
            context.retain_new_instructions(history);
        }
        context.retain_new_images(history.iter().map(|item| &item.item), |image, detail| {
            *detail = None;
            Some(Cow::Borrowed(image))
        });
        let existing_context_tokens = history
            .iter()
            .map(|item| estimate_input_tokens(&item.item))
            .fold(0usize, usize::saturating_add);
        let setup_tokens = setup
            .iter()
            .map(|item| estimate_input_tokens(&item.item))
            .fold(0usize, usize::saturating_add);
        let maximum = max_input_tokens.saturating_sub(INPUT_TOKEN_MARGIN);
        let ids = if let Some(policy) = &evidence.elastic {
            let prepared = context
                .prepare_elastic_entries(policy)
                .ok()?
                .prepare_delivery(history)
                .ok()?;
            let ids = ReservedElasticItemIds::reserve(&prepared, &self.item_id_generator);
            context = prepared
                .enforce_budget(
                    RequestBudget {
                        max_input_tokens: maximum,
                        existing_context_tokens: existing_context_tokens
                            .saturating_add(setup_tokens)
                            .saturating_add(ids.tokens()),
                    },
                    GuardianBudgetOmission
                        .render()
                        .into_text()
                        .unwrap_or_default(),
                )
                .ok()?;
            Some(ids)
        } else {
            None
        };
        let truncations = std::mem::take(&mut context.truncations);
        let section_costs = context.section_costs().collect();
        let mut suffix = context.into_annotated_messages();
        if let Some(ids) = ids {
            ids.assign(&mut suffix).ok()?;
        }
        let (mut input, new_input_tokens) = LunaSampler::finish_input(self, setup, suffix);
        let input_tokens = existing_context_tokens.saturating_add(new_input_tokens);
        if input_tokens > maximum {
            return None;
        }
        if let Some(mut history) = conversation.take_history() {
            history.append(&mut input);
            input = history;
        }
        Some(PreparedRequest {
            input,
            cursor: next_cursor,
            truncations,
            section_costs,
            existing_context_tokens,
            input_tokens,
        })
    }
}

/// Admitted input and accounting for one retained review.
pub(super) struct PreparedRequest {
    pub(super) input: Vec<ResponseItemEnvelope>,
    pub(super) cursor: TranscriptCursor,
    pub(super) truncations: Vec<TruncationObservation>,
    pub(super) section_costs: Vec<(&'static str, codex_guardian_context::SectionCost)>,
    pub(super) existing_context_tokens: usize,
    pub(super) input_tokens: usize,
}
