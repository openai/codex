//! Maintains render-ready state for the file-search popup in the bottom pane.
//!
//! This module owns the transient UI state that sits between async file searches
//! and bottom-pane rendering. It tracks the last displayed query versus the most
//! recently typed query, caches the last result set, and carries shared
//! scroll/selection state so the list remains stable while new results arrive.
//!
//! The popup does not execute searches or decide when it is shown. Callers drive the
//! state machine by calling [`FileSearchPopup::set_query`],
//! [`FileSearchPopup::set_matches`], or [`FileSearchPopup::set_empty_prompt`]. The
//! correctness invariant is that displayed matches always correspond to
//! `display_query`; results for older queries are ignored so the UI cannot regress,
//! and scroll state is preserved across updates to avoid jumpy selection.
//!
//! An explicit empty-query state renders a hint instead of issuing searches, and
//! the `waiting` flag switches between "loading" and "no matches" messaging when
//! the list is empty. Selection state remains authoritative even while results are
//! inflight, so navigation continues to feel stable.

use codex_file_search::FileMatch;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;

/// Holds the render and selection state for the file-search popup.
///
/// This struct is a small UI state machine: it tracks the query the user most
/// recently typed, the query that produced the currently visible matches, and
/// whether a search is still in flight. Callers update the query, provide
/// matches as they arrive, and the struct maintains a stable selection even as
/// results change. The popup renders by converting cached matches into display
/// rows and then delegating to the shared selection popup renderer.
///
/// The primary invariant is that `matches` always correspond to `display_query`.
/// When results arrive out of order, stale payloads are dropped so the UI never
/// regresses to an older search.
pub(crate) struct FileSearchPopup {
    /// Query that produced the currently displayed `matches`.
    ///
    /// This is updated only when new results arrive so the UI never shows stale
    /// results for an older search.
    display_query: String,
    /// Most recently typed query, even if the search has not completed yet.
    ///
    /// The field may differ from `display_query` while an async search is in
    /// flight; it is used to drop stale result sets when they arrive out of
    /// order.
    pending_query: String,
    /// When `true`, the popup is waiting on results for `pending_query`.
    ///
    /// The renderer uses this to decide whether to show a loading hint or a
    /// "no matches" message when the list is empty.
    waiting: bool,
    /// Cached matches for `display_query`, with paths relative to the search root.
    ///
    /// The order is whatever the search backend provided, so renderers preserve it
    /// when mapping into display rows.
    matches: Vec<FileMatch>,
    /// Shared selection and scroll position for the result list.
    ///
    /// This is the single source of truth for selection, so all movement helpers
    /// and renderers must keep it in sync with `matches`.
    state: ScrollState,
}

impl FileSearchPopup {
    /// Create an empty popup state in the initial waiting mode.
    ///
    /// The initial waiting flag ensures the first render shows a loading hint
    /// while the async search kicks off.
    pub(crate) fn new() -> Self {
        Self {
            display_query: String::new(),
            pending_query: String::new(),
            waiting: true,
            matches: Vec::new(),
            state: ScrollState::new(),
        }
    }

    /// Update the query and transition the popup into the waiting state.
    ///
    /// The transition keeps existing matches when the new query is an extension
    /// of `display_query`, so the list stays visible while newer results are
    /// computed. Otherwise the list and selection state are cleared to avoid
    /// mixing unrelated result sets.
    pub(crate) fn set_query(&mut self, query: &str) {
        if query == self.pending_query {
            return;
        }

        // Determine if current matches are still relevant.
        let keep_existing = query.starts_with(&self.display_query);

        self.pending_query.clear();
        self.pending_query.push_str(query);

        self.waiting = true;

        if !keep_existing {
            self.matches.clear();
            self.state.reset();
        }
    }

    /// Put the popup into an idle state used for an empty query (just "@").
    ///
    /// This shows a hint instead of results until the user types more characters
    /// and we can issue a real search.
    pub(crate) fn set_empty_prompt(&mut self) {
        self.display_query.clear();
        self.pending_query.clear();
        self.waiting = false;
        self.matches.clear();

        // Reset selection/scroll state when showing the empty prompt.
        self.state.reset();
    }

    /// Replace matches when results arrive for the active `pending_query`.
    ///
    /// Stale results are ignored so that out-of-order responses cannot regress
    /// the visible list. Selection is clamped to the new list length and then
    /// made visible within the popup's row budget.
    pub(crate) fn set_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.pending_query {
            return; // Stale response.
        }

        self.display_query = query.to_string();
        self.matches = matches;
        self.waiting = false;
        let len = self.matches.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move the selection cursor up, wrapping within the current result list.
    ///
    /// The selection is re-centered within the visible rows after moving.
    pub(crate) fn move_up(&mut self) {
        let len = self.matches.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move the selection cursor down, wrapping within the current result list.
    ///
    /// The selection is re-centered within the visible rows after moving.
    pub(crate) fn move_down(&mut self) {
        let len = self.matches.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Return the selected match path, if any selection exists.
    ///
    /// The returned path matches the entries stored in [`FileMatch::path`],
    /// which are already relative to the search root.
    pub(crate) fn selected_match(&self) -> Option<&str> {
        self.state
            .selected_idx
            .and_then(|idx| self.matches.get(idx))
            .map(|file_match| file_match.path.as_str())
    }

    /// Report the popup height needed to render the current state.
    ///
    /// The height is clamped so the popup remains visible even when there are
    /// no results yet, and is capped by [`MAX_POPUP_ROWS`] to avoid overflow.
    pub(crate) fn calculate_required_height(&self) -> u16 {
        // Row count depends on whether we already have matches. If no matches
        // yet (e.g. initial search or query with no results) reserve a single
        // row so the popup is still visible. When matches are present we show
        // up to MAX_POPUP_ROWS regardless of the waiting flag so the list
        // remains stable while a newer search is in-flight.

        self.matches.len().clamp(1, MAX_POPUP_ROWS) as u16
    }
}

impl WidgetRef for &FileSearchPopup {
    /// Render the popup rows into the provided buffer.
    ///
    /// This converts search matches into `GenericDisplayRow` values, translating
    /// match indices into `usize` at the UI boundary, then delegates to the
    /// shared row renderer with the appropriate empty-state message.
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Convert matches to GenericDisplayRow, translating indices to usize at the UI boundary.
        let rows_all: Vec<GenericDisplayRow> = if self.matches.is_empty() {
            Vec::new()
        } else {
            self.matches
                .iter()
                .map(|m| GenericDisplayRow {
                    name: m.path.clone(),
                    match_indices: m
                        .indices
                        .as_ref()
                        .map(|v| v.iter().map(|&i| i as usize).collect()),
                    display_shortcut: None,
                    description: None,
                    wrap_indent: None,
                })
                .collect()
        };

        let empty_message = if self.waiting {
            "loading..."
        } else {
            "no matches"
        };

        render_rows(
            area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows_all,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );
    }
}
