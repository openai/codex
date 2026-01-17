//! Manages shell-style history navigation for the chat composer.
//!
//! This module keeps track of persistent history entries that are fetched
//! on-demand, plus the in-session submissions that have not been persisted
//! yet. It owns the navigation cursor and the last inserted history text so
//! callers can decide when Up/Down should navigate versus edit the composer.
//! Rendering and text area mutation live elsewhere; this module only returns
//! replacement text or issues history fetch requests through [`AppEventSender`].

use std::collections::HashMap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_core::protocol::Op;

/// Tracks history state and selection for chat-composer navigation.
///
/// The history list is the concatenation of persistent entries (fetched by
/// offset) followed by the in-session submissions. The state machine owns the
/// navigation cursor and the last text inserted via history so it can decide
/// when Up/Down should browse rather than edit. Callers apply the returned text
/// to the composer and forward any history fetch requests to the backend.
pub(crate) struct ChatComposerHistory {
    /// Identifier of the active history log, set by session configuration.
    ///
    /// This is echoed in `GetHistoryEntryResponse` to guard against stale
    /// responses after a session change.
    history_log_id: Option<u64>,
    /// Count of persisted entries present when the session started.
    ///
    /// This value defines the boundary between persisted entries and
    /// `local_history` in the combined index space.
    history_entry_count: usize,

    /// Messages submitted during this UI session, ordered from oldest to newest.
    local_history: Vec<String>,

    /// Cache of persistent history entries keyed by their global offset.
    fetched_history: HashMap<usize, String>,

    /// Current cursor within the combined (persistent + local) history.
    ///
    /// `None` means the user is not currently browsing history.
    history_cursor: Option<isize>,

    /// The text last inserted into the composer via history navigation.
    ///
    /// This is used to decide whether Up/Down should keep navigating or return
    /// to normal editing.
    last_history_text: Option<String>,
}

impl ChatComposerHistory {
    /// Build an empty history state with no session metadata.
    pub fn new() -> Self {
        Self {
            history_log_id: None,
            history_entry_count: 0,
            local_history: Vec::new(),
            fetched_history: HashMap::new(),
            history_cursor: None,
            last_history_text: None,
        }
    }

    /// Reset the state for a newly configured session.
    ///
    /// This clears cached entries and local submissions because they are tied
    /// to the previous log identifier.
    pub fn set_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.history_log_id = Some(log_id);
        self.history_entry_count = entry_count;
        self.fetched_history.clear();
        self.local_history.clear();
        self.history_cursor = None;
        self.last_history_text = None;
    }

    /// Record a message submitted by the user so it can be recalled later.
    ///
    /// This clears any active navigation state and de-duplicates consecutive
    /// identical entries.
    pub fn record_local_submission(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.history_cursor = None;
        self.last_history_text = None;

        // Avoid inserting a duplicate if identical to the previous entry.
        if self.local_history.last().is_some_and(|prev| prev == text) {
            return;
        }

        self.local_history.push(text.to_string());
    }

    /// Clear navigation tracking so the next Up key resumes from the newest entry.
    pub fn reset_navigation(&mut self) {
        self.history_cursor = None;
        self.last_history_text = None;
    }

    /// Decide whether Up/Down should navigate history for the current editor state.
    ///
    /// Navigation is allowed when the editor is empty, or when the cursor is at
    /// the start and the editor still contains the last recalled history entry.
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

    /// Handle an Up press and return replacement text if available.
    ///
    /// This advances the cursor to the previous entry and returns cached text
    /// when present. If the entry is persistent and not cached, a fetch request
    /// is sent and `None` is returned until the response arrives.
    pub fn navigate_up(&mut self, app_event_tx: &AppEventSender) -> Option<String> {
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

    /// Handle a Down press and return replacement text if available.
    ///
    /// This advances the cursor toward newer entries. Moving past the newest
    /// entry exits history browsing and returns an empty string to restore the
    /// editor to its non-history state.
    pub fn navigate_down(&mut self, app_event_tx: &AppEventSender) -> Option<String> {
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

    /// Integrate an async history entry response.
    ///
    /// The response is cached for future navigation. If it matches the current
    /// cursor position, the entry is returned so the caller can update the
    /// editor immediately.
    pub fn on_entry_response(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
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
        None
    }

    /// Resolve a history index to cached text or trigger an async fetch.
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
            history.on_entry_response(1, 2, Some("latest".into()))
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
            history.on_entry_response(1, 1, Some("older".into()))
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
}
