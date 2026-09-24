//! Hit testing and editable mouse selections share the textarea's rendered wrap and scroll state.

use super::*;
use crate::text_selection::SelectionUnit;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Position;

#[derive(Debug)]
pub(super) struct MouseSelection {
    pending_copy: Option<(u64, Range<usize>)>,
    origin: Range<usize>,
    unit: SelectionUnit,
    dragging: bool,
    moved: bool,
}

impl TextArea {
    pub(in crate::bottom_pane) fn defer_copy(&mut self, id: u64) {
        if let Some(range) = self.mouse_selection_range()
            && let Some(selection) = &mut self.mouse_selection
        {
            selection.pending_copy = Some((id, range));
        }
    }

    pub(in crate::bottom_pane) fn finish_copy(
        &mut self,
        completion: &(u64, crate::clipboard_copy::worker::CopyResult),
        current: bool,
    ) -> Option<usize> {
        let pending = self.mouse_selection.as_mut()?.pending_copy.as_ref()?;
        if pending.0 != completion.0 {
            return None;
        }
        let (_, range) = self.mouse_selection.as_mut()?.pending_copy.take()?;
        if !current || self.mouse_selection_range().as_ref() != Some(&range) {
            return None;
        }
        let characters = self.text()[range].chars().count();
        if completion.1 == Ok(crate::clipboard_copy::CopyStatus::Confirmed) {
            self.set_cursor(self.cursor());
        }
        Some(characters)
    }

    pub(in crate::bottom_pane) fn contains_mouse(&self, event: MouseEvent) -> bool {
        self.rendered_area
            .get()
            .contains(Position::new(event.column, event.row))
    }

    pub(crate) fn end_mouse_drag(&mut self) {
        if let Some(selection) = &mut self.mouse_selection {
            selection.dragging = false;
        }
    }

    pub(crate) fn mouse_selection_range(&self) -> Option<Range<usize>> {
        let origin = &self.mouse_selection.as_ref()?.origin;
        let range = origin.start.min(self.cursor_pos)..origin.end.max(self.cursor_pos);
        (!range.is_empty()).then_some(range)
    }

    pub(super) fn delete_mouse_selection(&mut self) -> bool {
        let Some(range) = self.mouse_selection_range() else {
            return false;
        };
        // A pointer-selected edit cannot be replayed as a keyboard-relative Vim command.
        self.vim_commands = VimCommandState::default();
        self.replace_range(range, "");
        true
    }

    pub(crate) fn handle_mouse(&mut self, event: MouseEvent, state: TextAreaState) -> bool {
        let area = self.rendered_area.get();
        let dragging = self.mouse_selection.as_ref().is_some_and(|s| s.dragging);
        if area.is_empty() {
            self.end_mouse_drag();
            return false;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left)
                if event.modifiers.is_empty() && self.contains_mouse(event) => {}
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
                if dragging => {}
            _ => return false,
        }

        if matches!(event.kind, MouseEventKind::Up(MouseButton::Left))
            && self.mouse_selection.as_ref().is_some_and(|s| !s.moved)
        {
            self.end_mouse_drag();
            return true;
        }
        let line = {
            let lines = self.wrapped_lines(area.width);
            let row = usize::from(state.scroll)
                + usize::from(event.row.saturating_sub(area.y).min(area.height - 1));
            // Dragging beyond the viewport advances one wrapped row per event.
            let row = if event.row < area.y {
                row.saturating_sub(1)
            } else if event.row >= area.bottom() {
                row + 1
            } else {
                row
            };
            lines[row.min(lines.len() - 1)].clone()
        };
        let col = usize::from(event.column.saturating_sub(area.x).min(area.width));
        let pos = self.position_at_display_col_on_line(line.start, line.end - 1, col);
        let down = matches!(event.kind, MouseEventKind::Down(MouseButton::Left));
        let mut selection = if down {
            if self.mouse_selection.is_none() {
                self.last_click = None;
            }
            let clicks =
                crate::text_selection::click_count(&mut self.last_click, event.column, event.row);
            MouseSelection {
                pending_copy: None,
                origin: pos..pos,
                // Padding remains an insertion target on repeated clicks.
                unit: SelectionUnit::from_clicks(if pos == line.end - 1 { 1 } else { clicks }),
                dragging: true,
                moved: false,
            }
        } else if let Some(selection) = self.mouse_selection.take() {
            selection
        } else {
            return false;
        };
        let unit = selection.unit;
        let range = if unit == SelectionUnit::Character {
            let pos = self.clamp_pos_to_nearest_boundary(pos);
            pos..pos
        } else if unit == SelectionUnit::Word
            && let Some(element) = self.elements.iter().find(|e| e.range.contains(&pos))
        {
            element.range.clone()
        } else {
            self.expand_range_to_element_boundaries(unit.range(&self.text, pos))
        };
        self.preferred_col = None;
        self.clear_vim_replace_recovery();
        self.vim_pending = VimPending::None;
        if down {
            selection.origin = range.clone();
        }
        self.cursor_pos = if range.start < selection.origin.start {
            range.start
        } else {
            range.end
        };
        selection.moved |= !down;
        selection.dragging = !matches!(event.kind, MouseEventKind::Up(MouseButton::Left));
        self.mouse_selection = Some(selection);
        true
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
