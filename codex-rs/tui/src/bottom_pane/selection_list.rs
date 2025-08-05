//! Generic selection-list abstraction shared by model/execution selectors and other popups.
//!
//! This module provides `SelectionItem` (a value with name/description/aliases),
//! and `SelectionList` which maintains filtering and scroll/selection state.
//! The UI layer can convert items to `GenericDisplayRow` for rendering via
//! `selection_popup_common::render_rows`.

use codex_common::fuzzy_match::fuzzy_match;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;

/// One selectable item in a generic selection list.
#[derive(Clone)]
pub(crate) struct SelectionItem<T> {
    pub value: T,
    pub name: String,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    pub is_current: bool,
}

impl<T> SelectionItem<T> {
    pub fn new(value: T, name: String) -> Self {
        Self {
            value,
            name,
            description: None,
            aliases: Vec::new(),
            is_current: false,
        }
    }

    pub fn with_description(mut self, desc: Option<String>) -> Self {
        self.description = desc;
        self
    }

    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    pub fn mark_current(mut self, is_current: bool) -> Self {
        self.is_current = is_current;
        self
    }
}

/// Generic selection list state and fuzzy filtering.
pub(crate) struct SelectionList<T> {
    items: Vec<SelectionItem<T>>,
    query: String,
    pub state: ScrollState,
}

impl<T: Clone> SelectionList<T> {
    pub fn new(items: Vec<SelectionItem<T>>) -> Self {
        let mut this = Self {
            items,
            query: String::new(),
            state: ScrollState::new(),
        };
        let visible_len = this.visible_rows().len();
        this.state.clamp_selection(visible_len);
        this.state
            .ensure_visible(visible_len, visible_len.min(MAX_POPUP_ROWS));
        this
    }

    pub fn set_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query.clear();
        self.query.push_str(query);
        let visible_len = self.visible_rows().len();
        if visible_len == 0 {
            self.state.reset();
        } else {
            self.state.selected_idx = Some(0);
            self.state
                .ensure_visible(visible_len, visible_len.min(MAX_POPUP_ROWS));
        }
    }

    pub fn move_up(&mut self) {
        let len = self.visible_rows().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub fn move_down(&mut self) {
        let len = self.visible_rows().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub fn selected_value(&self) -> Option<T> {
        let rows = self.visible_rows();
        self.state
            .selected_idx
            .and_then(|idx| rows.get(idx))
            .and_then(|r| r.1)
            .cloned()
    }

    /// Visible rows paired with a reference to the underlying value (if any).
    /// The GenericDisplayRow contains only presentation data; we pair it with
    /// an Option<&T> so callers can map the selection back to a value.
    pub fn visible_rows(&self) -> Vec<(GenericDisplayRow, Option<&T>)> {
        let query = self.query.trim();

        let to_row = |it: &SelectionItem<T>, match_indices: Option<Vec<usize>>| GenericDisplayRow {
            name: it.name.clone(),
            match_indices,
            is_current: it.is_current,
            description: it.description.clone(),
        };

        if query.is_empty() {
            return self
                .items
                .iter()
                .map(|it| (to_row(it, None), Some(&it.value)))
                .collect();
        }

        let mut out: Vec<(GenericDisplayRow, Option<&T>, i32, usize)> = Vec::new();

        for it in self.items.iter() {
            if let Some((indices, score)) = fuzzy_match(&it.name, query) {
                out.push((
                    to_row(it, Some(indices)),
                    Some(&it.value),
                    score,
                    it.name.len(),
                ));
                continue;
            }
            let mut best_alias_score: Option<i32> = None;
            for alias in it.aliases.iter() {
                if let Some((_idx, score)) = fuzzy_match(alias, query) {
                    best_alias_score = Some(best_alias_score.map_or(score, |s| s.max(score)));
                }
            }
            if let Some(score) = best_alias_score {
                out.push((to_row(it, None), Some(&it.value), score, it.name.len()));
            }
        }

        out.sort_by(|a, b| {
            a.2.cmp(&b.2)
                .then_with(|| a.0.name.cmp(&b.0.name))
                .then_with(|| a.3.cmp(&b.3))
        });

        out.into_iter()
            .map(|(row, val, _score, _)| (row, val))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::SelectionItem;
    use super::SelectionList;

    #[test]
    fn selection_list_query_and_navigation() {
        let items = vec![
            SelectionItem::new("a", "Auto".to_string()).with_aliases(vec!["auto".into()]),
            SelectionItem::new("u", "Untrusted".to_string()).with_aliases(vec!["untrusted".into()]),
            SelectionItem::new("r", "Read only".to_string()).with_aliases(vec!["read-only".into()]),
        ];

        let mut list = SelectionList::new(items);

        let rows = list.visible_rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(list.selected_value(), Some("a"));

        list.move_up();
        assert_eq!(list.selected_value(), Some("r"));
        list.move_down();
        assert_eq!(list.selected_value(), Some("a"));

        list.set_query("auto");
        let rows = list.visible_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(list.selected_value(), Some("a"));

        list.set_query("read-only");
        let rows = list.visible_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(list.selected_value(), Some("r"));

        list.set_query("not-a-match");
        let rows = list.visible_rows();
        assert_eq!(rows.len(), 0);
        assert!(list.selected_value().is_none());
    }
}
