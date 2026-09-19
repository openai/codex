//! Fit picker chrome before assigning result rows so short panels retain their controls.
//!
//! Optional gaps and previews yield first; headers, tabs, search, and at least one
//! result remain visible whenever the available height can accommodate them.
//! Overflow spacers are reserved separately from the minimum result content.

use ratatui::layout::Rect;

pub(super) struct PickerLayoutSizes {
    pub header: u16,
    pub header_gap: u16,
    pub tabs: u16,
    pub search: u16,
    pub rows: u16,
    pub side: u16,
}

pub(super) fn picker_areas(area: Rect, sizes: PickerLayoutSizes) -> [Rect; 10] {
    let tabs = sizes.tabs.min(area.height);
    let search = sizes.search.min(area.height.saturating_sub(tabs));
    let minimum_rows = sizes.rows.min(if tabs > 0 { 1 } else { 3 });
    let header = sizes
        .header
        .min(area.height.saturating_sub(tabs + search + minimum_rows));
    let mut remaining = area.height.saturating_sub(header + tabs + search);
    let indicators = u16::from(remaining >= minimum_rows + 2);
    remaining = remaining.saturating_sub(indicators * 2);
    let minimum_rows = sizes.rows.min(/*other*/ 3).min(remaining);
    let header_gap = if header > 0 {
        sizes.header_gap.min(remaining.saturating_sub(minimum_rows))
    } else {
        0
    };
    remaining = remaining.saturating_sub(header_gap);
    let tab_gap = u16::from(tabs > 0 && search > 0 && remaining > minimum_rows);
    remaining = remaining.saturating_sub(tab_gap);
    let side = sizes.side.min(remaining.saturating_sub(minimum_rows + 1));
    let side_gap = u16::from(side > 0);
    let rows = sizes.rows.min(remaining.saturating_sub(side + side_gap));
    let mut y = area.y;
    [
        header, header_gap, tabs, tab_gap, search, indicators, rows, indicators, side_gap, side,
    ]
    .map(|height| {
        let rect = Rect::new(area.x, y, area.width, height);
        y = y.saturating_add(height);
        rect
    })
}
