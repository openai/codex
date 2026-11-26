//! The sidebar chooser belongs to its parent conversation and can remain open without taking input.

use crate::app_server_session::SideChat;
use crate::footer_hint::Hint;
use crate::footer_hint::HintFocus;
use crate::footer_hint::HintLine;
use crate::keymap::KeymapContext;
use crate::keymap::ListAction;
use crate::keymap::RuntimeKeymap;
use crate::line_truncation::filename_label;
use crate::line_truncation::middle_label;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::live_wrap::take_prefix_by_width;
use crate::markdown_render::FileLink;
use crate::sidebar_tab_strip::SidebarTab;
use crate::sidebar_tab_strip::TabState;
use crate::sidebar_tab_strip::TabStrip;
use crate::sidebar_tab_strip::TabTarget;
use crate::thread_label::ThreadLabelStyle;
use codex_protocol::ThreadId;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
use std::collections::HashMap;
use std::ops::Range;

const WIDE_WIDTH: u16 = 58;
const FIRST_CHOICE_ROW: usize = 4;
const CHOOSER_BODY_START: usize = 1;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChooserAction {
    Open(usize),
    Back,
    Hide,
    FilePath,
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

pub(crate) fn creation_failure_hints(
    keymap: &RuntimeKeymap,
    focus: HintFocus,
    width: u16,
) -> Line<'static> {
    let hints = [(ListAction::Accept, "retry"), (ListAction::Cancel, "back")]
        .into_iter()
        .filter_map(|(action, label)| Some(Hint::key(list_hint(keymap, action)?, label)));
    focus.render_row(hints, width).line
}

