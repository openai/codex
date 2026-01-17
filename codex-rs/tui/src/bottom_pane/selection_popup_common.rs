//! Shared rendering helpers for bottom-pane selection popups.
//!
//! This module renders [`GenericDisplayRow`] values into a popup area, aligning
//! descriptions to a shared column, applying fuzzy-match highlights, and
//! styling the active selection. The renderer is presentation-only: callers own
//! the filtered row list and the [`ScrollState`] that decides which rows are
//! visible.
//!
//! The layout is intentionally manual rather than using `Table` widgets so it
//! can compute wrapping and truncation consistently across popups. It treats
//! `match_indices` as character indices into `name` and pads descriptions based
//! on the widest visible name, including disabled markers when present. Selected
//! rows are restyled with a cyan bold highlight so the same rendering logic
//! applies to both active and inactive entries. Height calculations mirror the
//! wrapping rules used at render time so callers can size popups without
//! reimplementing layout logic.
//!
//! Callers should treat `match_indices` as character indices, not byte offsets,
//! and should keep `ScrollState` synchronized with the list length so the visible
//! window logic stays stable.
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::key_hint::KeyBinding;

use super::scroll_state::ScrollState;

/// Describes one row of selectable content in a popup list.
#[derive(Default)]
pub(crate) struct GenericDisplayRow {
    /// Primary label rendered at the start of the row.
    pub name: String,
    /// Optional shortcut rendered in parentheses after the name.
    pub display_shortcut: Option<KeyBinding>,
    /// Character indices within `name` that should be bolded for matches.
    ///
    /// Indices refer to the `char` positions in `name`, not byte offsets.
    pub match_indices: Option<Vec<usize>>,
    /// Optional secondary description rendered in a dim style.
    pub description: Option<String>,
    /// Optional disabled message, shown next to the name and in the description column.
    pub disabled_reason: Option<String>,
    /// Optional indentation (in spaces) for wrapped lines; defaults to the description column.
    pub wrap_indent: Option<usize>,
}

/// Wrap a styled line to the given width using zero indentation.
///
/// This is a convenience helper for popups that want wrapping without aligned
/// continuation lines.
pub(crate) fn wrap_styled_line<'a>(line: &'a Line<'a>, width: u16) -> Vec<Line<'a>> {
    use crate::wrapping::RtOptions;
    use crate::wrapping::word_wrap_line;

    let width = width.max(1) as usize;
    let opts = RtOptions::new(width)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(""));
    word_wrap_line(line, opts)
}

/// Measure the display width of a line using Unicode-width semantics.
fn line_width(line: &Line<'_>) -> usize {
    line.iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

/// Truncate a line to `max_width` cells without adding an ellipsis.
///
/// Span styles are preserved for any characters that fit within the limit.
fn truncate_line_to_width(line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let mut used = 0usize;
    let mut spans_out: Vec<Span<'static>> = Vec::new();

    for span in line.spans {
        let text = span.content.into_owned();
        let style = span.style;
        let span_width = UnicodeWidthStr::width(text.as_str());

        if span_width == 0 {
            spans_out.push(Span::styled(text, style));
            continue;
        }

        if used >= max_width {
            break;
        }

        if used + span_width <= max_width {
            used += span_width;
            spans_out.push(Span::styled(text, style));
            continue;
        }

        let mut truncated = String::new();
        for ch in text.chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + ch_width > max_width {
                break;
            }
            truncated.push(ch);
            used += ch_width;
        }

        if !truncated.is_empty() {
            spans_out.push(Span::styled(truncated, style));
        }

        break;
    }

    Line::from(spans_out)
}

/// Truncate a line to `max_width`, appending an ellipsis when it overflows.
///
/// The ellipsis inherits the style of the final visible span to keep emphasis
/// consistent with the truncated content.
fn truncate_line_with_ellipsis_if_overflow(line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let width = line_width(&line);
    if width <= max_width {
        return line;
    }

    let truncated = truncate_line_to_width(line, max_width.saturating_sub(1));
    let mut spans = truncated.spans;
    let ellipsis_style = spans.last().map(|span| span.style).unwrap_or_default();
    spans.push(Span::styled("…", ellipsis_style));
    Line::from(spans)
}

