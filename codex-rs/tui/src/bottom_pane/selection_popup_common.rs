use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Constraint;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Cell;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::Widget;

use super::scroll_state::ScrollState;

/// A generic representation of a display row for selection popups.
pub(crate) struct GenericDisplayRow {
    pub name: String,
    pub match_indices: Option<Vec<usize>>, // indices to bold (char positions)
    pub is_current: bool,
    pub description: Option<String>, // optional grey text after the name
    pub styled_name: Option<Vec<Span<'static>>>,
}

impl GenericDisplayRow {}

/// Render a list of rows using the provided ScrollState, with shared styling
/// and behavior for selection popups.
pub(crate) fn render_rows(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    _dim_non_selected: bool,
    empty_message: &str,
) {
    let mut rows: Vec<Row> = Vec::new();

    if rows_all.is_empty() {
        rows.push(Row::new(vec![Cell::from(Line::from(
            empty_message.dim().italic(),
        ))]));
    } else {
        let max_rows_from_area = area.height as usize;
        let visible_rows = max_results
            .min(rows_all.len())
            .min(max_rows_from_area.max(1));

        // Compute starting index based on scroll state and selection.
        let mut start_idx = state.scroll_top.min(rows_all.len().saturating_sub(1));
        if let Some(sel) = state.selected_idx {
            if sel < start_idx {
                start_idx = sel;
            } else if visible_rows > 0 {
                let bottom = start_idx + visible_rows - 1;
                if sel > bottom {
                    start_idx = sel + 1 - visible_rows;
                }
            }
        }

        for (i, row) in rows_all
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(visible_rows)
        {
            let GenericDisplayRow {
                name,
                match_indices,
                is_current: _is_current,
                description,
                styled_name,
            } = row;

            // Highlight fuzzy indices when present.
            let mut spans: Vec<Span> = if let Some(styled) = styled_name.as_ref() {
                let mut spans = styled.clone();
                if Some(i) == state.selected_idx
                    && let Some(label_idx) = spans
                        .iter()
                        .position(|span| !span.content.trim().is_empty())
                    && let Some(label_span) = spans.get_mut(label_idx)
                {
                    let content = label_span.content.clone().into_owned();
                    *label_span = content.cyan().bold();
                }
                spans
            } else {
                let mut spans = Vec::with_capacity(name.len());
                if let Some(idxs) = match_indices.as_ref() {
                    let mut idx_iter = idxs.iter().peekable();
                    for (char_idx, ch) in name.chars().enumerate() {
                        if idx_iter.peek().is_some_and(|next| **next == char_idx) {
                            idx_iter.next();
                            spans.push(ch.to_string().bold());
                        } else {
                            spans.push(ch.to_string().into());
                        }
                    }
                } else {
                    spans.push(name.clone().into());
                }
                spans
            };

            if let Some(desc) = description.as_ref() {
                spans.push("  ".into());
                spans.push(desc.clone().dim());
            }

            let mut cell = Cell::from(Line::from(spans));

            if Some(i) == state.selected_idx {
                cell = cell.style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }
            rows.push(Row::new(vec![cell]));
        }
    }

    let table = Table::new(rows, vec![Constraint::Percentage(100)])
        .block(
            Block::default()
                .borders(Borders::LEFT)
                .border_type(BorderType::QuadrantOutside)
                .border_style(Style::default().add_modifier(Modifier::DIM)),
        )
        .widths([Constraint::Percentage(100)]);

    table.render(area, buf);
}