pub(crate) fn footer(
    keymap: &RuntimeKeymap,
    area: Rect,
    choice: usize,
    tabs: ChatTabs<'_>,
    new_chat: NewChatState,
    focus: HintFocus,
    showing_path: bool,
) -> HintLine<ChooserAction> {
    let hint = |action| list_hint(keymap, action);
    let width = body_row(area, area.y).width;
    let button = |key, label, action: Option<ChooserAction>| {
        action.map(|action| Hint::Button {
            key,
            label,
            action,
            before_focus: action == ChooserAction::Back,
        })
    };
    if showing_path {
        let back = button(hint(ListAction::Cancel), "back", Some(ChooserAction::Back));
        let scroll = hint(ListAction::MoveDown).map(|key| Hint::Key(key, "scroll".into()));
        return focus.render_row(back.into_iter().chain(scroll), width);
    }
    let selected_file = tabs.file(choice);
    let visible = choice_is_visible(area, tabs, choice);
    let movement = [ListAction::MoveUp, ListAction::MoveDown]
        .into_iter()
        .filter(|_| {
            new_chat == NewChatState::Available
                || !tabs.files.is_empty()
                || tabs.agents.iter().any(|agent| agent.selectable)
        })
        .filter_map(hint)
        .collect::<Vec<_>>()
        .join("/");
    let hide = keymap
        .primary_hint(KeymapContext::Global, "hide_pane")
        .map(super::key_hint::ShortcutHint::display_label);
    let can_open = visible
        && !tabs.agent_is_disabled(choice)
        && (new_chat == NewChatState::Available
            || selected_file.is_some()
            || tabs.agent(choice).is_some())
        && (selected_file.is_none() || tabs.file_preview_unavailable.is_none());
    let hints = [
        button(
            hint(ListAction::Accept),
            if tabs.retry_row(choice)
                || tabs.opening.is_some_and(|o| {
                    Some(o.target.child) == tabs.choice(choice)
                        && matches!(o.state, OpenState::Failed(_))
                })
            {
                "retry"
            } else {
                "open"
            },
            can_open.then_some(ChooserAction::Open(choice)),
        ),
        button(
            hint(ListAction::Cancel),
            if tabs
                .opening
                .is_some_and(|o| !matches!(o.state, OpenState::Failed(_)))
            {
                "cancel opening"
            } else {
                "back"
            },
            Some(ChooserAction::Back),
        ),
        button(
            file_path_key_available(keymap).then(|| "p".to_owned()),
            "path",
            (visible && selected_file.is_some()).then_some(ChooserAction::FilePath),
        ),
        (!movement.is_empty()).then(|| Hint::Key(movement, "move".into())),
        button(hide, "hide pane", Some(ChooserAction::Hide)),
    ];
    focus.render_row(hints.into_iter().flatten(), width)
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

pub(crate) struct Opening {
    pub(crate) target: crate::app::thread_attachment::Target,
    pub(crate) state: OpenState,
}

pub(crate) enum OpenState {
    Waiting,
    Loading(std::time::Instant),
    Failed(String),
}

/// An agent displayed in the shared sidebar chooser.
pub(crate) struct AgentRow {
    pub(crate) id: ThreadId,
    pub(crate) name: String,
    pub(crate) symbol: &'static str,
    pub(crate) status: &'static str,
    pub(crate) selectable: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct ChatTabs<'a> {
    pub(crate) label_style: ThreadLabelStyle,
    pub(crate) discovery: &'a Discovery,
    pub(crate) opening: Option<&'a Opening>,
    pub(crate) ids: &'a [SidebarTab],
    // Chooser index zero is reserved for New side chat.
    pub(crate) choices: &'a [SideChat],
    pub(crate) agents: &'a [AgentRow],
    pub(crate) agent_title: Option<&'a str>,
    pub(crate) latest_prompts: &'a HashMap<ThreadId, String>,
    pub(crate) selected: Option<SidebarTab>,
    pub(crate) files: &'a [FileLink],
    pub(crate) files_loading: bool,
    pub(crate) files_cwd: Option<&'a PathUri>,
    pub(crate) file_preview_unavailable: Option<&'static str>,
    pub(crate) scroll: ChooserScroll,
}

impl<'a> ChatTabs<'a> {
    pub(crate) fn visible_chat_ids(self, area: Rect, choice: usize) -> Vec<ThreadId> {
        ChoiceRows::new(area, self)
            .visible_choices(self.scroll, choice)
            .filter_map(|(index, _)| self.choice(index))
            .collect()
    }

    pub(crate) fn choice_count(self) -> usize {
        self.file_start() + self.files.len()
    }

    pub(crate) fn agent(self, index: usize) -> Option<&'a AgentRow> {
        self.agents.get(index.checked_sub(self.first_agent())?)
    }

    pub(crate) fn agent_index(self, id: ThreadId) -> Option<usize> {
        self.agents
            .iter()
            .position(|agent| agent.id == id)
            .map(|offset| self.first_agent() + offset)
    }

    pub(crate) fn agent_is_disabled(self, index: usize) -> bool {
        self.agent(index).is_some_and(|agent| !agent.selectable)
    }

    fn first_agent(self) -> usize {
        let end = self.choices.len() + 1;
        end + usize::from(self.retry_row(end))
    }

    pub(crate) fn file_start(self) -> usize {
        self.first_agent() + self.agents.len()
    }

    pub(crate) fn file_offset(self, index: usize) -> Option<usize> {
        index
            .checked_sub(self.file_start())
            .filter(|&i| i < self.files.len())
    }

    pub(crate) fn file(self, index: usize) -> Option<&'a FileLink> {
        self.files.get(self.file_offset(index)?)
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

    pub(crate) fn choice_index(self, id: ThreadId) -> Option<usize> {
        self.choices
            .iter()
            .position(|choice| choice.id == id)
            .map(|index| index + 1)
    }
}