/// Compute a shared description-column start for the currently visible rows.
///
/// The column is based on the widest visible name plus two spaces of padding
/// and is clamped to leave at least one column available for the description.
/// Using only the visible range keeps wrapped alignment stable as the user
/// scrolls.
fn compute_desc_col(
    rows_all: &[GenericDisplayRow],
    start_idx: usize,
    visible_items: usize,
    content_width: u16,
) -> usize {
    let visible_range = start_idx..(start_idx + visible_items);
    let max_name_width = rows_all
        .iter()
        .enumerate()
        .filter(|(i, _)| visible_range.contains(i))
        .map(|(_, r)| {
            let mut spans: Vec<Span> = vec![r.name.clone().into()];
            if r.disabled_reason.is_some() {
                spans.push(" (disabled)".dim());
            }
            Line::from(spans).width()
        })
        .max()
        .unwrap_or(0);
    let mut desc_col = max_name_width.saturating_add(2);
    if (desc_col as u16) >= content_width {
        desc_col = content_width.saturating_sub(1) as usize;
    }
    desc_col
}

/// Determine how many spaces to indent wrapped lines for a row.
///
/// When `wrap_indent` is not set, rows with descriptions or disabled reasons
/// align their wrapped content under the shared description column.
fn wrap_indent(row: &GenericDisplayRow, desc_col: usize, max_width: u16) -> usize {
    let max_indent = max_width.saturating_sub(1) as usize;
    let indent = row.wrap_indent.unwrap_or_else(|| {
        if row.description.is_some() || row.disabled_reason.is_some() {
            desc_col
        } else {
            0
        }
    });
    indent.min(max_indent)
}

/// Build the full display line for a row with description padding.
///
/// The name is truncated to reserve space for the description column, match
/// indices are bolded, disabled markers are appended, and the description is
/// dimmed and aligned to `desc_col`.
fn build_full_line(row: &GenericDisplayRow, desc_col: usize) -> Line<'static> {
    let combined_description = match (&row.description, &row.disabled_reason) {
        (Some(desc), Some(reason)) => Some(format!("{desc} (disabled: {reason})")),
        (Some(desc), None) => Some(desc.clone()),
        (None, Some(reason)) => Some(format!("disabled: {reason}")),
        (None, None) => None,
    };

    // Enforce single-line name: allow at most desc_col - 2 cells for name,
    // reserving two spaces before the description column.
    let name_limit = combined_description
        .as_ref()
        .map(|_| desc_col.saturating_sub(2))
        .unwrap_or(usize::MAX);

    let mut name_spans: Vec<Span> = Vec::with_capacity(row.name.len());
    let mut used_width = 0usize;
    let mut truncated = false;

    if let Some(idxs) = row.match_indices.as_ref() {
        let mut idx_iter = idxs.iter().peekable();
        for (char_idx, ch) in row.name.chars().enumerate() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
            let next_width = used_width.saturating_add(ch_w);
            if next_width > name_limit {
                truncated = true;
                break;
            }
            used_width = next_width;

            if idx_iter.peek().is_some_and(|next| **next == char_idx) {
                idx_iter.next();
                name_spans.push(ch.to_string().bold());
            } else {
                name_spans.push(ch.to_string().into());
            }
        }
    } else {
        for ch in row.name.chars() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
            let next_width = used_width.saturating_add(ch_w);
            if next_width > name_limit {
                truncated = true;
                break;
            }
            used_width = next_width;
            name_spans.push(ch.to_string().into());
        }
    }

    if truncated {
        // If there is at least one cell available, add an ellipsis.
        // When name_limit is 0, we still show an ellipsis to indicate truncation.
        name_spans.push("…".into());
    }

    if row.disabled_reason.is_some() {
        name_spans.push(" (disabled)".dim());
    }

    let this_name_width = Line::from(name_spans.clone()).width();
    let mut full_spans: Vec<Span> = name_spans;
    if let Some(display_shortcut) = row.display_shortcut {
        full_spans.push(" (".into());
        full_spans.push(display_shortcut.into());
        full_spans.push(")".into());
    }
    if let Some(desc) = combined_description.as_ref() {
        let gap = desc_col.saturating_sub(this_name_width);
        if gap > 0 {
            full_spans.push(" ".repeat(gap).into());
        }
        full_spans.push(desc.clone().dim());
    }
    Line::from(full_spans)
}

