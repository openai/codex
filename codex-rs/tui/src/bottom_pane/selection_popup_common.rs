use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
// Note: Table-based layout previously used Constraint; the manual renderer
// below no longer requires it.
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;

use crate::key_hint::ShortcutHint;
use crate::line_truncation::line_width;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::accent_style;
use crate::style::user_message_style;
use crate::width::display_width;

use super::scroll_state::ScrollState;
use super::selection_row_layout::SelectionDescriptionLayout;
use super::selection_row_layout::build_full_line;
use super::selection_row_layout::line_to_owned;

/// Render-ready representation of one row in a selection popup.
///
/// This type contains presentation-focused fields that are intentionally more
/// concrete than source domain models. `match_indices` are character offsets
/// into `name`, and `wrap_indent` is interpreted in terminal cell columns.
#[derive(Default)]
pub(crate) struct GenericDisplayRow {
    pub name: String,
    /// Optional full-width selection treatment; other rows use the shared text accent.
    pub selection_style: Option<Style>,
    pub name_prefix_spans: Vec<Span<'static>>,
    pub display_shortcut: Option<ShortcutHint>,
    pub match_indices: Option<Vec<usize>>, // indices to bold (char positions)
    pub description: Option<String>,       // optional grey text after the name
    /// Required result identity, retained even when the description column hides.
    pub category_tag: Option<String>,
    pub disabled_reason: Option<String>, // optional disabled message
    pub is_disabled: bool,
    pub wrap_indent: Option<usize>, // optional indent for wrapped lines
}

/// Controls how selection rows choose the split between left/right name/description columns.
///
/// Callers should use the same mode for both measurement and rendering, or the
/// popup can reserve the wrong number of lines and clip content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ColumnWidthMode {
    /// Derive column placement from only the visible viewport rows.
    #[default]
    AutoVisible,
    /// Derive column placement from all rows so scrolling does not shift columns.
    AutoAllRows,
}

/// Column-width behavior plus an optional shared left-column width override.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ColumnWidthConfig {
    pub mode: ColumnWidthMode,
    pub name_column_width: Option<usize>,
    pub description_layout: SelectionDescriptionLayout,
}

impl ColumnWidthConfig {
    pub(crate) const fn new(mode: ColumnWidthMode, name_column_width: Option<usize>) -> Self {
        Self {
            mode,
            name_column_width,
            description_layout: SelectionDescriptionLayout::Columns,
        }
    }

    pub(crate) const fn with_description_layout(
        mut self,
        description_layout: SelectionDescriptionLayout,
    ) -> Self {
        self.description_layout = description_layout;
        self
    }
}

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

fn compute_desc_col(
    rows_all: &[GenericDisplayRow],
    start_idx: usize,
    visible_items: usize,
    content_width: u16,
    column_width: ColumnWidthConfig,
) -> usize {
    if content_width <= 1 {
        return 0;
    }

    // Leave at least 30% of the content width available for descriptions.
    let max_desc_col = usize::from(content_width.saturating_sub(/*rhs*/ 1));
    let max_auto_desc_col =
        max_desc_col.min((usize::from(content_width) * 70 / 100).max(/*other*/ 1));
    let (start, count) = match column_width.mode {
        ColumnWidthMode::AutoVisible => (start_idx, visible_items),
        ColumnWidthMode::AutoAllRows => (0, rows_all.len()),
    };
    let max_name_width = rows_all
        .iter()
        .skip(start)
        .take(count)
        .map(|row| {
            let mut spans = row.name_prefix_spans.clone();
            spans.push(row.name.clone().into());
            if row.disabled_reason.is_some() {
                spans.push(" (disabled)".dim());
            }
            line_width(&Line::from(spans))
        })
        .max()
        .unwrap_or(/*default*/ 0);

    let desc_col = column_width
        .name_column_width
        .map(|width| width.max(max_name_width))
        .unwrap_or(max_name_width)
        .saturating_add(/*rhs*/ 2)
        .min(max_auto_desc_col);
    if column_width
        .description_layout
        .should_hide(content_width, desc_col)
    {
        0
    } else {
        desc_col
    }
}

