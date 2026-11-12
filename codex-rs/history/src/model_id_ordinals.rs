//! Recovers seeded allocation positions from a compaction checkpoint and subsequent thread items.

use codex_protocol::ResponseItemId;
use codex_utils_model_facing_id::ModelFacingId;

use crate::RetainedContextEvent;
use crate::RolloutItem;

/// Next item and window ordinals. Legacy strings have no allocation provenance.
pub fn next_model_id_ordinals(records: &[RolloutItem]) -> (u64, u64) {
    let mut item = None;
    let mut window = None;
    for record in records {
        match record {
            RolloutItem::SessionMeta(meta) => {
                if let Some(initial) = &meta.meta.context_window {
                    window = window.max(initial.window_id.allocation().map(ModelFacingId::ordinal));
                }
            }
            RolloutItem::Compacted(checkpoint) => {
                if let Some(next) = checkpoint
                    .resume_metadata
                    .as_ref()
                    .and_then(|m| m.next_item_ordinal)
                {
                    item = item.max(next.checked_sub(1));
                }
                for entry in checkpoint.replacement_history.iter().flatten().chain(
                    checkpoint
                        .guardian_history
                        .iter()
                        .flat_map(|history| &history.0),
                ) {
                    item = item.max(envelope_ordinal(entry));
                }
                if let Some(context) = &checkpoint.retained_context {
                    item = item.max(context.max_source_ordinal());
                }
                if let Some(id) = &checkpoint.window_id {
                    window = window.max(id.allocation().map(ModelFacingId::ordinal));
                }
            }
            RolloutItem::ResponseItem(envelope) => {
                item = item.max(envelope_ordinal(envelope));
            }
            RolloutItem::InterAgentCommunication(message) => {
                item = item.max(message.id.as_ref().and_then(item_ordinal));
            }
            RolloutItem::RetainedContext(RetainedContextEvent::DeliveredAssistantMessage {
                retained_source: Some(source),
                ..
            }) => {
                item = item.max(item_ordinal(&source.revision));
            }
            RolloutItem::DynamicToolsUpdated { .. }
            | RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::TurnContext(_)
            | RolloutItem::TokenUsageRecord(_)
            | RolloutItem::WorldState(_)
            | RolloutItem::SecurityRiskScore(_)
            | RolloutItem::RetainedContext(_)
            | RolloutItem::EventMsg(_)
            | RolloutItem::RealtimeItem(_) => {}
        }
    }
    let next = |last: Option<u64>| last.map_or(0, |ordinal| ordinal.wrapping_add(1));
    (next(item), next(window))
}

/// Advances the durable cursor in append order, including allocations first seen in a checkpoint.
pub fn stamp_item_ordinals(mut next: u64, items: &mut [RolloutItem]) -> u64 {
    for item in items {
        next = next.max(next_model_id_ordinals(std::slice::from_ref(item)).0);
        if let RolloutItem::Compacted(checkpoint) = item
            && let Some(metadata) = &mut checkpoint.resume_metadata
            && next != 0
        {
            metadata.next_item_ordinal = Some(next);
        }
    }
    next
}

fn envelope_ordinal(entry: &crate::ResponseItemEnvelope) -> Option<u64> {
    entry.item.id().and_then(item_ordinal).max(
        entry
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.retained_source.as_ref())
            .and_then(|source| item_ordinal(&source.revision)),
    )
}

pub(super) fn item_ordinal(id: &ResponseItemId) -> Option<u64> {
    id.allocation()
        .map(|allocation| allocation.model_id.ordinal())
}
