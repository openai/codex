//! Session-wide mutable state.

use codex_protocol::models::{ContentItem, ResponseItem};
use codex_protocol::protocol::{
    ContextItemSummary, ContextItemsEvent, PruneCategory, PruneRange, ENVIRONMENT_CONTEXT_OPEN_TAG,
    USER_INSTRUCTIONS_OPEN_TAG,
};
use std::collections::BTreeSet;

use crate::conversation_history::ConversationHistory;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;

/// Persistent, session-scoped state previously stored directly on `Session`.
#[derive(Default)]
pub(crate) struct SessionState {
    pub(crate) history: ConversationHistory,
    pub(crate) token_info: Option<TokenUsageInfo>,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
    // Optional inclusion mask for Advanced Prune. When None, all items are included.
    include_mask: Option<BTreeSet<usize>>,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new() -> Self {
        Self {
            history: ConversationHistory::new(),
            ..Default::default()
        }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        let old_len = self.history.len();
        self.history.record_items(items);
        let new_len = self.history.len();
        if let Some(mask) = &mut self.include_mask {
            for idx in old_len..new_len {
                mask.insert(idx);
            }
        }
    }

    pub(crate) fn history_snapshot(&self) -> Vec<ResponseItem> {
        self.history.contents()
    }

    pub(crate) fn replace_history(&mut self, items: Vec<ResponseItem>) {
        self.history.replace(items);
        // Reset include_mask because indices changed completely.
        self.include_mask = None;
    }


    // Token/rate limit helpers
    pub(crate) fn update_token_info_from_usage(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<u64>,
    ) {
        self.token_info = TokenUsageInfo::new_or_append(
            &self.token_info,
            &Some(usage.clone()),
            model_context_window,
        );
    }

    pub(crate) fn set_rate_limits(&mut self, snapshot: RateLimitSnapshot) {
        self.latest_rate_limits = Some(snapshot);
    }

    pub(crate) fn token_info_and_rate_limits(
        &self,
    ) -> (Option<TokenUsageInfo>, Option<RateLimitSnapshot>) {
        (self.token_info.clone(), self.latest_rate_limits.clone())
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: u64) {
        match &mut self.token_info {
            Some(info) => info.fill_to_context_window(context_window),
            None => {
                self.token_info = Some(TokenUsageInfo::full_context_window(context_window));
            }
        }
    }

    // Pending input/approval lives in TurnState.
}

impl SessionState {
    /// Return a filtered history after applying the inclusion mask.
    pub(crate) fn filtered_history(&self) -> Vec<ResponseItem> {
        match &self.include_mask {
            None => self.history.contents(),
            Some(mask) => self
                .history
                .contents()
                .into_iter()
                .enumerate()
                .filter_map(|(idx, item)| mask.contains(&idx).then_some(item))
                .collect(),
        }
    }

    /// Ensure include_mask is initialized to "all included".
    fn ensure_mask_all_included(&mut self) {
        if self.include_mask.is_none() {
            // Use a contents() snapshot to compute length (restored behavior).
            let len = self.history.contents().len();
            self.include_mask = Some((0..len).collect());
        }
    }

    /// Set inclusion for given indices. Ignores out-of-range indices.
    pub(crate) fn set_context_inclusion(&mut self, indices: &[usize], included: bool) {
        self.ensure_mask_all_included();
        if let Some(mask) = &mut self.include_mask {
            // Use a contents() snapshot to compute length (restored behavior).
            let len = self.history.contents().len();
            for &idx in indices {
                if idx >= len {
                    continue;
                }
                if included {
                    mask.insert(idx);
                } else {
                    mask.remove(&idx);
                }
            }
        }
    }

    /// Delete items by index from history and update the inclusion mask accordingly.
    pub(crate) fn prune_by_indices(&mut self, mut indices: Vec<usize>) {
        indices.sort_unstable_by(|a, b| b.cmp(a));
        let mut items = self.history.contents();
        let mut changed = false;
        for idx in indices {
            if idx < items.len() {
                items.remove(idx);
                changed = true;
                if let Some(mask) = &mut self.include_mask {
                    mask.remove(&idx);
                    // Shift indices greater than idx by -1
                    let mut shifted: BTreeSet<usize> = BTreeSet::new();
                    for &m in mask.iter() {
                        shifted.insert(if m > idx { m - 1 } else { m });
                    }
                    *mask = shifted;
                }
            }
        }
        if changed {
            self.history.replace(items);
        }
    }

