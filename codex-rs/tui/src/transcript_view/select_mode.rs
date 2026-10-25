//! Keyboard-owned transcript cursor and mark state over the existing selection snapshot.
//!
//! An empty `Selection` is the caret. Marking only records its fixed and active anchors here;
//! copying, retained revisions, serialization, and asynchronous completion remain owned by
//! `Selection`.

use super::*;
use crate::key_hint::KeyBindingListExt;
use crate::text_selection::SelectionUnit;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use unicode_segmentation::UnicodeSegmentation;

mod motion;

#[derive(Clone, Copy, Default)]
pub(super) struct SelectMode {
    pub(super) mark: Option<Mark>,
    pub(super) preferred_column: Option<u16>,
    pub(super) search_origin: Option<SelectSearchOrigin>,
}

#[derive(Clone, Copy)]
pub(super) struct Mark {
    anchor: Anchor,
    active: Anchor,
    anchor_edges: Option<(Anchor, Anchor)>,
    active_edges: Option<(Anchor, Anchor)>,
    unit: SelectionUnit,
}

#[derive(Clone, Copy)]
pub(super) struct SelectSearchOrigin {
    pub(super) mark: Option<Mark>,
    pub(super) preferred_column: Option<u16>,
    pub(super) start: Anchor,
    pub(super) end: Anchor,
}

#[derive(Clone, Copy)]
enum CursorMotion {
    Character(KeyCode),
    Rows(isize),
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    LineEnd,
    Beginning,
    End,
}

fn pager_motion(
    keymap: &crate::keymap::RuntimeKeymap,
    key: KeyEvent,
    area_height: u16,
) -> Option<CursorMotion> {
    let pager = &keymap.pager;
    if pager.jump_top.is_pressed(key) {
        return Some(CursorMotion::Beginning);
    }
    if pager.jump_bottom.is_pressed(key) {
        return Some(CursorMotion::End);
    }
    let page = isize::try_from(area_height.saturating_sub(1).max(1)).unwrap_or(isize::MAX);
    let half = isize::try_from(area_height.div_ceil(2)).unwrap_or(isize::MAX);
    [
        (&pager.scroll_up, -1),
        (&pager.scroll_down, 1),
        (&pager.page_up, -page),
        (&pager.page_down, page),
        (&pager.half_page_up, -half),
        (&pager.half_page_down, half),
    ]
    .into_iter()
    .find_map(|(action_bindings, rows)| {
        action_bindings
            .is_pressed(key)
            .then_some(CursorMotion::Rows(rows))
    })
}

impl SelectMode {
    pub(super) fn cursor(self, selection: &Selection) -> Anchor {
        self.mark.map_or(selection.end, |mark| mark.active)
    }
}

impl TranscriptView {
    /// Enter at the accepted Find hit, preferring the bottommost visible selectable position.
    pub(crate) fn begin_select_mode(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        if self.select_mode.is_some() {
            return true;
        }
        self.copy_mode = None;
        self.cancel_beginning();
        let has_range = self.has_selection_range();
        if self.selection.is_none() {
            let cursor = self
                .search
                .match_anchor()
                .or_else(|| {
                    self.visible.iter().rev().find_map(|visible| {
                        let offset = visible.layout.first_cursor_position(visible.row)?;
                        Some(Anchor {
                            key: visible.key,
                            index: visible.index,
                            offset,
                            row_bias: visible.layout.row_for_offset(offset) as isize
                                - visible.row as isize,
                        })
                    })
                })
                .or_else(|| self.select_boundary(cells, /*end*/ true));
            let Some(cursor) = cursor else {
                return false;
            };
            let snapshot = self.capture_snapshot(cells);
            self.selection = Some(Selection::keyboard(snapshot, cursor));
        }
        if self.position == Position::Latest {
            self.hold_position();
        }
        self.select_mode = Some(SelectMode::default());
        if has_range {
            self.adopt_select_mouse_range();
        } else if self.search.is_reading() {
            self.accept_search_into_select_mode(cells);
        }
        self.reveal_select_cursor(cells);
        true
    }

    pub(crate) fn is_select_mode(&self) -> bool {
        self.select_mode.is_some()
    }

