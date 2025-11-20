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
    }

    /// Record a message submitted by the user in the current session so it can
    /// be recalled later.
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

    /// Reset navigation tracking so the next Up key resumes from the latest entry.
    pub fn reset_navigation(&mut self) {
        self.history_cursor = None;
        self.last_history_text = None;
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

    // ---------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------

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
