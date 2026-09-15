//! Thread-owned capture and bounded storage for completed REPL evidence.
//! Immutable snapshots feed shared rendering; sequence and eviction stay host-owned.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_features::Feature;
use codex_guardian_context::NodeReplContext;
use codex_guardian_context::NodeReplResponse;
pub(crate) use codex_guardian_context::NodeReplReviewEvidenceMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::user_input::UserInput;
use codex_protocol::user_input::UserInput::Image;
use codex_protocol::user_input::UserInput::Text;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_string::take_bytes_at_char_boundary;

use crate::guardian::GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS;
use crate::guardian::guardian_truncate_text;
use crate::session::turn_context::TurnContext;

const MAX_PROVENANCE_BYTES: usize = 128;

pub(crate) fn node_repl_review_evidence_mode(turn: &TurnContext) -> NodeReplReviewEvidenceMode {
    let features = &turn.config.features;
    if turn.model_info().computer_use_review_required()
        || features.enabled(Feature::GuardianEnhancedNodeReplTranscripts)
            && features.enabled(Feature::GuardianNodeReplTranscriptImages)
    {
        NodeReplReviewEvidenceMode::Multimodal
    } else if features.enabled(Feature::GuardianEnhancedNodeReplTranscripts) {
        NodeReplReviewEvidenceMode::TextOnly
    } else {
        NodeReplReviewEvidenceMode::Disabled
    }
}

#[derive(Clone, Debug)]
struct NodeReplReviewResponse {
    sequence: u64,
    provenance: String,
    items: Vec<UserInput>,
}

impl NodeReplReviewResponse {
    fn has_images(&self) -> bool {
        self.items.iter().any(|item| matches!(item, Image { .. }))
    }

    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(std::mem::size_of::<Arc<Self>>())
            .saturating_add(self.provenance.len())
            .saturating_add(
                self.items
                    .capacity()
                    .saturating_mul(std::mem::size_of::<UserInput>()),
            )
            .saturating_add(self.items.iter().fold(0_usize, |bytes, item| {
                bytes.saturating_add(match item {
                    Text { text, .. } => text.len(),
                    Image { image_url, .. } => image_url.len(),
                    _ => 0,
                })
            }))
    }

    fn discard_images(&mut self) -> usize {
        let before = self.retained_bytes();
        self.items.retain(|item| !matches!(item, Image { .. }));
        self.items.shrink_to_fit();
        before.saturating_sub(self.retained_bytes())
    }
}

#[derive(Debug, Default)]
struct NodeReplReviewEvidenceState {
    responses: VecDeque<Arc<NodeReplReviewResponse>>,
    next_sequence: u64,
    retained_bytes: usize,
    image_capture_enabled: bool,
}

/// Bounded, thread-scoped evidence collected from nested `node_repl` or `cua_repl` calls.
#[derive(Debug, Default)]
pub struct NodeReplReviewEvidence(Mutex<NodeReplReviewEvidenceState>);

impl NodeReplReviewEvidence {
    pub(crate) const MAX_RETAINED_BYTES: usize = 8 * 1024 * 1024;

    /// Enables screenshot capture even when synchronous Guardian transcripts are disabled.
    pub fn enable_image_capture(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .image_capture_enabled = true;
    }

    pub(crate) fn image_capture_enabled(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .image_capture_enabled
    }

    /// Returns the screenshots currently retained for this thread, oldest first.
    pub fn images(&self) -> Vec<ContentItem> {
        let state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut seen_images = HashSet::new();
        state
            .responses
            .iter()
            .flat_map(|response| &response.items)
            .filter_map(|item| match item {
                Image { image_url, detail } if seen_images.insert(image_url) => {
                    Some(ContentItem::InputImage {
                        image: ImageReference::Inline {
                            image_url: image_url.clone(),
                        },
                        detail: *detail,
                    })
                }
                _ => None,
            })
            .collect()
    }

