use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::BrowserView;
use codex_terminal_browser::HumanNavigationAction;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthChar;

use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;

const MAX_URL_BYTES: usize = 4 * 1024;
const NAVIGATION_CONTROLS: &str = " [<] [>] [R] ";
const LOCATION_PREFIX: &str = " URL ";
const LOCATION_PREFIX_WIDTH: u16 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BrowserChromeFocus {
    #[default]
    Page,
    Location,
    LocationSelected,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BrowserChromeState {
    focus: BrowserChromeFocus,
    draft: String,
    cursor: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BrowserChromeKeyResult {
    Ignored,
    Consumed,
    Navigate(HumanNavigationAction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BrowserChromeMouseResult {
    Ignored,
    Consumed(Option<HumanNavigationAction>),
}

impl BrowserChromeState {
    pub(crate) fn sync_url(&mut self, url: Option<&str>) {
        if self.is_location_focused() {
            return;
        }
        let url = url.unwrap_or("about:blank");
        if self.draft != url {
            self.draft = url.to_string();
            self.cursor = self.draft.len();
        }
    }

    pub(crate) fn is_location_focused(&self) -> bool {
        matches!(
            self.focus,
            BrowserChromeFocus::Location | BrowserChromeFocus::LocationSelected
        )
    }

    pub(crate) fn focus_page(&mut self) {
        self.focus = BrowserChromeFocus::Page;
    }

    pub(crate) fn focus_location(&mut self) {
        self.focus = BrowserChromeFocus::LocationSelected;
        self.cursor = self.draft.len();
    }

    pub(crate) fn handle_key(&mut self, event: KeyEvent) -> BrowserChromeKeyResult {
        if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return BrowserChromeKeyResult::Ignored;
        }
        if self.focus == BrowserChromeFocus::Page {
            if event.code == KeyCode::Char('l')
                && event
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
            {
                self.focus_location();
                return BrowserChromeKeyResult::Consumed;
            }
            return BrowserChromeKeyResult::Ignored;
        }

        match event.code {
            KeyCode::Esc => {
                self.focus_page();
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Enter => {
                let raw_url = self.draft.trim();
                if raw_url.is_empty() {
                    return BrowserChromeKeyResult::Consumed;
                }
                let url = if raw_url.contains("://") {
                    raw_url.to_string()
                } else {
                    format!("https://{raw_url}")
                };
                self.focus_page();
                BrowserChromeKeyResult::Navigate(HumanNavigationAction::Goto(url))
            }
            KeyCode::Char('a')
                if event
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER) =>
            {
                self.cursor = self.draft.len();
                self.focus = BrowserChromeFocus::LocationSelected;
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Char(character)
                if !event.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                let mut encoded = [0; 4];
                let character = character.encode_utf8(&mut encoded);
                self.clear_selection();
                if self.draft.len().saturating_add(character.len()) <= MAX_URL_BYTES {
                    self.draft.insert_str(self.cursor, character);
                    self.cursor += character.len();
                }
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Backspace => {
                if !self.clear_selection() && self.cursor > 0 {
                    let previous = previous_char_boundary(&self.draft, self.cursor);
                    self.draft.drain(previous..self.cursor);
                    self.cursor = previous;
                }
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Delete => {
                if !self.clear_selection() && self.cursor < self.draft.len() {
                    let next = next_char_boundary(&self.draft, self.cursor);
                    self.draft.drain(self.cursor..next);
                }
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Left => {
                self.cursor = if self.focus == BrowserChromeFocus::LocationSelected {
                    0
                } else {
                    previous_char_boundary(&self.draft, self.cursor)
                };
                self.focus = BrowserChromeFocus::Location;
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Right => {
                self.cursor = if self.focus == BrowserChromeFocus::LocationSelected {
                    self.draft.len()
                } else {
                    next_char_boundary(&self.draft, self.cursor)
                };
                self.focus = BrowserChromeFocus::Location;
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::Home => {
                self.cursor = 0;
                self.focus = BrowserChromeFocus::Location;
                BrowserChromeKeyResult::Consumed
            }
            KeyCode::End => {
                self.cursor = self.draft.len();
                self.focus = BrowserChromeFocus::Location;
                BrowserChromeKeyResult::Consumed
            }
            _ => BrowserChromeKeyResult::Consumed,
        }
    }

    pub(crate) fn handle_paste(&mut self, pasted: &str) -> bool {
        if !self.is_location_focused() {
            return false;
        }
        self.clear_selection();
        let remaining = MAX_URL_BYTES.saturating_sub(self.draft.len());
        let end = pasted.floor_char_boundary(remaining.min(pasted.len()));
        self.draft.insert_str(self.cursor, &pasted[..end]);
        self.cursor += end;
        true
    }

    fn clear_selection(&mut self) -> bool {
        if self.focus != BrowserChromeFocus::LocationSelected {
            return false;
        }
        self.draft.clear();
        self.cursor = 0;
        self.focus = BrowserChromeFocus::Location;
        true
    }

    pub(crate) fn handle_mouse_primary(
        &mut self,
        event: MousePrimaryEvent,
        header: Rect,
    ) -> BrowserChromeMouseResult {
        if header.is_empty() || !header.contains((event.column, event.row).into()) {
            return BrowserChromeMouseResult::Ignored;
        }
        if event.kind != MousePrimaryEventKind::Press {
            return BrowserChromeMouseResult::Consumed(None);
        }
        let column = event.column.saturating_sub(header.x);
        let row = event.row.saturating_sub(header.y);
        let navigation = if row == 0 {
            match column {
                1..=3 => Some(HumanNavigationAction::Back),
                5..=7 => Some(HumanNavigationAction::Forward),
                9..=11 => Some(HumanNavigationAction::Reload),
                _ => None,
            }
        } else if row == 1 {
            self.focus_location();
            None
        } else {
            None
        };
        BrowserChromeMouseResult::Consumed(navigation)
    }

    fn location_view(&self, width: u16) -> LocationSlice<'_> {
        if width == 0 {
            return LocationSlice {
                text: "",
                cursor_col: 0,
            };
        }
        let (start, before_width) = if self.focus == BrowserChromeFocus::Page {
            (0, 0)
        } else {
            let cursor_budget = usize::from(width.saturating_sub(/*rhs*/ 1));
            let mut start = self.cursor;
            let mut before_width = 0;
            for (index, character) in self.draft[..self.cursor].char_indices().rev() {
                let character_width = character.width().unwrap_or(/*default*/ 0);
                if before_width + character_width > cursor_budget {
                    break;
                }
                before_width += character_width;
                start = index;
            }
            (start, before_width)
        };
        let mut end = start;
        let mut visible_width = 0;
        for (offset, character) in self.draft[start..].char_indices() {
            let character_width = character.width().unwrap_or(/*default*/ 0);
            if visible_width + character_width > usize::from(width) {
                break;
            }
            visible_width += character_width;
            end = start + offset + character.len_utf8();
        }
        LocationSlice {
            text: &self.draft[start..end],
            cursor_col: u16::try_from(before_width).unwrap_or(/*default*/ u16::MAX),
        }
    }
}

struct LocationSlice<'a> {
    text: &'a str,
    cursor_col: u16,
}

pub(crate) fn render_browser_chrome(
    view: &BrowserView,
    state: &BrowserChromeState,
    area: Rect,
    buf: &mut Buffer,
) -> Option<(u16, u16)> {
    if area.height == 0 {
        return None;
    }
    let title = view.title.as_deref().unwrap_or("Carbonyl");
    let status = status_label(&view.status);
    Line::from(vec![
        NAVIGATION_CONTROLS.cyan(),
        format!("{title} ").into(),
        status.dim(),
    ])
    .render_ref(area, buf);
    if area.height == 1 {
        return None;
    }
    let location_width = area.width.saturating_sub(LOCATION_PREFIX_WIDTH);
    let location = state.location_view(location_width);
    let location_style = if state.is_location_focused() {
        location.text.reversed()
    } else {
        location.text.dim()
    };
    Line::from(vec![LOCATION_PREFIX.cyan(), location_style]).render_ref(
        Rect::new(
            area.x,
            area.y.saturating_add(/*rhs*/ 1),
            area.width,
            /*height*/ 1,
        ),
        buf,
    );
    state.is_location_focused().then(|| {
        (
            area.x
                .saturating_add(LOCATION_PREFIX_WIDTH)
                .saturating_add(location.cursor_col)
                .min(area.right().saturating_sub(/*rhs*/ 1)),
            area.y.saturating_add(/*rhs*/ 1),
        )
    })
}

fn status_label(status: &BrowserStatus) -> &'static str {
    match status {
        BrowserStatus::Unavailable { .. } => "unavailable",
        BrowserStatus::Idle => "idle",
        BrowserStatus::Starting => "starting",
        BrowserStatus::Running => "running",
        BrowserStatus::Crashed { .. } => "crashed",
    }
}

fn previous_char_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .char_indices()
        .next_back()
        .map_or(/*default*/ 0, |(index, _)| index)
}

fn next_char_boundary(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .char_indices()
        .nth(/*n*/ 1)
        .map_or(text.len(), |(offset, _)| cursor + offset)
}

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod tests;