    /// Mark matching categories as excluded (non-destructive prune).
    pub(crate) fn prune_by_categories(&mut self, categories: &[PruneCategory], _range: &PruneRange) {
        if categories.is_empty() {
            return;
        }
        let items = self.history.contents();
        let mut to_exclude: Vec<usize> = Vec::new();
        for (idx, item) in items.iter().enumerate() {
            if let Some(cat) = categorize(item)
                && categories.iter().any(|c| c == &cat) {
                    to_exclude.push(idx);
                }
        }
        self.set_context_inclusion(&to_exclude, false);
    }

    /// Build a ContextItemsEvent summarizing items and their inclusion state.
    pub(crate) fn build_context_items_event(&self) -> ContextItemsEvent {
        let items = self.history.contents();
        let mask = self.include_mask.as_ref();
        let mut out: Vec<ContextItemSummary> = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
            if let Some(category) = categorize(item) {
                let included = match mask {
                    None => true,
                    Some(m) => m.contains(&idx),
                };
                let preview = preview_for(item);
                out.push(ContextItemSummary {
                    index: idx,
                    category,
                    preview,
                    included,
                });
            }
        }
        ContextItemsEvent { total: out.len(), items: out }
    }
}

/// Map a ResponseItem to a PruneCategory.
fn categorize(item: &ResponseItem) -> Option<PruneCategory> {
    use ResponseItem::*;
    match item {
        Message { role, content, .. } => {
            if let Some(text) = first_text(content) {
                let t = text.trim();
                if starts_with_case_insensitive(t, ENVIRONMENT_CONTEXT_OPEN_TAG) {
                    return Some(PruneCategory::EnvironmentContext);
                }
                if starts_with_case_insensitive(t, USER_INSTRUCTIONS_OPEN_TAG) {
                    return Some(PruneCategory::UserInstructions);
                }
            }
            if role == "assistant" {
                Some(PruneCategory::AssistantMessage)
            } else if role == "user" {
                Some(PruneCategory::UserMessage)
            } else {
                None
            }
        }
        Reasoning { .. } => Some(PruneCategory::Reasoning),
        FunctionCall { .. } | CustomToolCall { .. } | LocalShellCall { .. } | WebSearchCall { .. } => {
            Some(PruneCategory::ToolCall)
        }
        FunctionCallOutput { .. } | CustomToolCallOutput { .. } => Some(PruneCategory::ToolOutput),
        Other => None,
    }
}

fn first_text(items: &[ContentItem]) -> Option<&str> {
    for c in items {
        match c {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => return Some(text),
            _ => {}
        }
    }
    None
}

fn starts_with_case_insensitive(text: &str, prefix: &str) -> bool {
    let pl = prefix.len();
    match text.get(..pl) {
        Some(head) => head.eq_ignore_ascii_case(prefix),
        None => false, // not enough bytes or not on a char boundary — cannot match
    }
}

fn preview_for(item: &ResponseItem) -> String {
    use ResponseItem::*;
    const MAX: usize = 80;
    match item {
        Message { role, content, .. } => {
            let raw = first_text(content).unwrap_or("");
            let mut s = raw.trim();
            if let Some(idx) = s.find('\n') {
                s = &s[..idx];
            }
            let mut out = format!("{role}: {s}");
            if out.len() > MAX {
                out.truncate(MAX);
            }
            out
        }
        Reasoning { .. } => "<reasoning>…".to_string(),
        FunctionCall { name, .. } => format!("tool call: {name}"),
        FunctionCallOutput { output, .. } => {
            let mut s = output.content.trim().to_string();
            if s.len() > MAX {
                s.truncate(MAX);
            }
            format!("tool output: {s}")
        }
        CustomToolCall { name, .. } => format!("tool call: {name}"),
        CustomToolCallOutput { output, .. } => {
            let mut s = output.trim().to_string();
            if s.len() > MAX {
                s.truncate(MAX);
            }
            format!("tool output: {s}")
        }
        LocalShellCall { status, .. } => format!("shell: {status:?}"),
        WebSearchCall { action, .. } => match action {
            codex_protocol::models::WebSearchAction::Search { query } => {
                format!("search: {query}")
            }
            codex_protocol::models::WebSearchAction::Other => "search".to_string(),
        },
        Other => String::from("other"),
    }
}
