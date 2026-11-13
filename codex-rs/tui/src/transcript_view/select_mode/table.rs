//! Keyboard table marks move between logical cells and use the shared table selection ranges.

use super::*;
use crate::transcript_view::text::TableCellHit;
use itertools::Itertools;
use std::ops::Range;

#[derive(Clone, Copy)]
pub(in crate::transcript_view) struct TableMark {
    pub(super) anchor: CellPosition,
    pub(super) active: CellPosition,
    pub(in crate::transcript_view) kind: TableMarkKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::transcript_view) enum TableMarkKind {
    Cells,
    Text,
}

#[derive(Clone, Copy)]
pub(in crate::transcript_view) struct CellPosition {
    cell: (usize, usize),
    // Trailing empty grid cells share the row's end offset.
    pub(super) padding_column: Option<u16>,
}

fn table_hit(layout: &TextLayout, cursor: Anchor) -> Option<TableCellHit> {
    let row = layout
        .row_for_offset(cursor.offset)
        .saturating_add_signed(cursor.row_bias.saturating_neg());
    let column = layout.column_for_offset_on_row(row, cursor.offset);
    layout.table_cell_at(row, column, cursor.offset)
}

impl TranscriptView {
    pub(in crate::transcript_view) fn table_mark(&self) -> Option<TableMark> {
        self.select_mode?.mark?.table
    }

    pub(in crate::transcript_view) fn select_cursor_is_on_table(&self) -> bool {
        let (Some(mode), Some(selection)) = (self.select_mode, self.selection.as_ref()) else {
            return false;
        };
        let cursor = mode.cursor(selection);
        mode.table_caret.is_some()
            || self
                .visible
                .iter()
                .find(|visible| visible.key == cursor.key)
                .is_some_and(|visible| table_hit(&visible.layout, cursor).is_some())
    }

    pub(super) fn start_select_table(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        if self
            .table_mark()
            .is_some_and(|table| table.kind == TableMarkKind::Cells)
        {
            self.clear_select_mark();
            return true;
        }
        let (Some(mut mode), Some(selection)) = (self.select_mode, self.selection.as_ref()) else {
            return false;
        };
        let cursor = mode.cursor(selection);
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let index = self.resolve(cells, cursor);
        let Some(layout) = self.layout(cells, index) else {
            return false;
        };
        let cell = mode
            .mark
            .and_then(|mark| mark.table.map(|table| table.active))
            .or(mode.table_caret)
            .or_else(|| {
                table_hit(&layout, cursor).map(|hit| CellPosition {
                    cell: hit.endpoint.cell,
                    padding_column: None,
                })
            });
        let Some(cell) = cell else { return false };
        mode.table_caret = None;
        self.cancel_pending_select_search();
        self.cancel_beginning();
        mode.mark = Some(Mark {
            anchor: cursor,
            active: cursor,
            anchor_edges: None,
            active_edges: None,
            unit: SelectionUnit::Character,
            table: Some(TableMark {
                anchor: cell,
                active: cell,
                kind: TableMarkKind::Cells,
            }),
        });
        mode.preferred_column = None;
        self.select_mode = Some(mode);
        self.sync_select_range(cells);
        true
    }

    pub(super) fn start_select_cell_text(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        let Some(mut mode) = self.select_mode else {
            return;
        };
        let Some((mut mark, table)) = mode.mark.and_then(|mark| Some((mark, mark.table?))) else {
            return;
        };
        if table.kind == TableMarkKind::Text {
            self.clear_select_mark();
            return;
        }
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let Some((_, ranges)) = self.cell_text_ranges(cells, mark.active, table.active) else {
            return;
        };
        let Some(offset) = ranges
            .iter()
            .find(|range| range.contains(&mark.active.offset))
            .map(|_| mark.active.offset)
            .or_else(|| {
                ranges
                    .iter()
                    .find(|range| !range.is_empty())
                    .map(|range| range.start)
            })
        else {
            return;
        };
        let cursor = Anchor {
            offset,
            row_bias: 0,
            ..mark.active
        };
        let cell = CellPosition {
            padding_column: None,
            ..table.active
        };
        mark.anchor = cursor;
        mark.active = cursor;
        mark.table = Some(TableMark {
            anchor: cell,
            active: cell,
            kind: TableMarkKind::Text,
        });
        mode.mark = Some(mark);
        mode.preferred_column = None;
        self.cancel_pending_select_search();
        self.select_mode = Some(mode);
        self.sync_select_range(cells);
        self.reveal_select_cursor(cells);
    }

