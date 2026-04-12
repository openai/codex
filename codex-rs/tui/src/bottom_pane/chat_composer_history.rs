//! The chat composer history module owns shell-style recall and incremental search traversal.
//!
//! It combines persistent cross-session entries with local in-session entries into one offset
//! space. Persistent entries are fetched lazily and re-enter this state machine through
//! [`ChatComposerHistory::on_entry_response`], while local entries are already available with full
//! draft metadata.
//!
//! Ctrl+R search is modeled separately from normal Up/Down navigation because it has different
//! guarantees: query edits restart from the newest match, repeated Older/Newer keys move through
//! unique matching text, pending persistent fetches continue the same scan after the response
//! arrives, and boundary hits must not advance hidden cursor state. Search deduplication is scoped
//! to a single active search session and uses exact prompt text; it does not mutate stored history
//! or change normal history browsing.
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::MentionBinding;
use crate::mention_codec::decode_history_mentions;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::TextElement;

/// A composer history entry that can rehydrate draft state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryEntry {
    /// Raw text stored in history (may include placeholder strings).
    pub(crate) text: String,
    /// Text element ranges for placeholders inside `text`.
    pub(crate) text_elements: Vec<TextElement>,
    /// Local image paths captured alongside `text_elements`.
    pub(crate) local_image_paths: Vec<PathBuf>,
    /// Remote image URLs restored with this draft.
    pub(crate) remote_image_urls: Vec<String>,
    /// Mention bindings for tool/app/skill references inside `text`.
    pub(crate) mention_bindings: Vec<MentionBinding>,
    /// Placeholder-to-payload pairs used to restore large paste content.
    pub(crate) pending_pastes: Vec<(String, String)>,
}

