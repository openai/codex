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
    let primary_column_width = rows
        .iter()
        .skip(start_idx)
        .take(visible_items)
        .map(primary_text_width)
        .max()
        .unwrap_or(0);
    for (idx, row) in rows.iter().enumerate().skip(start_idx).take(visible_items) {
        if cur_y >= area.y + area.height {
            break;
        }

        let selected = Some(idx) == state.selected_idx;
        let line = build_line(row, selected, area.width as usize, primary_column_width);
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

fn build_line(
    row: &SearchResult,
    selected: bool,
    width: usize,
    primary_column_width: usize,
) -> Line<'static> {
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
    let tag = row.mention_type.span(base_style);
    let tag_width = tag.width();
    let content_width = width.saturating_sub(tag_width.saturating_add(2));
    let content = truncate_line_with_ellipsis_if_overflow(
        content_line(row, base_style, dim_style, primary_column_width),
        content_width,
    );
    let rendered_content_width = content.width();
    let mut spans = Vec::new();
    spans.extend(content.spans);
    let padding = width.saturating_sub(rendered_content_width.saturating_add(tag_width));
    if padding > 0 {
        spans.push(" ".repeat(padding).set_style(dim_style));
    }
    spans.push(tag);

    Line::from(spans)
}

fn content_line(
    row: &SearchResult,
    base_style: Style,
    dim_style: Style,
    primary_column_width: usize,
) -> Line<'static> {
    let mut spans = Vec::new();
    spans.extend(primary_spans(row, base_style));
    if let Some(secondary) = secondary_line(row, base_style, dim_style) {
        let padding = primary_column_width
            .saturating_sub(primary_text_width(row))
            .saturating_add(2);
        spans.push(" ".repeat(padding).set_style(dim_style));
        spans.extend(secondary.spans);
    }

    Line::from(spans)
}

fn primary_spans(row: &SearchResult, base_style: Style) -> Vec<Span<'static>> {
    if let Some(file_name) = file_name(row) {
        let style = if row.mention_type == MentionType::File {
            base_style.fg(Color::Cyan)
        } else {
            base_style
        };
        return vec![file_name.to_string().set_style(style)];
    }

    let mut spans = Vec::with_capacity(row.display_name.len());
    let name_style = match row.mention_type {
        MentionType::Plugin => base_style.magenta(),
        MentionType::Skill => base_style.dim(),
        MentionType::File | MentionType::Directory => base_style,
    };
    if let Some(indices) = row.match_indices.as_ref() {
        let mut idx_iter = indices.iter().peekable();
        for (char_idx, ch) in row.display_name.chars().enumerate() {
            let mut style = name_style;
            if idx_iter.peek().is_some_and(|next| **next == char_idx) {
                idx_iter.next();
                style = style.bold();
            }
            spans.push(ch.to_string().set_style(style));
        }
    } else {
        spans.push(row.display_name.clone().set_style(name_style));
    }

    spans
}

fn secondary_line(
    row: &SearchResult,
    base_style: Style,
    dim_style: Style,
) -> Option<Line<'static>> {
    if file_name(row).is_some() {
        let mut spans = path_spans(row, base_style);
        if let Some(description) = row
            .description
            .as_deref()
            .filter(|description| !description.is_empty())
        {
            spans.push("  ".set_style(dim_style));
            spans.push(description.to_string().set_style(dim_style));
        }
        return Some(Line::from(spans));
    }

    row.description
        .as_deref()
        .filter(|description| !description.is_empty())
        .map(|description| Line::from(description.to_string().set_style(dim_style)))
}

fn path_spans(row: &SearchResult, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(row.display_name.len());
    let file_name_start = file_name_start(row);
    let path_style = base_style.dim();
    if file_name_start == 0 {
        spans.push("./".set_style(path_style));
    } else if let Some(indices) = row.match_indices.as_ref() {
        let mut idx_iter = indices.iter().peekable();
        for (char_idx, ch) in row.display_name.chars().enumerate().take(file_name_start) {
            let mut style = path_style;
            if idx_iter.peek().is_some_and(|next| **next == char_idx) {
                idx_iter.next();
                style = style.bold();
            }
            spans.push(ch.to_string().set_style(style));
        }
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
                .set_style(path_style),
        );
    } else {
        spans.push(row.display_name.clone().set_style(base_style));
    }
    spans
}

fn primary_text_width(row: &SearchResult) -> usize {
    file_name(row)
        .map(|file_name| file_name.chars().count())
        .unwrap_or_else(|| row.display_name.chars().count())
}

fn file_name(row: &SearchResult) -> Option<&str> {
    let file_name_start = file_name_start(row);
    if file_name_start == usize::MAX {
        return None;
    }
    if file_name_start == 0 {
        return Some(&row.display_name);
    }

    let byte_start = row
        .display_name
        .char_indices()
        .nth(file_name_start)
        .map(|(idx, _)| idx)
        .unwrap_or(row.display_name.len());
    Some(&row.display_name[byte_start..])
}

fn file_name_start(row: &SearchResult) -> usize {
    match row.selection {
        Selection::File(_) if row.mention_type.is_filesystem() => row
            .display_name
            .rfind(['/', '\\'])
            .map(|idx| row.display_name[..idx + 1].chars().count())
            .unwrap_or(0),
        Selection::File(_) | Selection::Tool { .. } => usize::MAX,
    }
}
