use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
// Note: Table-based layout previously used Constraint; the manual renderer
// below no longer requires it.
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;
use std::borrow::Cow;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::key_hint::KeyBinding;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;

use super::scroll_state::ScrollState;

/// Render-ready representation of one row in a selection popup.
///
/// This type contains presentation-focused fields that are intentionally more
/// concrete than source domain models. `match_indices` are character offsets
/// into `name`, and `wrap_indent` is interpreted in terminal cell columns.
#[derive(Default)]
pub(crate) struct GenericDisplayRow {
    pub name: String,
    pub display_shortcut: Option<KeyBinding>,
    pub match_indices: Option<Vec<usize>>, // indices to bold (char positions)
    pub description: Option<String>,       // optional grey text after the name
    pub category_tag: Option<String>,      // optional right-side category label
    pub disabled_reason: Option<String>,   // optional disabled message
    pub is_disabled: bool,
    pub wrap_indent: Option<usize>, // optional indent for wrapped lines
}

/// Controls how selection rows choose the split between left/right name/description columns.
///
/// Callers should use the same mode for both measurement and rendering, or the
/// popup can reserve the wrong number of lines and clip content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum ColumnWidthMode {
    /// Derive column placement from only the visible viewport rows.
    #[default]
    AutoVisible,
    /// Derive column placement from all rows so scrolling does not shift columns.
    AutoAllRows,
    /// Use a fixed two-column split: 30% left (name), 70% right (description).
    Fixed,
}

// Fixed split used by explicitly fixed column mode: 30% label, 70%
// description.
const FIXED_LEFT_COLUMN_NUMERATOR: usize = 3;
const FIXED_LEFT_COLUMN_DENOMINATOR: usize = 10;

const MENU_SURFACE_INSET_V: u16 = 1;
const MENU_SURFACE_INSET_H: u16 = 2;

/// Apply the shared "menu surface" padding used by bottom-pane overlays.
///
/// Rendering code should generally call [`render_menu_surface`] and then lay
/// out content inside the returned inset rect.
pub(crate) fn menu_surface_inset(area: Rect) -> Rect {
    area.inset(Insets::vh(MENU_SURFACE_INSET_V, MENU_SURFACE_INSET_H))
}

/// Total vertical padding introduced by the menu surface treatment.
pub(crate) const fn menu_surface_padding_height() -> u16 {
    MENU_SURFACE_INSET_V * 2
}

/// Paint the shared menu background and return the inset content area.
///
/// This keeps the surface treatment consistent across selection-style overlays
/// (for example `/model`, approvals, and request-user-input). Callers should
/// render all inner content in the returned rect, not the original area.
pub(crate) fn render_menu_surface(area: Rect, buf: &mut Buffer) -> Rect {
    if area.is_empty() {
        return area;
    }
    Block::default()
        .style(user_message_style())
        .render(area, buf);
    menu_surface_inset(area)
}

/// Wrap a styled line while preserving span styles.
///
/// The function clamps `width` to at least one terminal cell so callers can use
/// it safely with narrow layouts.
pub(crate) fn wrap_styled_line<'a>(line: &'a Line<'a>, width: u16) -> Vec<Line<'a>> {
    use crate::wrapping::RtOptions;
    use crate::wrapping::word_wrap_line;

    let width = width.max(1) as usize;
    let opts = RtOptions::new(width)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(""));
    word_wrap_line(line, opts)
}

