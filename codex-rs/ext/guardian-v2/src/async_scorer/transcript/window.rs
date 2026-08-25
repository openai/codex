use std::collections::VecDeque;

use super::TranscriptConfig;
use super::TranscriptEntry;
use super::TranscriptEntryKind;

const MIN_RECENT_TOOL_ENTRIES: usize = 5;

#[derive(Default)]
struct EntryPool {
    indices: VecDeque<usize>,
    tokens: usize,
}

impl EntryPool {
    fn push(&mut self, index: usize, tokens: usize) {
        self.indices.push_back(index);
        self.tokens += tokens;
    }

    fn pop_oldest(&mut self, entries: &[TranscriptEntry]) {
        if let Some(index) = self.indices.pop_front() {
            self.tokens -= entries[index].tokens;
        }
    }

    fn evict_oldest(&mut self, count: usize, entries: &[TranscriptEntry]) {
        for _ in 0..count.min(self.indices.len()) {
            self.pop_oldest(entries);
        }
    }
}

pub(super) struct TranscriptWindow<'a> {
    entries: &'a [TranscriptEntry],
    message_token_budget: usize,
    tool_token_budget: usize,
    max_entries: usize,
    protected_messages: EntryPool,
    ordinary_messages: EntryPool,
    tools: EntryPool,
}

impl<'a> TranscriptWindow<'a> {
    pub(super) fn new(
        entries: &'a [TranscriptEntry],
        config: &TranscriptConfig,
        message_token_budget: usize,
    ) -> Self {
        Self {
            entries,
            message_token_budget,
            tool_token_budget: config.max_tool_transcript_tokens,
            max_entries: config.max_recent_non_user_entries,
            protected_messages: EntryPool::default(),
            ordinary_messages: EntryPool::default(),
            tools: EntryPool::default(),
        }
    }

    pub(super) fn insert(&mut self, index: usize) {
        let kind = self.entries[index].kind;
        let tokens = self.entries[index].tokens;

        if kind == TranscriptEntryKind::User
            || self.max_entries == 0
            || !self.make_room_for_token_budget(kind, tokens)
            || !self.make_room_for_entry_limit(kind)
        {
            return;
        }

        self.pool_mut(kind).push(index, tokens);
    }

    pub(super) fn into_indices(self) -> impl Iterator<Item = usize> {
        self.protected_messages
            .indices
            .into_iter()
            .chain(self.ordinary_messages.indices)
            .chain(self.tools.indices)
    }

    fn make_room_for_token_budget(&mut self, kind: TranscriptEntryKind, tokens: usize) -> bool {
        let token_budget = match kind {
            TranscriptEntryKind::ProtectedMessage | TranscriptEntryKind::Message => {
                self.message_token_budget
            }
            TranscriptEntryKind::Tool => self.tool_token_budget,
            TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
        };
        let protected_tokens = if kind == TranscriptEntryKind::Message {
            self.protected_messages.tokens
        } else {
            0
        };

        // Rejected commentary must not evict the protected context it cannot fit beside.
        if protected_tokens.saturating_add(tokens) > token_budget {
            return false;
        }

        let eviction_order: &[TranscriptEntryKind] = match kind {
            TranscriptEntryKind::ProtectedMessage => &[
                TranscriptEntryKind::Message,
                TranscriptEntryKind::ProtectedMessage,
            ],
            TranscriptEntryKind::Message => &[TranscriptEntryKind::Message],
            TranscriptEntryKind::Tool => &[TranscriptEntryKind::Tool],
            TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
        };
        let entries = self.entries;

        for &eviction_kind in eviction_order {
            if self.has_token_capacity(kind, tokens, token_budget) {
                break;
            }

            let pool_len = self.pool(eviction_kind).indices.len();
            if pool_len == 0 {
                continue;
            }

            self.pool_mut(eviction_kind)
                .evict_oldest(pool_len.div_ceil(2), entries);
            while !self.has_token_capacity(kind, tokens, token_budget)
                && !self.pool(eviction_kind).indices.is_empty()
            {
                self.pool_mut(eviction_kind).pop_oldest(entries);
            }
        }

        self.has_token_capacity(kind, tokens, token_budget)
    }

    fn make_room_for_entry_limit(&mut self, kind: TranscriptEntryKind) -> bool {
        let retained_count = self.protected_messages.indices.len()
            + self.ordinary_messages.indices.len()
            + self.tools.indices.len();
        if retained_count < self.max_entries {
            return true;
        }

        // Smaller windows still leave one slot available for protected context.
        let minimum_existing_tools = MIN_RECENT_TOOL_ENTRIES
            .min(self.max_entries.saturating_sub(1))
            .saturating_sub(usize::from(kind == TranscriptEntryKind::Tool));
        let removable_tools = self
            .tools
            .indices
            .len()
            .saturating_sub(minimum_existing_tools);
        let removable_count = self.ordinary_messages.indices.len() + removable_tools;

        if removable_count > 0 {
            let evidence_count = self.ordinary_messages.indices.len() + self.tools.indices.len();
            let eviction_count = evidence_count.div_ceil(2).min(removable_count);

            for _ in 0..eviction_count {
                let oldest_tool = (self.tools.indices.len() > minimum_existing_tools)
                    .then(|| self.tools.indices.front())
                    .flatten();
                let pool = match (self.ordinary_messages.indices.front(), oldest_tool) {
                    (Some(message), Some(tool)) if message < tool => &mut self.ordinary_messages,
                    (Some(_), Some(_)) | (None, Some(_)) => &mut self.tools,
                    (Some(_), None) => &mut self.ordinary_messages,
                    (None, None) => unreachable!("removable evidence was counted before eviction"),
                };
                pool.pop_oldest(self.entries);
            }

            return true;
        }

        if matches!(
            kind,
            TranscriptEntryKind::ProtectedMessage | TranscriptEntryKind::Tool
        ) {
            let protected_count = self.protected_messages.indices.len();
            self.protected_messages
                .evict_oldest(protected_count.div_ceil(2), self.entries);
            return true;
        }

        false
    }

    fn has_token_capacity(
        &self,
        kind: TranscriptEntryKind,
        tokens: usize,
        token_budget: usize,
    ) -> bool {
        let retained_tokens = match kind {
            TranscriptEntryKind::ProtectedMessage | TranscriptEntryKind::Message => self
                .protected_messages
                .tokens
                .saturating_add(self.ordinary_messages.tokens),
            TranscriptEntryKind::Tool => self.tools.tokens,
            TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
        };

        retained_tokens.saturating_add(tokens) <= token_budget
    }

    fn pool(&self, kind: TranscriptEntryKind) -> &EntryPool {
        match kind {
            TranscriptEntryKind::ProtectedMessage => &self.protected_messages,
            TranscriptEntryKind::Message => &self.ordinary_messages,
            TranscriptEntryKind::Tool => &self.tools,
            TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
        }
    }

    fn pool_mut(&mut self, kind: TranscriptEntryKind) -> &mut EntryPool {
        match kind {
            TranscriptEntryKind::ProtectedMessage => &mut self.protected_messages,
            TranscriptEntryKind::Message => &mut self.ordinary_messages,
            TranscriptEntryKind::Tool => &mut self.tools,
            TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
        }
    }
}