pub(crate) fn tab_strip(
    area: Rect,
    chooser: TabState,
    chat: TabState,
    tabs: ChatTabs<'_>,
    pointer: Option<Position>,
    document_title: impl Fn(uuid::Uuid, usize) -> String,
    buf: &mut Buffer,
) -> Vec<(TabTarget<SidebarTab>, Rect)> {
    TabStrip {
        ids: tabs.ids,
        selected: tabs.selected,
    }
    .render(area, buf, chooser, chat, pointer, |id, area, style, buf| {
        let width = usize::from(area.width.saturating_sub(1));
        let mut title = match id {
            SidebarTab::Document(id) => Line::from(document_title(id, width)),
            SidebarTab::Subagents => Line::from(middle_label(
                tabs.agent_title.unwrap_or("Agent transcript"),
                width.saturating_sub(1),
            )),
            SidebarTab::Chat(id) => {
                let name = match tabs.choice_index(id) {
                    Some(index) => {
                        let ordinal = index - 1;
                        match side_chat_label(&tabs.choices[ordinal]) {
                            Some(name) => name.to_owned(),
                            None if tabs.choices.len() > 1 => format!("Side {}", ordinal + 1),
                            None => "Side chat".to_owned(),
                        }
                    }
                    None => "Side chat".to_owned(),
                };
                tabs.label_style.render(Line::from(name), width)
            }
        };
        title.spans.insert(0, " ".into());
        title.style(style).render(area, buf);
    })
}