fn line_width(line: &Line<'_>) -> usize {
    line.iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

fn line_to_owned(line: Line<'_>) -> Line<'static> {
    Line {
        style: line.style,
        alignment: line.alignment,
        spans: line
            .spans
            .into_iter()
            .map(|span| Span {
                style: span.style,
                content: Cow::Owned(span.content.into_owned()),
            })
            .collect(),
    }
}

pub(crate) fn truncate_line_to_width(line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let Line {
        style,
        alignment,
        spans,
    } = line;
    let mut used = 0usize;
    let mut spans_out: Vec<Span<'static>> = Vec::new();

    for span in spans {
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

    Line {
        style,
        alignment,
        spans: spans_out,
    }
}

pub(crate) fn truncate_line_with_ellipsis_if_overflow(
    line: Line<'static>,
    max_width: usize,
) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let width = line_width(&line);
    if width <= max_width {
        return line;
    }

    let truncated = truncate_line_to_width(line, max_width.saturating_sub(1));
    let Line {
        style,
        alignment,
        mut spans,
    } = truncated;
    let ellipsis_style = spans.last().map(|span| span.style).unwrap_or_default();
    spans.push(Span::styled("…", ellipsis_style));
    Line {
        style,
        alignment,
        spans,
    }
}

fn compute_desc_col(
    rows_all: &[GenericDisplayRow],
    start_idx: usize,
    visible_items: usize,
    content_width: u16,
    col_width_mode: ColumnWidthMode,
) -> usize {
    if content_width <= 1 {
        return 0;
    }

    let max_desc_col = content_width.saturating_sub(1) as usize;
    // Reuse the existing fixed split constants to derive the auto cap:
    // if fixed mode is 30/70 (label/description), auto mode caps label width
    // at 70% to keep at least 30% available for descriptions.
    let max_auto_desc_col = max_desc_col.min(
        ((content_width as usize * (FIXED_LEFT_COLUMN_DENOMINATOR - FIXED_LEFT_COLUMN_NUMERATOR))
            / FIXED_LEFT_COLUMN_DENOMINATOR)
            .max(1),
    );
    match col_width_mode {
        ColumnWidthMode::Fixed => ((content_width as usize * FIXED_LEFT_COLUMN_NUMERATOR)
            / FIXED_LEFT_COLUMN_DENOMINATOR)
            .clamp(1, max_desc_col),
        ColumnWidthMode::AutoVisible | ColumnWidthMode::AutoAllRows => {
            let max_name_width = match col_width_mode {
                ColumnWidthMode::AutoVisible => rows_all
                    .iter()
                    .enumerate()
                    .skip(start_idx)
                    .take(visible_items)
                    .map(|(_, row)| {
                        let mut spans: Vec<Span> = vec![row.name.clone().into()];
                        if row.disabled_reason.is_some() {
                            spans.push(" (disabled)".dim());
                        }
                        Line::from(spans).width()
                    })
                    .max()
                    .unwrap_or(0),
                ColumnWidthMode::AutoAllRows => rows_all
                    .iter()
                    .map(|row| {
                        let mut spans: Vec<Span> = vec![row.name.clone().into()];
                        if row.disabled_reason.is_some() {
                            spans.push(" (disabled)".dim());
                        }
                        Line::from(spans).width()
                    })
                    .max()
                    .unwrap_or(0),
                ColumnWidthMode::Fixed => 0,
            };

            max_name_width.saturating_add(2).min(max_auto_desc_col)
        }
    }
}

/// Determine how many spaces to indent wrapped lines for a row.
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

fn should_wrap_name_in_column(row: &GenericDisplayRow) -> bool {
    // This path intentionally targets plain option rows that opt into wrapped
    // labels. Styled/fuzzy-matched rows keep the legacy combined-line path.
    row.wrap_indent.is_some()
        && row.description.is_some()
        && row.disabled_reason.is_none()
        && row.match_indices.is_none()
        && row.display_shortcut.is_none()
        && row.category_tag.is_none()
}

fn wrap_two_column_row(row: &GenericDisplayRow, desc_col: usize, width: u16) -> Vec<Line<'static>> {
    let Some(description) = row.description.as_deref() else {
        return Vec::new();
    };

    let width = width.max(1);
    let max_desc_col = width.saturating_sub(1) as usize;
    if max_desc_col == 0 {
        // No valid description column exists at this width; let callers fall
        // back to single-line wrapping path.
        return Vec::new();
    }

    let desc_col = desc_col.clamp(1, max_desc_col);
    let left_width = desc_col.saturating_sub(2).max(1);
    let right_width = width.saturating_sub(desc_col as u16).max(1) as usize;
    let name_wrap_indent = row
        .wrap_indent
        .unwrap_or(0)
        .min(left_width.saturating_sub(1));

    let name_subsequent_indent = " ".repeat(name_wrap_indent);
    let name_options = textwrap::Options::new(left_width)
        .initial_indent("")
        .subsequent_indent(name_subsequent_indent.as_str());
    let name_lines = textwrap::wrap(row.name.as_str(), name_options);

    let desc_options = textwrap::Options::new(right_width).initial_indent("");
    let desc_lines = textwrap::wrap(description, desc_options);

    let rows = name_lines.len().max(desc_lines.len()).max(1);
    let mut out = Vec::with_capacity(rows);
    for idx in 0..rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if let Some(name) = name_lines.get(idx) {
            spans.push(name.to_string().into());
        }

        if let Some(desc) = desc_lines.get(idx) {
            let left_used = spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum::<usize>();
            let gap = if left_used == 0 {
                desc_col
            } else {
                desc_col.saturating_sub(left_used).max(2)
            };
            if gap > 0 {
                spans.push(" ".repeat(gap).into());
            }
            spans.push(desc.to_string().dim());
        }

        out.push(Line::from(spans));
    }

    out
}

