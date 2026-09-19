//! Shared row presentation for dedicated picker workflows.
//!
//! These adapters keep domain-specific state and save policies with their views.
//! Measurement and painting share columns, wrapping, and overflow spacer rows.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::picker_style;
use super::scroll_state::ScrollState;
use super::selection_picker_layout::PickerLayoutSizes;
use super::selection_picker_layout::picker_areas;
use super::selection_popup_common as rows;
use super::selection_popup_common::ColumnWidthConfig;
use super::selection_popup_common::ColumnWidthMode;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_row_layout::SelectionDescriptionLayout;
use crate::render::Insets;
use crate::render::RectExt;

const COLUMNS: ColumnWidthConfig = ColumnWidthConfig {
    mode: ColumnWidthMode::AutoAllRows,
    name_column_width: None,
    description_layout: SelectionDescriptionLayout::HideWhenNarrow {
        min_description_width: 24,
    },
};

pub(super) fn measure_rows_height(
    rows: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    width: u16,
) -> u16 {
    rows::measure_rows_height_with_col_width_mode(rows, state, max_results, width, COLUMNS)
        .saturating_add(/*rhs*/ 2)
}

pub(super) fn render_rows(
    area: Rect,
    buf: &mut Buffer,
    rows: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) -> u16 {
    let body = area.inset(Insets::vh(u16::from(area.height >= 3), /*h*/ 0));
    let rendered = rows::render_rows_with_col_width_mode(
        body,
        buf,
        rows,
        state,
        max_results,
        empty_message,
        COLUMNS,
    );
    picker_style::render_scroll_indicators(area, buf, rendered);
    rendered.lines.saturating_add(/*rhs*/ 2).min(area.height)
}

pub(super) fn render_rows_single_line(
    area: Rect,
    buf: &mut Buffer,
    rows: &[GenericDisplayRow],
    state: &ScrollState,
    max_results: usize,
    empty_message: &str,
) -> u16 {
    let body = area.inset(Insets::vh(u16::from(area.height >= 3), /*h*/ 0));
    let rendered = rows::render_rows_single_line_with_col_width_mode(
        body,
        buf,
        rows,
        state,
        max_results,
        empty_message,
        COLUMNS,
    );
    picker_style::render_scroll_indicators(area, buf, rendered);
    rendered.lines.saturating_add(/*rhs*/ 2).min(area.height)
}

/// Keep headers and search above a list, yielding optional gaps on short screens.
pub(super) fn layout(area: Rect, header: u16, search: u16, rows: u16) -> [Rect; 3] {
    let [header, _, _, _, search, above, rows, below, _, _] = picker_areas(
        area.inset(Insets::tlbr(
            /*top*/ 1, /*left*/ 2, /*bottom*/ 0, /*right*/ 2,
        )),
        PickerLayoutSizes {
            header,
            header_gap: 1,
            tabs: 0,
            search,
            rows: rows.saturating_sub(/*rhs*/ 2),
            side: 0,
        },
    );
    [
        header,
        search,
        Rect::new(
            area.x,
            above.y,
            area.width,
            above.height + rows.height + below.height,
        ),
    ]
}
