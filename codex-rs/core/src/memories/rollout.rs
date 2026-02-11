use crate::error::CodexErr;
use crate::error::Result;
use crate::rollout::policy::should_persist_response_item;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

/// Controls which rollout item kinds are retained for stage-1 memory extraction.
#[derive(Debug, Clone, Copy)]
pub(super) struct StageOneRolloutFilter {
    /// Keep `RolloutItem::ResponseItem` entries.
    pub(super) keep_response_items: bool,
    /// Keep `RolloutItem::Compacted` entries (converted to assistant messages).
    pub(super) keep_compacted_items: bool,
    /// Restricts kept `ResponseItem` entries. Defaults to rollout persistence policy.
    pub(super) response_item_filter: fn(&ResponseItem) -> bool,
    /// Optional cap on retained items after filtering.
    pub(super) max_items: Option<usize>,
}

impl StageOneRolloutFilter {
    pub(super) const fn response_and_compacted_items() -> Self {
        Self {
            keep_response_items: true,
            keep_compacted_items: true,
            response_item_filter: should_persist_response_item,
            max_items: None,
        }
    }
}

impl Default for StageOneRolloutFilter {
    fn default() -> Self {
        Self::response_and_compacted_items()
    }
}

/// Extracts stage-1 memory items from rollout JSONL entries.
///
/// `RolloutItem::Compacted` entries are converted to assistant messages so the
/// model sees the same response-item shape as normal transcript content.
pub(super) fn filter_rollout_response_items(
    items: &[RolloutItem],
    filter: StageOneRolloutFilter,
) -> Vec<ResponseItem> {
    let mut out = Vec::new();
    for item in items {
        match item {
            RolloutItem::ResponseItem(response_item)
                if filter.keep_response_items && (filter.response_item_filter)(response_item) =>
            {
                out.push(response_item.clone());
            }
            RolloutItem::Compacted(compacted) if filter.keep_compacted_items => {
                let compacted_as_message = ResponseItem::from(compacted.clone());
                if (filter.response_item_filter)(&compacted_as_message) {
                    out.push(compacted_as_message);
                }
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::EventMsg(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::Compacted(_) => {}
        }

        if let Some(limit) = filter.max_items
            && out.len() >= limit
        {
            break;
        }
    }
    out
}

/// Serializes filtered stage-1 memory items for prompt inclusion.
pub(super) fn serialize_filtered_rollout_response_items(
    items: &[RolloutItem],
    filter: StageOneRolloutFilter,
) -> Result<String> {
    let filtered = filter_rollout_response_items(items, filter);
    serde_json::to_string(&filtered).map_err(|err| {
        CodexErr::InvalidRequest(format!("failed to serialize rollout memory: {err}"))
    })
}
