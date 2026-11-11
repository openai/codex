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
                    item = next.checked_sub(1);
                }
                if let Some(id) = &checkpoint.window_id {
                    window = window.max(id.allocation().map(ModelFacingId::ordinal));
                }
            }
            RolloutItem::ResponseItem(envelope) => {
                item = item.max(envelope.item.id().and_then(item_ordinal));
                // The retained revision is local. Guardian delivery metadata quotes other threads.
                if let Some(source) = envelope
                    .metadata
                    .as_ref()
                    .and_then(|m| m.retained_source.as_ref())
                {
                    item = item.max(item_ordinal(&source.revision));
                }
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

fn item_ordinal(id: &ResponseItemId) -> Option<u64> {
    id.allocation()
        .map(|allocation| allocation.model_id.ordinal())
}
