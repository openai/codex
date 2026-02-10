use crate::error::CodexErr;
use crate::error::Result;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

/// Bitmask selector for `ResponseItem` variants retained from rollout JSONL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StageOneResponseItemKinds(u16);

impl StageOneResponseItemKinds {
    const MESSAGE: u16 = 1 << 0;
    const REASONING: u16 = 1 << 1;
    const LOCAL_SHELL_CALL: u16 = 1 << 2;
    const FUNCTION_CALL: u16 = 1 << 3;
    const FUNCTION_CALL_OUTPUT: u16 = 1 << 4;
    const CUSTOM_TOOL_CALL: u16 = 1 << 5;
    const CUSTOM_TOOL_CALL_OUTPUT: u16 = 1 << 6;
    const WEB_SEARCH_CALL: u16 = 1 << 7;
    const GHOST_SNAPSHOT: u16 = 1 << 8;
    const COMPACTION: u16 = 1 << 9;
    const OTHER: u16 = 1 << 10;

    pub(super) const fn all() -> Self {
        Self(
            Self::MESSAGE
                | Self::REASONING
                | Self::LOCAL_SHELL_CALL
                | Self::FUNCTION_CALL
                | Self::FUNCTION_CALL_OUTPUT
                | Self::CUSTOM_TOOL_CALL
                | Self::CUSTOM_TOOL_CALL_OUTPUT
                | Self::WEB_SEARCH_CALL
                | Self::GHOST_SNAPSHOT
                | Self::COMPACTION
                | Self::OTHER,
        )
    }

    #[cfg(test)]
    pub(super) const fn messages_only() -> Self {
        Self(Self::MESSAGE)
    }

    const fn contains(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }

    fn keep(self, item: &ResponseItem) -> bool {
        match item {
            ResponseItem::Message { .. } => self.contains(Self::MESSAGE),
            ResponseItem::Reasoning { .. } => self.contains(Self::REASONING),
            ResponseItem::LocalShellCall { .. } => self.contains(Self::LOCAL_SHELL_CALL),
            ResponseItem::FunctionCall { .. } => self.contains(Self::FUNCTION_CALL),
            ResponseItem::FunctionCallOutput { .. } => self.contains(Self::FUNCTION_CALL_OUTPUT),
            ResponseItem::CustomToolCall { .. } => self.contains(Self::CUSTOM_TOOL_CALL),
            ResponseItem::CustomToolCallOutput { .. } => {
                self.contains(Self::CUSTOM_TOOL_CALL_OUTPUT)
            }
            ResponseItem::WebSearchCall { .. } => self.contains(Self::WEB_SEARCH_CALL),
            ResponseItem::GhostSnapshot { .. } => self.contains(Self::GHOST_SNAPSHOT),
            ResponseItem::Compaction { .. } => self.contains(Self::COMPACTION),
            ResponseItem::Other => self.contains(Self::OTHER),
        }
    }
}

impl Default for StageOneResponseItemKinds {
    fn default() -> Self {
        Self::all()
    }
}

/// Controls which rollout item kinds are retained for stage-1 memory extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StageOneRolloutFilter {
    /// Keep `RolloutItem::ResponseItem` entries.
    pub(super) keep_response_items: bool,
    /// Keep `RolloutItem::Compacted` entries (converted to assistant messages).
    pub(super) keep_compacted_items: bool,
    /// Restricts kept `ResponseItem` entries by variant.
    pub(super) response_item_kinds: StageOneResponseItemKinds,
    /// Optional cap on retained items after filtering.
    pub(super) max_items: Option<usize>,
}

impl StageOneRolloutFilter {
    pub(super) const fn response_and_compacted_items() -> Self {
        Self {
            keep_response_items: true,
            keep_compacted_items: true,
            response_item_kinds: StageOneResponseItemKinds::all(),
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
                if filter.keep_response_items && filter.response_item_kinds.keep(response_item) =>
            {
                out.push(response_item.clone());
            }
            RolloutItem::Compacted(compacted) if filter.keep_compacted_items => {
                let compacted_as_message = ResponseItem::from(compacted.clone());
                if filter.response_item_kinds.keep(&compacted_as_message) {
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