pub(crate) fn side_chat_label(chat: &SideChat) -> Option<&str> {
    chat.name
        .as_deref()
        .unwrap_or(&chat.preview)
        .trim()
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChooserScroll {
    top: usize,
    // The wheel can move the viewport away from the keyboard selection.
    pub(crate) wheel_scrolled: bool,
}

struct ChoiceRows {
    wide: bool,
    height: usize,
    chats_header: Option<usize>,
    agents_header: Option<usize>,
    files_header: Option<usize>,
    choices: Vec<Range<usize>>,
}

impl ChoiceRows {
    fn new(area: Rect, tabs: ChatTabs<'_>) -> Self {
        let width = body_row(area, area.y).width;
        let wide = width >= WIDE_WIDTH;
        let height = usize::from(area.height.saturating_sub(3));
        let mut row = FIRST_CHOICE_ROW;
        let file_start = tabs.file_start();
        let mut chats_header = None;
        let mut agents_header = None;
        let mut files_header = None;
        let choices = (0..tabs.choice_count())
            .map(|index| {
                if index == 1 && !tabs.choices.is_empty() {
                    chats_header = Some(row + 1);
                    row += 3;
                } else if index == 1 && tabs.retry_row(index) {
                    row += 2;
                }
                let file = tabs.file_offset(index).is_some();
                let agent = tabs.agent(index).is_some();
                if agent && index == tabs.first_agent() {
                    agents_header = Some(row + 1);
                    row += 3;
                }
                if file && index == file_start {
                    files_header = Some(row + 1);
                    row += 3;
                }
                let start = row;
                let subtitle = if index == 0 || file || agent {
                    !wide
                } else {
                    tabs.prompt(index).is_some()
                };
                row += 1 + usize::from(subtitle);
                start..row
            })
            .collect();
        Self {
            wide,
            height,
            chats_header,
            agents_header,
            files_header,
            choices,
        }
    }

    fn max_top(&self) -> usize {
        self.choices[self.choices.len() - 1]
            .end
            .saturating_sub(CHOOSER_BODY_START + self.height)
    }

    fn top(&self, scroll: ChooserScroll, choice: usize) -> usize {
        let top = scroll.top.min(self.max_top());
        if scroll.wheel_scrolled || self.height == 0 {
            return top;
        }
        if choice == 0 && top == 0 {
            return 0;
        }
        let lines = &self.choices[choice];
        let latest = lines.start - CHOOSER_BODY_START;
        let earliest = lines.end.saturating_sub(CHOOSER_BODY_START + self.height);
        top.clamp(earliest.min(latest), latest).min(self.max_top())
    }

    fn visible_choices(
        &self,
        scroll: ChooserScroll,
        choice: usize,
    ) -> impl Iterator<Item = (usize, &Range<usize>)> {
        let start = CHOOSER_BODY_START + self.top(scroll, choice);
        let end = start + self.height;
        let first = self.choices.partition_point(|lines| lines.end <= start);
        self.choices
            .iter()
            .enumerate()
            .skip(first)
            .take_while(move |(_, lines)| lines.start < end && start < end)
    }
}

pub(crate) fn wheel_scroll(
    area: Rect,
    tabs: ChatTabs<'_>,
    choice: usize,
    delta: isize,
) -> ChooserScroll {
    let rows = ChoiceRows::new(area, tabs);
    ChooserScroll {
        top: rows
            .top(tabs.scroll, choice)
            .saturating_add_signed(delta)
            .min(rows.max_top()),
        wheel_scrolled: true,
    }
}

pub(crate) fn navigate(
    area: Rect,
    tabs: ChatTabs<'_>,
    choice: usize,
    action: ListAction,
) -> (usize, ChooserScroll) {
    let rows = ChoiceRows::new(area, tabs);
    let top = rows.top(tabs.scroll, choice);
    let last = rows.choices.len() - 1;
    let selected = &rows.choices[choice];
    let height = rows.height.max(1);
    let visible = rows.visible_choices(tabs.scroll, choice);
    let choice = match action {
        ListAction::JumpTop => return (0, ChooserScroll::default()),
        ListAction::JumpBottom => last,
        ListAction::MoveUp | ListAction::PageUp
            if selected.start >= CHOOSER_BODY_START + top + rows.height =>
        {
            visible.last().map_or(choice, |(index, _)| index)
        }
        ListAction::MoveDown | ListAction::PageDown if selected.end <= CHOOSER_BODY_START + top => {
            visible
                .into_iter()
                .next()
                .map_or(choice, |(index, _)| index)
        }
        ListAction::MoveUp => choice.saturating_sub(1),
        ListAction::MoveDown => (choice + 1).min(last),
        ListAction::PageUp => rows
            .choices
            .partition_point(|lines| lines.end <= selected.start.saturating_sub(height))
            .min(choice.saturating_sub(1)),
        ListAction::PageDown => rows
            .choices
            .partition_point(|lines| lines.start <= selected.start + height)
            .saturating_sub(1)
            .max((choice + 1).min(last)),
        _ => return (choice, tabs.scroll),
    };
    let selectable = |index: &usize| !tabs.agent_is_disabled(*index);
    let choice = match action {
        ListAction::MoveDown | ListAction::PageDown => (choice..=last)
            .find(selectable)
            .or_else(|| (0..choice).rev().find(selectable)),
        _ => (0..=choice).rev().find(selectable),
    }
    .unwrap_or_default();
    if choice == 0 {
        return (choice, ChooserScroll::default());
    }
    let mut scroll = ChooserScroll {
        top,
        wheel_scrolled: false,
    };
    scroll.top = rows.top(scroll, choice);
    (choice, scroll)
}

pub(crate) fn choice_is_visible(area: Rect, tabs: ChatTabs<'_>, choice: usize) -> bool {
    ChoiceRows::new(area, tabs)
        .visible_choices(tabs.scroll, choice)
        .any(|(index, _)| index == choice)
}

#[cfg(test)]
pub(crate) fn choice_at(area: Rect, tabs: ChatTabs<'_>, choice: usize, y: u16) -> Option<usize> {
    row_targets(area, tabs, choice, NewChatState::Available)
        .find_map(|(index, row)| row.contains((row.x, y).into()).then_some(index))
}

pub(crate) fn row_targets(
    area: Rect,
    tabs: ChatTabs<'_>,
    choice: usize,
    new_chat: NewChatState,
) -> impl Iterator<Item = (usize, Rect)> + '_ {
    let rows = ChoiceRows::new(area, tabs);
    let top = rows.top(tabs.scroll, choice);
    let start = CHOOSER_BODY_START + top;
    let end = start + rows.height;
    rows.choices
        .into_iter()
        .enumerate()
        .filter_map(move |(index, lines)| {
            let first = lines.start.max(start);
            let last = lines.end.min(end);
            (first < last
                && !tabs.agent_is_disabled(index)
                && match tabs.file(index) {
                    Some(_) => tabs.file_preview_unavailable.is_none(),
                    None => new_chat == NewChatState::Available || tabs.agent(index).is_some(),
                })
            .then(|| Rect {
                height: (last - first) as u16,
                ..body_row(area, area.y + (first - top) as u16)
            })
            .map(|rect| (index, rect))
        })
}

