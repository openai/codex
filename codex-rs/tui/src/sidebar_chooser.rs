//! The sidebar chooser belongs to its parent conversation and can remain open without taking input.

use crate::footer_hint;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use crate::sidebar_tab_strip::TabState;
use crate::sidebar_tab_strip::TabStrip;
use codex_protocol::ThreadId;
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

pub(crate) fn chat_tabs(ids: &[ThreadId], selected: Option<ThreadId>) -> TabStrip<'_, ThreadId> {
    TabStrip {
        ids,
        selected,
        tab_width: CHAT_TAB.len() as u16,
    }
}

pub(crate) fn tab_strip(
    area: Rect,
    chooser: TabState,
    chat: TabState,
    tabs: TabStrip<'_, ThreadId>,
    buf: &mut Buffer,
) {
    tabs.render(area, buf, chooser, chat, |index, _| {
        if tabs.ids.len() == 1 {
            Line::from(CHAT_TAB)
        } else {
            Line::from(format!(" Side {} ", index + 1))
        }
    });
}

fn choice_rows(area: Rect, count: usize, choice: usize) -> (u16, std::ops::Range<usize>) {
    let height = if body_row(area, area.y).width < WIDE_WIDTH {
        2
    } else {
        1
    };
    let visible = usize::from(area.height.saturating_sub(5).div_ceil(height));
    let start = (choice + 1).saturating_sub(visible);
    (height, start..(start + visible).min(count))
}

pub(crate) fn choice_at(area: Rect, count: usize, choice: usize, y: u16) -> Option<usize> {
    if y >= area.bottom().saturating_sub(1) {
        return None;
    }
    let (height, rows) = choice_rows(area, count, choice);
    let offset = y.checked_sub(area.y)?.checked_sub(4)?;
    rows.into_iter().nth(usize::from(offset / height))
}

pub(crate) fn paint(
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    keymap: &RuntimeKeymap,
    tabs: TabStrip<'_, ThreadId>,
    choice: usize,
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
    tab_strip(area, chooser, TabState::Absent, tabs, buf);
    let row = |y| body_row(area, y);
    let footer_y = area.bottom() - 1;
    let mut paint = |offset, line: Line<'_>| {
        let y = area.y.saturating_add(offset);
        if y < footer_y {
            line.render(row(y), buf);
        }
    };
    paint(2, Line::from("Open in sidebar").bold());
    let (height, rows) = choice_rows(area, tabs.ids.len() + 1, choice);
    for (offset, index) in rows.enumerate() {
        let marker = if index == choice { '›' } else { ' ' };
        let row_style = if index == choice {
            style
        } else {
            Default::default()
        };
        let (title, detail) = if index == tabs.ids.len() {
            (
                format!("{marker} + New side chat"),
                "Start from the main chat",
            )
        } else {
            let title = if tabs.ids.len() == 1 {
                "Side chat".to_string()
            } else {
                format!("Side chat {}", index + 1)
            };
            (format!("{marker}   {title}"), "Reopen side chat")
        };
        let y = 4 + offset as u16 * height;
        if height == 1 {
            let pad = usize::from(row(area.y).width - 1) - detail.len();
            paint(
                y,
                Line::from(format!("{title:<pad$}{detail}")).style(row_style),
            );
        } else {
            paint(y, Line::from(title).style(row_style));
            paint(y + 1, Line::from(format!("    {detail}")).style(row_style));
        }
    }
    if footer_y > area.y {
        footer(keymap, area).line.render(row(footer_y), buf);
    }
}