impl HistoryEntry {
    pub(crate) fn new(text: String) -> Self {
        let decoded = decode_history_mentions(&text);
        Self {
            text: decoded.text,
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
            mention_bindings: decoded
                .mentions
                .into_iter()
                .map(|mention| MentionBinding {
                    mention: mention.mention,
                    path: mention.path,
                })
                .collect(),
            pending_pastes: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_pending(
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
        pending_pastes: Vec<(String, String)>,
    ) -> Self {
        Self {
            text,
            text_elements,
            local_image_paths,
            remote_image_urls: Vec::new(),
            mention_bindings: Vec::new(),
            pending_pastes,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_pending_and_remote(
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
        pending_pastes: Vec<(String, String)>,
        remote_image_urls: Vec<String>,
    ) -> Self {
        Self {
            text,
            text_elements,
            local_image_paths,
            remote_image_urls,
            mention_bindings: Vec::new(),
            pending_pastes,
        }
    }
}

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
    /// Local entries retain full draft state (text elements, image paths, pending pastes, remote image URLs).
    local_history: Vec<HistoryEntry>,

    /// Cache of persistent history entries fetched on-demand (text-only).
    fetched_history: HashMap<usize, HistoryEntry>,

    /// Current cursor within the combined (persistent + local) history. `None`
    /// indicates the user is *not* currently browsing history.
    history_cursor: Option<isize>,

    /// The text that was last inserted into the composer as a result of
    /// history navigation. Used to decide if further Up/Down presses should be
    /// treated as navigation versus normal cursor movement, together with the
    /// "cursor at line boundary" check in [`Self::should_handle_navigation`].
    last_history_text: Option<String>,

    /// Active incremental history search, if Ctrl+R search mode is open.
    search: Option<HistorySearchState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistorySearchDirection {
    /// Traverse toward older history offsets.
    Older,
    /// Traverse toward newer history offsets.
    Newer,
}

/// Result of a single incremental history search step.
///
/// `Pending` means a persistent entry lookup has been requested and the caller should keep the
/// visible search session open until [`ChatComposerHistory::on_entry_response`] supplies the next
/// result. `AtBoundary` means the current selected match is still valid but the requested direction
/// has no further unique match; callers should avoid treating it like a query miss.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HistorySearchResult {
    Found(HistoryEntry),
    Pending,
    AtBoundary,
    NotFound,
}

/// Result of integrating an asynchronous persistent history response.
///
/// A response can satisfy normal Up/Down navigation, resume a pending Ctrl+R search scan, or be
/// ignored if it belongs to a stale log or an offset the composer no longer needs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HistoryEntryResponse {
    Found(HistoryEntry),
    Search(HistorySearchResult),
    Ignored,
}

/// State for one active Ctrl+R search query.
///
/// The state keeps two cursors: `selected_offset` is the raw combined-history offset used to
/// continue scanning, while `selected_match_index` points into `unique_matches` so already
/// discovered unique results can be revisited without rescanning duplicate offsets. `seen_texts`
/// intentionally keys on exact prompt text because the UI previews and accepts text, not the
/// storage identity of each historical record.
#[derive(Clone, Debug)]
struct HistorySearchState {
    query: String,
    query_lower: String,
    selected_offset: Option<usize>,
    unique_matches: Vec<UniqueHistoryMatch>,
    selected_match_index: Option<usize>,
    seen_texts: HashSet<String>,
    awaiting: Option<PendingHistorySearch>,
    exhausted_older: bool,
    exhausted_newer: bool,
}

/// A unique search match cached with enough draft state to be selected again.
///
/// The vector of these matches is kept in newest-to-oldest offset order. Storing the entry beside
/// the offset avoids depending on later cache lookups when the user moves Newer/Older among matches
/// that have already been discovered.
#[derive(Clone, Debug)]
struct UniqueHistoryMatch {
    offset: usize,
    entry: HistoryEntry,
}

/// Persistent-history lookup currently blocking an incremental search scan.
///
/// The pending request records the direction and boundary behavior that were active when the fetch
/// was issued so the response can either return a unique match or continue scanning as if no async
/// gap had occurred.
#[derive(Clone, Copy, Debug)]
struct PendingHistorySearch {
    offset: usize,
    direction: HistorySearchDirection,
    boundary_if_exhausted: bool,
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
            search: None,
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
        self.search = None;
    }

    /// Record a message submitted by the user in the current session so it can
    /// be recalled later.
    pub fn record_local_submission(&mut self, entry: HistoryEntry) {
        if entry.text.is_empty()
            && entry.text_elements.is_empty()
            && entry.local_image_paths.is_empty()
            && entry.remote_image_urls.is_empty()
            && entry.mention_bindings.is_empty()
            && entry.pending_pastes.is_empty()
        {
            return;
        }
        self.history_cursor = None;
        self.last_history_text = None;
        self.search = None;

        // Avoid inserting a duplicate if identical to the previous entry.
        if self.local_history.last().is_some_and(|prev| prev == &entry) {
            return;
        }

        self.local_history.push(entry);
    }

    /// Reset navigation tracking so the next Up key resumes from the latest entry.
    pub fn reset_navigation(&mut self) {
        self.history_cursor = None;
        self.last_history_text = None;
        self.search = None;
    }

    pub fn reset_search(&mut self) {
        self.search = None;
    }

    /// Returns whether Up/Down should navigate history for the current textarea state.
    ///
    /// Empty text always enables history traversal. For non-empty text, this requires both:
    ///
    /// - the current text exactly matching the last recalled history entry, and
    /// - the cursor being at a line boundary (start or end).
    ///
    /// This boundary gate keeps multiline cursor movement usable while preserving shell-like
    /// history recall. If callers moved the cursor into the middle of a recalled entry and still
    /// forced navigation, users would lose normal vertical movement within the draft.
    pub fn should_handle_navigation(&self, text: &str, cursor: usize) -> bool {
        if self.history_entry_count == 0 && self.local_history.is_empty() {
            return false;
        }

        if text.is_empty() {
            return true;
        }

        // Textarea is not empty – only navigate when text matches the last
        // recalled history entry and the cursor is at a line boundary. This
        // keeps shell-like Up/Down recall working while still allowing normal
        // multiline cursor movement from interior positions.
        if cursor != 0 && cursor != text.len() {
            return false;
        }

        matches!(&self.last_history_text, Some(prev) if prev == text)
    }

    /// Handle <Up>. Returns true when the key was consumed and the caller
    /// should request a redraw.
    pub fn navigate_up(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        self.search = None;
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
    pub fn navigate_down(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        self.search = None;
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
                Some(HistoryEntry::new(String::new()))
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
    ) -> HistoryEntryResponse {
        if self.history_log_id != Some(log_id) {
            return HistoryEntryResponse::Ignored;
        }

        let entry = entry.map(HistoryEntry::new);
        if let Some(entry) = entry.clone() {
            self.fetched_history.insert(offset, entry);
        }

        if self
            .search
            .as_ref()
            .and_then(|search| search.awaiting)
            .is_some_and(|pending| pending.offset == offset)
        {
            let pending = self
                .search
                .as_ref()
                .and_then(|search| search.awaiting)
                .unwrap_or(PendingHistorySearch {
                    offset,
                    direction: HistorySearchDirection::Older,
                    boundary_if_exhausted: false,
                });
            if let Some(entry) = entry
                && self.search_matches(&entry)
                && self.search_result_is_unique(&entry)
            {
                return HistoryEntryResponse::Search(self.search_match(offset, entry));
            }
            return HistoryEntryResponse::Search(self.advance_search_after(
                offset,
                pending.direction,
                pending.boundary_if_exhausted,
                app_event_tx,
            ));
        }

        if self.history_cursor == Some(offset as isize) {
            let Some(entry) = entry else {
                return HistoryEntryResponse::Ignored;
            };
            self.last_history_text = Some(entry.text.clone());
            return HistoryEntryResponse::Found(entry);
        }

        HistoryEntryResponse::Ignored
    }

    /// Advance the active Ctrl+R search and return the next visible search state.
    ///
    /// Callers pass `restart` after opening search or editing the query; that clears the unique
    /// match cache and starts from the end of combined history. Repeated calls with the same query
    /// and `restart == false` move relative to the current unique match, preserving the selected
    /// entry at boundaries. Calling this while a previous persistent lookup is still pending will
    /// keep returning `Pending`; otherwise a stale response could race with a newer user action and
    /// replace the composer with an unexpected entry.
    pub fn search(
        &mut self,
        query: &str,
        direction: HistorySearchDirection,
        restart: bool,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let total_entries = self.total_entries();
        if total_entries == 0 {
            self.search = Some(HistorySearchState::new(query));
            return HistorySearchResult::NotFound;
        }

        let query_changed = self
            .search
            .as_ref()
            .is_none_or(|search| search.query != query);
        if !query_changed
            && !restart
            && self
                .search
                .as_ref()
                .and_then(|search| search.awaiting)
                .is_some()
        {
            return HistorySearchResult::Pending;
        }

        if query_changed || restart || self.search.is_none() {
            self.search = Some(HistorySearchState::new(query));
        } else if let Some(search) = self.search.as_mut() {
            search.awaiting = None;
        }

        let boundary_if_exhausted = !restart
            && self
                .search
                .as_ref()
                .and_then(|search| search.selected_offset)
                .is_some();
        if !restart
            && !query_changed
            && let Some(result) = self.select_cached_unique_match(direction)
        {
            return result;
        }
        if boundary_if_exhausted
            && self
                .search
                .as_ref()
                .is_some_and(|search| search.is_exhausted(direction))
        {
            return HistorySearchResult::AtBoundary;
        }

        let start_offset =
            self.search_start_offset(total_entries, direction, query_changed || restart);
        let Some(start_offset) = start_offset else {
            return self.exhausted_search_result(direction, boundary_if_exhausted);
        };

        let result =
            self.advance_search_from(start_offset, direction, boundary_if_exhausted, app_event_tx);
        if matches!(result, HistorySearchResult::NotFound) {
            self.exhausted_search_result(direction, boundary_if_exhausted)
        } else {
            result
        }
    }

    // ---------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------

    fn total_entries(&self) -> usize {
        self.history_entry_count + self.local_history.len()
    }

    fn search_start_offset(
        &self,
        total_entries: usize,
        direction: HistorySearchDirection,
        restart: bool,
    ) -> Option<usize> {
        let selected = self
            .search
            .as_ref()
            .and_then(|search| search.selected_offset);
        match direction {
            HistorySearchDirection::Older => {
                if restart {
                    total_entries.checked_sub(1)
                } else {
                    selected.and_then(|offset| offset.checked_sub(1))
                }
            }
            HistorySearchDirection::Newer => {
                if restart {
                    Some(0)
                } else {
                    selected
                        .and_then(|offset| offset.checked_add(1))
                        .filter(|offset| *offset < total_entries)
                }
            }
        }
    }

    fn advance_search_after(
        &mut self,
        offset: usize,
        direction: HistorySearchDirection,
        boundary_if_exhausted: bool,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let next_offset = match direction {
            HistorySearchDirection::Older => offset.checked_sub(1),
            HistorySearchDirection::Newer => offset
                .checked_add(1)
                .filter(|next| *next < self.total_entries()),
        };
        let Some(next_offset) = next_offset else {
            return self.exhausted_search_result(direction, boundary_if_exhausted);
        };
        let result =
            self.advance_search_from(next_offset, direction, boundary_if_exhausted, app_event_tx);
        if matches!(result, HistorySearchResult::NotFound) {
            self.exhausted_search_result(direction, boundary_if_exhausted)
        } else {
            result
        }
    }

    fn advance_search_from(
        &mut self,
        mut offset: usize,
        direction: HistorySearchDirection,
        boundary_if_exhausted: bool,
        app_event_tx: &AppEventSender,
    ) -> HistorySearchResult {
        let total_entries = self.total_entries();
        while offset < total_entries {
            if let Some(entry) = self.entry_at_cached_offset(offset) {
                if self.search_matches(&entry) && self.search_result_is_unique(&entry) {
                    return self.search_match(offset, entry);
                }
            } else if offset < self.history_entry_count
                && let Some(log_id) = self.history_log_id
            {
                if let Some(search) = self.search.as_mut() {
                    search.awaiting = Some(PendingHistorySearch {
                        offset,
                        direction,
                        boundary_if_exhausted,
                    });
                }
                app_event_tx.send(AppEvent::CodexOp(Op::GetHistoryEntryRequest {
                    offset,
                    log_id,
                }));
                return HistorySearchResult::Pending;
            }

            let next_offset = match direction {
                HistorySearchDirection::Older => offset.checked_sub(1),
                HistorySearchDirection::Newer => {
                    offset.checked_add(1).filter(|next| *next < total_entries)
                }
            };
            let Some(next_offset) = next_offset else {
                return HistorySearchResult::NotFound;
            };
            offset = next_offset;
        }

        HistorySearchResult::NotFound
    }

    fn entry_at_cached_offset(&self, offset: usize) -> Option<HistoryEntry> {
        if offset >= self.history_entry_count {
            self.local_history
                .get(offset - self.history_entry_count)
                .cloned()
        } else {
            self.fetched_history.get(&offset).cloned()
        }
    }

    fn search_matches(&self, entry: &HistoryEntry) -> bool {
        let Some(search) = self.search.as_ref() else {
            return false;
        };
        search.query.is_empty() || entry.text.to_lowercase().contains(&search.query_lower)
    }

    fn search_result_is_unique(&self, entry: &HistoryEntry) -> bool {
        self.search
            .as_ref()
            .is_none_or(|search| !search.seen_texts.contains(entry.text.as_str()))
    }

    fn search_match(&mut self, offset: usize, entry: HistoryEntry) -> HistorySearchResult {
        self.history_cursor = Some(offset as isize);
        self.last_history_text = Some(entry.text.clone());
        if let Some(search) = self.search.as_mut() {
            search.selected_offset = Some(offset);
            search.record_match(offset, &entry);
            search.awaiting = None;
            search.exhausted_older = false;
            search.exhausted_newer = false;
        }
        HistorySearchResult::Found(entry)
    }

    fn select_cached_unique_match(
        &mut self,
        direction: HistorySearchDirection,
    ) -> Option<HistorySearchResult> {
        let next_index = {
            let search = self.search.as_ref()?;
            let selected_index = search.selected_match_index?;
            match direction {
                HistorySearchDirection::Older => {
                    let next_index = selected_index + 1;
                    (next_index < search.unique_matches.len()).then_some(next_index)?
                }
                HistorySearchDirection::Newer => selected_index.checked_sub(1)?,
            }
        };

        let history_match = self.search.as_ref()?.unique_matches[next_index].clone();
        self.history_cursor = Some(history_match.offset as isize);
        self.last_history_text = Some(history_match.entry.text.clone());
        if let Some(search) = self.search.as_mut() {
            search.select_match(next_index);
        }
        Some(HistorySearchResult::Found(history_match.entry))
    }

    fn exhausted_search_result(
        &mut self,
        direction: HistorySearchDirection,
        boundary_if_exhausted: bool,
    ) -> HistorySearchResult {
        if let Some(search) = self.search.as_mut() {
            search.awaiting = None;
            if boundary_if_exhausted {
                search.mark_exhausted(direction);
            }
        }

        if boundary_if_exhausted {
            HistorySearchResult::AtBoundary
        } else {
            HistorySearchResult::NotFound
        }
    }

    fn populate_history_at_index(
        &mut self,
        global_idx: usize,
        app_event_tx: &AppEventSender,
    ) -> Option<HistoryEntry> {
        if global_idx >= self.history_entry_count {
            // Local entry.
            if let Some(entry) = self
                .local_history
                .get(global_idx - self.history_entry_count)
                .cloned()
            {
                self.last_history_text = Some(entry.text.clone());
                return Some(entry);
            }
        } else if let Some(entry) = self.fetched_history.get(&global_idx).cloned() {
            self.last_history_text = Some(entry.text.clone());
            return Some(entry);
        } else if let Some(log_id) = self.history_log_id {
            app_event_tx.send(AppEvent::CodexOp(Op::GetHistoryEntryRequest {
                offset: global_idx,
                log_id,
            }));
        }
        None
    }
}

impl HistorySearchState {
    fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            query_lower: query.to_lowercase(),
            selected_offset: None,
            unique_matches: Vec::new(),
            selected_match_index: None,
            seen_texts: HashSet::new(),
            awaiting: None,
            exhausted_older: false,
            exhausted_newer: false,
        }
    }

