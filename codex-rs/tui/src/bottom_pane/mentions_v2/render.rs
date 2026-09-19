//! Render unified mentions with the shared picker frame and the live result catalog.
//!
//! Reserve the same result viewport across filters and searches. Preserve the
//! filesystem columns, fuzzy matches and right-aligned type metadata. Bound the
//! primary column so long names cannot crowd out distinguishing parent paths.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;

use super::candidate::MentionType;
use super::candidate::SearchResult;
use super::candidate::Selection;
use super::footer::render_footer;
use super::search_mode::SearchMode;
use crate::bottom_pane::picker_style::render_scroll_indicators;
use crate::bottom_pane::picker_style::selection_style;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_picker_layout::PickerLayoutSizes;
use crate::bottom_pane::selection_picker_layout::picker_areas;
use crate::bottom_pane::selection_popup_common::RenderedRows;
use crate::bottom_pane::selection_tabs::render_filled_tab_bar;

/// Two header lines, header/tab gaps, tabs, query, two overflow rows and help.
pub(super) const POPUP_HEIGHT: u16 = MAX_POPUP_ROWS as u16 + 9;

pub(super) fn render_popup(
    area: Rect,
    buf: &mut Buffer,
    rows: &[SearchResult],
    state: &ScrollState,
    empty_message: &str,
    search_mode: SearchMode,
    query: &str,
) {
    ratatui::widgets::Clear.render(area, buf);
    if area.is_empty() {
        return;
    }
    // At very short heights, retain the selected result before adding controls.
    if area.height < 4 {
        render_rows(area, buf, rows, state, empty_message);
        return;
    }

    let content = Rect {
        height: area.height - 1,
        ..area
    };
    let [header, _, tabs, _, search, above, list, below, _, _] = picker_areas(
        content,
        PickerLayoutSizes {
            header: 2,
            header_gap: 1,
            tabs: 1,
            search: 1,
            rows: MAX_POPUP_ROWS as u16,
            side: 0,
        },
    );
    let inset = |rect: Rect| Rect {
        x: rect.x.saturating_add(/*rhs*/ 2).min(rect.right()),
        width: rect.width.saturating_sub(/*rhs*/ 4),
        ..rect
    };
    for (offset, line) in [
        Line::from("Mentions".bold()),
        Line::from("Files, directories, plugins, skills and tasks".dim()),
    ]
    .into_iter()
    .take(usize::from(header.height))
    .enumerate()
    {
        line.render(
            inset(Rect {
                y: header.y + offset as u16,
                height: 1,
                ..header
            }),
            buf,
        );
    }
    let modes = [
        SearchMode::Results,
        SearchMode::FilesystemOnly,
        SearchMode::Tools,
    ];
    let active = modes
        .iter()
        .position(|mode| *mode == search_mode)
        .unwrap_or_default();
    render_filled_tab_bar(&modes.map(SearchMode::label), active, inset(tabs), buf);
    let query = if query.is_empty() {
        "Type to search mentions"
    } else {
        query
    };
    truncate_line_with_ellipsis_if_overflow(
        Line::from(query.to_owned().dim()),
        usize::from(inset(search).width),
    )
    .render(inset(search), buf);
    let rendered = render_rows(list, buf, rows, state, empty_message);
    if above.height > 0 && below.height > 0 {
        render_scroll_indicators(
            Rect {
                y: above.y,
                height: below.bottom() - above.y,
                ..area
            },
            buf,
            rendered,
        );
    }
    render_footer(
        inset(Rect {
            y: area.bottom() - 1,
            height: 1,
            ..area
        }),
        buf,
    );
}

fn render_rows(
    area: Rect,
    buf: &mut Buffer,
    rows: &[SearchResult],
    state: &ScrollState,
    empty_message: &str,
) -> RenderedRows {
    if area.is_empty() {
        return RenderedRows::default();
    }
    if rows.is_empty() {
        Line::from(vec!["  ".into(), empty_message.italic()]).render(area, buf);
        return RenderedRows {
            lines: 1,
            ..Default::default()
        };
    }

    let visible_items = MAX_POPUP_ROWS.min(rows.len()).min(usize::from(area.height));
    let mut start_idx = state
        .scroll_top
        .min(rows.len().saturating_sub(visible_items));
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
        .map(primary_text_width)
        .max()
        .unwrap_or(/*default*/ 0);
    for (idx, row) in rows.iter().enumerate().skip(start_idx).take(visible_items) {
        if cur_y >= area.y + area.height {
            break;
        }

        let selected = Some(idx) == state.selected_idx;
        let line = build_line(row, selected, usize::from(area.width), primary_column_width);
        line.render(
            Rect {
                x: area.x,
                y: cur_y,
                width: area.width,
                height: 1,
            },
            buf,
        );
        cur_y = cur_y.saturating_add(/*rhs*/ 1);
    }
    RenderedRows {
        lines: visible_items as u16,
        items: visible_items,
        has_above: start_idx > 0,
        has_below: start_idx + visible_items < rows.len(),
    }
}

fn build_line(
    row: &SearchResult,
    selected: bool,
    width: usize,
    primary_column_width: usize,
) -> Line<'static> {
    let base_style = Style::default();
    let dim_style = Style::default().dim();
    let tag = row.mention_type.span(base_style);
    let tag_width = tag.width();
    let gutter = if selected { "› " } else { "  " };
    let gutter_width = UnicodeWidthStr::width(gutter);
    let content_width =
        width.saturating_sub(gutter_width.saturating_add(tag_width).saturating_add(2));
    // Keep room for secondary text even when an off-screen result has a long name.
    let primary_column_width =
        primary_column_width.min(content_width.saturating_sub(/*rhs*/ 2) / 2);
    let content = truncate_line_with_ellipsis_if_overflow(
        content_line(row, base_style, dim_style, primary_column_width),
        content_width,
    );
    let rendered_content_width = content.width();
    let mut spans = vec![gutter.into()];
    spans.extend(content.spans);
    let padding = width.saturating_sub(
        gutter_width
            .saturating_add(rendered_content_width)
            .saturating_add(tag_width),
    );
    if padding > 0 {
        spans.push(" ".repeat(padding).set_style(dim_style));
    }
    spans.push(tag);
    if selected {
        let style = selection_style();
        spans.iter_mut().for_each(|span| span.style = style);
    }

    let line = Line::from(spans);
    if selected {
        line.style(selection_style())
    } else {
        line
    }
}

fn content_line(
    row: &SearchResult,
    base_style: Style,
    dim_style: Style,
    primary_column_width: usize,
) -> Line<'static> {
    let primary = Line::from(primary_spans(row, base_style));
    if let Some(secondary) = secondary_line(row, base_style, dim_style) {
        let primary = truncate_line_with_ellipsis_if_overflow(primary, primary_column_width);
        let padding = primary_column_width
            .saturating_sub(primary.width())
            .saturating_add(/*rhs*/ 2);
        let mut spans = primary.spans;
        spans.push(" ".repeat(padding).set_style(dim_style));
        spans.extend(secondary.spans);
        Line::from(spans)
    } else {
        primary
    }
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
        MentionType::Plugin => base_style.fg(crate::style::accent_color()),
        MentionType::Skill => base_style.dim(),
        MentionType::Task => base_style.cyan(),
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
        .map(UnicodeWidthStr::width)
        .unwrap_or_else(|| UnicodeWidthStr::width(row.display_name.as_str()))
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

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
