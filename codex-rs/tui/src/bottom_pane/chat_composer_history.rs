use std::collections::HashMap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_core::protocol::Op;

/// State machine that manages shell-style history navigation (Up/Down) inside
/// the chat composer. This struct is intentionally decoupled from the
/// rendering widget so the logic remains isolated and easier to test.
pub(crate) struct ChatComposerHistory {
    /// Identifier of the history log as reported by `SessionConfiguredEvent`.
    history_log_id: Option<u64>,
    /// Number of entries already present in the persistent cross-session
    /// history file when the session started.
    history_entry_count: usize,

    /// Messages submitted by the user *during this UI session* (newest at END).
    local_history: Vec<String>,

    /// Cache of persistent history entries fetched on-demand.
    fetched_history: HashMap<usize, String>,

    /// Current cursor within the combined (persistent + local) history. `None`
    /// indicates the user is *not* currently browsing history.
    history_cursor: Option<isize>,

    /// The text that was last inserted into the composer as a result of
    /// history navigation. Used to decide if further Up/Down presses should be
    /// treated as navigation versus normal cursor movement.
    last_history_text: Option<String>,

    reverse_search: Option<ReverseSearchState>,
}

#[derive(Clone, Debug)]
pub(crate) enum HistorySearchResult {
    Found(String),
    Pending,
    NotFound,
}

#[derive(Clone, Debug)]
struct ReverseSearchState {
    query: String,
    query_lower: String,
    next_offset: Option<isize>,
    awaiting_offset: Option<usize>,
}

impl ChatComposerHistory {
    pub fn new() -> Self {
        Self {
            history_log_id: None,
            history_entry_count: 0,
            local_history: Vec::new(),
            fetched_history: HashMap::new(),
            history_cursor: None,
            last_history_text: None,
            reverse_search: None,
        }
    }

