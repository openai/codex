//! The sidebar chooser belongs to its parent conversation and can remain open without taking input.

use crate::footer_hint;
use crate::keymap::KeymapContext;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crate::sidebar_tab_strip::TabState;
use crate::sidebar_tab_strip::TabStrip;
use codex_protocol::ThreadId;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
use std::ops::Range;

const WIDE_WIDTH: u16 = 58;
const FIRST_CHOICE_ROW: u16 = 4;

pub(crate) fn body_row(area: Rect, y: u16) -> Rect {
    Rect::new(area.x + 1, y, area.width.saturating_sub(3), 1)
}

pub(crate) fn resize_hint(full: &str, width: u16) -> Line<'_> {
    let full = Line::from(full);
    if full.width() <= usize::from(width) {
        full.dim()
    } else {
        Line::from("←/→ resize · Enter/Esc done").dim()
    }
}

pub(crate) enum ChooserAction {
    Open(usize),
    Back,
    Hide,
}

fn list_hint(keymap: &RuntimeKeymap, action: ListAction) -> Option<String> {
    keymap
        .list
        .primary_hint(action)
        .map(super::key_hint::ShortcutHint::display_label)
}

pub(crate) fn creation_failure_text(message: &str, keymap: &RuntimeKeymap) -> String {
    let hints = [(ListAction::Accept, "retry"), (ListAction::Cancel, "back")]
        .into_iter()
        .filter_map(|(action, label)| Some(format!("{} {label}", list_hint(keymap, action)?)))
        .collect::<Vec<_>>();
    if hints.is_empty() {
        message.to_owned()
    } else {
        format!("{message}\n{}", hints.join(" · "))
    }
}

pub(crate) fn footer(
    keymap: &RuntimeKeymap,
    area: Rect,
    choice: usize,
) -> (footer_hint::HintLine, Vec<Option<ChooserAction>>) {
    let hint = |action| list_hint(keymap, action);
    let movement = [ListAction::MoveUp, ListAction::MoveDown]
        .into_iter()
        .filter_map(hint)
        .collect::<Vec<_>>()
        .join("/");
    let hide = keymap
        .primary_hint(KeymapContext::Global, "hide_pane")
        .map(super::key_hint::ShortcutHint::display_label);
    let (hints, actions): (Vec<_>, Vec<_>) = [
        (
            hint(ListAction::Accept),
            "open",
            Some(ChooserAction::Open(choice)),
        ),
        (hint(ListAction::Cancel), "back", Some(ChooserAction::Back)),
        ((!movement.is_empty()).then_some(movement), "move", None),
        (hide, "hide pane", Some(ChooserAction::Hide)),
    ]
    .into_iter()
    .filter_map(|(keys, label, action)| Some((footer_hint::shortcut(&keys?, label), action)))
    .unzip();
    (
        footer_hint::HintLine::new(hints, body_row(area, area.y).width),
        actions,
    )
}

const CHAT_TAB: &str = " Side chat ";

#[derive(Clone, Copy, Default)]
pub(crate) struct ChatTabs<'a> {
    pub(crate) ids: &'a [ThreadId],
    pub(crate) selected: Option<ThreadId>,
}

impl<'a> ChatTabs<'a> {
    pub(crate) fn choice_count(self) -> usize {
        self.ids.len() + 1
    }

    pub(crate) fn choice(self, index: usize) -> Option<ThreadId> {
        self.ids.get(index.checked_sub(1)?).copied()
    }

    pub(crate) fn choice_index(self, id: ThreadId) -> Option<usize> {
        self.ids
            .iter()
            .position(|choice| *choice == id)
            .map(|index| index + 1)
    }

    pub(crate) fn strip(self) -> TabStrip<'a, ThreadId> {
        TabStrip {
            ids: self.ids,
            selected: self.selected,
            tab_width: CHAT_TAB.len() as u16,
        }
    }
}

pub(crate) fn tab_strip(
    area: Rect,
    chooser: TabState,
    chat: TabState,
    tabs: ChatTabs<'_>,
    hovered_tab: Option<ThreadId>,
    buf: &mut Buffer,
) {
    tabs.strip()
        .render(area, buf, chooser, chat, hovered_tab, |index, _| {
            if tabs.ids.len() > 1 {
                Line::from(format!(" Side {} ", index + 1))
            } else {
                Line::from(CHAT_TAB)
            }
        });
}