/// Render a list of rows using the provided [`ScrollState`].
///
/// The renderer computes the visible range, derives a shared description column,
/// and then wraps each row to the available width while keeping the selected row
/// styled in cyan bold. If `rows_all` is empty, the `empty_message` is rendered
/// in a dim italic style. Selection and scroll state are resolved against the
/// item list so wrapping remains item-based even when rows span multiple lines.
pub(crate) fn render_rows(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) {
    if rows_all.is_empty() {
        if area.height > 0 {
            Line::from(empty_message.dim().italic()).render(area, buf);
        }
        return;
    }

    // Determine which logical rows (items) are visible given the selection and
    // the max_results clamp. Scrolling is still item-based for simplicity.
    let visible_items = max_results
        .min(rows_all.len())
        .min(area.height.max(1) as usize);

    let mut start_idx = state.scroll_top.min(rows_all.len().saturating_sub(1));
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

    let desc_col = compute_desc_col(rows_all, start_idx, visible_items, area.width);

    // Render items, wrapping descriptions and aligning wrapped lines under the
    // shared description column. Stop when we run out of vertical space.
    let mut cur_y = area.y;
    for (i, row) in rows_all
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_items)
    {
        if cur_y >= area.y + area.height {
            break;
        }

        let mut full_line = build_full_line(row, desc_col);
        if Some(i) == state.selected_idx {
            // Match previous behavior: cyan + bold for the selected row.
            // Reset the style first to avoid inheriting dim from keyboard shortcuts.
            full_line.spans.iter_mut().for_each(|span| {
                span.style = Style::default().fg(Color::Cyan).bold();
            });
        }

        // Wrap with subsequent indent aligned to the description column.
        use crate::wrapping::RtOptions;
        use crate::wrapping::word_wrap_line;
        let continuation_indent = wrap_indent(row, desc_col, area.width);
        let options = RtOptions::new(area.width as usize)
            .initial_indent(Line::from(""))
            .subsequent_indent(Line::from(" ".repeat(continuation_indent)));
        let wrapped = word_wrap_line(&full_line, options);

        // Render the wrapped lines.
        for line in wrapped {
            if cur_y >= area.y + area.height {
                break;
            }
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
}

/// Render rows as a single line each (no wrapping), truncating overflow with an ellipsis.
///
/// This shares the same visible-range and selection rules as [`render_rows`],
/// but uses single-line truncation instead of wrapping for dense popups. The
/// description column logic still applies so alignment matches wrapped views.
pub(crate) fn render_rows_single_line(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) {
    if rows_all.is_empty() {
        if area.height > 0 {
            Line::from(empty_message.dim().italic()).render(area, buf);
        }
        return;
    }

    let visible_items = max_results
        .min(rows_all.len())
        .min(area.height.max(1) as usize);

    let mut start_idx = state.scroll_top.min(rows_all.len().saturating_sub(1));
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

    let desc_col = compute_desc_col(rows_all, start_idx, visible_items, area.width);

    let mut cur_y = area.y;
    for (i, row) in rows_all
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_items)
    {
        if cur_y >= area.y + area.height {
            break;
        }

        let mut full_line = build_full_line(row, desc_col);
        if Some(i) == state.selected_idx {
            full_line.spans.iter_mut().for_each(|span| {
                span.style = Style::default().fg(Color::Cyan).bold();
            });
        }

        let full_line = truncate_line_with_ellipsis_if_overflow(full_line, area.width as usize);
        full_line.render(
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

/// Compute the number of terminal rows required to render the visible items.
///
/// This mirrors the wrapping and alignment logic used by [`render_rows`], using
/// the same description column alignment and wrapping options. The returned
/// height is clamped to at least one row to account for the empty-state message
/// when there are no rows to render.
pub(crate) fn measure_rows_height(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
) -> u16 {
    if rows_all.is_empty() {
        return 1; // placeholder "no matches" line
    }

    let content_width = width.saturating_sub(1).max(1);

    let visible_items = max_results.min(rows_all.len());
    let mut start_idx = state.scroll_top.min(rows_all.len().saturating_sub(1));
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

    let desc_col = compute_desc_col(rows_all, start_idx, visible_items, content_width);

    use crate::wrapping::RtOptions;
    use crate::wrapping::word_wrap_line;
    let mut total: u16 = 0;
    for row in rows_all
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_items)
        .map(|(_, r)| r)
    {
        let full_line = build_full_line(row, desc_col);
        let continuation_indent = wrap_indent(row, desc_col, content_width);
        let opts = RtOptions::new(content_width as usize)
            .initial_indent(Line::from(""))
            .subsequent_indent(Line::from(" ".repeat(continuation_indent)));
        let wrapped_lines = word_wrap_line(&full_line, opts).len();
        total = total.saturating_add(wrapped_lines as u16);
    }
    total.max(1)
}