fn wrap_standard_row(row: &GenericDisplayRow, desc_col: usize, width: u16) -> Vec<Line<'static>> {
    use crate::wrapping::RtOptions;
    use crate::wrapping::word_wrap_line;

    let full_line = build_full_line(row, desc_col);
    let continuation_indent = wrap_indent(row, desc_col, width);
    let options = RtOptions::new(width.max(1) as usize)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(" ".repeat(continuation_indent)));
    word_wrap_line(&full_line, options)
        .into_iter()
        .map(line_to_owned)
        .collect()
}

fn wrap_row_lines(row: &GenericDisplayRow, desc_col: usize, width: u16) -> Vec<Line<'static>> {
    if should_wrap_name_in_column(row) {
        let wrapped = wrap_two_column_row(row, desc_col, width);
        if !wrapped.is_empty() {
            return wrapped;
        }
    }

    wrap_standard_row(row, desc_col, width)
}

fn apply_row_state_style(lines: &mut [Line<'static>], selected: bool, is_disabled: bool) {
    if selected {
        for line in lines.iter_mut() {
            line.spans.iter_mut().for_each(|span| {
                span.style = Style::default().fg(Color::Cyan).bold();
            });
        }
    }
    if is_disabled {
        for line in lines.iter_mut() {
            line.spans.iter_mut().for_each(|span| {
                span.style = span.style.dim();
            });
        }
    }
}

fn compute_item_window_start(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_items: usize,
) -> usize {
    if rows_all.is_empty() || max_items == 0 {
        return 0;
    }

    let mut start_idx = state.scroll_top.min(rows_all.len().saturating_sub(1));
    if let Some(sel) = state.selected_idx {
        if sel < start_idx {
            start_idx = sel;
        } else {
            let bottom = start_idx.saturating_add(max_items.saturating_sub(1));
            if sel > bottom {
                start_idx = sel + 1 - max_items;
            }
        }
    }
    start_idx
}

fn is_selected_visible_in_wrapped_viewport(
    rows_all: &[GenericDisplayRow],
    start_idx: usize,
    max_items: usize,
    selected_idx: usize,
    desc_col: usize,
    width: u16,
    viewport_height: u16,
) -> bool {
    if viewport_height == 0 {
        return false;
    }

    let mut used_lines = 0usize;
    let viewport_height = viewport_height as usize;
    for (idx, row) in rows_all.iter().enumerate().skip(start_idx).take(max_items) {
        let row_lines = wrap_row_lines(row, desc_col, width).len().max(1);
        // Keep rendering semantics in sync: always show the first row, even if
        // it overflows the viewport.
        if used_lines > 0 && used_lines.saturating_add(row_lines) > viewport_height {
            break;
        }
        if idx == selected_idx {
            return true;
        }
        used_lines = used_lines.saturating_add(row_lines);
        if used_lines >= viewport_height {
            break;
        }
    }
    false
}

fn adjust_start_for_wrapped_selection_visibility(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_items: usize,
    desc_measure_items: usize,
    width: u16,
    viewport_height: u16,
    col_width_mode: ColumnWidthMode,
) -> usize {
    let mut start_idx = compute_item_window_start(rows_all, state, max_items);
    let Some(sel) = state.selected_idx else {
        return start_idx;
    };
    if viewport_height == 0 {
        return start_idx;
    }

    // If wrapped row heights push the selected item out of view, advance the
    // item window until the selected row is visible.
    while start_idx < sel {
        let desc_col = compute_desc_col(
            rows_all,
            start_idx,
            desc_measure_items,
            width,
            col_width_mode,
        );
        if is_selected_visible_in_wrapped_viewport(
            rows_all,
            start_idx,
            max_items,
            sel,
            desc_col,
            width,
            viewport_height,
        ) {
            break;
        }
        start_idx = start_idx.saturating_add(1);
    }
    start_idx
}

/// Build the full display line for a row with the description padded to start
/// at `desc_col`. Applies fuzzy-match bolding when indices are present and
/// dims the description.
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
    if let Some(tag) = row.category_tag.as_deref().filter(|tag| !tag.is_empty()) {
        full_spans.push("  ".into());
        full_spans.push(tag.to_string().dim());
    }
    Line::from(full_spans)
}

