//! Admits classifier requests before moving committed history, without advancing the cursor.
//! The complete immutable prefix, setup and new evidence share the classifier input limit.
//! Instruction and sync-review delivery metadata stay with history. Previously delivered
//! evidence is omitted before budgeting, using the same instruction filtering as sync.

use codex_guardian_context::TranscriptCursor;
use codex_guardian_context::TruncationObservation;
use codex_guardian_context::estimate_input_tokens;
use codex_guardian_reviewer::ConversationCheckpoint;
use codex_guardian_reviewer::ConversationState;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

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
    ) -> Option<PreparedRequest<'_>> {
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
        let (mut context, _) = evidence.clone().compose(cursor, history).ok()?;
        context.retain_new_instructions(history);
        if let Some(snapshot) = conversation.snapshot() {
            context.retain_images(|image, _| {
                !snapshot.history().iter().any(|item| match &item.item {
                    ResponseItem::Message { content, .. } => content.iter().any(|item| {
                        matches!(item, ContentItem::InputImage { image: retained, .. } if retained == image)
                    }),
                    _ => false,
                })
            });
        }
        let truncations = std::mem::take(&mut context.truncations);
        let section_costs = context.section_costs().collect();
        let history = conversation.snapshot().map(ConversationCheckpoint::history);
        let existing_context_tokens = history.map_or(0, |history| {
            history
                .iter()
                .map(|item| estimate_input_tokens(&item.item))
                .fold(0usize, usize::saturating_add)
        });
        // A continuation already owns its setup. Normalize only the new suffix while
        // borrowing the immutable prefix for accounting; do not clone or rewrite it.
        let (mut input, new_input_tokens) = LunaSampler::prepare_input(
            self,
            history.map(|_| Vec::new()),
            context.into_annotated_messages(),
        );
        let input_tokens = existing_context_tokens.saturating_add(new_input_tokens);
        if input_tokens > max_input_tokens.saturating_sub(/*rhs*/ 256) {
            return None;
        }
        if let Some(mut history) = conversation.take_history() {
            history.append(&mut input);
            input = history;
        }
        Some(PreparedRequest {
            sampling: self,
            input,
            cursor: next_cursor,
            truncations,
            section_costs,
            existing_context_tokens,
            input_tokens,
        })
    }
}

/// Admitted input and its token count, tied to the sampling settings used to prepare it.
pub(super) struct PreparedRequest<'a> {
    pub(super) sampling: &'a LunaSamplingRequest,
    pub(super) input: Vec<ResponseItemEnvelope>,
    pub(super) cursor: TranscriptCursor,
    pub(super) truncations: Vec<TruncationObservation>,
    pub(super) section_costs: Vec<(&'static str, codex_guardian_context::SectionCost)>,
    pub(super) existing_context_tokens: usize,
    pub(super) input_tokens: usize,
}