    /// Finish an explicit beginning traversal after the last older page is installed.
    pub(crate) fn complete_select_beginning(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        if self.select_mode.is_some() {
            self.move_select_cursor(cells, CursorMotion::Beginning);
        }
    }

    /// Leave the mode while retaining its displayed revision at the paused reading position.
    pub(crate) fn end_select_mode(&mut self) {
        if self.select_mode.take().is_none() {
            return;
        }
        self.cancel_beginning();
        if self.position == Position::Latest {
            self.hold_position();
        }
        if let Some(selection) = self.selection.take() {
            self.held_reading = Some(selection.snapshot);
        }
    }

    pub(crate) fn handle_select_mode_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        if self.select_mode.is_none() || key.kind == KeyEventKind::Release {
            return None;
        }
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        let pressed = key.kind == KeyEventKind::Press;
        if crate::text_selection::is_copy_key(key)
            || (code == KeyCode::Char('y') && modifiers == KeyModifiers::NONE)
            || (code == KeyCode::Enter
                && modifiers == KeyModifiers::NONE
                && key.kind == KeyEventKind::Press)
        {
            let Some(text) = self.selected_text(cells).filter(|text| !text.is_empty()) else {
                if code == KeyCode::Enter {
                    return Some(self.exit_select_mode());
                }
                return Some(ViewAction::Changed);
            };
            return Some(if code == KeyCode::Enter {
                ViewAction::CopyAndFollow(text)
            } else {
                ViewAction::Copy(text)
            });
        }
        if pressed && code == KeyCode::Esc && modifiers == KeyModifiers::NONE {
            if self.select_mode.is_some_and(|mode| mode.mark.is_some()) {
                self.clear_select_mark();
                return Some(ViewAction::Changed);
            }
            return Some(self.exit_select_mode());
        }
        if pressed && code == KeyCode::Char('q') && modifiers == KeyModifiers::NONE {
            return Some(self.exit_select_mode());
        }
        if pressed && code == KeyCode::Char('/') && modifiers == KeyModifiers::NONE {
            self.begin_search();
            return Some(ViewAction::Changed);
        }
        if pressed
            && matches!(code, KeyCode::Char('v') | KeyCode::Char(' '))
            && modifiers == KeyModifiers::NONE
        {
            self.start_select_mark(cells, SelectionUnit::Character);
            return Some(ViewAction::Changed);
        }
        if pressed && code == KeyCode::Char('v') && modifiers == KeyModifiers::SHIFT {
            self.start_select_mark(cells, SelectionUnit::Line);
            return Some(ViewAction::Changed);
        }
        if pressed && code == KeyCode::Char('o') && modifiers == KeyModifiers::NONE {
            self.swap_select_ends(cells);
            return Some(ViewAction::Changed);
        }
        if code == KeyCode::Char('n')
            && matches!(modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
            && self.repeat_select_search(cells, modifiers == KeyModifiers::SHIFT)
        {
            return Some(ViewAction::Changed);
        }

        let shift_arrow = modifiers == KeyModifiers::SHIFT
            && matches!(
                code,
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
            );
        if shift_arrow && self.select_mode.is_some_and(|mode| mode.mark.is_none()) {
            self.start_select_mark(cells, SelectionUnit::Character);
        }
        let motion = match (code, modifiers) {
            (
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down,
                KeyModifiers::NONE | KeyModifiers::SHIFT,
            ) => {
                if matches!(code, KeyCode::Up | KeyCode::Down) {
                    CursorMotion::Rows(if code == KeyCode::Up { -1 } else { 1 })
                } else {
                    CursorMotion::Character(code)
                }
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) => CursorMotion::Character(KeyCode::Left),
            (KeyCode::Char('l'), KeyModifiers::NONE) => CursorMotion::Character(KeyCode::Right),
            (KeyCode::Char('k'), KeyModifiers::NONE) => CursorMotion::Rows(-1),
            (KeyCode::Char('j'), KeyModifiers::NONE) => CursorMotion::Rows(1),
            (KeyCode::Char('w'), KeyModifiers::NONE) => CursorMotion::WordForward,
            (KeyCode::Char('b'), KeyModifiers::NONE) => CursorMotion::WordBackward,
            (KeyCode::Char('e'), KeyModifiers::NONE) => CursorMotion::WordEnd,
            (KeyCode::Home | KeyCode::Char('0'), KeyModifiers::NONE) => CursorMotion::LineStart,
            (KeyCode::End, KeyModifiers::NONE)
            | (KeyCode::Char('$'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                CursorMotion::LineEnd
            }
            (KeyCode::PageUp, KeyModifiers::NONE) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                CursorMotion::Rows(
                    -isize::try_from(self.area.height.saturating_sub(1).max(1))
                        .unwrap_or(isize::MAX),
                )
            }
            (KeyCode::PageDown, KeyModifiers::NONE)
            | (KeyCode::Char('f'), KeyModifiers::CONTROL) => CursorMotion::Rows(
                isize::try_from(self.area.height.saturating_sub(1).max(1)).unwrap_or(isize::MAX),
            ),
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => CursorMotion::Rows(
                -isize::try_from(self.area.height.div_ceil(2)).unwrap_or(isize::MAX),
            ),
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => CursorMotion::Rows(
                isize::try_from(self.area.height.div_ceil(2)).unwrap_or(isize::MAX),
            ),
            (KeyCode::Char('g'), KeyModifiers::NONE) | (KeyCode::Home, KeyModifiers::CONTROL) => {
                CursorMotion::Beginning
            }
            (KeyCode::Char('g'), KeyModifiers::SHIFT) => CursorMotion::End,
            (KeyCode::End, KeyModifiers::CONTROL) => CursorMotion::End,
            _ if !self.is_search_editing() => {
                pager_motion(&self.disclosure.keymap, key, self.area.height)?
            }
            _ => return None,
        };
        self.move_select_cursor(cells, motion);
        Some(ViewAction::Changed)
    }

