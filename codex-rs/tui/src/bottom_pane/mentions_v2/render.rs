use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::Insets;
use crate::render::RectExt;

use super::candidate::MentionType;
use super::candidate::SearchResult;
use super::candidate::Selection;
use super::footer::render_footer;
use super::search_mode::SearchMode;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::scroll_state::ScrollState;

pub(super) fn render_popup(
    area: Rect,
    buf: &mut Buffer,
    rows: &[SearchResult],
    state: &ScrollState,
    empty_message: &str,
    search_mode: SearchMode,
) {
    let (list_area, hint_area) = if area.height > 2 {
        let hint_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        let list_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height - 2,
        };
        (list_area, Some(hint_area))
    } else {
        (area, None)
    };

    render_rows(
        list_area.inset(Insets::tlbr(
            /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
        )),
        buf,
        rows,
        state,
        empty_message,
    );

    if let Some(hint_area) = hint_area {
        let hint_area = Rect {
            x: hint_area.x + 2,
            y: hint_area.y,
            width: hint_area.width.saturating_sub(2),
            height: hint_area.height,
        };
        render_footer(hint_area, buf, search_mode);
    }
}

fn render_rows(
    area: Rect,
    buf: &mut Buffer,
    rows: &[SearchResult],
    state: &ScrollState,
    empty_message: &str,
) {
    if area.height == 0 {
        return;
    }
    if rows.is_empty() {
        Line::from(empty_message.italic()).render(area, buf);
        return;
    }

    let visible_items = MAX_POPUP_ROWS
        .min(rows.len())
        .min(area.height.max(1) as usize);
    let mut start_idx = state.scroll_top.min(rows.len().saturating_sub(1));
    if let Some(sel) = state.selected_idx {
        if sel < start_idx {
            start_idx = sel;
        } else if visible_items > 0 {
            let bottom = start_idx + visible_items - 1;
            if sel > bottom {
                start_idx = sel + 1 - visible_items;
            }
        }
    }

    let mut cur_y = area.y;
    for (idx, row) in rows.iter().enumerate().skip(start_idx).take(visible_items) {
        if cur_y >= area.y + area.height {
            break;
        }

        let selected = Some(idx) == state.selected_idx;
        let line =
            truncate_line_with_ellipsis_if_overflow(build_line(row, selected), area.width as usize);
        line.render(
            Rect {
                x: area.x,
                y: cur_y,
                width: area.width,
                height: 1,
            },
            buf,
        );
        cur_y = cur_y.saturating_add(1);
    }
}

fn build_line(row: &SearchResult, selected: bool) -> Line<'static> {
    let base_style = if selected {
        Style::default().bold()
    } else {
        Style::default()
    };
    let dim_style = if selected {
        Style::default().bold()
    } else {
        Style::default().dim()
    };
    let mut spans = Vec::new();
    spans.push(row.mention_type.span(base_style));
    spans.push("  ".set_style(dim_style));
    spans.extend(name_spans(row, base_style));
    if let Some(description) = row
        .description
        .as_deref()
        .filter(|description| !description.is_empty())
    {
        spans.push("  ".set_style(dim_style));
        spans.push(description.to_string().set_style(dim_style));
    }

    Line::from(spans)
}

fn name_spans(row: &SearchResult, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(row.display_name.len());
    let file_name_start = file_name_start(row);
    if let Some(indices) = row.match_indices.as_ref() {
        let mut idx_iter = indices.iter().peekable();
        for (char_idx, ch) in row.display_name.chars().enumerate() {
            let mut style = base_style;
            if char_idx >= file_name_start {
                style = style.fg(Color::Cyan);
            }
            if idx_iter.peek().is_some_and(|next| **next == char_idx) {
                idx_iter.next();
                style = style.bold();
            }
            spans.push(ch.to_string().set_style(style));
        }
    } else if file_name_start == 0 {
        spans.push(
            row.display_name
                .clone()
                .set_style(base_style.fg(Color::Cyan)),
        );
    } else if file_name_start != usize::MAX {
        let byte_start = row
            .display_name
            .char_indices()
            .nth(file_name_start)
            .map(|(idx, _)| idx)
            .unwrap_or(row.display_name.len());
        spans.push(
            row.display_name[..byte_start]
                .to_string()
                .set_style(base_style),
        );
        spans.push(
            row.display_name[byte_start..]
                .to_string()
                .set_style(base_style.fg(Color::Cyan)),
        );
    } else {
        spans.push(row.display_name.clone().set_style(base_style));
    }
    spans
}

fn file_name_start(row: &SearchResult) -> usize {
    match row.selection {
        Selection::File(_) if row.mention_type == MentionType::File => row
            .display_name
            .rfind(['/', '\\'])
            .map(|idx| row.display_name[..idx + 1].chars().count())
            .unwrap_or(0),
        Selection::File(_) | Selection::Tool { .. } => usize::MAX,
    }
}