pub(crate) fn footer_targets(
    area: Rect,
    hints: &HintLine<ChooserAction>,
) -> impl Iterator<Item = (ChooserAction, Rect)> + '_ {
    let row = body_row(area, area.bottom().saturating_sub(1));
    hints.targets.iter().filter_map(move |(range, action)| {
        if area.height < 2 {
            return None;
        }
        Some((
            *action,
            Rect::new(row.x + range.start, row.y, range.end - range.start, 1),
        ))
    })
}

pub(crate) fn file_path_key_available(keymap: &RuntimeKeymap) -> bool {
    keymap
        .list
        .action_for(crossterm::event::KeyCode::Char('p').into())
        .is_none()
}

pub(crate) fn scroll_file_path(body: Rect, path: &str, top: usize, action: ListAction) -> usize {
    let (path_rows, height) = file_path_layout(body, path);
    let limit = path_rows.len().saturating_sub(height);
    let top = top.min(limit);
    match action {
        ListAction::MoveUp => top.saturating_sub(1),
        ListAction::MoveDown => (top + 1).min(limit),
        ListAction::PageUp => top.saturating_sub(height),
        ListAction::PageDown => (top + height).min(limit),
        ListAction::JumpTop => 0,
        ListAction::JumpBottom => limit,
        _ => top,
    }
}

pub(crate) fn file_path_areas(area: Rect) -> [Rect; 2] {
    let footer_height = area.height.min(1);
    let top = footer_height + u16::from(area.height > 3);
    [
        Rect {
            height: area.height.saturating_sub(top + footer_height),
            ..body_row(area, area.y + top)
        },
        Rect {
            height: footer_height,
            ..body_row(area, area.bottom() - footer_height)
        },
    ]
}

fn file_path_layout(body: Rect, path: &str) -> (Vec<std::borrow::Cow<'_, str>>, usize) {
    let visible = usize::from(body.height.saturating_sub(u16::from(body.height > 2) * 2));
    let options = textwrap::Options::new(usize::from(body.width.max(1)))
        .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
        .word_splitter(textwrap::WordSplitter::NoHyphenation)
        .word_separator(textwrap::WordSeparator::Custom(|path| {
            Box::new(
                path.split_inclusive(['/', '\\'])
                    .map(textwrap::core::Word::from),
            )
        }));
    (textwrap::wrap(path, options), visible)
}

pub(crate) fn paint_file_path(body: Rect, path: &str, top: usize, buf: &mut Buffer) {
    let mut lines = Vec::new();
    if body.height > 2 {
        lines.extend([Line::from("File path").bold(), Line::default()]);
    }
    let (path, visible) = file_path_layout(body, path);
    let top = top.min(path.len().saturating_sub(visible));
    lines.extend(path.into_iter().skip(top).take(visible).map(Line::from));
    ratatui::widgets::Paragraph::new(lines).render(body, buf);
}

