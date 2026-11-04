//! Retains bounded unsampled mail and positions summaries before it and subsequent caller input.
//! Delivery is proven only by a regular inference checkpoint, never by compactor output.

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;

use crate::context_manager::estimate_item_token_count;
use crate::context_manager::sampled_through;

pub(crate) const MAX_RETAINED_AGENT_MESSAGE_TOKENS: i64 = 10_000;

pub(crate) fn pending_mail_sequence(history: &[ResponseItemEnvelope]) -> Option<u64> {
    let sampled_through = sampled_through(history);
    history
        .iter()
        .filter(|envelope| matches!(envelope.item, ResponseItem::AgentMessage { .. }))
        .filter_map(|envelope| envelope.metadata.as_ref()?.history_sequence)
        .find(|sequence| *sequence > sampled_through)
}

pub(crate) fn is_retained_pending_mail(envelope: &ResponseItemEnvelope, first: u64) -> bool {
    matches!(envelope.item, ResponseItem::AgentMessage { .. })
        && envelope
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.history_sequence)
            .is_some_and(|sequence| sequence >= first)
        && estimate_item_token_count(&envelope.item) <= MAX_RETAINED_AGENT_MESSAGE_TOKENS
}

pub(crate) fn insert_summary(
    mut retained: Vec<ResponseItemEnvelope>,
    summary: ResponseItemEnvelope,
    pending_sequence: Option<u64>,
) -> Vec<ResponseItemEnvelope> {
    let position = pending_sequence
        .and_then(|first| {
            retained.iter().position(|envelope| {
                envelope
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.history_sequence)
                    .is_some_and(|sequence| sequence >= first)
            })
        })
        .unwrap_or(retained.len());
    retained.insert(position, summary);
    retained
}
