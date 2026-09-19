//! Transcript gestures leave ordinary typing and composer editing with the existing input path.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Position as ScreenPosition;
use std::time::Duration;
use std::time::Instant;

use super::*;

pub(crate) enum ViewAction {
    Changed,
    Copy(String),
    CopyAndFollow(String),
    OpenLink(String),
}

/// Transcript jumps shared with input routing so legacy terminals preserve their Alt modifier.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum JumpTarget {
    Beginning,
    Latest,
}

impl JumpTarget {
    /// Include the Alt alternative for terminals that do not send Ctrl+Home/End.
    pub(crate) fn hint_label(self) -> String {
        let (control, alternate) = match self {
            Self::Beginning => (KeyCode::Home, KeyCode::Char('<')),
            Self::Latest => (KeyCode::End, KeyCode::Char('>')),
        };
        let alternate = crate::key_hint::alt(alternate).display_label();
        format!(
            "{alternate}/{}",
            crate::key_hint::ctrl(control).display_label()
        )
    }

    pub(crate) fn from_key(key: KeyEvent) -> Option<Self> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        match (key.code, key.modifiers) {
            (KeyCode::Home, KeyModifiers::CONTROL) => Some(Self::Beginning),
            (KeyCode::End, KeyModifiers::CONTROL) => Some(Self::Latest),
            // Zellij can omit the shifted alternate character from enhanced key events.
            (KeyCode::Char(','), modifiers)
                if modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                Some(Self::Beginning)
            }
            (KeyCode::Char('.'), modifiers)
                if modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                Some(Self::Latest)
            }
            // Terminals differ on whether shifted punctuation retains the Shift modifier.
            (KeyCode::Char('<'), modifiers)
                if modifiers.difference(KeyModifiers::SHIFT) == KeyModifiers::ALT =>
            {
                Some(Self::Beginning)
            }
            (KeyCode::Char('>'), modifiers)
                if modifiers.difference(KeyModifiers::SHIFT) == KeyModifiers::ALT =>
            {
                Some(Self::Latest)
            }
            _ => None,
        }
    }
}

impl TranscriptView {
    /// Keep selection and Find control keys ahead of configurable chord prefixes.
    pub(crate) fn owns_interaction_key(&self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        if (code == KeyCode::Char(' ') && modifiers == KeyModifiers::CONTROL)
            || JumpTarget::from_key(key).is_some()
        {
            return true;
        }
        if self.selection.is_some() {
            return (code == KeyCode::Char('c')
                && matches!(modifiers, KeyModifiers::CONTROL | KeyModifiers::SUPER))
                || (code == KeyCode::Char('c')
                    && modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT))
                || code == KeyCode::Esc
                || (code == KeyCode::Enter && modifiers == KeyModifiers::NONE)
                || (matches!(modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
                    && matches!(
                        code,
                        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                    ))
                || (modifiers == KeyModifiers::NONE
                    && matches!(code, KeyCode::PageUp | KeyCode::PageDown));
        }
        false
    }

    pub(crate) fn handle_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        if key.kind == KeyEventKind::Release {
            return None;
        }

        if self.selection.is_some() {
            if let Some(action) = self.handle_selection_key(key, cells) {
                return Some(action);
            }
            self.end_selection(cells);
        }
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char(' ') {
            self.begin_selection(cells, self.area.x, self.area.y, /*clicks*/ 1);
            if let Some(selection) = &mut self.selection {
                selection.dragging = false;
                selection.resume_on_empty = false;
            }
            return Some(ViewAction::Changed);
        }