    fn exit_select_mode(&mut self) -> ViewAction {
        self.end_select_mode();
        self.close_search_after_select_mode();
        ViewAction::ExitSelectMode
    }

    fn start_select_mark(&mut self, cells: &[Arc<dyn HistoryCell>], unit: SelectionUnit) {
        self.cancel_pending_select_search();
        let Some(mut mode) = self.select_mode else {
            return;
        };
        let Some(selection) = self.selection.as_ref() else {
            return;
        };
        let cursor = mode.cursor(selection);
        mode.mark = match mode.mark {
            Some(mark) if mark.unit == unit => {
                self.clear_select_mark();
                return;
            }
            Some(mark) => Some(Mark {
                anchor_edges: None,
                active_edges: None,
                unit,
                ..mark
            }),
            None => Some(Mark {
                anchor: cursor,
                active: cursor,
                anchor_edges: None,
                active_edges: None,
                unit,
            }),
        };
        mode.preferred_column = None;
        self.select_mode = Some(mode);
        self.sync_select_range(cells);
    }

    fn clear_select_mark(&mut self) {
        self.cancel_pending_select_search();
        let Some(mode) = &mut self.select_mode else {
            return;
        };
        let Some(selection) = &mut self.selection else {
            return;
        };
        let cursor = mode.cursor(selection);
        mode.mark = None;
        mode.preferred_column = None;
        selection.set_keyboard_range(cursor, cursor);
    }

    fn swap_select_ends(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        self.cancel_pending_select_search();
        let Some(mode) = &mut self.select_mode else {
            return;
        };
        let Some(mark) = &mut mode.mark else {
            return;
        };
        std::mem::swap(&mut mark.anchor, &mut mark.active);
        std::mem::swap(&mut mark.anchor_edges, &mut mark.active_edges);
        mode.preferred_column = None;
        self.sync_select_range(cells);
        self.reveal_select_cursor(cells);
    }

    pub(super) fn reveal_select_cursor(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        let Some(mode) = self.select_mode else {
            return;
        };
        let Some(selection) = self.selection.as_ref() else {
            return;
        };
        let cursor = mode.cursor(selection);
        let index = self.resolve(cells, cursor);
        let row = self.layout(cells, index).map_or(/*default*/ 0, |layout| {
            layout
                .row_for_offset(cursor.offset)
                .saturating_add_signed(cursor.row_bias.saturating_neg())
        });
        if !self
            .visible
            .iter()
            .any(|visible| visible.index == index && visible.row == row)
        {
            self.position = Position::Reading(Anchor { index, ..cursor });
        }
    }