    pub(crate) fn record(
        &self,
        tool_name: &str,
        cell_id: &str,
        call_id: &str,
        mut items: Vec<UserInput>,
    ) {
        if !items.iter().any(|item| matches!(item, Image { .. })) {
            let text = items
                .into_iter()
                .filter_map(|item| match item {
                    Text { text, .. } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            items = Vec::from_iter((!text.is_empty()).then(|| text_input(text)));
        }
        let mut bounded_items = Vec::with_capacity(items.len());
        let mut rendered_text_tokens = 0_usize;
        let mut remaining_source_text_tokens = items
            .iter()
            .map(|item| match item {
                Text { text, .. } => approx_token_count(text),
                _ => 0,
            })
            .fold(0_usize, usize::saturating_add);
        for item in items {
            let Text { text, .. } = item else {
                if matches!(item, Image { .. }) {
                    bounded_items.push(item);
                }
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            remaining_source_text_tokens =
                remaining_source_text_tokens.saturating_sub(approx_token_count(&text));
            let remaining_tokens =
                GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS.saturating_sub(rendered_text_tokens);
            let reserved_tail_tokens = remaining_source_text_tokens.min(remaining_tokens / 2);
            let (text, _) = guardian_truncate_text(
                &text.replace("</", "<\\/"),
                remaining_tokens.saturating_sub(reserved_tail_tokens),
            );
            let text_tokens = approx_token_count(&text);
            if text_tokens <= remaining_tokens {
                rendered_text_tokens = rendered_text_tokens.saturating_add(text_tokens);
                bounded_items.push(text_input(text));
            }
        }

        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state.next_sequence = state.next_sequence.saturating_add(1);
        let mut response = Arc::new(NodeReplReviewResponse {
            sequence: state.next_sequence,
            provenance: format!(
                "tool={} cell={} call={}",
                bounded_provenance(tool_name),
                bounded_provenance(cell_id),
                bounded_provenance(call_id)
            ),
            items: bounded_items,
        });
        if response.retained_bytes() > Self::MAX_RETAINED_BYTES {
            Arc::make_mut(&mut response).discard_images();
            if response.items.is_empty() {
                return;
            }
        }
        while state
            .retained_bytes
            .saturating_add(response.retained_bytes())
            > Self::MAX_RETAINED_BYTES
        {
            if state.responses.front().is_some_and(|retained| {
                retained
                    .items
                    .iter()
                    .any(|item| matches!(item, Text { .. }))
            }) {
                let reclaimed = state
                    .responses
                    .iter_mut()
                    .find(|retained| retained.has_images())
                    .map_or(0, |retained| Arc::make_mut(retained).discard_images());
                state.retained_bytes = state.retained_bytes.saturating_sub(reclaimed);
                if reclaimed > 0 || Arc::make_mut(&mut response).discard_images() > 0 {
                    if response.items.is_empty() {
                        return;
                    }
                    continue;
                }
            }
            let Some(evicted) = state.responses.pop_front() else {
                return;
            };
            state.retained_bytes = state
                .retained_bytes
                .saturating_sub(evicted.retained_bytes());
        }
        state.retained_bytes = state
            .retained_bytes
            .saturating_add(response.retained_bytes());
        state.responses.push_back(response);
    }

    pub(crate) fn snapshot_since(
        &self,
        reviewed_sequence: u64,
    ) -> Option<NodeReplReviewEvidenceSnapshot> {
        let state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if state.next_sequence <= reviewed_sequence {
            return None;
        }

        let responses = state
            .responses
            .iter()
            .filter(|response| response.sequence > reviewed_sequence)
            .cloned()
            .collect::<Vec<_>>();
        let retained_since_review = u64::try_from(responses.len()).unwrap_or(u64::MAX);

        Some(NodeReplReviewEvidenceSnapshot {
            omitted_responses: state
                .next_sequence
                .saturating_sub(reviewed_sequence)
                .saturating_sub(retained_since_review),
            sequence: state.next_sequence,
            responses,
        })
    }
}

fn bounded_provenance(value: &str) -> String {
    let sanitized = take_bytes_at_char_boundary(value, MAX_PROVENANCE_BYTES)
        .replace(['\n', '\r', '[', ']', '='], "_")
        .replace("</", "<\\/");
    take_bytes_at_char_boundary(&sanitized, MAX_PROVENANCE_BYTES).to_string()
}

fn text_input(text: String) -> UserInput {
    UserInput::Text {
        text,
        text_elements: Vec::new(),
    }
}

pub(crate) struct NodeReplReviewEvidenceSnapshot {
    responses: Vec<Arc<NodeReplReviewResponse>>,
    omitted_responses: u64,
    pub(crate) sequence: u64,
}

impl NodeReplReviewEvidenceSnapshot {
    pub(crate) fn context(&self, mode: NodeReplReviewEvidenceMode) -> NodeReplContext<'_> {
        NodeReplContext {
            responses: self
                .responses
                .iter()
                .map(|response| NodeReplResponse {
                    sequence: response.sequence,
                    provenance: &response.provenance,
                    items: &response.items,
                })
                .collect(),
            omitted_responses: self.omitted_responses,
            mode,
        }
    }
}

#[cfg(test)]
#[path = "node_repl_review_evidence_tests.rs"]
mod tests;