        self.handle_scroll_key(key, cells)
    }

    pub(crate) fn handle_mouse(
        &mut self,
        event: MouseEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        if let Some(action) = self.handle_follow_control_mouse(event) {
            return Some(action);
        }
        let inside = self
            .area
            .contains(ScreenPosition::new(event.column, event.row));
        let dragging = self
            .selection
            .as_ref()
            .is_some_and(|selection| selection.dragging);
        if !inside && !dragging {
            return None;
        }
        // Wheel navigation supersedes the last drag position. A real drag event can resume it.
        if matches!(
            event.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) && let Some(selection) = &mut self.selection
        {
            selection.pointer = None;
        }
        match event.kind {
            MouseEventKind::ScrollUp => self.scroll(cells, /*rows*/ -3),
            MouseEventKind::ScrollDown => self.scroll(cells, /*rows*/ 3),
            MouseEventKind::Down(MouseButton::Left) => return self.pointer_down(event, cells),
            MouseEventKind::Drag(MouseButton::Left) if dragging => {
                self.extend_selection(event.column, event.row);
            }
            MouseEventKind::Up(MouseButton::Left) if dragging => {
                if self
                    .selection
                    .as_ref()
                    .is_some_and(|selection| selection.moved && selection.pointer.is_some())
                {
                    self.extend_selection(event.column, event.row);
                }
                self.end_drag();
                if self.selected_text(cells).is_none() {
                    self.end_selection(cells);
                }
            }
            _ => return None,
        }
        Some(ViewAction::Changed)
    }

    fn handle_selection_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        // Kitty keyboard reporting delivers macOS Cmd+C as Super+C, including over SSH to
        // non-macOS hosts. Ghostty forwards it when there is no terminal-native selection.
        // Crossterm can encode Ctrl+Shift+C as uppercase C with only Control set.
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        if (matches!(modifiers, KeyModifiers::CONTROL | KeyModifiers::SUPER)
            && code == KeyCode::Char('c'))
            || (modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                && code == KeyCode::Char('c'))
            || (key.modifiers == KeyModifiers::NONE && key.code == KeyCode::Enter)
        {
            return Some(
                self.selected_text(cells)
                    .filter(|text| !text.is_empty())
                    .map_or(ViewAction::Changed, |text| {
                        if key.code == KeyCode::Enter {
                            ViewAction::CopyAndFollow(text)
                        } else {
                            ViewAction::Copy(text)
                        }
                    }),
            );
        }
        if key.code == KeyCode::Esc {
            self.end_selection(cells);
            return Some(ViewAction::Changed);
        }
        if matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
            && self.selection_key(cells, key.code)
        {
            return Some(ViewAction::Changed);
        }
        self.handle_scroll_key(key, cells)
    }

    fn handle_scroll_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        let jump = JumpTarget::from_key(key);
        match (key.code, key.modifiers) {
            (KeyCode::PageUp, KeyModifiers::NONE) => {
                self.scroll(
                    cells,
                    -(self.area.height.saturating_sub(/*rhs*/ 1).max(/*other*/ 1) as isize),
                );
            }
            (KeyCode::PageDown, KeyModifiers::NONE) => {
                self.scroll(
                    cells,
                    self.area.height.saturating_sub(/*rhs*/ 1).max(/*other*/ 1) as isize,
                );
            }
            (KeyCode::Esc, KeyModifiers::NONE) if self.can_return_to_latest() => {
                self.jump_to_latest();
            }
            _ if jump == Some(JumpTarget::Beginning) => {
                self.jump_to_beginning(cells);
            }
            _ if jump == Some(JumpTarget::Latest) => self.jump_to_latest(),
            _ => return None,
        }
        Some(ViewAction::Changed)
    }

    fn pointer_down(
        &mut self,
        event: MouseEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        let visible = self
            .visible
            .get(usize::from(event.row.checked_sub(self.area.y)?))?;
        if event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
        {
            return visible
                .layout
                .link_at(visible.row, event.column.saturating_sub(self.area.x))
                .map(ViewAction::OpenLink);
        }
        let now = Instant::now();
        let clicks = self
            .last_click
            .filter(|(at, column, row, _)| {
                now.duration_since(*at) < Duration::from_millis(/*millis*/ 400)
                    && *column == event.column
                    && *row == event.row
            })
            .map_or(/*default*/ 1, |(_, _, _, clicks)| clicks % 3 + 1);
        self.last_click = Some((now, event.column, event.row, clicks));
        if clicks >= 2
            && visible
                .layout
                .position_at(visible.row, event.column.saturating_sub(self.area.x))
                == visible.layout.position_at(visible.row, u16::MAX)
        {
            return None;
        }
        self.begin_selection(cells, event.column, event.row, clicks);
        Some(ViewAction::Changed)
    }
}