    fn is_exhausted(&self, direction: HistorySearchDirection) -> bool {
        match direction {
            HistorySearchDirection::Older => self.exhausted_older,
            HistorySearchDirection::Newer => self.exhausted_newer,
        }
    }

    fn mark_exhausted(&mut self, direction: HistorySearchDirection) {
        match direction {
            HistorySearchDirection::Older => self.exhausted_older = true,
            HistorySearchDirection::Newer => self.exhausted_newer = true,
        }
    }

    fn record_match(&mut self, offset: usize, entry: &HistoryEntry) {
        if let Some(index) = self
            .unique_matches
            .iter()
            .position(|history_match| history_match.offset == offset)
        {
            self.select_match(index);
            return;
        }

        self.seen_texts.insert(entry.text.clone());
        let insert_index = self
            .unique_matches
            .partition_point(|history_match| history_match.offset > offset);
        self.unique_matches.insert(
            insert_index,
            UniqueHistoryMatch {
                offset,
                entry: entry.clone(),
            },
        );
        self.select_match(insert_index);
    }

    fn select_match(&mut self, index: usize) {
        let Some(history_match) = self.unique_matches.get(index) else {
            return;
        };
        self.selected_offset = Some(history_match.offset);
        self.selected_match_index = Some(index);
        self.awaiting = None;
        self.exhausted_older = false;
        self.exhausted_newer = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn duplicate_submissions_are_not_recorded() {
        let mut history = ChatComposerHistory::new();

        // Empty submissions are ignored.
        history.record_local_submission(HistoryEntry::new(String::new()));
        assert_eq!(history.local_history.len(), 0);

        // First entry is recorded.
        history.record_local_submission(HistoryEntry::new("hello".to_string()));
        assert_eq!(history.local_history.len(), 1);
        assert_eq!(
            history.local_history.last().unwrap(),
            &HistoryEntry::new("hello".to_string())
        );

        // Identical consecutive entry is skipped.
        history.record_local_submission(HistoryEntry::new("hello".to_string()));
        assert_eq!(history.local_history.len(), 1);

        // Different entry is recorded.
        history.record_local_submission(HistoryEntry::new("world".to_string()));
        assert_eq!(history.local_history.len(), 2);
        assert_eq!(
            history.local_history.last().unwrap(),
            &HistoryEntry::new("world".to_string())
        );
    }

    #[test]
    fn navigation_with_async_fetch() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        // Pretend there are 3 persistent entries.
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 3);