/// Determine how many spaces to indent wrapped lines for a row.
fn wrap_indent(row: &GenericDisplayRow, desc_col: usize, max_width: u16) -> usize {
    let max_indent = max_width.saturating_sub(1) as usize;
    let indent = row.wrap_indent.unwrap_or_else(|| {
        if desc_col == 0 {
            line_width(&Line::from(row.name_prefix_spans.clone()))
        } else if row.description.is_some() || row.disabled_reason.is_some() {
            desc_col
        } else {
            0
        }
    });
    indent.min(max_indent)
}

fn should_wrap_name_in_column(row: &GenericDisplayRow) -> bool {
    // This path intentionally targets plain option rows that opt into wrapped
    // labels. Styled/fuzzy-matched rows use the combined-line path.
    row.wrap_indent.is_some()
        && row.description.is_some()
        && row.disabled_reason.is_none()
        && row.match_indices.is_none()
        && row.display_shortcut.is_none()
        && row.category_tag.is_none()
        && row.name_prefix_spans.is_empty()
}

fn wrap_two_column_row(row: &GenericDisplayRow, desc_col: usize, width: u16) -> Vec<Line<'static>> {
    use crate::wrapping::RtOptions;
    use crate::wrapping::word_wrap_lines;

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

    let name_options = RtOptions::new(left_width)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(" ".repeat(name_wrap_indent)));
    let name_lines = word_wrap_lines(row.name.lines(), name_options);

    let desc_options = RtOptions::new(right_width).initial_indent(Line::from(""));
    let desc_lines = word_wrap_lines(description.lines(), desc_options);

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
                .map(|span| display_width(span.content.as_ref()))
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

fn wrap_standard_row(
    row: &GenericDisplayRow,
    desc_col: usize,
    width: u16,
    description_layout: SelectionDescriptionLayout,
) -> Vec<Line<'static>> {
    use crate::wrapping::RtOptions;
    use crate::wrapping::word_wrap_line;

    let full_line = build_full_line(row, desc_col, width, description_layout);
    let continuation_indent = wrap_indent(row, desc_col, width);
    let options = RtOptions::new(width.max(1) as usize)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(" ".repeat(continuation_indent)));
    word_wrap_line(&full_line, options)
        .into_iter()
        .map(line_to_owned)
        .collect()
}

fn wrap_row_lines(
    row: &GenericDisplayRow,
    desc_col: usize,
    width: u16,
    description_layout: SelectionDescriptionLayout,
) -> Vec<Line<'static>> {
    if desc_col > 0 && should_wrap_name_in_column(row) {
        let wrapped = wrap_two_column_row(row, desc_col, width);
        if !wrapped.is_empty() {
            return wrapped;
        }
    }

    wrap_standard_row(row, desc_col, width, description_layout)
}

