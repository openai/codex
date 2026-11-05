//! The empty sidebar belongs to its parent conversation and can remain open without taking input.

use crate::footer_hint;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

const WIDE_WIDTH: u16 = 58;

fn body_row(area: Rect, y: u16) -> Rect {
    Rect::new(area.x + 1, y, area.width.saturating_sub(3), 1)
}

pub(crate) fn footer(keymap: &RuntimeKeymap, area: Rect) -> footer_hint::HintLine {
    let hide = keymap
        .primary_hint(KeymapContext::Global, "hide_pane")
        .map(|hint| footer_hint::shortcut(&hint.display_label(), "hide pane"));
    let hints = [
        footer_hint::shortcut("Enter", "open"),
        footer_hint::shortcut("Esc", "back"),
        footer_hint::shortcut("↑/↓", "move"),
    ];
    footer_hint::HintLine::new(hints.into_iter().chain(hide), body_row(area, area.y).width)
}

pub(crate) fn choice_contains(area: Rect, y: u16) -> bool {
    let rows = if body_row(area, area.y).width < WIDE_WIDTH {
        4..6
    } else {
        4..5
    };
    y < area.bottom().saturating_sub(1) && rows.contains(&y.saturating_sub(area.y))
}

pub(crate) fn paint(focused: bool, area: Rect, buf: &mut Buffer, keymap: &RuntimeKeymap) {
    Clear.render(area, buf);
    if area.is_empty() {
        return;
    }
    let style = if focused {
        crate::style::selection_style()
    } else {
        crate::style::quiet_workspace_style()
    };
    let strip_width = area.width.saturating_sub(1);
    let add_width = strip_width.min(3);
    let add = Rect::new(area.x + strip_width - add_width, area.y, add_width, 1);
    let tab_style = if focused {
        style.not_underlined()
    } else {
        crate::bottom_pane::active_tab_style()
    };
    Line::from(" + ").style(tab_style).render(add, buf);
    let row = |y| body_row(area, y);
    let footer_y = area.bottom() - 1;
    let mut paint = |offset, line: Line<'_>| {
        let y = area.y.saturating_add(offset);
        if y < footer_y {
            line.render(row(y), buf);
        }
    };
    paint(2, Line::from("Open in sidebar").bold());
    let detail = "Start from this conversation";
    let title = "› + New side chat";
    let width = row(area.y).width;
    if width >= WIDE_WIDTH {
        let pad = usize::from(width - 1) - detail.len();
        paint(4, Line::from(format!("{title:<pad$}{detail}")).style(style));
    } else {
        paint(4, Line::from(title).style(style));
        paint(5, Line::from(format!("    {detail}")).style(style));
    }
    if footer_y > area.y {
        footer(keymap, area).line.render(row(footer_y), buf);
    }
}