pub(crate) struct SidebarStyle<'a> {
    pub(crate) new_chat: NewChatState,
    pub(crate) use_theme_colors: bool,
    pub(crate) motion: crate::motion::MotionMode,
    pub(crate) frames: &'a crate::tui::FrameRequester,
    pub(crate) hovered_choice: Option<usize>,
    pub(crate) full_path: Option<&'a str>,
    pub(crate) path_scroll: usize,
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
    let width = usize::from(row(area.y).width.saturating_sub(1));
    let footer_y = area.bottom() - 1;
    if let Some(path) = sidebar_style.full_path {
        let [body, footer_area] = file_path_areas(area);
        paint_file_path(body, path, sidebar_style.path_scroll, buf);
        footer.render(footer_area, buf);
        return;
    }
    let rows = ChoiceRows::new(area, tabs);
    let waiting = sidebar_style.new_chat == NewChatState::WaitingForMainMessage
        && tabs.files.is_empty()
        && tabs.agents.is_empty();
    let top = if waiting {
        0
    } else {
        rows.top(tabs.scroll, choice)
    };
    let mut paint = |offset: usize, line: Line<'_>| {
        if let Some(y) = offset.checked_sub(top)
            && (CHOOSER_BODY_START..CHOOSER_BODY_START + rows.height).contains(&y)
        {
            let mut content = row(area.y + y as u16);
            if content.width > 0 {
                content.width -= 1;
                buf.set_style(
                    Rect::new(
                        content.right(),
                        content.y,
                        /*width*/ 1,
                        /*height*/ 1,
                    ),
                    line.style,
                );
            }
            line.render(content, buf);
        }
    };
    paint(2, Line::from("Open in sidebar").bold());
    if waiting {
        for (offset, text) in (FIRST_CHOICE_ROW..).zip(textwrap::wrap(
            "You can start a side chat after your first message in the main chat.",
            width.max(1),
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
            paint(
                3,
                Line::from(vec![
                    glyph.set_style(crate::style::accent_style()),
                    " ".into(),
                    label.dim(),
                ]),
            );
        }
        Discovery::Failed(error) => paint(3, Line::from(error.as_str().red())),
        Discovery::Idle if tabs.files_loading => paint(3, Line::from("Loading older files…").dim()),
        Discovery::Idle => {}
    }
    let secondary = crate::style::secondary_text_style();
    let row_style = |index| {
        if index == choice {
            ('›', style, style.not_bold())
        } else if sidebar_style.hovered_choice == Some(index) {
            let hover = crate::style::hover_style(crate::style::quiet_workspace_style());
            (' ', hover, hover.not_bold())
        } else {
            (' ', Default::default(), secondary)
        }
    };
    {
        let y = FIRST_CHOICE_ROW;
        let (marker, fill, subtitle) = row_style(0);
        let title = format!("{marker} + New side chat");
        let detail = match sidebar_style.new_chat {
            NewChatState::Available => "Start from this conversation",
            NewChatState::WaitingForMainMessage => "Available after first message",
        };
        let detail = Span::styled(detail, subtitle);
        if rows.wide {
            let pad = width - detail.width();
            paint(
                y,
                Line::from(vec![format!("{title:<pad$}").into(), detail]).style(fill),
            );
        } else {
            paint(y, Line::from(title).style(fill));
            paint(y + 1, Line::from(vec!["    ".into(), detail]).style(fill));
        }
    }
    if let Some(y) = rows.chats_header {
        let total = tabs.choices.len();
        paint(
            y,
            Line::from(format!("Side chats  {total}")).style(secondary),
        );
    }
    if let Some(y) = rows.agents_header {
        let total = tabs.agents.len();
        paint(
            y,
            Line::from(format!("Subagents  {total}")).style(secondary),
        );
    }
    if let Some(y) = rows.files_header
        && (top + CHOOSER_BODY_START..top + CHOOSER_BODY_START + rows.height).contains(&y)
    {
        let total = tabs.files.len();
        let group = if tabs
            .files
            .iter()
            .all(|entry| crate::markdown_render::is_markdown_file(&entry.path))
        {
            "Markdown files"
        } else {
            "Files"
        };
        paint(y, Line::from(format!("{group}  {total}")).style(secondary));
    }
    for (index, lines) in rows
        .visible_choices(tabs.scroll, choice)
        .skip_while(|(index, _)| *index == 0)
    {
        let (marker, fill, subtitle) = row_style(index);
        if let Some(agent) = tabs.agent(index) {
            let (marker, fill, subtitle) = if agent.selectable {
                (marker, fill, subtitle)
            } else {
                let disabled = Style::default().dim();
                (' ', disabled, disabled)
            };
            let name = format!("{marker} {} {}", agent.symbol, agent.name);
            let status = agent.status;
            if rows.wide {
                let title_width = width.saturating_sub(crate::width::display_width(status) + 2);
                let mut line = truncate_line_with_ellipsis_if_overflow(name.into(), title_width);
                let gap = title_width.saturating_sub(line.width()) + 2;
                line.spans
                    .push(format!("{:gap$}{status}", "").set_style(subtitle));
                paint(lines.start, line.style(fill));
            } else {
                let title = truncate_line_with_ellipsis_if_overflow(name.into(), width);
                paint(lines.start, title.style(fill));
                paint(
                    lines.start + 1,
                    Line::from(format!("    {status}")).style(subtitle),
                );
            }
            continue;
        }
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
        if let Some(link) = tabs.file(index) {
            let y = lines.start;
            let relative = tabs.files_cwd.and_then(|cwd| {
                LegacyAppPathString::from_string(&link.path)
                    .to_inferred_path_uri()?
                    .relative_path_from(cwd)
            });
            let path = relative.as_deref().unwrap_or(&link.path);
            let (directory, name) = path
                .rsplit_once(['/', '\\'])
                .unwrap_or(("Project root", path));
            let directory = tabs.file_preview_unavailable.unwrap_or(directory);
            let width = width.saturating_sub(4);
            if rows.wide {
                let name = filename_label(name, width.saturating_sub(5));
                let directory_width = width.saturating_sub(crate::width::display_width(&name) + 2);
                let directory = middle_label(directory, directory_width);
                paint(
                    y,
                    Line::from(vec![
                        format!("{marker} · {name}").into(),
                        format!("  {directory}").set_style(subtitle),
                    ])
                    .style(fill),
                );
            } else {
                let name = filename_label(name, width);
                paint(y, Line::from(format!("{marker} · {name}")).style(fill));
                if y + 1 < lines.end {
                    let directory = middle_label(directory, width);
                    paint(
                        y + 1,
                        Line::from(format!("    {directory}")).style(subtitle),
                    );
                }
            }
            continue;
        }
        let ordinal = index - 1;
        let chat = &tabs.choices[ordinal];
        let selected = index == choice && focused;
        let label = side_chat_label(chat);
        let title = if let Some(label) = label {
            label.to_owned()
        } else if tabs.choices.len() == 1 {
            "Side chat".to_string()
        } else {
            format!("Side chat {}", ordinal + 1)
        };
        let title = if sidebar_style.use_theme_colors && !selected {
            let identity = crate::thread_color::thread_color(chat.id);
            title.fg(crate::style::readable_color_on(identity, fill.bg))
        } else {
            title.into()
        };
        let opening = tabs.opening.filter(|o| o.target.child == chat.id);
        let symbol = if let Some(Opening {
            state: OpenState::Loading(started),
            ..
        }) = opening
        {
            Span::styled(
                crate::motion::loading_glyph(*started, sidebar_style.motion, sidebar_style.frames),
                fill.fg(crate::style::accent_color_on(fill.bg)),
            )
        } else {
            Span::styled("○", if selected { style } else { secondary })
        };
        let mut spans = vec![format!("{marker} ").into(), symbol, " ".into()];
        let label_style = label.map_or(ThreadLabelStyle::Plain, |_| tabs.label_style);
        if let Some(opening) = opening {
            let status = match &opening.state {
                OpenState::Waiting => "Waiting to open…".dim(),
                OpenState::Loading(_) => "Opening…".dim(),
                OpenState::Failed(error) => format!("Retry · {error}").red(),
            };
            let width = width.saturating_sub(6);
            let title = label_style.render(
                Line::from(title),
                width.saturating_sub(status.width()).max(width / 3),
            );
            spans.extend(title.spans);
            spans.extend(["  ".into(), status]);
        } else {
            spans.extend(
                label_style
                    .render(Line::from(title), width.saturating_sub(4))
                    .spans,
            );
        }
        let line = Line::from(spans).style(fill);
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
    // Keep these in the right margin, outside the clickable list rows.
    for (visible, y, glyph) in [
        (top > 0, area.y + 1, "↑"),
        (top < rows.max_top(), footer_y.saturating_sub(1), "↓"),
    ] {
        if visible && rows.height > 0 && area.width >= 3 {
            Line::from(glyph)
                .style(secondary)
                .render(Rect::new(area.right() - 2, y, 1, 1), buf);
        }
    }
    if footer_y > area.y {
        footer.render(row(footer_y), buf);
    }
}
