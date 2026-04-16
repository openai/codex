use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::render::renderable::Renderable;

use super::SelectionItem;

const TAB_GAP_WIDTH: usize = 2;

pub(crate) struct SelectionTab {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) header: Box<dyn Renderable>,
    pub(crate) items: Vec<SelectionItem>,
}

pub(crate) fn tab_bar_height(tabs: &[SelectionTab], active_idx: usize, width: u16) -> u16 {
    if tabs.is_empty() {
        return 0;
    }
    tab_bar_lines(tabs, active_idx, width)
        .len()
        .try_into()
        .unwrap_or(u16::MAX)
}

pub(crate) fn render_tab_bar(
    tabs: &[SelectionTab],
    active_idx: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    for (offset, line) in tab_bar_lines(tabs, active_idx, area.width)
        .into_iter()
        .take(area.height as usize)
        .enumerate()
    {
        line.render(
            Rect {
                x: area.x,
                y: area.y.saturating_add(offset as u16),
                width: area.width,
                height: 1,
            },
            buf,
        );
    }
}

fn tab_bar_lines(tabs: &[SelectionTab], active_idx: usize, width: u16) -> Vec<Line<'static>> {
    if tabs.is_empty() {
        return Vec::new();
    }

    let max_width = width.max(1) as usize;
    let mut lines = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for (idx, tab) in tabs.iter().enumerate() {
        let unit = tab_unit(tab.label.as_str(), idx == active_idx);
        let unit_width = Line::from(unit.clone()).width();
        let gap_width = if current_spans.is_empty() {
            0
        } else {
            TAB_GAP_WIDTH
        };

        if !current_spans.is_empty() && current_width + gap_width + unit_width > max_width {
            lines.push(Line::from(current_spans));
            current_spans = Vec::new();
            current_width = 0;
        }

        if !current_spans.is_empty() {
            current_spans.push("  ".into());
            current_width += TAB_GAP_WIDTH;
        }
        current_width += unit_width;
        current_spans.extend(unit);
    }

    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }
    lines
}

fn tab_unit(label: &str, active: bool) -> Vec<Span<'static>> {
    if active {
        vec![
            "[".cyan().bold(),
            label.to_string().cyan().bold(),
            "]".cyan().bold(),
        ]
    } else {
        vec![label.to_string().dim()]
    }
}
