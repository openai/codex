//! The sidebar chooser belongs to its parent conversation and can remain open without taking input.

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

const CHAT_TAB: &str = " Side chat ";

pub(crate) fn strip_targets(area: Rect) -> (Rect, Rect) {
    let width = area.width.saturating_sub(1);
    let plus = width.min(3);
    let tabs = width - plus;
    let height = area.height.min(1);
    let tab_width = tabs.min(CHAT_TAB.len() as u16);
    (
        Rect::new(area.x, area.y, tab_width, height),
        Rect::new(area.x + tabs, area.y, plus, height),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabState {
    Absent,
    Active,
    Inactive,
}

pub(crate) fn tab_strip(area: Rect, chooser: TabState, chat: TabState, buf: &mut Buffer) {
    let (tab, add) = strip_targets(area);
    let quiet = crate::style::secondary_text_style();
    let tab_style = |state| match state {
        TabState::Active => crate::style::selection_style().not_underlined(),
        TabState::Inactive => crate::bottom_pane::active_tab_style(),
        TabState::Absent => quiet,
    };
    if chat != TabState::Absent {
        let style = if chooser == TabState::Absent {
            tab_style(chat)
        } else {
            quiet
        };
        Line::from(CHAT_TAB).style(style).render(tab, buf);
    }
    Line::from(" + ").style(tab_style(chooser)).render(add, buf);
}

pub(crate) fn choice_contains(area: Rect, y: u16) -> bool {
    let rows = if body_row(area, area.y).width < WIDE_WIDTH {
        4..6
    } else {
        4..5
    };
    y < area.bottom().saturating_sub(1) && rows.contains(&y.saturating_sub(area.y))
}

pub(crate) fn paint(
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    keymap: &RuntimeKeymap,
    chat: TabState,
) {
    Clear.render(area, buf);
    if area.is_empty() {
        return;
    }
    let style = if focused {
        crate::style::selection_style()
    } else {
        crate::style::quiet_workspace_style()
    };
    let chooser = if focused {
        TabState::Active
    } else {
        TabState::Inactive
    };
    tab_strip(area, chooser, chat, buf);
    let row = |y| body_row(area, y);
    let footer_y = area.bottom() - 1;
    let mut paint = |offset, line: Line<'_>| {
        let y = area.y.saturating_add(offset);
        if y < footer_y {
            line.render(row(y), buf);
        }
    };
    paint(2, Line::from("Open in sidebar").bold());
    let (title, detail) = if chat == TabState::Absent {
        ("› + New side chat", "Start from this conversation")
    } else {
        ("›   Side chat", "Return to this conversation")
    };
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
