//! Selection freezes entry order and displayed revisions while canonical history keeps advancing.
//! The snapshot shares cell ownership; only visible and selected text layouts are pinned.

use crossterm::event::KeyCode;
use ratatui::layout::Position as ScreenPosition;
use unicode_segmentation::UnicodeSegmentation;

use super::*;
use crate::text_selection::SelectionUnit;

pub(super) struct Selection {
    pub(super) snapshot: ViewSnapshot,
    pub(super) start: Anchor,
    pub(super) end: Anchor,
    pub(super) dragging: bool,
    pub(super) moved: bool,
    pub(super) resume_on_empty: bool,
    pub(super) pointer: Option<ScreenPosition>,
    pub(super) pressed_link: Option<String>,
    origin: (Anchor, Anchor),
    unit: SelectionUnit,
    preferred_column: Option<u16>,
}

impl TranscriptView {
    pub(super) fn begin_selection(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        column: u16,
        row: u16,
        clicks: u8,
    ) {
        let Some((anchor, layout)) = self.hit_test(column, row) else {
            return;
        };
        self.cancel_beginning();
        let unit = SelectionUnit::from_clicks(clicks);
        let range = unit.range(layout.text(), anchor.offset);
        let snapshot = self.capture_snapshot(cells);
        self.held_reading = None;
        let start = Anchor {
            offset: range.start,
            ..anchor
        };
        let end = Anchor {
            offset: range.end,
            ..anchor
        };
        let was_following = self.is_following();
        self.selection = Some(Selection {
            snapshot,
            start,
            end,
            origin: (start, end),
            unit,
            preferred_column: None,
            dragging: true,
            moved: false,
            resume_on_empty: was_following,
            pointer: Some(ScreenPosition::new(column, row)),
            pressed_link: None,
        });
        if was_following {
            self.hold_position();
        }
    }

    /// Rejoin current history, retaining Find offsets or a replaced group until navigation.
    /// Real selections keep their reading position, including a retired live revision.
    pub(crate) fn end_selection(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        let Some(selection) = self.selection.take() else {
            return;
        };
        let empty = (selection.start.key, selection.start.offset)
            == (selection.end.key, selection.end.offset);
        if empty && selection.resume_on_empty {
            self.position = Position::Latest;
            self.release_live_reading();
            return;
        }
        if self.position == Position::Latest {
            self.hold_position();
        }
        if let Position::Reading(anchor) = self.position {
            if self.search.has_active_query()
                || !cells.iter().any(|cell| EntryKey::cell(cell) == anchor.key)
            {
                self.held_reading = Some(selection.snapshot);
                return;
            }
            let index = self.resolve(cells, anchor);
            let key = self.entry_key(cells, index);
            let (offset, row_bias) = if key == anchor.key {
                (anchor.offset, anchor.row_bias)
            } else {
                (0, 0)
            };
            self.position = Position::Reading(Anchor {
                key,
                index,
                offset,
                row_bias,
            });
        }
    }

    pub(super) fn extend_selection(&mut self, column: u16, row: u16) {
        let Some((end, layout)) = self.hit_test(column, row) else {
            return;
        };
        let Some(mut selection) = self.selection.take() else {
            return;
        };
        let cells = Arc::clone(&selection.snapshot.cells);
        let range = selection.unit.range(layout.text(), end.offset);
        let origin = selection.origin;
        let backwards = (self.resolve(&cells, end), end.offset)
            < (self.resolve(&cells, origin.0), origin.0.offset);
        selection.start = if backwards { origin.1 } else { origin.0 };
        selection.end = Anchor {
            offset: if backwards { range.start } else { range.end },
            ..end
        };
        selection.snapshot.pinned.entry(end.key).or_insert(layout);
        selection.moved = true;
        selection.pointer = Some(ScreenPosition::new(column, row));
        selection.preferred_column = None;
        self.pin_selection_range(&cells, &mut selection);
        self.selection = Some(selection);
    }

    /// A pointer-down anchor alone does not select text or need to hide decoration.
    pub(crate) fn has_selection_range(&self) -> bool {
        self.selection.as_ref().is_some_and(|selection| {
            selection.start.key != selection.end.key
                || selection.start.offset != selection.end.offset
        })
    }

