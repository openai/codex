use codex_protocol::models::ResponseItem;

use crate::rollout::recorder::RolloutItem;

/// Whether a `ResponseItem` should be persisted in rollout files.
/// Note: Recording now persists all items; this helper remains for callers that
/// want a conservative filter of user-visible items.
#[inline]
pub(crate) fn is_persisted_response_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(ResponseItem::Message { .. })
        | RolloutItem::ResponseItem(ResponseItem::Reasoning { .. })
        | RolloutItem::ResponseItem(ResponseItem::LocalShellCall { .. })
        | RolloutItem::ResponseItem(ResponseItem::FunctionCall { .. })
        | RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput { .. })
        | RolloutItem::ResponseItem(ResponseItem::CustomToolCall { .. })
        | RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput { .. }) => true,
        RolloutItem::ResponseItem(ResponseItem::WebSearchCall { .. })
        | RolloutItem::ResponseItem(ResponseItem::Other) => false,
        // Non-ResponseItem variants: treat as persisted
        RolloutItem::SessionMeta(_) | RolloutItem::EventMsg(_) => true,
    }
}
