//! The sidebar chooser belongs to its parent conversation and can remain open without taking input.

use crate::app_server_session::SideChat;
use crate::footer_hint;
use crate::keymap::KeymapContext;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::live_wrap::take_prefix_by_width;
use crate::sidebar_tab_strip::TabState;
use crate::sidebar_tab_strip::TabStrip;
use crate::sidebar_tab_strip::TabTarget;
use codex_protocol::ThreadId;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
use std::collections::HashMap;
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewChatState {
    WaitingForMainMessage,
    Available,
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
    tabs: ChatTabs<'_>,
    new_chat: NewChatState,
) -> (footer_hint::HintLine, Vec<Option<ChooserAction>>) {
    let hint = |action| list_hint(keymap, action);
    let movement = [ListAction::MoveUp, ListAction::MoveDown]
        .into_iter()
        .filter(|_| new_chat == NewChatState::Available)
        .filter_map(hint)
        .collect::<Vec<_>>()
        .join("/");
    let hide = keymap
        .primary_hint(KeymapContext::Global, "hide_pane")
        .map(super::key_hint::ShortcutHint::display_label);
    let (hints, actions): (Vec<_>, Vec<_>) = [
        (
            hint(ListAction::Accept).filter(|_| new_chat == NewChatState::Available),
            if tabs.retry_row(choice) {
                "retry"
            } else {
                "open"
            },
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

pub(crate) fn close_cell(tab: Rect) -> Rect {
    Rect::new(
        tab.right().saturating_sub(1),
        tab.y,
        u16::from(tab.width >= 3),
        tab.height,
    )
}

#[derive(Default)]
pub(crate) enum Discovery {
    #[default]
    Idle,
    Loading {
        request: uuid::Uuid,
        started: std::time::Instant,
        retry: bool,
    },
    Failed(String),
}

impl Discovery {
    pub(crate) fn request(&self) -> Option<uuid::Uuid> {
        match self {
            Self::Loading { request, .. } => Some(*request),
            Self::Idle | Self::Failed(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ChatTabs<'a> {
    pub(crate) discovery: &'a Discovery,
    pub(crate) ids: &'a [ThreadId],
    // Chooser index zero is reserved for New side chat.
    pub(crate) choices: &'a [SideChat],
    pub(crate) latest_prompts: &'a HashMap<ThreadId, String>,
    pub(crate) selected: Option<ThreadId>,
}

impl<'a> ChatTabs<'a> {
    pub(crate) fn visible_chat_ids(self, area: Rect, choice: usize) -> Vec<ThreadId> {
        choice_rows(area, self, choice)
            .visible_chats(self, choice)
            .filter_map(|(index, _)| self.choice(index))
            .collect()
    }

    pub(crate) fn choice_count(self) -> usize {
        self.choices.len() + 1 + usize::from(self.retry_row(self.choices.len() + 1))
    }

    pub(crate) fn retry_row(self, index: usize) -> bool {
        matches!(
            self.discovery,
            Discovery::Failed(_) | Discovery::Loading { retry: true, .. }
        ) && index == self.choices.len() + 1
    }

    pub(crate) fn choice(self, index: usize) -> Option<ThreadId> {
        Some(self.choices.get(index.checked_sub(1)?)?.id)
    }

    fn prompt(self, index: usize) -> Option<&'a str> {
        self.latest_prompts
            .get(&self.choice(index)?)
            .map(String::as_str)
            .filter(|prompt| !prompt.is_empty())
    }

    fn row_height(self, index: usize) -> u16 {
        1 + u16::from(self.prompt(index).is_some())
    }

    fn fitting_choices(self, indices: impl Iterator<Item = usize>, height: usize) -> usize {
        let mut used = 0;
        indices
            .take_while(|&index| {
                used += usize::from(self.row_height(index));
                used <= height
            })
            .count()
    }

    pub(crate) fn choice_index(self, id: ThreadId) -> Option<usize> {
        self.choices
            .iter()
            .position(|choice| choice.id == id)
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
    hovered_target: Option<TabTarget<ThreadId>>,
    buf: &mut Buffer,
) {
    tabs.strip().render(
        area,
        buf,
        chooser,
        chat,
        hovered_target,
        |id, area, style, buf| {
            let close = close_cell(area);
            let label = Rect::new(area.x, area.y, area.width - close.width, area.height);
            let title = match tabs.choice_index(id) {
                Some(number) => match side_chat_label(&tabs.choices[number - 1]) {
                    Some(name) => truncate_line_with_ellipsis_if_overflow(
                        Line::from(format!(" {name}")),
                        usize::from(label.width),
                    ),
                    None if tabs.choices.len() > 1 => Line::from(format!(" Side {number} ")),
                    None => Line::from(CHAT_TAB),
                },
                None => Line::from(CHAT_TAB),
            };
            title.style(style).render(label, buf);
            Line::from("×").style(style).render(close, buf);
        },
    );
}

pub(crate) fn side_chat_label(chat: &SideChat) -> Option<&str> {
    chat.name
        .as_deref()
        .unwrap_or(&chat.preview)
        .trim()
        .lines()
        .next()
        .map(str::trim)
}

struct ChoiceRows {
    wide: bool,
    new_chat: Range<u16>,
    group: Option<u16>,
    chats: Range<u16>,
}

impl ChoiceRows {
    /// Title and subtitle occupy one selectable entry, even when the last line is clipped.
    fn visible_chats(
        &self,
        tabs: ChatTabs<'_>,
        choice: usize,
    ) -> impl Iterator<Item = (usize, Range<u16>)> {
        let mut rows = self.chats.clone();
        let selected = choice.max(1);
        let spare = rows
            .len()
            .saturating_sub(usize::from(tabs.row_height(selected)));
        let first = selected - tabs.fitting_choices((1..selected).rev(), spare);
        (first..tabs.choice_count()).map_while(move |index| {
            let start = rows.next()?;
            if tabs.prompt(index).is_some() {
                rows.next();
            }
            Some((index, start..rows.start))
        })
    }
}

fn choice_rows(area: Rect, tabs: ChatTabs<'_>, choice: usize) -> ChoiceRows {
    let count = tabs.choice_count();
    let wide = body_row(area, area.y).width >= WIDE_WIDTH;
    let new_height = if wide { 1 } else { 2 };
    let footer = area.height.saturating_sub(1);
    let new_end = (FIRST_CHOICE_ROW + new_height).min(footer);
    let after_group = 1 + u16::from(!tabs.choices.is_empty());
    let group = (count > 1 && footer >= new_end + 1 + after_group + tabs.row_height(choice))
        .then_some(new_end + 1);
    // In short panes, let the New row scroll away so the selection remains visible.
    let new_chat = FIRST_CHOICE_ROW..if choice == 0 || group.is_some() {
        new_end.max(FIRST_CHOICE_ROW)
    } else {
        FIRST_CHOICE_ROW
    };
    let chat_start = group.map_or(new_chat.end, |row| row + after_group);
    ChoiceRows {
        wide,
        new_chat,
        group,
        chats: chat_start..footer,
    }
}

pub(crate) fn page_size(
    area: Rect,
    tabs: ChatTabs<'_>,
    choice: usize,
    action: ListAction,
) -> usize {
    let rows = choice_rows(area, tabs, choice);
    let height = rows.chats.len();
    match action {
        ListAction::PageUp => tabs.fitting_choices((0..choice).rev(), height),
        ListAction::PageDown if choice == 0 => rows.visible_chats(tabs, choice).count(),
        ListAction::PageDown => tabs.fitting_choices(choice..tabs.choice_count(), height),
        _ => 1,
    }
    .max(1)
}

pub(crate) fn choice_at(area: Rect, tabs: ChatTabs<'_>, choice: usize, y: u16) -> Option<usize> {
    let rows = choice_rows(area, tabs, choice);
    let offset = y.checked_sub(area.y)?;
    if rows.new_chat.contains(&offset) {
        return Some(0);
    }
    rows.visible_chats(tabs, choice)
        .find_map(|(index, lines)| lines.contains(&offset).then_some(index))
}

pub(crate) struct SidebarStyle<'a> {
    pub(crate) new_chat: NewChatState,
    pub(crate) use_theme_colors: bool,
    pub(crate) motion: crate::motion::MotionMode,
    pub(crate) frames: &'a crate::tui::FrameRequester,
}

pub(crate) fn paint(
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    footer: Line<'_>,
    tabs: ChatTabs<'_>,
    choice: usize,
    sidebar_style: &SidebarStyle<'_>,
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
    if sidebar_style.new_chat == NewChatState::WaitingForMainMessage {
        for (offset, text) in (FIRST_CHOICE_ROW..).zip(textwrap::wrap(
            "You can start a side chat after your first message in the main chat.",
            usize::from(row(area.y).width.max(1)),
        )) {
            paint(offset, Line::from(text).dim());
        }
        footer.render(row(footer_y), buf);
        return;
    }
    match tabs.discovery {
        Discovery::Loading { started, .. } => {
            let glyph =
                crate::motion::loading_glyph(*started, sidebar_style.motion, sidebar_style.frames);
            let label = if tabs.choices.is_empty() {
                "Loading side chats…"
            } else {
                "Refreshing side chats…"
            };
            paint(3, Line::from(format!("{glyph} {label}").dim()));
        }
        Discovery::Failed(error) => paint(3, Line::from(error.as_str().red())),
        Discovery::Idle => {}
    }
    let rows = choice_rows(area, tabs, choice);
    let secondary = crate::style::secondary_text_style();
    let row_style = |index| {
        if index == choice {
            ('›', style, style.not_bold())
        } else {
            (' ', Default::default(), secondary)
        }
    };
    if !rows.new_chat.is_empty() {
        let y = rows.new_chat.start;
        let (marker, fill, subtitle) = row_style(0);
        let title = format!("{marker} + New side chat");
        let detail = Span::styled("Start from the main chat", subtitle);
        if rows.wide {
            let pad = usize::from(row(area.y).width - 1) - detail.width();
            paint(
                y,
                Line::from(vec![format!("{title:<pad$}").into(), detail]).style(fill),
            );
        } else {
            paint(y, Line::from(title).style(fill));
            paint(y + 1, Line::from(vec!["    ".into(), detail]).style(fill));
        }
    }
    if let Some(y) = rows.group
        && !tabs.choices.is_empty()
    {
        let total = tabs.choices.len();
        paint(
            y,
            Line::from(format!("Side chats  {total}")).style(secondary),
        );
    }
    let width = usize::from(row(area.y).width);
    for (index, lines) in rows.visible_chats(tabs, choice) {
        let (marker, fill, subtitle) = row_style(index);
        if tabs.retry_row(index) {
            let label = if tabs.discovery.request().is_some() {
                "Retrying…"
            } else {
                "Retry loading side chats"
            };
            paint(
                lines.start,
                Line::from(format!("{marker}   {label}")).style(fill),
            );
            continue;
        }
        let chat = &tabs.choices[index - 1];
        let selected = index == choice && focused;
        let title = if let Some(label) = side_chat_label(chat) {
            label.to_owned()
        } else if tabs.choices.len() == 1 {
            "Side chat".to_string()
        } else {
            format!("Side chat {index}")
        };
        let title = if sidebar_style.use_theme_colors && !selected {
            let identity = crate::thread_color::thread_color(chat.id);
            title.fg(crate::style::readable_color_on(identity, fill.bg))
        } else {
            title.into()
        };
        let symbol = Span::styled("○ ", if selected { style } else { secondary });
        let line = Line::from(vec![format!("{marker} ").into(), symbol, title]).style(fill);
        paint(
            lines.start,
            truncate_line_with_ellipsis_if_overflow(line, width),
        );
        if lines.len() == 2
            && let Some(prompt) = tabs.prompt(index)
        {
            // Bound the copy for large pastes before clipping the indented line.
            let (mut preview, rest, _) = take_prefix_by_width(prompt, width);
            if !rest.is_empty() {
                preview.push('…');
            }
            let line = Line::from(format!("    {preview}")).style(subtitle);
            paint(
                lines.start + 1,
                truncate_line_with_ellipsis_if_overflow(line, width),
            );
        }
    }
    if footer_y > area.y {
        footer.render(row(footer_y), buf);
    }
}