    /// Update metadata when a new session is configured.
    pub fn set_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.history_log_id = Some(log_id);
        self.history_entry_count = entry_count;
        self.fetched_history.clear();
        self.local_history.clear();
        self.history_cursor = None;
        self.last_history_text = None;
        self.reverse_search = None;
    }

    /// Record a message submitted by the user in the current session so it can
    /// be recalled later.
    pub fn record_local_submission(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.history_cursor = None;
        self.last_history_text = None;
        self.reverse_search = None;

        // Avoid inserting a duplicate if identical to the previous entry.
        if self.local_history.last().is_some_and(|prev| prev == text) {
            return;
        }

        self.local_history.push(text.to_string());
    }

    /// Reset navigation tracking so the next Up key resumes from the latest entry.
    pub fn reset_navigation(&mut self) {
        self.history_cursor = None;
        self.last_history_text = None;
        self.reverse_search = None;
    }

    /// Should Up/Down key presses be interpreted as history navigation given
    /// the current content and cursor position of `textarea`?
    pub fn should_handle_navigation(&self, text: &str, cursor: usize) -> bool {
        if self.history_entry_count == 0 && self.local_history.is_empty() {
            return false;
        }

        if text.is_empty() {
            return true;
        }

        // Textarea is not empty – only navigate when cursor is at start and
        // text matches last recalled history entry so regular editing is not
        // hijacked.
        if cursor != 0 {
            return false;
        }

        matches!(&self.last_history_text, Some(prev) if prev == text)
    }

    /// Handle <Up>. Returns true when the key was consumed and the caller
    /// should request a redraw.
    pub fn navigate_up(&mut self, app_event_tx: &AppEventSender) -> Option<String> {
        self.reverse_search = None;
        let total_entries = self.history_entry_count + self.local_history.len();
        if total_entries == 0 {
            return None;
        }

        let next_idx = match self.history_cursor {
            None => (total_entries as isize) - 1,
            Some(0) => return None, // already at oldest
            Some(idx) => idx - 1,
        };

        self.history_cursor = Some(next_idx);
        self.populate_history_at_index(next_idx as usize, app_event_tx)
    }

    /// Handle <Down>.
    pub fn navigate_down(&mut self, app_event_tx: &AppEventSender) -> Option<String> {
        self.reverse_search = None;
        let total_entries = self.history_entry_count + self.local_history.len();
        if total_entries == 0 {
            return None;
        }

        let next_idx_opt = match self.history_cursor {
            None => return None, // not browsing
            Some(idx) if (idx as usize) + 1 >= total_entries => None,
            Some(idx) => Some(idx + 1),
        };

        match next_idx_opt {
            Some(idx) => {
                self.history_cursor = Some(idx);
                self.populate_history_at_index(idx as usize, app_event_tx)
            }
            None => {
                // Past newest – clear and exit browsing mode.
                self.history_cursor = None;
                self.last_history_text = None;
                Some(String::new())
            }
        }
    }

    /// Integrate a GetHistoryEntryResponse event.
    pub fn on_entry_response(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
        app_event_tx: &AppEventSender,
    ) -> Option<String> {
        if self.history_log_id != Some(log_id) {
            return None;
        }
        let text = entry?;
        self.fetched_history.insert(offset, text.clone());

        if self.history_cursor == Some(offset as isize) {
            self.last_history_text = Some(text.clone());
            return Some(text);
        }

        if let Some(search) = &mut self.reverse_search
            && search.awaiting_offset == Some(offset) {
                search.awaiting_offset = None;
                if Self::matches_query(&text, search) {
                    self.history_cursor = Some(offset as isize);
                    self.last_history_text = Some(text.clone());
                    search.next_offset = offset.checked_sub(1).map(|o| o as isize);
                    return Some(text);
                }
                if let HistorySearchResult::Found(next) = self.advance_reverse_search(app_event_tx)
                {
                    return Some(next);
                }
            }
        None
    }

    pub fn reverse_search(
        &mut self,
        query: &str,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let total_entries = self.total_entries();
        if total_entries == 0 {
            self.reverse_search = None;
            return HistorySearchResult::NotFound;
        }

        let base_offset = match &self.reverse_search {
            Some(existing) if existing.query == query => existing.next_offset,
            _ => match self.history_cursor {
                Some(cur) if cur > 0 => Some(cur - 1),
                Some(_) => None,
                None => Some((total_entries as isize) - 1),
            },
        };

        let next_offset = match base_offset {
            Some(offset) if offset >= 0 => Some(offset),
            _ => None,
        };

        if next_offset.is_none() {
            self.reverse_search = None;
            return HistorySearchResult::NotFound;
        }

        self.reverse_search = Some(ReverseSearchState {
            query: query.to_string(),
            query_lower: query.to_lowercase(),
            next_offset,
            awaiting_offset: None,
        });

        self.advance_reverse_search(app_event_tx)
    }

    // ---------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------

    fn total_entries(&self) -> usize {
        self.history_entry_count + self.local_history.len()
    }

    fn advance_reverse_search(&mut self, app_event_tx: &AppEventSender) -> HistorySearchResult {
        let total_entries = self.total_entries();
        let Some(search) = &mut self.reverse_search else {
            return HistorySearchResult::NotFound;
        };

        while let Some(offset) = search.next_offset {
            if offset < 0 {
                self.reverse_search = None;
                return HistorySearchResult::NotFound;
            }
            let offset_usize = offset as usize;
            search.next_offset = offset.checked_sub(1);

            if offset_usize >= total_entries {
                self.reverse_search = None;
                return HistorySearchResult::NotFound;
            }

            if offset_usize >= self.history_entry_count {
                if let Some(text) = self
                    .local_history
                    .get(offset_usize - self.history_entry_count)
                {
                    if Self::matches_query(text, search) {
                        return self.search_match(offset_usize, text.clone());
                    }
                    continue;
                }
            } else if let Some(text) = self.fetched_history.get(&offset_usize) {
                if Self::matches_query(text, search) {
                    return self.search_match(offset_usize, text.clone());
                }
                continue;
            } else if let Some(log_id) = self.history_log_id {
                search.awaiting_offset = Some(offset_usize);
                let op = Op::GetHistoryEntryRequest {
                    offset: offset_usize,
                    log_id,
                };
                app_event_tx.send(AppEvent::CodexOp(op));
                return HistorySearchResult::Pending;
            }
        }

        self.reverse_search = None;
        HistorySearchResult::NotFound
    }

    fn search_match(&mut self, offset: usize, text: String) -> HistorySearchResult {
        self.history_cursor = Some(offset as isize);
        self.last_history_text = Some(text.clone());
        if let Some(search) = &mut self.reverse_search {
            search.awaiting_offset = None;
            search.next_offset = offset.checked_sub(1).map(|o| o as isize);
        }
        HistorySearchResult::Found(text)
    }

    fn matches_query(text: &str, search: &ReverseSearchState) -> bool {
        if search.query.is_empty() {
            return true;
        }
        text.to_lowercase().contains(&search.query_lower)
    }

    fn populate_history_at_index(
        &mut self,
        global_idx: usize,
        app_event_tx: &AppEventSender,
    ) -> Option<String> {
        if global_idx >= self.history_entry_count {
            // Local entry.
            if let Some(text) = self
                .local_history
                .get(global_idx - self.history_entry_count)
            {
                self.last_history_text = Some(text.clone());
                return Some(text.clone());
            }
        } else if let Some(text) = self.fetched_history.get(&global_idx) {
            self.last_history_text = Some(text.clone());
            return Some(text.clone());
        } else if let Some(log_id) = self.history_log_id {
            let op = Op::GetHistoryEntryRequest {
                offset: global_idx,
                log_id,
            };
            app_event_tx.send(AppEvent::CodexOp(op));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use codex_core::protocol::Op;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn duplicate_submissions_are_not_recorded() {
        let mut history = ChatComposerHistory::new();

        // Empty submissions are ignored.
        history.record_local_submission("");
        assert_eq!(history.local_history.len(), 0);

        // First entry is recorded.
        history.record_local_submission("hello");
        assert_eq!(history.local_history.len(), 1);
        assert_eq!(history.local_history.last().unwrap(), "hello");

        // Identical consecutive entry is skipped.
        history.record_local_submission("hello");
        assert_eq!(history.local_history.len(), 1);

        // Different entry is recorded.
        history.record_local_submission("world");
        assert_eq!(history.local_history.len(), 2);
        assert_eq!(history.local_history.last().unwrap(), "world");
    }

    #[test]
    fn navigation_with_async_fetch() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        // Pretend there are 3 persistent entries.
        history.set_metadata(1, 3);

        // First Up should request offset 2 (latest) and await async data.
        assert!(history.should_handle_navigation("", 0));
        assert!(history.navigate_up(&tx).is_none()); // don't replace the text yet

        // Verify that an AppEvent::CodexOp with the correct GetHistoryEntryRequest was sent.
        let event = rx.try_recv().expect("expected AppEvent to be sent");
        let AppEvent::CodexOp(history_request1) = event else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 2
            },
            history_request1
        );

        // Inject the async response.
        assert_eq!(
            Some("latest".into()),
            history.on_entry_response(1, 2, Some("latest".into()), &tx)
        );

        // Next Up should move to offset 1.
        assert!(history.navigate_up(&tx).is_none()); // don't replace the text yet

        // Verify second CodexOp event for offset 1.
        let event2 = rx.try_recv().expect("expected second event");
        let AppEvent::CodexOp(history_request_2) = event2 else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 1
            },
            history_request_2
        );

        assert_eq!(
            Some("older".into()),
            history.on_entry_response(1, 1, Some("older".into()), &tx)
        );
    }

    #[test]
    fn reset_navigation_resets_cursor() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(1, 3);
        history.fetched_history.insert(1, "command2".into());
        history.fetched_history.insert(2, "command3".into());

        assert_eq!(Some("command3".into()), history.navigate_up(&tx));
        assert_eq!(Some("command2".into()), history.navigate_up(&tx));

        history.reset_navigation();
        assert!(history.history_cursor.is_none());
        assert!(history.last_history_text.is_none());

        assert_eq!(Some("command3".into()), history.navigate_up(&tx));
    }

    #[test]
    fn reverse_search_walks_local_history() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.record_local_submission("first prompt");
        history.record_local_submission("second prompt with key");
        history.record_local_submission("another prompt with key");

        match history.reverse_search("key", &tx) {
            HistorySearchResult::Found(text) => assert_eq!("another prompt with key", text),
            other => panic!("expected immediate match, got {other:?}"),
        }
        match history.reverse_search("key", &tx) {
            HistorySearchResult::Found(text) => assert_eq!("second prompt with key", text),
            other => panic!("expected second match, got {other:?}"),
        }
        assert!(matches!(
            history.reverse_search("key", &tx),
            HistorySearchResult::NotFound
        ));
    }

    #[test]
    fn reverse_search_fetches_persistent_history_until_match() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(42, 2);
        history.record_local_submission("local prompt");

        assert!(matches!(
            history.reverse_search("needle", &tx),
            HistorySearchResult::Pending
        ));

        let first_event = rx.try_recv().expect("expected request for latest entry");
        let AppEvent::CodexOp(first_request) = first_event else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 42,
                offset: 1
            },
            first_request
        );

        assert!(
            history
                .on_entry_response(42, 1, Some("irrelevant".into()), &tx)
                .is_none()
        );

        let second_event = rx.try_recv().expect("expected request for older entry");
        let AppEvent::CodexOp(second_request) = second_event else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 42,
                offset: 0
            },
            second_request
        );

        assert_eq!(
            Some("persistent needle".into()),
            history.on_entry_response(42, 0, Some("persistent needle".into()), &tx)
        );
    }
}