/// Render a list of rows using the provided ScrollState, with shared styling
/// and behavior for selection popups.
/// Returns the number of terminal lines actually rendered (including the
/// single-line empty placeholder when shown).
fn render_rows_inner(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
    col_width_mode: ColumnWidthMode,
) -> u16 {
    if rows_all.is_empty() {
        if area.height > 0 {
            Line::from(empty_message.dim().italic()).render(area, buf);
        }
        // Count the placeholder line only when there is vertical space to draw it.
        return u16::from(area.height > 0);
    }

    let max_items = max_results.min(rows_all.len());
    if max_items == 0 {
        return 0;
    }
    let desc_measure_items = max_items.min(area.height.max(1) as usize);

    // Keep item-window semantics, then correct for wrapped row heights so the
    // selected row remains visible in a line-based viewport.
    let start_idx = adjust_start_for_wrapped_selection_visibility(
        rows_all,
        state,
        max_items,
        desc_measure_items,
        area.width,
        area.height,
        col_width_mode,
    );

    let desc_col = compute_desc_col(
        rows_all,
        start_idx,
        desc_measure_items,
        area.width,
        col_width_mode,
    );

    // Render items, wrapping descriptions and aligning wrapped lines under the
    // shared description column. Stop when we run out of vertical space.
    let mut cur_y = area.y;
    let mut rendered_lines: u16 = 0;
    for (i, row) in rows_all.iter().enumerate().skip(start_idx).take(max_items) {
        if cur_y >= area.y + area.height {
            break;
        }

        let mut wrapped = wrap_row_lines(row, desc_col, area.width);
        apply_row_state_style(
            &mut wrapped,
            Some(i) == state.selected_idx && !row.is_disabled,
            row.is_disabled,
        );

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
            rendered_lines = rendered_lines.saturating_add(1);
        }
    }

    rendered_lines
}

/// Render a list of rows using the provided ScrollState, with shared styling
/// and behavior for selection popups.
/// Description alignment is computed from visible rows only, which allows the
/// layout to adapt tightly to the current viewport.
///
/// This function should be paired with [`measure_rows_height`] when reserving
/// space; pairing it with a different measurement mode can cause clipping.
/// Returns the number of terminal lines actually rendered.
pub(crate) fn render_rows(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) -> u16 {
    render_rows_inner(
        area,
        buf,
        rows_all,
        state,
        max_results,
        empty_message,
        ColumnWidthMode::AutoVisible,
    )
}

/// Render a list of rows using the provided ScrollState, with shared styling
/// and behavior for selection popups.
/// This mode keeps column placement stable while scrolling by sizing the
/// description column against the full dataset.
///
/// This function should be paired with
/// [`measure_rows_height_stable_col_widths`] so reserved and rendered heights
/// stay in sync.
/// Returns the number of terminal lines actually rendered.
pub(crate) fn render_rows_stable_col_widths(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) -> u16 {
    render_rows_inner(
        area,
        buf,
        rows_all,
        state,
        max_results,
        empty_message,
        ColumnWidthMode::AutoAllRows,
    )
}

