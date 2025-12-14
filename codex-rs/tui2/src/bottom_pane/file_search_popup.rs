use codex_file_search::FileMatch;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt;
use crate::subagent_search::SubAgentMatch;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;

#[derive(Debug, Clone)]
pub(crate) enum AtCompletionItem {
    SubAgent(SubAgentMatch),
    File(FileMatch),
}

/// Visual state for the file-search popup.
pub(crate) struct FileSearchPopup {
    /// Query corresponding to the `matches` currently shown.
    display_query: String,
    /// Latest query typed by the user. May differ from `display_query` when
    /// a search is still in-flight.
    pending_query: String,
    /// When `true` we are still waiting for results for `pending_query`.
    waiting: bool,
    /// Cached matches; paths relative to the search dir.
    matches: Vec<AtCompletionItem>,
    /// Shared selection/scroll state.
    state: ScrollState,
}

impl FileSearchPopup {
    pub(crate) fn new() -> Self {
        Self {
            display_query: String::new(),
            pending_query: String::new(),
            waiting: true,
            matches: Vec::new(),
            state: ScrollState::new(),
        }
    }

    /// Update the query and reset state to *waiting*.
    pub(crate) fn set_query(&mut self, query: &str) {
        if query == self.pending_query {
            return;
        }

        // Determine if current matches are still relevant.
        let keep_existing = query.starts_with(&self.display_query);

        self.pending_query.clear();
        self.pending_query.push_str(query);

        self.waiting = true; // waiting for new results

        if !keep_existing {
            self.matches.clear();
            self.state.reset();
        }
    }

    /// Put the popup into an "idle" state used for an empty query (just "@").
    /// Shows a hint instead of matches until the user types more characters.
    pub(crate) fn set_empty_prompt(&mut self) {
        self.display_query.clear();
        self.pending_query.clear();
        self.waiting = false;
        self.matches.clear();
        // Reset selection/scroll state when showing the empty prompt.
        self.state.reset();
    }

    /// Replace matches when a `FileSearchResult` arrives.
    /// Replace matches. Only applied when `query` matches `pending_query`.
    pub(crate) fn set_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.pending_query {
            return; // stale
        }

        self.display_query = query.to_string();
        self.matches = matches.into_iter().map(AtCompletionItem::File).collect();
        self.waiting = false;
        let len = self.matches.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn set_at_matches(
        &mut self,
        query: &str,
        subagents: Vec<SubAgentMatch>,
        matches: Vec<FileMatch>,
    ) {
        if query != self.pending_query {
            return; // stale
        }

        self.display_query = query.to_string();
        self.matches = subagents
            .into_iter()
            .map(AtCompletionItem::SubAgent)
            .chain(matches.into_iter().map(AtCompletionItem::File))
            .collect();
        self.waiting = false;
        let len = self.matches.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move selection cursor up.
    pub(crate) fn move_up(&mut self) {
        let len = self.matches.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move selection cursor down.
    pub(crate) fn move_down(&mut self) {
        let len = self.matches.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn selected_match(&self) -> Option<&str> {
        self.state
            .selected_idx
            .and_then(|idx| self.matches.get(idx))
            .and_then(|item| match item {
                AtCompletionItem::File(file_match) => Some(file_match.path.as_str()),
                AtCompletionItem::SubAgent(_) => None,
            })
    }

    pub(crate) fn selected_item(&self) -> Option<&AtCompletionItem> {
        self.state
            .selected_idx
            .and_then(|idx| self.matches.get(idx))
    }

    pub(crate) fn calculate_required_height(&self) -> u16 {
        // Row count depends on whether we already have matches. If no matches
        // yet (e.g. initial search or query with no results) reserve a single
        // row so the popup is still visible. When matches are present we show
        // up to MAX_RESULTS regardless of the waiting flag so the list
        // remains stable while a newer search is in-flight.

        self.matches.len().clamp(1, MAX_POPUP_ROWS) as u16
    }
}

impl WidgetRef for &FileSearchPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Convert matches to GenericDisplayRow, translating indices to usize at the UI boundary.
        let rows_all: Vec<GenericDisplayRow> = if self.matches.is_empty() {
            Vec::new()
        } else {
            self.matches
                .iter()
                .map(|m| match m {
                    AtCompletionItem::File(file_match) => GenericDisplayRow {
                        name: file_match.path.clone(),
                        match_indices: file_match
                            .indices
                            .as_ref()
                            .map(|v| v.iter().map(|&i| i as usize).collect()),
                        display_shortcut: None,
                        description: None,
                        wrap_indent: None,
                        name_style: None,
                    },
                    AtCompletionItem::SubAgent(sub) => GenericDisplayRow {
                        name: format!("agents:{}", sub.name),
                        match_indices: None,
                        display_shortcut: None,
                        description: Some(sub.description.clone()),
                        wrap_indent: None,
                        name_style: sub.color.map(|c| Style::default().fg(c)),
                    },
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
