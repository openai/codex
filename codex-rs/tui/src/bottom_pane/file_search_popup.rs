use codex_file_search::FileMatch;
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

/// Maximum number of suggestions shown in the popup.
const MAX_RESULTS: usize = 8;

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
    matches: Vec<FileMatch>,
    /// Currently selected index inside `matches` (if any).
    selected_idx: Option<usize>,
}

impl FileSearchPopup {
    pub(crate) fn new() -> Self {
        Self {
            display_query: String::new(),
            pending_query: String::new(),
            waiting: true,
            matches: Vec::new(),
            selected_idx: None,
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
            self.selected_idx = None;
        }
    }

    /// Replace matches when a `FileSearchResult` arrives.
    /// Replace matches. Only applied when `query` matches `pending_query`.
    pub(crate) fn set_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.pending_query {
            return; // stale
        }

        self.display_query = query.to_string();
        self.matches = matches;
        self.waiting = false;
        self.selected_idx = if self.matches.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Move selection cursor up.
    pub(crate) fn move_up(&mut self) {
        if let Some(idx) = self.selected_idx {
            if idx > 0 {
                self.selected_idx = Some(idx - 1);
            }
        }
    }

    /// Move selection cursor down.
    pub(crate) fn move_down(&mut self) {
        if let Some(idx) = self.selected_idx {
            if idx + 1 < self.matches.len() {
                self.selected_idx = Some(idx + 1);
            }
        } else if !self.matches.is_empty() {
            self.selected_idx = Some(0);
        }
    }

    pub(crate) fn selected_match(&self) -> Option<&str> {
        self.selected_idx
            .and_then(|idx| self.matches.get(idx))
            .map(|file_match| file_match.path.as_str())
    }

    /// Preferred height (rows) including border.
    pub(crate) fn calculate_required_height(&self, _area: &Rect) -> u16 {
        // Row count depends on whether we already have matches. If no matches
        // yet (e.g. initial search or query with no results) reserve a single
        // row so the popup is still visible. When matches are present we show
        // up to MAX_RESULTS regardless of the waiting flag so the list
        // remains stable while a newer search is in-flight.
        let rows = if self.matches.is_empty() {
            1
        } else {
            self.matches.len().clamp(1, MAX_RESULTS)
        } as u16;
        rows + 2 // border
    }
}

impl WidgetRef for &FileSearchPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Prepare rows.
        let rows: Vec<Row> = if self.matches.is_empty() {
            vec![Row::new(vec![Cell::from(" no matches ")])]
        } else {
            self.matches
                .iter()
                .take(MAX_RESULTS)
                .enumerate()
                .map(|(i, file_match)| {
                    let FileMatch { path, indices, .. } = file_match;
                    let path = path.as_str();
                    #[allow(clippy::expect_used)]
                    let indices = indices.as_ref().expect("indices should be present");

                    // Build spans with bold on matching indices.
                    let mut idx_iter = indices.iter().peekable();
                    let mut spans: Vec<Span> = Vec::with_capacity(path.len());

                    for (char_idx, ch) in path.chars().enumerate() {
                        let mut style = Style::default();
                        if idx_iter
                            .peek()
                            .is_some_and(|next| **next == char_idx as u32)
                        {
                            idx_iter.next();
                            style = style.add_modifier(Modifier::BOLD);
                        }
                        spans.push(Span::styled(ch.to_string(), style));
                    }

                    // Create cell from the spans.
                    let mut cell = Cell::from(Line::from(spans));

                    // If selected, also paint yellow.
                    if Some(i) == self.selected_idx {
                        cell = cell.style(Style::default().fg(Color::Yellow));
                    }

                    Row::new(vec![cell])
                })
                .collect()
        };

        let mut title = format!(" @{} ", self.pending_query);
        if self.waiting {
            title.push_str(" (searching …)");
        }

        let table = Table::new(rows, vec![Constraint::Percentage(100)])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(title),
            )
            .widths([Constraint::Percentage(100)]);

        table.render(area, buf);
    }
}
