use crate::protocol::EventMsg;
use crate::rollout::recorder::RolloutItem;

/// Whether a `ResponseItem` should be persisted in rollout files.
/// Note: Recording now persists all items; this helper remains for callers that
/// want a conservative filter of user-visible items.
#[inline]
pub(crate) fn is_persisted_response_item(item: &RolloutItem) -> bool {
    match item {
        // Persist all response items (append-only transcript)
        RolloutItem::ResponseItem(_) => true,
        // Persist only selected event messages; drop deltas/noise
        RolloutItem::EventMsg(ev) => matches!(
            ev,
            EventMsg::UserMessage(_)
                | EventMsg::AgentMessage(_)
                | EventMsg::AgentReasoning(_)
                | EventMsg::TokenCount(_)
        ),
        // Always persist session meta
        RolloutItem::SessionMeta(_) => true,
    }
}