/// Render a list of rows using the provided ScrollState and explicit
/// [`ColumnWidthMode`] behavior.
///
/// This is the low-level entry point for callers that need to thread a mode
/// through higher-level configuration.
/// Returns the number of terminal lines actually rendered.
pub(crate) fn render_rows_with_col_width_mode(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
    col_width_mode: ColumnWidthMode,
) -> u16 {
    render_rows_inner(
        area,
        buf,
        rows_all,
        state,
        max_results,
        empty_message,
        col_width_mode,
    )
}

/// Render rows as a single line each (no wrapping), truncating overflow with an ellipsis.
///
/// This path always uses viewport-local width alignment and is best for dense
/// list UIs where multi-line descriptions would add too much vertical churn.
/// Returns the number of terminal lines actually rendered.
pub(crate) fn render_rows_single_line(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) -> u16 {
    if rows_all.is_empty() {
        if area.height > 0 {
            Line::from(empty_message.dim().italic()).render(area, buf);
        }
        // Count the placeholder line only when there is vertical space to draw it.
        return u16::from(area.height > 0);
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

    let desc_col = compute_desc_col(
        rows_all,
        start_idx,
        visible_items,
        area.width,
        ColumnWidthMode::AutoVisible,
    );

    let mut cur_y = area.y;
    let mut rendered_lines: u16 = 0;
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
        if Some(i) == state.selected_idx && !row.is_disabled {
            full_line.spans.iter_mut().for_each(|span| {
                span.style = Style::default().fg(Color::Cyan).bold();
            });
        }
        if row.is_disabled {
            full_line.spans.iter_mut().for_each(|span| {
                span.style = span.style.dim();
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
        rendered_lines = rendered_lines.saturating_add(1);
    }

    rendered_lines
}

/// Compute the number of terminal rows required to render up to `max_results`
/// items from `rows_all` given the current scroll/selection state and the
/// available `width`. Accounts for description wrapping and alignment so the
/// caller can allocate sufficient vertical space.
///
/// This function matches [`render_rows`] semantics (`AutoVisible` column
/// sizing). Mixing it with stable or fixed render modes can under- or
/// over-estimate required height.
pub(crate) fn measure_rows_height(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
) -> u16 {
    measure_rows_height_inner(
        rows_all,
        state,
        max_results,
        width,
        ColumnWidthMode::AutoVisible,
    )
}

/// Measures selection-row height while using full-dataset column alignment.
/// This should be paired with [`render_rows_stable_col_widths`] so layout
/// reservation matches rendering behavior.
pub(crate) fn measure_rows_height_stable_col_widths(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
) -> u16 {
    measure_rows_height_inner(
        rows_all,
        state,
        max_results,
        width,
        ColumnWidthMode::AutoAllRows,
    )
}

/// Measure selection-row height using explicit [`ColumnWidthMode`] behavior.
///
/// This is the low-level companion to [`render_rows_with_col_width_mode`].
pub(crate) fn measure_rows_height_with_col_width_mode(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
    col_width_mode: ColumnWidthMode,
) -> u16 {
    measure_rows_height_inner(rows_all, state, max_results, width, col_width_mode)
}

fn measure_rows_height_inner(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
    col_width_mode: ColumnWidthMode,
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

    let desc_col = compute_desc_col(
        rows_all,
        start_idx,
        visible_items,
        content_width,
        col_width_mode,
    );

    let mut total: u16 = 0;
    for row in rows_all
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_items)
        .map(|(_, r)| r)
    {
        let wrapped_lines = wrap_row_lines(row, desc_col, content_width).len();
        total = total.saturating_add(wrapped_lines as u16);
    }
    total.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn one_cell_width_falls_back_without_panic_for_wrapped_two_column_rows() {
        let row = GenericDisplayRow {
            name: "1. Very long option label".to_string(),
            description: Some("Very long description".to_string()),
            wrap_indent: Some(4),
            ..Default::default()
        };

        let two_col = wrap_two_column_row(&row, 0, 1);
        assert_eq!(two_col.len(), 0);
    }
}