fn apply_row_state_style(
    lines: &mut [Line<'static>],
    selected: bool,
    is_disabled: bool,
    selection_style: Option<Style>,
) {
    if selected {
        for line in lines.iter_mut() {
            if let Some(style) = selection_style {
                line.style = style;
            }
            line.spans.iter_mut().for_each(|span| {
                let Some(selected) = selection_style else {
                    span.style = accent_style();
                    return;
                };
                let secondary = span.style.add_modifier.contains(Modifier::DIM);
                let foreground = span.style.fg.or(selected.fg).unwrap_or_default();
                span.style = selected.patch(span.style).not_dim();
                span.style.bg = selected.bg;
                if selected.add_modifier.contains(Modifier::REVERSED) {
                    span.style.fg = selected.fg;
                    span.style = span.style.reversed();
                } else {
                    span.style.fg = Some(crate::style::readable_color_on(foreground, selected.bg));
                    span.style = span.style.not_reversed();
                }
                if secondary {
                    span.style = span.style.not_bold();
                }
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

#[derive(Clone, Copy)]
struct WrappedViewport {
    width: u16,
    height: u16,
    description_layout: SelectionDescriptionLayout,
}

fn is_selected_visible_in_wrapped_viewport(
    rows_all: &[GenericDisplayRow],
    start_idx: usize,
    max_items: usize,
    selected_idx: usize,
    desc_col: usize,
    viewport: WrappedViewport,
) -> bool {
    if viewport.height == 0 {
        return false;
    }

    let mut used_lines = 0usize;
    let viewport_height = viewport.height as usize;
    for (idx, row) in rows_all.iter().enumerate().skip(start_idx).take(max_items) {
        let row_lines = wrap_row_lines(row, desc_col, viewport.width, viewport.description_layout)
            .len()
            .max(1);
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
    column_width: ColumnWidthConfig,
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
        let desc_col =
            compute_desc_col(rows_all, start_idx, desc_measure_items, width, column_width);
        if is_selected_visible_in_wrapped_viewport(
            rows_all,
            start_idx,
            max_items,
            sel,
            desc_col,
            WrappedViewport {
                width,
                height: viewport_height,
                description_layout: column_width.description_layout,
            },
        ) {
            break;
        }
        start_idx = start_idx.saturating_add(1);
    }
    start_idx
}

/// Counts painted rows and items and reports content hidden outside the viewport.
/// The line count includes the single-line empty placeholder when shown.
#[derive(Clone, Copy, Default)]
pub(crate) struct RenderedRows {
    pub(crate) lines: u16,
    pub(crate) items: usize,
    pub(crate) has_above: bool,
    /// Includes a description whose final wrapped lines were clipped.
    pub(crate) has_below: bool,
}

fn render_rows_inner(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
    column_width: ColumnWidthConfig,
) -> RenderedRows {
    if rows_all.is_empty() {
        if area.height > 0 {
            Line::from(empty_message.dim().italic()).render(area, buf);
        }
        // Count the placeholder line only when there is vertical space to draw it.
        return RenderedRows {
            lines: u16::from(area.height > 0),
            items: 0,
            ..Default::default()
        };
    }

    let max_items = max_results.min(rows_all.len());
    if max_items == 0 {
        return RenderedRows::default();
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
        column_width,
    );

    let desc_col = compute_desc_col(
        rows_all,
        start_idx,
        desc_measure_items,
        area.width,
        column_width,
    );

    // Render items, wrapping descriptions and aligning wrapped lines under the
    // shared description column. Stop when we run out of vertical space.
    let mut cur_y = area.y;
    let mut rendered_lines: u16 = 0;
    let mut rendered_items = 0;
    let mut clipped = false;
    for (i, row) in rows_all.iter().enumerate().skip(start_idx).take(max_items) {
        if cur_y >= area.y + area.height {
            break;
        }

        let mut wrapped =
            wrap_row_lines(row, desc_col, area.width, column_width.description_layout);
        clipped |= wrapped.len() > usize::from(area.bottom().saturating_sub(cur_y));
        apply_row_state_style(
            &mut wrapped,
            Some(i) == state.selected_idx && !row.is_disabled,
            row.is_disabled,
            row.selection_style,
        );

        // Render the wrapped lines.
        let mut rendered_item = false;
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
            rendered_item = true;
        }
        if rendered_item {
            rendered_items += 1;
        }
    }

    RenderedRows {
        lines: rendered_lines,
        items: rendered_items,
        has_above: start_idx > 0,
        has_below: clipped || start_idx + rendered_items < rows_all.len(),
    }
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
        ColumnWidthConfig::default(),
    )
    .lines
}

/// Render a list of rows using the provided ScrollState and explicit
/// [`ColumnWidthMode`] behavior.
///
/// This is the low-level entry point for callers that need to thread a mode
/// through higher-level configuration.
/// Returns the terminal lines and selectable items actually rendered.
pub(crate) fn render_rows_with_col_width_mode(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
    column_width: ColumnWidthConfig,
) -> RenderedRows {
    render_rows_inner(
        area,
        buf,
        rows_all,
        state,
        max_results,
        empty_message,
        column_width,
    )
}

/// Render a list of rows as a single line each (no wrapping), truncating overflow with an
/// ellipsis while honoring the configured column width behavior.
/// Returns the terminal lines and selectable items actually rendered.
pub(crate) fn render_rows_single_line_with_col_width_mode(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
    column_width: ColumnWidthConfig,
) -> RenderedRows {
    if rows_all.is_empty() {
        if area.height > 0 {
            Line::from(empty_message.dim().italic()).render(area, buf);
        }
        // Count the placeholder line only when there is vertical space to draw it.
        return RenderedRows {
            lines: u16::from(area.height > 0),
            items: 0,
            ..Default::default()
        };
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

    let desc_col = compute_desc_col(rows_all, start_idx, visible_items, area.width, column_width);

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

        let mut full_line =
            build_full_line(row, desc_col, area.width, column_width.description_layout);
        apply_row_state_style(
            std::slice::from_mut(&mut full_line),
            Some(i) == state.selected_idx && !row.is_disabled,
            row.is_disabled,
            row.selection_style,
        );

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

    RenderedRows {
        lines: rendered_lines,
        items: rendered_lines as usize,
        has_above: start_idx > 0,
        has_below: start_idx + usize::from(rendered_lines) < rows_all.len(),
    }
}

/// Compute the number of terminal rows required to render up to `max_results`
/// items from `rows_all` given the current scroll/selection state and the
/// available `width`. Pass the render area's width after applying caller insets;
/// this helper does not reserve a scrollbar column. Accounts for description
/// wrapping and alignment so the caller can allocate sufficient vertical space.
///
/// This function matches [`render_rows`] semantics (`AutoVisible` column
/// sizing). Mixing it with stable render modes can under- or
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
        ColumnWidthConfig::default(),
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
    column_width: ColumnWidthConfig,
) -> u16 {
    measure_rows_height_inner(rows_all, state, max_results, width, column_width)
}

fn measure_rows_height_inner(
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
    column_width: ColumnWidthConfig,
) -> u16 {
    if rows_all.is_empty() {
        return 1; // placeholder "no matches" line
    }

    // Match the renderer's full row width so exact-fit descriptions do not reserve an extra line.
    let content_width = width.max(/*other*/ 1);

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
        column_width,
    );

    let mut total: u16 = 0;
    for row in rows_all
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_items)
        .map(|(_, r)| r)
    {
        let wrapped_lines = wrap_row_lines(
            row,
            desc_col,
            content_width,
            column_width.description_layout,
        )
        .len();
        total = total.saturating_add(wrapped_lines as u16);
    }
    total.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::style::Modifier;

    #[test]
    fn narrow_description_columns_hide_without_stacking() {
        let rows = [
            GenericDisplayRow {
                name: "日本語".into(),
                name_prefix_spans: vec!["› 1. ".into()],
                description: Some("Details".into()),
                ..Default::default()
            },
            GenericDisplayRow {
                name: "Blocked".into(),
                name_prefix_spans: vec!["  2. ".into()],
                disabled_reason: Some("Unavailable".into()),
                is_disabled: true,
                ..Default::default()
            },
        ];
        let state = ScrollState::new();
        let columns = ColumnWidthConfig::new(
            ColumnWidthMode::AutoAllRows,
            /*name_column_width*/ None,
        )
        .with_description_layout(SelectionDescriptionLayout::HideWhenNarrow {
            min_description_width: 24,
        });
        let mut snapshots = Vec::new();
        for width in [48, 49, 80] {
            let height = measure_rows_height_with_col_width_mode(
                &rows, &state, /*max_results*/ 2, width, columns,
            );
            assert_eq!(height, 2);
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            let mut wrapped = Buffer::empty(area);
            render_rows_with_col_width_mode(
                area,
                &mut wrapped,
                &rows,
                &state,
                /*max_results*/ 2,
                "",
                columns,
            );
            let mut single_line = Buffer::empty(area);
            render_rows_single_line_with_col_width_mode(
                area,
                &mut single_line,
                &rows,
                &state,
                /*max_results*/ 2,
                "",
                columns,
            );
            assert_eq!(wrapped, single_line);
            let text = wrapped
                .content
                .chunks(usize::from(width))
                .map(|row| {
                    row.iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(text.contains("Details"), width >= 49);
            assert_eq!(text.contains("Unavailable"), width >= 49);
            assert!(text.contains("Blocked (disabled)"));
            snapshots.push(format!("width={width}\n{text}"));
        }
        insta::assert_snapshot!("description_column_visibility", snapshots.join("\n\n"));
    }

    #[test]
    fn exact_fit_description_reserves_only_the_rendered_row() {
        let rows = [GenericDisplayRow {
            name: "/model".to_string(),
            description: Some("fits exactly".to_string()),
            ..Default::default()
        }];
        let state = ScrollState::new();
        let width = 20;
        let height = measure_rows_height(&rows, &state, /*max_results*/ 1, width);
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buf = Buffer::empty(area);
        let rendered = render_rows(
            area,
            &mut buf,
            &rows,
            &state,
            /*max_results*/ 1,
            "no matches",
        );
        assert_eq!(height, rendered);
        let text = buf
            .content()
            .chunks(usize::from(width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(text, @"/model  fits exactly");
    }

    #[test]
    fn one_cell_width_falls_back_without_panic_for_wrapped_two_column_rows() {
        let row = GenericDisplayRow {
            name: "1. Very long option label".to_string(),
            description: Some("Very long description".to_string()),
            wrap_indent: Some(4),
            ..Default::default()
        };

        let two_col = wrap_two_column_row(&row, /*desc_col*/ 0, /*width*/ 1);
        assert_eq!(two_col.len(), 0);
    }

    #[test]
    fn popup_name_truncation_counts_halfwidth_sound_marks() {
        for (name, desc_col, match_index, expected) in
            [("abｶﾞc", 6, None, "abｶﾞ…"), ("aｶﾞc", 4, Some(1), "a…")]
        {
            let row = GenericDisplayRow {
                name: name.to_string(),
                description: Some("description".to_string()),
                match_indices: match_index.map(|index| vec![index]),
                ..Default::default()
            };
            let text = build_full_line(
                &row,
                desc_col,
                /*width*/ 80,
                SelectionDescriptionLayout::Columns,
            )
            .to_string();

            assert!(text.starts_with(expected), "unexpected row: {text:?}");
        }
    }

    #[test]
    fn fuzzy_matched_emoji_graphemes_keep_description_alignment() {
        let rows = [
            GenericDisplayRow {
                name: "👍🏻".to_string(),
                match_indices: Some(vec![1]),
                description: Some("description".to_string()),
                ..Default::default()
            },
            GenericDisplayRow {
                name: "👨‍👩‍👧‍👦".to_string(),
                match_indices: Some(vec![2]),
                description: Some("description".to_string()),
                ..Default::default()
            },
        ];
        let area = Rect::new(0, 0, /*width*/ 20, /*height*/ 2);
        let mut buf = Buffer::empty(area);

        for (row_index, row) in rows.iter().enumerate() {
            let line = build_full_line(
                row,
                /*desc_col*/ 4,
                area.width,
                SelectionDescriptionLayout::Columns,
            );
            let name = line.spans.first().expect("fuzzy-matched name span");
            assert_eq!(name.content.as_ref(), row.name);
            assert!(name.style.add_modifier.contains(Modifier::BOLD));

            let row_area = Rect::new(area.x, area.y + row_index as u16, area.width, 1);
            ratatui::widgets::Widget::render(
                ratatui::widgets::Paragraph::new(line),
                row_area,
                &mut buf,
            );
            assert_eq!(buf[(4, row_index as u16)].symbol(), "d");
        }

        insta::assert_snapshot!("popup_fuzzy_matched_emoji_graphemes", format!("{buf:?}"));
    }

    #[test]
    fn wrapped_two_column_rows_count_halfwidth_sound_marks() {
        let rows = vec![GenericDisplayRow {
            name: "abｶﾞc".to_string(),
            description: Some("abﾊﾟc".to_string()),
            wrap_indent: Some(0),
            ..Default::default()
        }];
        let area = Rect::new(0, 0, 9, 2);
        let mut buf = Buffer::empty(area);

        let rendered_lines = render_rows(
            area,
            &mut buf,
            &rows,
            &ScrollState::default(),
            /*max_results*/ 1,
            "no rows",
        );
        let rendered = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(rendered_lines, 2);
        insta::assert_snapshot!(rendered, @r"
        abｶﾞ  ab
        c     ﾊﾟ c
        ");
    }

    #[test]
    fn wrapped_two_column_rows_preserve_hard_line_breaks() {
        let row = GenericDisplayRow {
            name: "first\nsecond".to_string(),
            description: Some("alpha\nbeta".to_string()),
            wrap_indent: Some(0),
            ..Default::default()
        };

        let rendered = wrap_two_column_row(&row, /*desc_col*/ 8, /*width*/ 24)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        insta::assert_snapshot!(rendered, @r"
        first   alpha
        second  beta
        ");
    }

    #[test]
    fn selection_preserves_semantic_spans_and_secondary_hierarchy() {
        for (fg, bg) in [
            ((32, 32, 32), (255, 255, 255)),
            ((230, 230, 230), (18, 20, 30)),
        ] {
            crate::terminal_palette::with_test_default_colors(
                crate::terminal_probe::DefaultColors { fg, bg },
                || {
                    for selected in [
                        crate::style::selection_style(),
                        Style::default()
                            .fg(Color::Reset)
                            .bg(Color::Reset)
                            .bold()
                            .reversed(),
                    ] {
                        for single_line in [true, false] {
                            let rows = [GenericDisplayRow {
                                name_prefix_spans: vec![
                                    "L".underlined(),
                                    "M ".into(),
                                    "Ready ".green().italic(),
                                ],
                                name: "provider".into(),
                                description: Some("secondary details".into()),
                                selection_style: Some(selected),
                                ..Default::default()
                            }];
                            let state = ScrollState {
                                selected_idx: Some(0),
                                scroll_top: 0,
                            };
                            let area = Rect::new(
                                /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 1,
                            );
                            let mut buffer = Buffer::empty(area);
                            if single_line {
                                render_rows_single_line_with_col_width_mode(
                                    area,
                                    &mut buffer,
                                    &rows,
                                    &state,
                                    /*max_results*/ 1,
                                    "",
                                    ColumnWidthConfig::default(),
                                );
                            } else {
                                render_rows(
                                    area,
                                    &mut buffer,
                                    &rows,
                                    &state,
                                    /*max_results*/ 1,
                                    "",
                                );
                            }
                            let line = buffer
                                .content
                                .iter()
                                .map(ratatui::buffer::Cell::symbol)
                                .collect::<String>();
                            let status = &buffer[(3, 0)];
                            let description_x =
                                line.find("secondary").expect("visible secondary text") as u16;
                            assert_eq!(
                                (
                                    buffer[(0, 0)].modifier,
                                    status.modifier,
                                    buffer[(description_x, 0)].modifier
                                ),
                                (
                                    Modifier::BOLD
                                        | Modifier::UNDERLINED
                                        | (selected.add_modifier & Modifier::REVERSED),
                                    Modifier::BOLD
                                        | Modifier::ITALIC
                                        | (selected.add_modifier & Modifier::REVERSED),
                                    selected.add_modifier & Modifier::REVERSED,
                                )
                            );
                            assert_eq!(
                                status.fg,
                                if selected.add_modifier.contains(Modifier::REVERSED) {
                                    Color::Reset
                                } else {
                                    Color::Green
                                }
                            );
                            assert_eq!(buffer[(0, 0)].fg, selected.fg.unwrap());
                            assert!(
                                buffer
                                    .content
                                    .iter()
                                    .all(|cell| cell.bg == selected.bg.unwrap())
                            );
                        }
                    }
                },
            );
        }
    }

    #[test]
    fn selected_rows_use_the_shared_accent_style() {
        let rows = vec![GenericDisplayRow {
            name: "selected".to_string(),
            ..Default::default()
        }];
        let state = ScrollState {
            selected_idx: Some(0),
            ..Default::default()
        };
        let area = Rect::new(0, 0, 16, 1);
        let mut buf = Buffer::empty(area);

        render_rows(
            area, &mut buf, &rows, &state, /*max_results*/ 1, "no rows",
        );

        let style = buf[(0, 0)].style();
        let expected = accent_style();
        assert_eq!(style.fg, expected.fg);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
