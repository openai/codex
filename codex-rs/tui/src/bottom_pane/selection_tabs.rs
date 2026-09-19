//! Single-row filled tab headers for selection lists.
//!
//! Filled tabs always retain the active tab and reserve arrows for hidden neighbors.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;

use super::SelectionItem;
use super::picker_style::active_tab_style;

pub(crate) struct SelectionTab {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) header: Box<dyn Renderable>,
    pub(crate) items: Vec<SelectionItem>,
}

pub(super) fn tab_bar_height(tabs: &[SelectionTab], width: u16) -> u16 {
    u16::from(!tabs.is_empty() && width > 0)
}

pub(super) fn render_tab_bar(
    tabs: &[SelectionTab],
    active_idx: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    let labels = tabs
        .iter()
        .map(|tab| tab.label.as_str())
        .collect::<Vec<_>>();
    render_filled_tab_bar(&labels, active_idx, area, buf);
}

/// Render a fixed-height tab strip and return the visible tab hit targets.
pub(crate) fn render_filled_tab_bar(
    labels: &[&str],
    active_idx: usize,
    area: Rect,
    buf: &mut Buffer,
) -> Vec<(usize, Rect)> {
    if labels.is_empty() || area.is_empty() {
        return Vec::new();
    }
    let active_idx = active_idx.min(labels.len() - 1);
    let widths = labels
        .iter()
        .map(|label| Line::from(*label).width() + 2)
        .collect::<Vec<_>>();
    let occupied = |start: usize, end: usize| {
        widths[start..end].iter().sum::<usize>() + end - start - 1
            + usize::from(start > 0) * 2
            + usize::from(end < labels.len()) * 2
    };
    let mut start = active_idx;
    let mut end = active_idx + 1;
    while start > 0 && occupied(start - 1, end) <= usize::from(area.width) {
        start -= 1;
    }
    while end < labels.len() && occupied(start, end + 1) <= usize::from(area.width) {
        end += 1;
    }

    // Tiny strips prioritize a readable active label over navigation hints.
    let show_left = start > 0 && area.width >= 5;
    let show_right = end < labels.len() && area.width >= 7;
    let mut x = area.x;
    if show_left {
        Line::from("‹")
            .dim()
            .render(Rect::new(x, area.y, /*width*/ 1, /*height*/ 1), buf);
        x += 2;
    }
    let right = area.right().saturating_sub(u16::from(show_right) * 2);
    let mut regions = Vec::new();
    for idx in start..end {
        let width = widths[idx].min(usize::from(right.saturating_sub(x))) as u16;
        let line = Line::from(format!(" {} ", labels[idx]));
        let line = truncate_line_with_ellipsis_if_overflow(line, usize::from(width));
        let line = if idx == active_idx {
            line.style(active_tab_style())
        } else {
            line.dim()
        };
        let tab = Rect::new(x, area.y, width, /*height*/ 1);
        line.render(tab, buf);
        regions.push((idx, tab));
        x = x.saturating_add(width).saturating_add(/*rhs*/ 1);
    }
    if show_right {
        Line::from("›").dim().render(
            Rect::new(
                area.right() - 1,
                area.y,
                /*width*/ 1,
                /*height*/ 1,
            ),
            buf,
        );
    }
    regions
}

#[cfg(test)]
#[path = "selection_tabs_tests.rs"]
mod tests;
