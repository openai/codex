use codex_common::fuzzy_match::fuzzy_indices;
use codex_common::fuzzy_match::fuzzy_match;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Constraint;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Cell;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

/// Maximum number of options shown in the popup.
const MAX_RESULTS: usize = 8;

/// Visual state for the model-selection popup.
pub(crate) struct ModelSelectionPopup {
    /// The current model (pinned and color-coded when visible).
    current_model: String,
    /// All available model options (deduplicated externally as needed).
    options: Vec<String>,
    /// Current filter query (derived from the composer, e.g. after `/model`).
    query: String,
    /// Currently selected index among the visible rows (if any).
    selected_idx: Option<usize>,
}

impl ModelSelectionPopup {
    pub(crate) fn new(current_model: &str, options: Vec<String>) -> Self {
        Self {
            current_model: current_model.to_string(),
            options,
            query: String::new(),
            selected_idx: None,
        }
    }

    /// Update the current model and option list. Resets/clamps selection as needed.
    pub(crate) fn set_options(&mut self, current_model: &str, options: Vec<String>) {
        self.current_model = current_model.to_string();
        self.options = options;
        let visible_len = self.visible_rows().len();
        self.selected_idx = match visible_len {
            0 => None,
            _ => Some(self.selected_idx.unwrap_or(0).min(visible_len - 1)),
        };
    }

    /// Update the fuzzy filter query.
    pub(crate) fn set_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query.clear();
        self.query.push_str(query);
        // Reset/clamp selection based on new filtered list.
        let visible_len = self.visible_rows().len();
        self.selected_idx = match visible_len {
            0 => None,
            _ => Some(0),
        };
    }

    /// Move selection cursor up.
    pub(crate) fn move_up(&mut self) {
        if let Some(idx) = self.selected_idx {
            if idx > 0 {
                self.selected_idx = Some(idx - 1);
            }
        } else if !self.visible_rows().is_empty() {
            self.selected_idx = Some(0);
        }
    }

    /// Move selection cursor down.
    pub(crate) fn move_down(&mut self) {
        let len = self.visible_rows().len();
        if len == 0 {
            self.selected_idx = None;
            return;
        }
        match self.selected_idx {
            Some(idx) if idx + 1 < len => self.selected_idx = Some(idx + 1),
            None => self.selected_idx = Some(0),
            _ => {}
        }
    }

    /// Currently selected model name, if any.
    pub(crate) fn selected_model(&self) -> Option<String> {
        let rows = self.visible_rows();
        self.selected_idx.and_then(|idx| {
            rows.get(idx)
                .map(|DisplayRow::Model { name, .. }| name.clone())
        })
    }

    /// Preferred height (rows) including border.
    pub(crate) fn calculate_required_height(&self) -> u16 {
        self.visible_rows().len().clamp(1, MAX_RESULTS) as u16
    }

    /// Compute rows to display applying fuzzy filtering and pinning current model.
    fn visible_rows(&self) -> Vec<DisplayRow> {
        // Build candidate list excluding the current model.
        let mut others: Vec<&str> = self
            .options
            .iter()
            .map(|s| s.as_str())
            .filter(|m| *m != self.current_model)
            .collect();

        // Keep original ordering for non-search.
        if self.query.trim().is_empty() {
            let mut rows: Vec<DisplayRow> = Vec::new();
            // Current model first.
            rows.push(DisplayRow::Model {
                name: self.current_model.clone(),
                match_indices: None,
                is_current: true,
            });
            for name in others.drain(..) {
                rows.push(DisplayRow::Model {
                    name: name.to_string(),
                    match_indices: None,
                    is_current: false,
                });
            }
            return rows;
        }

        // Searching: include current model only if it matches.
        let mut rows: Vec<DisplayRow> = Vec::new();
        if let Some(indices) = fuzzy_indices(&self.current_model, &self.query) {
            rows.push(DisplayRow::Model {
                name: self.current_model.clone(),
                match_indices: Some(indices),
                is_current: true,
            });
        }

        // Fuzzy-match the rest and sort by score, then name, then match tightness.
        let mut matches: Vec<(String, Vec<usize>, i32)> = Vec::new();
        for name in others.into_iter() {
            if let Some((indices, score)) = fuzzy_match(name, &self.query) {
                matches.push((name.to_string(), indices, score));
            }
        }
        matches.sort_by(|(a_name, a_idx, a_score), (b_name, b_idx, b_score)| {
            a_score
                .cmp(b_score)
                .then_with(|| a_name.cmp(b_name))
                .then_with(|| a_idx.len().cmp(&b_idx.len()))
        });

        for (name, indices, _score) in matches.into_iter() {
            if name != self.current_model {
                rows.push(DisplayRow::Model {
                    name,
                    match_indices: Some(indices),
                    is_current: false,
                });
            }
        }

        rows
    }
}

/// Row in the model popup.
enum DisplayRow {
    Model {
        name: String,
        match_indices: Option<Vec<usize>>, // indices to bold (char positions)
        is_current: bool,
    },
}

impl WidgetRef for &ModelSelectionPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows_all = self.visible_rows();

        let mut rows: Vec<Row> = Vec::new();
        if rows_all.is_empty() {
            rows.push(Row::new(vec![Cell::from(Line::from(Span::styled(
                "no matches",
                Style::default().add_modifier(Modifier::ITALIC | Modifier::DIM),
            )))]));
        } else {
            for (i, row) in rows_all.into_iter().take(MAX_RESULTS).enumerate() {
                match row {
                    DisplayRow::Model {
                        name,
                        match_indices,
                        is_current,
                    } => {
                        // Highlight fuzzy indices when present.
                        let mut spans: Vec<Span> = Vec::with_capacity(name.len());
                        if let Some(idxs) = match_indices.as_ref() {
                            let mut idx_iter = idxs.iter().peekable();
                            for (char_idx, ch) in name.chars().enumerate() {
                                let mut style = Style::default();
                                if idx_iter.peek().is_some_and(|next| **next == char_idx) {
                                    idx_iter.next();
                                    style = style.add_modifier(Modifier::BOLD);
                                }
                                spans.push(Span::styled(ch.to_string(), style));
                            }
                        } else {
                            spans.push(Span::raw(name.clone()));
                        }

                        let mut cell = Cell::from(Line::from(spans));
                        if Some(i) == self.selected_idx {
                            cell = cell.style(
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            );
                        } else if is_current {
                            cell = cell.style(Style::default().fg(Color::Cyan));
                        }
                        rows.push(Row::new(vec![cell]));
                    }
                }
            }
        }

        let table = Table::new(rows, vec![Constraint::Percentage(100)])
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_type(BorderType::QuadrantOutside)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .widths([Constraint::Percentage(100)]);

        table.render(area, buf);
    }
}
