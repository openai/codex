use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
// Note: Table-based layout previously used Constraint; the manual renderer
// below no longer requires it.
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;

use crate::key_hint::KeyBinding;
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
use super::selection_row_layout::wrap_stacked_row;

/// Render-ready representation of one row in a selection popup.
///
/// This type contains presentation-focused fields that are intentionally more
/// concrete than source domain models. `match_indices` are character offsets
/// into `name`, and `wrap_indent` is interpreted in terminal cell columns.
#[derive(Default)]
pub(crate) struct GenericDisplayRow {
    pub name: String,
    pub name_prefix_spans: Vec<Span<'static>>,
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

    let max_desc_col = content_width.saturating_sub(1) as usize;
    // Reuse the existing fixed split constants to derive the auto cap:
    // if fixed mode is 30/70 (label/description), auto mode caps label width
    // at 70% to keep at least 30% available for descriptions.
    let max_auto_desc_col = max_desc_col.min(
        ((content_width as usize * (FIXED_LEFT_COLUMN_DENOMINATOR - FIXED_LEFT_COLUMN_NUMERATOR))
            / FIXED_LEFT_COLUMN_DENOMINATOR)
            .max(1),
    );
    match column_width.mode {
        ColumnWidthMode::Fixed => ((content_width as usize * FIXED_LEFT_COLUMN_NUMERATOR)
            / FIXED_LEFT_COLUMN_DENOMINATOR)
            .clamp(1, max_desc_col),
        ColumnWidthMode::AutoVisible | ColumnWidthMode::AutoAllRows => {
            let max_name_width = match column_width.mode {
                ColumnWidthMode::AutoVisible => rows_all
                    .iter()
                    .enumerate()
                    .skip(start_idx)
                    .take(visible_items)
                    .map(|(_, row)| {
                        let mut spans = row.name_prefix_spans.clone();
                        spans.push(row.name.clone().into());
                        if row.disabled_reason.is_some() {
                            spans.push(" (disabled)".dim());
                        }
                        line_width(&Line::from(spans))
                    })
                    .max()
                    .unwrap_or(0),
                ColumnWidthMode::AutoAllRows => rows_all
                    .iter()
                    .map(|row| {
                        let mut spans = row.name_prefix_spans.clone();
                        spans.push(row.name.clone().into());
                        if row.disabled_reason.is_some() {
                            spans.push(" (disabled)".dim());
                        }
                        line_width(&Line::from(spans))
                    })
                    .max()
                    .unwrap_or(0),
                ColumnWidthMode::Fixed => 0,
            };

            column_width
                .name_column_width
                .map(|width| width.max(max_name_width))
                .unwrap_or(max_name_width)
                .saturating_add(2)
                .min(max_auto_desc_col)
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

    let full_line = build_full_line(row, desc_col, description_layout);
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
    if description_layout.should_stack(width, desc_col) {
        return wrap_stacked_row(row, width);
    }
    if should_wrap_name_in_column(row) {
        let wrapped = wrap_two_column_row(row, desc_col, width);
        if !wrapped.is_empty() {
            return wrapped;
        }
    }

    wrap_standard_row(row, desc_col, width, description_layout)
}

fn apply_row_state_style(lines: &mut [Line<'static>], selected: bool, is_disabled: bool) {
    if selected {
        for line in lines.iter_mut() {
            line.spans.iter_mut().for_each(|span| {
                span.style = accent_style();
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
    column_width: ColumnWidthConfig,
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
    for (i, row) in rows_all.iter().enumerate().skip(start_idx).take(max_items) {
        if cur_y >= area.y + area.height {
            break;
        }

        let mut wrapped =
            wrap_row_lines(row, desc_col, area.width, column_width.description_layout);
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
        ColumnWidthConfig::default(),
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
    column_width: ColumnWidthConfig,
) -> u16 {
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
    render_rows_single_line_with_col_width_mode(
        area,
        buf,
        rows_all,
        state,
        max_results,
        empty_message,
        ColumnWidthConfig::default(),
    )
}

/// Render a list of rows as a single line each (no wrapping), truncating overflow with an
/// ellipsis while honoring the configured column width behavior.
pub(crate) fn render_rows_single_line_with_col_width_mode(
    area: Rect,
    buf: &mut Buffer,
    rows_all: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
    column_width: ColumnWidthConfig,
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

        let mut full_line = build_full_line(row, desc_col, column_width.description_layout);
        if Some(i) == state.selected_idx && !row.is_disabled {
            full_line.spans.iter_mut().for_each(|span| {
                span.style = accent_style();
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
    use ratatui::style::Modifier;

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
            let text =
                build_full_line(&row, desc_col, SelectionDescriptionLayout::Columns).to_string();

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
