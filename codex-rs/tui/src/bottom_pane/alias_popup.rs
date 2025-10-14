use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use crate::render::Insets;
use crate::render::RectExt;
use codex_common::fuzzy_match::fuzzy_match;

pub(crate) struct AliasPopup {
    filter: String,
    aliases: Vec<String>,
    state: ScrollState,
}

impl AliasPopup {
    pub(crate) fn new(names: Vec<String>) -> Self {
        let mut aliases = names;
        aliases.sort();
        Self {
            filter: String::new(),
            aliases,
            state: ScrollState::new(),
        }
    }

    pub(crate) fn set_aliases(&mut self, names: Vec<String>) {
        let mut aliases = names;
        aliases.sort();
        self.aliases = aliases;
        self.state.clamp_selection(self.filtered_items().len());
    }

    pub(crate) fn on_composer_text_change(&mut self, filter: &str) {
        self.filter = filter.to_string();
        let matches_len = self.filtered_items().len();
        self.state.clamp_selection(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        measure_rows_height(
            &self.rows_from_matches(self.filtered()),
            &self.state,
            MAX_POPUP_ROWS,
            width,
        )
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn move_down(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn selected_alias(&self) -> Option<String> {
        let matches = self.filtered_items();
        self.state
            .selected_idx
            .and_then(|idx| matches.get(idx).cloned())
    }

    fn filtered(&self) -> Vec<(String, Option<Vec<usize>>, i32)> {
        let filter = self.filter.trim();
        if filter.is_empty() {
            return self
                .aliases
                .iter()
                .map(|alias| (alias.clone(), None, 0))
                .collect();
        }

        let mut out = Vec::new();
        for alias in &self.aliases {
            if let Some((indices, score)) = fuzzy_match(alias, filter) {
                out.push((alias.clone(), Some(indices), score));
            }
        }
        out.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
        out
    }

    fn filtered_items(&self) -> Vec<String> {
        self.filtered()
            .into_iter()
            .map(|(name, _, _)| name)
            .collect()
    }

    fn rows_from_matches(
        &self,
        matches: Vec<(String, Option<Vec<usize>>, i32)>,
    ) -> Vec<GenericDisplayRow> {
        matches
            .into_iter()
            .map(|(alias, match_indices, _)| GenericDisplayRow {
                name: format!("//{alias}"),
                display_shortcut: None,
                match_indices: match_indices.map(|idxs| idxs.into_iter().collect()),
                is_current: false,
                description: None,
            })
            .collect()
    }
}

impl WidgetRef for AliasPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.rows_from_matches(self.filtered());
        render_rows(
            area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "no aliases",
        );
    }
}
