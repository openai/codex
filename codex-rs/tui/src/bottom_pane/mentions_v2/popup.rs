use codex_file_search::FileMatch;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::candidate::Candidate;
use super::candidate::SearchResult;
use super::candidate::Selection;
use super::filter::filtered_candidates;
use super::render::render_popup;
use super::search_mode::SearchMode;
use super::search_mode::SearchModeSettings;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::scroll_state::ScrollState;

pub(crate) struct Popup {
    query: String,
    file_search: FileSearch,
    candidates: Vec<Candidate>,
    search_mode: SearchMode,
    search_mode_settings: SearchModeSettings,
    state: ScrollState,
}

impl Popup {
    pub(crate) fn new(
        candidates: Vec<Candidate>,
        search_mode_settings: SearchModeSettings,
    ) -> Self {
        Self {
            query: String::new(),
            file_search: FileSearch::default(),
            candidates,
            search_mode: search_mode_settings.favorite_search_mode(),
            search_mode_settings,
            state: ScrollState::new(),
        }
    }

    pub(crate) fn set_candidates(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.clamp_selection();
    }

    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.file_search.set_query(query);
        self.clamp_selection();
    }

    pub(crate) fn set_file_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        self.file_search.set_matches(query, matches);
        self.clamp_selection();
    }

    pub(crate) fn selected(&self) -> Option<Selection> {
        let rows = self.rows();
        let idx = self.state.selected_idx?;
        rows.get(idx).map(|row| row.selection.clone())
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.rows().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn move_down(&mut self) {
        let len = self.rows().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn previous_search_mode(&mut self) {
        self.search_mode = self.search_mode.previous();
        self.clamp_selection();
    }

    pub(crate) fn next_search_mode(&mut self) {
        self.search_mode = self.search_mode.next();
        self.clamp_selection();
    }

    pub(crate) fn remember_search_mode(&self, search_mode_settings: &mut SearchModeSettings) {
        search_mode_settings.toggle_favorite_search_mode(self.search_mode);
    }

    pub(crate) fn sync_search_mode_settings(&mut self, search_mode_settings: SearchModeSettings) {
        self.search_mode_settings = search_mode_settings;
    }

    pub(crate) fn calculate_required_height(&self, _width: u16) -> u16 {
        (MAX_POPUP_ROWS as u16).saturating_add(2)
    }

    fn clamp_selection(&mut self) {
        let len = self.rows().len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn rows(&self) -> Vec<SearchResult> {
        filtered_candidates(
            &self.candidates,
            &self.file_search.matches,
            &self.query,
            self.search_mode,
            self.file_search.should_show_matches(),
        )
    }
}

impl WidgetRef for Popup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        render_popup(
            area,
            buf,
            &self.rows(),
            &self.state,
            self.file_search.empty_message(),
            self.search_mode,
        );
    }
}

#[derive(Default)]
struct FileSearch {
    pending_query: String,
    display_query: String,
    waiting: bool,
    matches: Vec<FileMatch>,
}

impl FileSearch {
    fn set_query(&mut self, query: &str) {
        if query.is_empty() {
            self.pending_query.clear();
            self.display_query.clear();
            self.waiting = false;
            self.matches.clear();
        } else if query != self.pending_query {
            self.pending_query = query.to_string();
            self.waiting = true;
        }
    }

    fn set_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.pending_query {
            return;
        }

        self.display_query = query.to_string();
        self.matches = matches.into_iter().take(MAX_POPUP_ROWS).collect();
        self.waiting = false;
    }

    fn should_show_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    fn empty_message(&self) -> &'static str {
        if self.waiting {
            "loading..."
        } else {
            "no matches"
        }
    }
}
