//! Transcript gestures leave ordinary typing and composer editing with the existing input path.
//! Stationary link clicks open on release; dragging or scrolling keeps the gesture in selection.

use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Position as ScreenPosition;

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
    pub(crate) fn navigate_pager(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
        keymap: &crate::keymap::PagerKeymap,
    ) -> bool {
        if keymap.jump_top.is_pressed(key) {
            self.jump_to_beginning(cells);
            return true;
        }
        if keymap.jump_bottom.is_pressed(key) {
            self.jump_to_latest();
            return true;
        }
        let page = self.area.height.max(/*other*/ 1) as isize;
        let half = (page + 1) / 2;
        let delta = [
            (&keymap.scroll_up, -1),
            (&keymap.scroll_down, 1),
            (&keymap.page_up, -page),
            (&keymap.page_down, page),
            (&keymap.half_page_up, -half),
            (&keymap.half_page_down, half),
        ]
        .into_iter()
        .find_map(|(bindings, delta)| bindings.is_pressed(key).then_some(delta));
        let Some(delta) = delta else {
            return false;
        };
        self.scroll(cells, delta);
        true
    }

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
        self.is_search_active()
            && (matches!(code, KeyCode::Esc | KeyCode::Enter)
                || (modifiers == KeyModifiers::CONTROL
                    && matches!(code, KeyCode::Char('c' | 'n' | 'p'))))
    }

    pub(crate) fn handle_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if let Some(action) = self.handle_disclosure_key(key, cells) {
            return Some(action);
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
        if self.handle_search_key(key, cells) {
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
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && event
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
            && let Some((area, tip)) = &self.composer_tip
            && area.contains(ScreenPosition::new(event.column, event.row))
        {
            let column = usize::from(event.column - area.x);
            return tip
                .hyperlinks
                .iter()
                .find(|link| link.columns.contains(&column))?
                .terminal_destination()
                .map(ViewAction::OpenLink);
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
            MouseEventKind::Down(MouseButton::Right) if inside => {
                return self
                    .selected_text(cells)
                    .filter(|text| !text.is_empty())
                    .map(ViewAction::Copy);
            }
            MouseEventKind::Down(MouseButton::Left) => return self.pointer_down(event, cells),
            MouseEventKind::Drag(MouseButton::Left) if dragging => {
                self.extend_selection(event.column, event.row);
            }
            MouseEventKind::Up(MouseButton::Left) if dragging => {
                // Open only a stationary click. Dragging back to the origin is still selection,
                // and wheel scrolling clears the pointer even when it cannot move the viewport.
                let link = self.selection.as_mut().and_then(|selection| {
                    (inside
                        && !selection.moved
                        && selection.pointer == Some(ScreenPosition::new(event.column, event.row))
                        && event.modifiers.is_empty())
                    .then(|| selection.pressed_link.take())
                    .flatten()
                });
                let link = link.filter(|destination| {
                    self.visible
                        .get(usize::from(event.row - self.area.y))
                        .and_then(|visible| {
                            visible
                                .layout
                                .link_at(visible.row, event.column - self.area.x)
                        })
                        .as_ref()
                        == Some(destination)
                });
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
                if let Some(link) = link {
                    return Some(ViewAction::OpenLink(link));
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
        if crate::text_selection::is_copy_key(key)
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
        if event.modifiers.is_empty() && self.toggle_disclosure_at(cells, event.column, event.row) {
            return Some(ViewAction::Changed);
        }
        self.disclosure.focused = None;
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
        let clicks =
            crate::text_selection::click_count(&mut self.last_click, event.column, event.row);
        if clicks >= 2
            && visible
                .layout
                .position_at(visible.row, event.column.saturating_sub(self.area.x))
                == visible.layout.position_at(visible.row, u16::MAX)
        {
            return None;
        }
        let link = (clicks == 1 && event.modifiers.is_empty())
            .then(|| {
                visible
                    .layout
                    .link_at(visible.row, event.column.saturating_sub(self.area.x))
            })
            .flatten();
        self.begin_selection(cells, event.column, event.row, clicks);
        if let Some(selection) = &mut self.selection {
            selection.pressed_link = link;
        }
        Some(ViewAction::Changed)
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "right_click_copy_tests.rs"]
mod right_click_copy_tests;