    pub(super) fn move_cell_text_cursor(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        mark: Mark,
        cell: CellPosition,
        motion: CursorMotion,
        preferred_column: &mut Option<u16>,
    ) -> Option<Anchor> {
        let (layout, ranges) = self.cell_text_ranges(cells, mark.anchor, cell)?;
        let cursor = mark.active;
        let offset = if let CursorMotion::Rows(rows) = motion {
            let row = layout.row_for_offset(cursor.offset);
            let column = *preferred_column
                .get_or_insert_with(|| layout.column_for_offset_on_row(row, cursor.offset));
            let first = layout.row_for_offset(ranges.first()?.start);
            let last = layout.row_for_offset(ranges.last()?.end.saturating_sub(1));
            let mut current = row;
            let mut remaining = rows.unsigned_abs();
            let mut offset = cursor.offset;
            while let Some(row) = current.checked_add_signed(rows.signum())
                && (first..=last).contains(&row)
                && remaining > 0
            {
                current = row;
                if let Some(fragment) = layout.table_cell_on_row(&ranges, row)
                    && !fragment.is_empty()
                {
                    let last = layout.text()[fragment.clone()]
                        .grapheme_indices(/*is_extended*/ true)
                        .next_back()
                        .map_or(fragment.start, |(offset, _)| fragment.start + offset);
                    offset = layout.position_at(row, column).clamp(fragment.start, last);
                    remaining -= 1;
                }
            }
            offset
        } else {
            *preferred_column = None;
            let fragments = ranges
                .iter()
                .map(|range| (range.start, &layout.text()[range.clone()]));
            let mut offsets = fragments.clone().flat_map(|(start, text)| {
                text.grapheme_indices(/*is_extended*/ true)
                    .map(move |(offset, _)| start + offset)
                    .filter(|&offset| layout.is_cursor_position(offset))
            });
            match motion {
                CursorMotion::Character(KeyCode::Left) => {
                    offsets.rfind(|&offset| offset < cursor.offset)
                }
                CursorMotion::Character(KeyCode::Right) => {
                    offsets.find(|&offset| offset > cursor.offset)
                }
                CursorMotion::LineStart => offsets.next(),
                CursorMotion::LineEnd => offsets.next_back(),
                CursorMotion::WordForward | CursorMotion::WordBackward | CursorMotion::WordEnd => {
                    // Scan each original line once. Escaped pipes can leave gaps
                    // between its displayed fragments.
                    let lines = layout
                        .table_word_fragments(&ranges)
                        .chunk_by(|(text, _, _)| std::ptr::from_ref(*text));
                    let mut offsets = (&lines).into_iter().flat_map(|(_, fragments)| {
                        let mut fragments = fragments.peekable();
                        let text = fragments.peek().map_or("", |&(text, _, _)| text);
                        super::motion::selectable_words(text).filter_map(move |(offset, word)| {
                            let boundary = if matches!(motion, CursorMotion::WordEnd) {
                                offset + word.grapheme_indices(/*is_extended*/ true).next_back()?.0
                            } else {
                                offset
                            };
                            while fragments
                                .peek()
                                .is_some_and(|(_, range, _)| range.end <= boundary)
                            {
                                fragments.next();
                            }
                            let (_, range, start) = fragments.peek()?;
                            range
                                .contains(&boundary)
                                .then(|| start + (boundary - range.start))
                        })
                    });
                    if matches!(motion, CursorMotion::WordBackward) {
                        offsets.take_while(|&offset| offset < cursor.offset).last()
                    } else {
                        offsets.find(|&offset| offset > cursor.offset)
                    }
                }
                CursorMotion::Rows(_)
                | CursorMotion::Character(_)
                | CursorMotion::Beginning
                | CursorMotion::End => None,
            }?
        };
        Some(Anchor {
            offset,
            row_bias: 0,
            ..cursor
        })
    }

    fn cell_text_ranges(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        anchor: Anchor,
        cell: CellPosition,
    ) -> Option<(Arc<TextLayout>, Vec<Range<usize>>)> {
        let index = self.resolve(cells, anchor);
        let layout = self.layout(cells, index)?;
        let hit = layout.table_cell_at_offset(anchor.offset, cell.cell)?;
        let ranges = layout.table_selection_ranges(&hit, &hit);
        Some((layout, ranges))
    }

    pub(super) fn move_select_table(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        mut mode: SelectMode,
        mut mark: Mark,
        motion: CursorMotion,
    ) {
        let Some(table) = &mut mark.table else { return };
        let (row, column) = table.active.cell;
        let cell = match motion {
            CursorMotion::Character(KeyCode::Left) => column.checked_sub(1).map(|col| (row, col)),
            CursorMotion::Character(KeyCode::Right) => Some((row, column + 1)),
            CursorMotion::Rows(-1) => row.checked_sub(1).map(|row| (row, column)),
            CursorMotion::Rows(1) => Some((row + 1, column)),
            _ => None,
        };
        let Some(cell) = cell else { return };
        let index = self.resolve(cells, mark.anchor);
        let Some(layout) = self.layout(cells, index) else {
            return;
        };
        let Some(anchor) = layout.table_cell_at_offset(mark.active.offset, table.active.cell)
        else {
            return;
        };
        if cell == table.anchor.cell {
            mark.active = mark.anchor;
            table.active = table.anchor;
        } else {
            let Some((offset, padding_column)) = layout.table_cell_position(&anchor, cell) else {
                return;
            };
            mark.active = Anchor {
                offset,
                row_bias: 0,
                ..mark.anchor
            };
            table.active = CellPosition {
                cell,
                padding_column,
            };
        }
        mode.mark = Some(mark);
        self.select_mode = Some(mode);
        self.sync_select_range(cells);
        self.reveal_select_cursor(cells);
    }

    pub(super) fn sync_select_table(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        mark: Mark,
        table: TableMark,
    ) {
        let index = self.resolve(cells, mark.anchor);
        let Some(layout) = self.layout(cells, index) else {
            return;
        };
        let (Some(anchor), Some(active)) = (
            layout.table_cell_at_offset(mark.anchor.offset, table.anchor.cell),
            layout.table_cell_at_offset(mark.active.offset, table.active.cell),
        ) else {
            return;
        };
        if let Some(selection) = &mut self.selection {
            selection.set_keyboard_table_range(
                mark.anchor,
                &layout,
                anchor,
                active,
                (table.kind == TableMarkKind::Text).then_some(mark.active),
            );
            selection
                .snapshot
                .pinned
                .entry(mark.anchor.key)
                .or_insert(layout);
        }
    }
}