        // First Up should request offset 2 (latest) and await async data.
        assert!(history.should_handle_navigation("", /*cursor*/ 0));
        assert!(history.navigate_up(&tx).is_none()); // don't replace the text yet

        // Verify that a history lookup request was sent.
        let event = rx.try_recv().expect("expected AppEvent to be sent");
        let AppEvent::CodexOp(op) = event else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 2,
            },
            op
        );

        // Inject the async response.
        assert_eq!(
            HistoryEntryResponse::Found(HistoryEntry::new("latest".to_string())),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 2,
                Some("latest".into()),
                &tx
            )
        );

        // Next Up should move to offset 1.
        assert!(history.navigate_up(&tx).is_none()); // don't replace the text yet

        // Verify second lookup request for offset 1.
        let event2 = rx.try_recv().expect("expected second event");
        let AppEvent::CodexOp(op) = event2 else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 1,
            },
            op
        );

        assert_eq!(
            HistoryEntryResponse::Found(HistoryEntry::new("older".to_string())),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 1,
                Some("older".into()),
                &tx
            )
        );
    }

    #[test]
    fn search_matches_local_history_and_stops_at_boundaries() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.record_local_submission(HistoryEntry::new("git status".to_string()));
        history.record_local_submission(HistoryEntry::new("cargo test -p codex-tui".to_string()));
        history.record_local_submission(HistoryEntry::new("git diff".to_string()));

        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git diff".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git status".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::AtBoundary,
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::AtBoundary,
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git diff".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Newer,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::AtBoundary,
            history.search(
                "git",
                HistorySearchDirection::Newer,
                /*restart*/ false,
                &tx
            )
        );
    }

    #[test]
    fn search_skips_duplicate_local_matches() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.record_local_submission(HistoryEntry::new("git status".to_string()));
        history.record_local_submission(HistoryEntry::new("cargo test -p codex-tui".to_string()));
        history.record_local_submission(HistoryEntry::new("git status".to_string()));
        history.record_local_submission(HistoryEntry::new("git diff".to_string()));

        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git diff".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git status".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::AtBoundary,
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git diff".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Newer,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("git status".to_string())),
            history.search(
                "git",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
    }

    #[test]
    fn repeated_boundary_search_does_not_refetch_persistent_history() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 3);

        assert_eq!(
            HistorySearchResult::Pending,
            history.search(
                "needle",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
        let _ = rx.try_recv().expect("expected latest lookup");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Found(HistoryEntry::new(
                "needle latest".to_string()
            ))),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 2,
                Some("needle latest".into()),
                &tx,
            )
        );

        assert_eq!(
            HistorySearchResult::Pending,
            history.search(
                "needle",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        let _ = rx.try_recv().expect("expected next older lookup");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Pending),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 1,
                Some("not a match".into()),
                &tx,
            )
        );
        let _ = rx.try_recv().expect("expected oldest lookup");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::AtBoundary),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 0,
                Some("also not a match".into()),
                &tx,
            )
        );
        assert!(rx.try_recv().is_err());

        assert_eq!(
            HistorySearchResult::AtBoundary,
            history.search(
                "needle",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn search_fetches_persistent_history_until_match() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 3);

        assert_eq!(
            HistorySearchResult::Pending,
            history.search(
                "older",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
        let AppEvent::CodexOp(op) = rx.try_recv().expect("expected latest lookup") else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 2,
            },
            op
        );

        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Pending),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 2,
                Some("latest".into()),
                &tx
            )
        );
        let AppEvent::CodexOp(op) = rx.try_recv().expect("expected next lookup") else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 1,
            },
            op
        );

        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Found(HistoryEntry::new(
                "older command".to_string()
            ))),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 1,
                Some("older command".into()),
                &tx
            )
        );
    }

    #[test]
    fn search_skips_duplicate_persistent_matches() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 4);

        assert_eq!(
            HistorySearchResult::Pending,
            history.search(
                "needle",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
        let _ = rx.try_recv().expect("expected latest lookup");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Found(HistoryEntry::new(
                "needle same".to_string()
            ))),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 3,
                Some("needle same".into()),
                &tx,
            )
        );

        assert_eq!(
            HistorySearchResult::Pending,
            history.search(
                "needle",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        let _ = rx.try_recv().expect("expected duplicate lookup");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Pending),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 2,
                Some("needle same".into()),
                &tx,
            )
        );
        let _ = rx.try_recv().expect("expected next lookup after duplicate");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Pending),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 1,
                Some("not a match".into()),
                &tx,
            )
        );
        let _ = rx.try_recv().expect("expected oldest lookup");
        assert_eq!(
            HistoryEntryResponse::Search(HistorySearchResult::Found(HistoryEntry::new(
                "needle older".to_string()
            ))),
            history.on_entry_response(
                /*log_id*/ 1,
                /*offset*/ 0,
                Some("needle older".into()),
                &tx,
            )
        );
        assert_eq!(
            HistorySearchResult::AtBoundary,
            history.search(
                "needle",
                HistorySearchDirection::Older,
                /*restart*/ false,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("needle same".to_string())),
            history.search(
                "needle",
                HistorySearchDirection::Newer,
                /*restart*/ false,
                &tx
            )
        );
    }

    #[test]
    fn search_is_case_insensitive_and_empty_query_finds_latest() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.record_local_submission(HistoryEntry::new("Build Release".to_string()));

        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("Build Release".to_string())),
            history.search(
                "release",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
        assert_eq!(
            HistorySearchResult::Found(HistoryEntry::new("Build Release".to_string())),
            history.search(
                "",
                HistorySearchDirection::Older,
                /*restart*/ true,
                &tx
            )
        );
    }

    #[test]
    fn reset_navigation_resets_cursor() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 3);
        history
            .fetched_history
            .insert(1, HistoryEntry::new("command2".to_string()));
        history
            .fetched_history
            .insert(2, HistoryEntry::new("command3".to_string()));

        assert_eq!(
            Some(HistoryEntry::new("command3".to_string())),
            history.navigate_up(&tx)
        );
        assert_eq!(
            Some(HistoryEntry::new("command2".to_string())),
            history.navigate_up(&tx)
        );

        history.reset_navigation();
        assert!(history.history_cursor.is_none());
        assert!(history.last_history_text.is_none());

        assert_eq!(
            Some(HistoryEntry::new("command3".to_string())),
            history.navigate_up(&tx)
        );
    }

    #[test]
    fn should_handle_navigation_when_cursor_is_at_line_boundaries() {
        let mut history = ChatComposerHistory::new();
        history.record_local_submission(HistoryEntry::new("hello".to_string()));
        history.last_history_text = Some("hello".to_string());

        assert!(history.should_handle_navigation("hello", /*cursor*/ 0));
        assert!(history.should_handle_navigation("hello", "hello".len()));
        assert!(!history.should_handle_navigation("hello", /*cursor*/ 1));
        assert!(!history.should_handle_navigation("other", /*cursor*/ 0));
    }
}