struct ChoiceRows {
    wide: bool,
    new_chat: Range<u16>,
    group: Option<u16>,
    chat_start: u16,
    chats: Range<usize>,
}

fn choice_rows(area: Rect, count: usize, choice: usize) -> ChoiceRows {
    let wide = body_row(area, area.y).width >= WIDE_WIDTH;
    let new_height = if wide { 1 } else { 2 };
    let footer = area.height.saturating_sub(1);
    let new_end = (FIRST_CHOICE_ROW + new_height).min(footer);
    let group = (count > 1 && footer > new_end + 2).then_some(new_end + 1);
    // In short panes, let the New row scroll away so the selection remains visible.
    let new_chat = FIRST_CHOICE_ROW..if choice == 0 || group.is_some() {
        new_end.max(FIRST_CHOICE_ROW)
    } else {
        FIRST_CHOICE_ROW
    };
    let chat_start = group.map_or(new_chat.end, |row| row + 1);
    let visible = usize::from(footer.saturating_sub(chat_start));
    let first = (choice + 1).saturating_sub(visible).max(1);
    ChoiceRows {
        wide,
        new_chat,
        group,
        chat_start,
        chats: first..(first + visible).min(count),
    }
}

pub(crate) fn page_size(area: Rect, count: usize, choice: usize) -> usize {
    choice_rows(area, count, choice).chats.len().max(1)
}

pub(crate) fn choice_at(area: Rect, count: usize, choice: usize, y: u16) -> Option<usize> {
    let mut rows = choice_rows(area, count, choice);
    let offset = y.checked_sub(area.y)?;
    if rows.new_chat.contains(&offset) {
        return Some(0);
    }
    rows.chats
        .nth(usize::from(offset.checked_sub(rows.chat_start)?))
}

pub(crate) fn paint(
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    footer: Line<'_>,
    tabs: ChatTabs<'_>,
    choice: usize,
    use_theme_colors: bool,
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
    let row = |y| body_row(area, y);
    let footer_y = area.bottom() - 1;
    let mut paint = |offset, line: Line<'_>| {
        let y = area.y.saturating_add(offset);
        if y < footer_y {
            line.render(row(y), buf);
        }
    };
    paint(2, Line::from("Open in sidebar").bold());
    let count = tabs.choice_count();
    let rows = choice_rows(area, count, choice);
    let row_style = |index| {
        if index == choice {
            ('›', style)
        } else {
            (' ', Default::default())
        }
    };
    let secondary = crate::style::secondary_text_style();
    if !rows.new_chat.is_empty() {
        let y = rows.new_chat.start;
        let (marker, fill) = row_style(0);
        let title = format!("{marker} + New side chat");
        let detail = "Start from the main chat";
        if rows.wide {
            let pad = usize::from(row(area.y).width - 1) - detail.len();
            paint(y, Line::from(format!("{title:<pad$}{detail}")).style(fill));
        } else {
            paint(y, Line::from(title).style(fill));
            paint(y + 1, Line::from(format!("    {detail}")).style(fill));
        }
    }
    if let Some(y) = rows.group {
        let total = count - 1;
        paint(
            y,
            Line::from(format!("  Side chats  {total}")).style(secondary),
        );
    }
    for (offset, index) in rows.chats.enumerate() {
        let id = tabs.ids[index - 1];
        let selected = index == choice && focused;
        let (marker, fill) = row_style(index);
        let title = if count == 2 {
            "Side chat".to_string()
        } else {
            format!("Side chat {index}")
        };
        let title = if use_theme_colors && !selected {
            let identity = crate::thread_color::thread_color(id);
            title.fg(crate::style::readable_color_on(identity, fill.bg))
        } else {
            title.into()
        };
        let symbol = Span::styled("○ ", if selected { style } else { secondary });
        paint(
            rows.chat_start + offset as u16,
            Line::from(vec![format!("{marker} ").into(), symbol, title]).style(fill),
        );
    }
    if footer_y > area.y {
        footer.render(row(footer_y), buf);
    }
}