    pub(super) fn render_select_cursor(&self, buf: &mut Buffer) {
        let (Some(mode), Some(selection)) = (self.select_mode, self.selection.as_ref()) else {
            return;
        };
        let cursor = mode.cursor(selection);
        let Some((y, visible)) = self.visible.iter().enumerate().find(|(_, visible)| {
            visible.key == cursor.key
                && visible.row
                    == visible
                        .layout
                        .row_for_offset(cursor.offset)
                        .saturating_add_signed(cursor.row_bias.saturating_neg())
        }) else {
            return;
        };
        let column = visible
            .layout
            .column_for_offset_on_row(visible.row, cursor.offset);
        if column < self.area.width {
            let area = Rect::new(
                self.area.x + column,
                self.area.y + y as u16,
                /*width*/ 1,
                /*height*/ 1,
            );
            buf.set_style(area, crate::style::selection_style().underlined());
        }
    }

    pub(super) fn adopt_select_mouse_range(&mut self) {
        self.cancel_pending_select_search();
        let has_range = self.has_selection_range();
        let Some(selection) = &self.selection else {
            return;
        };
        let (start, end, unit) = (selection.start, selection.end, selection.unit());
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or_default();
        let mark = has_range.then(|| {
            if self.anchor_order(cells, start) <= self.anchor_order(cells, end) {
                let active = self.before_grapheme(cells, end);
                let anchor_end = self
                    .selection_endpoint_bounds(cells, start, unit)
                    .map_or(start, |edges| edges.1);
                let active_start = self
                    .selection_endpoint_bounds(cells, active, unit)
                    .map_or(active, |edges| edges.0);
                Mark {
                    anchor: start,
                    active,
                    anchor_edges: Some((start, anchor_end)),
                    active_edges: Some((active_start, end)),
                    unit,
                }
            } else {
                let anchor = self.before_grapheme(cells, start);
                let anchor_start = self
                    .selection_endpoint_bounds(cells, anchor, unit)
                    .map_or(anchor, |edges| edges.0);
                let active_end = self
                    .selection_endpoint_bounds(cells, end, unit)
                    .map_or(end, |edges| edges.1);
                Mark {
                    anchor,
                    active: end,
                    anchor_edges: Some((anchor_start, start)),
                    active_edges: Some((end, active_end)),
                    unit,
                }
            }
        });
        if let Some(mode) = &mut self.select_mode {
            mode.preferred_column = None;
            mode.mark = mark;
        }
    }

    pub(super) fn move_select_cursor_to_search_match(
        &mut self,
        mut anchor: Anchor,
        layout: &TextLayout,
        cells: &[Arc<dyn HistoryCell>],
    ) {
        let text = layout.text();
        let containing = text
            .grapheme_indices(/*is_extended*/ true)
            .rfind(|(offset, _)| *offset <= anchor.offset)
            .map(|(offset, _)| offset);
        anchor.offset = containing
            .filter(|offset| layout.is_cursor_position(*offset))
            .or_else(|| {
                text.grapheme_indices(/*is_extended*/ true)
                    .map(|(offset, _)| offset)
                    .find(|offset| *offset >= anchor.offset && layout.is_cursor_position(*offset))
            })
            .or_else(|| {
                text.grapheme_indices(/*is_extended*/ true)
                    .map(|(offset, _)| offset)
                    .rfind(|offset| layout.is_cursor_position(*offset))
            })
            .unwrap_or(anchor.offset);

        let (Some(mut mode), Some(mut selection)) =
            (self.select_mode.take(), self.selection.take())
        else {
            return;
        };
        if let Some(origin) = mode.search_origin.take() {
            mode.mark = origin.mark;
            mode.preferred_column = origin.preferred_column;
            selection.set_keyboard_range(origin.start, origin.end);
        }
        mode.preferred_column = None;
        if let Some(mark) = &mut mode.mark {
            mark.active = anchor;
            mark.active_edges = None;
        } else {
            selection.set_keyboard_range(anchor, anchor);
        }
        self.select_mode = Some(mode);
        self.selection = Some(selection);
        self.sync_select_range(cells);
        self.reveal_select_cursor(cells);
    }
}

#[cfg(test)]
#[path = "select_mode_tests.rs"]
mod tests;