    pub(crate) fn selected_text(&mut self, cells: &[Arc<dyn HistoryCell>]) -> Option<String> {
        if !self.has_selection_range() {
            return None;
        }
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let selection = self.selection.as_ref()?;
        let mut start = selection.start;
        let mut end = selection.end;
        start.index = self.resolve(cells, start);
        end.index = self.resolve(cells, end);
        if (start.index, start.offset) > (end.index, end.offset) {
            std::mem::swap(&mut start, &mut end);
        }
        if start == end {
            return None;
        }
        let mut text = String::new();
        let mut previous: Option<Arc<TextLayout>> = None;
        for index in start.index..=end.index {
            let layout = self.layout(cells, index)?;
            if layout.row_count() == 0 {
                continue;
            }
            if let Some(previous) = &previous {
                text.push_str(previous.separator_after(&layout));
            }
            let begin = if index == start.index {
                start.offset
            } else {
                0
            };
            let finish = if index == end.index {
                end.offset
            } else {
                layout.text().len()
            };
            text.push_str(layout.text().get(begin..finish)?);
            previous = Some(layout);
        }
        text.retain(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'));
        Some(text)
    }

    /// Release selection after confirmed delivery. Failed or unacknowledged terminal writes
    /// retain the selected revision so the user can verify pasting and retry.
    pub(crate) fn copy_selected_text_with(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        text: &str,
        copy: impl FnOnce(&str) -> Result<crate::clipboard_copy::CopyStatus, String>,
    ) -> Result<crate::clipboard_copy::CopyStatus, String> {
        let result = copy(text);
        if matches!(result, Ok(crate::clipboard_copy::CopyStatus::Confirmed)) {
            self.end_selection(cells);
        }
        result
    }

    /// End the pointer gesture without discarding selected text when input ownership changes.
    pub(crate) fn end_drag(&mut self) {
        self.follow_control = Default::default();
        if !self.has_selection_range()
            && self
                .selection
                .as_ref()
                .is_some_and(|selection| selection.dragging && selection.resume_on_empty)
        {
            self.position = Position::Latest;
            self.selection = None;
            self.release_live_reading();
        }
        if let Some(selection) = &mut self.selection {
            selection.dragging = false;
        }
    }

    pub(super) fn selection_key(&mut self, cells: &[Arc<dyn HistoryCell>], code: KeyCode) -> bool {
        if !matches!(
            code,
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
        ) {
            return false;
        }
        let Some(end) = self.selection.as_ref().map(|selection| selection.end) else {
            return false;
        };
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let column = if matches!(code, KeyCode::Up | KeyCode::Down) {
            let Some(layout) = self.layout(cells, self.resolve(cells, end)) else {
                return true;
            };
            Some(
                self.selection
                    .as_ref()
                    .and_then(|selection| selection.preferred_column)
                    .unwrap_or_else(|| layout.column_for_offset(end.offset)),
            )
        } else {
            None
        };
        let Some(next) = self.move_selection_endpoint(cells, end, code, column) else {
            return true;
        };
        if let Some(mut selection) = self.selection.take() {
            selection.end = next;
            selection.dragging = false;
            selection.preferred_column = column;
            self.pin_selection_range(cells, &mut selection);
            self.selection = Some(selection);
        }
        let row = self
            .layout(cells, next.index)
            .map_or(/*default*/ 0, |layout| {
                layout
                    .row_for_offset(next.offset)
                    .saturating_add_signed(next.row_bias.saturating_neg())
            });
        if !self
            .visible
            .iter()
            .any(|visible| visible.index == next.index && visible.row == row)
        {
            self.position = Position::Reading(next);
        }
        true
    }

    pub(crate) fn tick_selection(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let Some(selection) = self
            .selection
            .as_ref()
            .filter(|selection| selection.dragging && selection.moved)
        else {
            return false;
        };
        let Some(pointer) = selection.pointer else {
            return false;
        };
        let direction = if pointer.y <= self.area.top() {
            -1
        } else if pointer.y >= self.area.bottom().saturating_sub(/*rhs*/ 1) {
            1
        } else {
            return false;
        };
        let previous = self.position;
        self.scroll(cells, direction);
        previous != self.position
    }

    pub(super) fn normalize_selection(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        if let Some(mut selection) = self.selection.take() {
            selection.start.index = self.resolve(cells, selection.start);
            selection.end.index = self.resolve(cells, selection.end);
            selection.origin.0.index = self.resolve(cells, selection.origin.0);
            selection.origin.1.index = self.resolve(cells, selection.origin.1);
            self.selection = Some(selection);
        }
    }

    pub(super) fn render_selection(&self, buf: &mut Buffer) {
        let Some(selection) = &self.selection else {
            return;
        };
        let (start, end) = if (selection.start.index, selection.start.offset)
            <= (selection.end.index, selection.end.offset)
        {
            (selection.start, selection.end)
        } else {
            (selection.end, selection.start)
        };
        for (y, visible) in self.visible.iter().enumerate() {
            if visible.index < start.index || visible.index > end.index {
                continue;
            }
            let begin = if visible.key == start.key {
                start.offset
            } else {
                0
            };
            let finish = if visible.key == end.key {
                end.offset
            } else {
                visible.layout.text().len()
            };
            let area = Rect::new(
                self.area.x,
                self.area.y + y as u16,
                self.area.width,
                /*height*/ 1,
            );
            visible
                .layout
                .highlight(begin..finish, area, buf, visible.row);
        }
    }

    pub(super) fn hit_test(&self, column: u16, row: u16) -> Option<(Anchor, Arc<TextLayout>)> {
        let row = row.saturating_sub(self.area.y) as usize;
        let visible = self
            .visible
            .get(row.min(self.visible.len().saturating_sub(/*rhs*/ 1)))?;
        Some((
            Anchor {
                key: visible.key,
                index: visible.index,
                offset: visible
                    .layout
                    .position_at(visible.row, column.saturating_sub(self.area.x)),
                row_bias: 0,
            },
            Arc::clone(&visible.layout),
        ))
    }

    pub(super) fn hold_position(&mut self) {
        if let Some(first) = self.visible.first() {
            self.position = Position::Reading(Anchor {
                key: first.key,
                index: first.index,
                offset: first.layout.position_at(first.row, /*column*/ 0),
                row_bias: first
                    .layout
                    .row_for_offset(first.layout.position_at(first.row, /*column*/ 0))
                    as isize
                    - first.row as isize,
            });
        }
    }

    fn pin_selection_range(&mut self, cells: &[Arc<dyn HistoryCell>], selection: &mut Selection) {
        selection.start.index = self.resolve(cells, selection.start);
        selection.end.index = self.resolve(cells, selection.end);
        for index in selection.start.index.min(selection.end.index)
            ..=selection.start.index.max(selection.end.index)
        {
            let key = self.entry_key(cells, index);
            if !selection.snapshot.pinned.contains_key(&key)
                && let Some(layout) = self.layout(cells, index)
            {
                selection.snapshot.pinned.insert(key, layout);
            }
        }
    }

    fn move_selection_endpoint(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        end: Anchor,
        code: KeyCode,
        preferred_column: Option<u16>,
    ) -> Option<Anchor> {
        let mut index = self.resolve(cells, end);
        let layout = self.layout(cells, index)?;
        let offset = layout
            .text()
            .floor_char_boundary(end.offset.min(layout.text().len()));
        let mut row_bias = 0;
        let offset = match code {
            KeyCode::Up | KeyCode::Down => {
                let direction = if code == KeyCode::Up { -1 } else { 1 };
                let row = layout
                    .row_for_offset(offset)
                    .saturating_add_signed(end.row_bias.saturating_neg());
                let (next, row) = self.move_rows(cells, index, row, direction);
                index = next;
                let next_layout = self.layout(cells, index)?;
                let offset = next_layout.position_at(
                    row,
                    preferred_column.unwrap_or_else(|| layout.column_for_offset(offset)),
                );
                row_bias = next_layout.row_for_offset(offset) as isize - row as isize;
                offset
            }
            KeyCode::Left if offset == 0 && index > 0 => {
                index -= 1;
                self.layout(cells, index)?.text().len()
            }
            KeyCode::Right if offset == layout.text().len() => {
                if self.layout(cells, index + 1).is_some() {
                    index += 1;
                    0
                } else {
                    offset
                }
            }
            KeyCode::Left => layout.text()[..offset]
                .grapheme_indices(/*is_extended*/ true)
                .next_back()
                .map_or(/*default*/ 0, |(offset, _)| offset),
            KeyCode::Right => {
                offset
                    + layout.text()[offset..]
                        .graphemes(/*is_extended*/ true)
                        .next()
                        .map_or(/*default*/ 0, str::len)
            }
            _ => return None,
        };
        Some(Anchor {
            key: self.entry_key(cells, index),
            index,
            offset,
            row_bias,
        })
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
