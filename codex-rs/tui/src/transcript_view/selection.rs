//! Selection freezes entry order and displayed revisions while canonical history keeps advancing.
//! The snapshot shares cell ownership; only visible and selected text layouts are pinned.

use crossterm::event::KeyCode;
use ratatui::layout::Position as ScreenPosition;
use std::ops::Range;
use std::ops::RangeInclusive;
use unicode_segmentation::UnicodeSegmentation;

use super::text::TableCellHit;
use super::text::TableEndpoint;
use super::*;
use crate::clipboard_copy::CopyFormat;
use crate::clipboard_copy::worker::CopyResult;
use crate::text_selection::SelectionUnit;
use crate::tui::Tui;

struct PendingCopy {
    id: u64,
    start: Anchor,
    end: Anchor,
    position: Position,
    detailed: bool,
    follow: bool,
    clear_selection: bool,
    characters: usize,
}

struct TableSelection {
    anchor: TableCellHit,
    // A head marks a whole-cell drag. Returning to its anchor selects the whole cell again.
    head: Option<TableEndpoint>,
    ranges: Vec<Range<usize>>,
}

impl TableSelection {
    fn rectangle_rows(&self) -> Option<RangeInclusive<usize>> {
        let anchor = self.anchor.endpoint;
        let head = self.head?;
        let (first, last) = (anchor.display_row(), head.display_row());
        (head != anchor).then(|| first.min(last)..=first.max(last))
    }

    fn bounds(&self) -> Range<usize> {
        self.ranges[0].start..self.ranges[self.ranges.len() - 1].end
    }

    fn drag(
        &mut self,
        layout: &TextLayout,
        row: usize,
        column: u16,
        pointer: Anchor,
        origin: (Anchor, Anchor),
    ) -> Option<(Anchor, Anchor)> {
        if pointer.key != origin.0.key || !layout.is_table_row(row, pointer.offset, &self.anchor) {
            return None;
        }
        if let Some(hit) = layout.table_cell_at(row, column, pointer.offset)
            && self.anchor.same_table(&hit)
        {
            let hit = if hit.endpoint == self.anchor.endpoint {
                &self.anchor
            } else {
                &hit
            };
            let endpoint = hit.endpoint;
            if self.head.unwrap_or(self.anchor.endpoint) != endpoint {
                self.ranges = layout.table_selection_ranges(&self.anchor, hit);
                self.head = Some(endpoint);
            }
        }
        let at = |offset| Anchor { offset, ..origin.0 };
        let bounds = self.bounds();
        if let Some(head) = self.head {
            let start = at(bounds.start);
            let end = at(bounds.end);
            return Some(
                if (head.display_row(), head.cell.1)
                    < (
                        self.anchor.endpoint.display_row(),
                        self.anchor.endpoint.cell.1,
                    )
                {
                    (end, start)
                } else {
                    (start, end)
                },
            );
        }
        let range = layout.table_cell_pointer_range(&self.ranges, row, pointer.offset);
        Some(if range.start < origin.0.offset {
            (origin.1, at(range.start))
        } else {
            (origin.0, at(range.end))
        })
    }
}

pub(super) struct Selection {
    pending_copy: Option<PendingCopy>,
    primary_owner: Option<Arc<()>>,
    pub(super) snapshot: ViewSnapshot,
    pub(super) start: Anchor,
    pub(super) end: Anchor,
    pub(super) dragging: bool,
    pub(super) moved: bool,
    moved_vertically: bool,
    pointer_origin_row: u16,
    pub(super) resume_on_empty: bool,
    pub(super) pointer: Option<ScreenPosition>,
    pub(super) pressed_link: Option<String>,
    origin: (Anchor, Anchor),
    table_cell: Option<TableSelection>,
    unit: SelectionUnit,
    preferred_column: Option<u16>,
}

impl Selection {
    pub(super) fn keyboard(snapshot: ViewSnapshot, cursor: Anchor) -> Self {
        Self {
            pending_copy: None,
            primary_owner: None,
            snapshot,
            start: cursor,
            end: cursor,
            dragging: false,
            moved: true,
            moved_vertically: false,
            pointer_origin_row: 0,
            resume_on_empty: false,
            pointer: None,
            pressed_link: None,
            origin: (cursor, cursor),
            table_cell: None,
            unit: SelectionUnit::Character,
            preferred_column: None,
        }
    }

    fn clear_table_cell_scope(&mut self) {
        if self.table_cell.take().is_some() {
            if self.unit == SelectionUnit::Character {
                self.origin.1 = self.origin.0;
            }
            self.pending_copy = None;
            self.primary_owner = None;
        }
    }

    pub(super) fn set_keyboard_range(&mut self, start: Anchor, end: Anchor) {
        self.clear_table_cell_scope();
        if (self.start, self.end) != (start, end) {
            self.primary_owner = None;
            self.pending_copy = None;
        }
        self.start = start;
        self.end = end;
        self.origin = (start, end);
        self.preferred_column = None;
    }

    pub(super) fn set_keyboard_marked_range(
        &mut self,
        start: Anchor,
        end: Anchor,
        origin: (Anchor, Anchor),
        unit: SelectionUnit,
    ) {
        self.set_keyboard_range(start, end);
        self.origin = origin;
        self.unit = unit;
    }

    pub(super) fn unit(&self) -> SelectionUnit {
        self.unit
    }
}

impl TranscriptView {
    pub(super) fn select_copy_range(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        index: usize,
        range: std::ops::Range<usize>,
    ) {
        let Some(layout) = self.layout(cells, index) else {
            return;
        };
        let key = self.entry_key(cells, index);
        let start = Anchor {
            key,
            index,
            offset: range.start,
            row_bias: 0,
        };
        let end = Anchor {
            offset: range.end,
            ..start
        };
        let mut snapshot = self.capture_snapshot(cells);
        snapshot.pinned.retain(|key, _| *key == EntryKey::Live);
        snapshot.pinned.insert(key, layout);
        self.selection = Some(Selection {
            pending_copy: None,
            primary_owner: None,
            snapshot,
            start,
            end,
            dragging: false,
            moved: false,
            moved_vertically: false,
            pointer_origin_row: 0,
            resume_on_empty: false,
            pointer: None,
            pressed_link: None,
            origin: (start, end),
            table_cell: None,
            unit: SelectionUnit::Character,
            preferred_column: None,
        });
        self.reveal_copy_context(cells);
    }

    pub(super) fn begin_selection(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        column: u16,
        row: u16,
        clicks: u8,
    ) {
        self.copy_mode = None;
        let Some((anchor, layout, layout_row)) = self.hit_test(column, row) else {
            return;
        };
        self.cancel_beginning();
        let unit = SelectionUnit::from_clicks(clicks);
        let table_cell = if self.select_mode.is_none()
            && matches!(clicks, 1 | 3)
            && let Some(cell) = layout.table_cell_at(
                layout_row,
                column.saturating_sub(self.area.x),
                anchor.offset,
            ) {
            let ranges = layout.table_selection_ranges(&cell, &cell);
            Some(TableSelection {
                head: (clicks == 3).then_some(cell.endpoint),
                anchor: cell,
                ranges,
            })
        } else {
            None
        };
        let range = match &table_cell {
            Some(table) if table.head.is_some() => table.bounds(),
            Some(table) => {
                layout.table_cell_pointer_range(&table.ranges, layout_row, anchor.offset)
            }
            None => unit.range(layout.text(), anchor.offset),
        };
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
            pending_copy: None,
            primary_owner: None,
            snapshot,
            start,
            end: if clicks == 1 { start } else { end },
            origin: (start, end),
            table_cell,
            unit,
            preferred_column: None,
            dragging: true,
            moved: false,
            moved_vertically: false,
            pointer_origin_row: row,
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
        self.copy_mode = None;
        let Some(selection) = self.selection.take() else {
            return;
        };
        if let Some(mode) = &mut self.select_mode {
            let cursor = mode.cursor(&selection);
            mode.mark = None;
            let mut selection = selection;
            selection.set_keyboard_range(cursor, cursor);
            self.selection = Some(selection);
            return;
        }
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
        let Some((end, layout, layout_row)) = self.hit_test(column, row) else {
            return;
        };
        let Some(mut selection) = self.selection.take() else {
            return;
        };
        self.copy_mode = None;
        // Shift-click extends the retained anchor across cells or entries.
        if !selection.dragging {
            selection.clear_table_cell_scope();
            selection.pointer_origin_row = row;
            selection.moved_vertically = false;
        }
        let cells = Arc::clone(&selection.snapshot.cells);
        let previous = (selection.start, selection.end);
        let table_head = |selection: &Selection| selection.table_cell.as_ref()?.head;
        let previous_head = table_head(&selection);
        let table_range = selection.table_cell.as_mut().and_then(|table| {
            table.drag(
                &layout,
                layout_row,
                column.saturating_sub(self.area.x),
                end,
                selection.origin,
            )
        });
        (selection.start, selection.end) = table_range.unwrap_or_else(|| {
            if selection.table_cell.is_some() && selection.unit == SelectionUnit::Line {
                return previous;
            }
            selection.clear_table_cell_scope();
            let origin = selection.origin;
            let range = selection.unit.range(layout.text(), end.offset);
            selection.snapshot.pinned.entry(end.key).or_insert(layout);
            let backwards = (self.resolve(&cells, end), end.offset)
                < (self.resolve(&cells, origin.0), origin.0.offset);
            (
                if backwards { origin.1 } else { origin.0 },
                Anchor {
                    offset: if backwards { range.start } else { range.end },
                    ..end
                },
            )
        });
        selection.moved = true;
        selection.moved_vertically |= selection.pointer_origin_row != row;
        selection.pointer = Some(ScreenPosition::new(column, row));
        selection.preferred_column = None;
        self.pin_selection_range(&cells, &mut selection);
        if previous != (selection.start, selection.end) || previous_head != table_head(&selection) {
            selection.primary_owner = None;
            selection.pending_copy = None;
        }
        self.selection = Some(selection);
    }

    /// A pointer-down anchor alone does not select text or need to hide decoration.
    pub(crate) fn has_selection_range(&self) -> bool {
        self.selection.as_ref().is_some_and(|selection| {
            selection.start.key != selection.end.key
                || selection.start.offset != selection.end.offset
                || selection
                    .table_cell
                    .as_ref()
                    .and_then(TableSelection::rectangle_rows)
                    .is_some()
        })
    }

    fn selected_ranges(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<Vec<(Arc<TextLayout>, std::ops::Range<usize>)>> {
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
        let mut ranges = Vec::new();
        for index in start.index..=end.index {
            let layout = self.layout(cells, index)?;
            if layout.row_count() == 0 {
                continue;
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
            ranges.push((layout, begin..finish));
        }
        Some(ranges)
    }

    fn table_selection(&self) -> Option<&TableSelection> {
        self.selection.as_ref()?.table_cell.as_ref()
    }

    pub(crate) fn selected_text(&mut self, cells: &[Arc<dyn HistoryCell>]) -> Option<String> {
        if let Some(table) = self.table_selection() {
            let rectangle_rows = table.rectangle_rows();
            let lines = self.selected_copy_lines(cells);
            let text = crate::markdown_copy::literal_selection(&lines, "", rectangle_rows.as_ref());
            return (!text.is_empty()).then_some(text);
        }
        let mut text = String::new();
        let mut previous: Option<Arc<TextLayout>> = None;
        for (layout, range) in self.selected_ranges(cells)? {
            if let Some(previous) = &previous {
                text.push_str(previous.separator_after(&layout));
            }
            text.push_str(layout.text().get(range)?);
            previous = Some(layout);
        }
        text.retain(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'));
        Some(text)
    }

    /// A PRIMARY write is silent: it neither claims CLIPBOARD was copied nor ends selection.
    pub(crate) fn publish_primary(&mut self, tui: &mut Tui, text: &str) {
        let (_, owner) = tui.copy_selection(
            text,
            CopyFormat::PlainText,
            SelectionCopy::on_select(
                /*clipboard*/ false, /*primary*/ true, /*clear_selection*/ false,
            ),
            text,
        );
        if let Some(selection) = &mut self.selection {
            selection.primary_owner = owner;
        }
    }

    /// Hide or cancel deferred publication without relinquishing the existing X11 selection.
    pub(crate) fn cancel_primary(&mut self) {
        if let Some(selection) = &mut self.selection {
            selection.primary_owner = None;
        }
    }

    fn selected_copy_lines(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Vec<crate::markdown_copy::SelectedLine> {
        let ranges = self.selected_ranges(cells).unwrap_or_default();
        let cell_ranges = self
            .table_selection()
            .map_or(&[][..], |table| &table.ranges);
        let mut lines = Vec::new();
        let mut previous: Option<Arc<TextLayout>> = None;
        for (layout, range) in ranges {
            let separator = previous
                .as_ref()
                .map_or("", |previous| previous.separator_after(&layout));
            layout.copy_lines(range, separator, cell_ranges, &mut lines);
            previous = Some(layout);
        }
        lines
    }

    /// Deselect only after confirmed delivery when the current copy policy requests it.
    /// Failures and unacknowledged terminal writes retain the selection.
    pub(crate) fn copy_selected_text_with(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        text: &str,
        clear_selection: bool,
        copy: impl FnOnce(
            &str,
            crate::clipboard_copy::CopyFormat,
        ) -> Result<crate::clipboard_copy::CopyStatus, String>,
    ) -> Result<crate::clipboard_copy::CopyStatus, String> {
        let lines = self.selected_copy_lines(cells);
        let rectangle_rows = self
            .table_selection()
            .and_then(TableSelection::rectangle_rows);
        let override_payload = self.copy_mode.as_ref().and_then(|mode| {
            if mode.whole {
                mode.response.as_ref().map(|(_, text)| {
                    (
                        text.to_string(),
                        crate::clipboard_copy::CopyFormat::Markdown,
                    )
                })
            } else {
                mode.source
                    .as_ref()
                    .map(|text| (text.clone(), crate::clipboard_copy::CopyFormat::PlainText))
            }
        });
        let (text, format) = override_payload.unwrap_or_else(|| {
            let (serialized, format) =
                crate::markdown_copy::selection(&lines, text, rectangle_rows.as_ref());
            if format == CopyFormat::Markdown {
                (
                    crate::markdown_copy::literal_selection(&lines, text, rectangle_rows.as_ref()),
                    CopyFormat::MarkdownSelection(serialized.into()),
                )
            } else {
                (serialized, format)
            }
        });
        let characters = text.chars().count();
        let result = copy(&text, format);
        self.show_copy_feedback(&result, characters);
        match result {
            Ok(crate::clipboard_copy::CopyStatus::Confirmed) => {
                if clear_selection {
                    let copy_mode = self.copy_mode.is_some();
                    self.end_selection(cells);
                    if copy_mode {
                        self.jump_to_latest();
                    }
                }
            }
            Ok(crate::clipboard_copy::CopyStatus::Pending(id)) => {
                if let Some(selection) = &mut self.selection {
                    selection.pending_copy = Some(PendingCopy {
                        id,
                        start: selection.start,
                        end: selection.end,
                        position: self.position,
                        detailed: self.detailed,
                        follow: clear_selection && self.copy_mode.is_some(),
                        clear_selection,
                        characters,
                    });
                }
            }
            Ok(
                crate::clipboard_copy::CopyStatus::Unconfirmed
                | crate::clipboard_copy::CopyStatus::Busy,
            )
            | Err(_) => {}
        }
        result
    }

    /// Apply selection copy keys consistently in owned, overlay and rendered-region views.
    /// Plain UI regions keep their visible text; transcript copies retain Markdown metadata.
    /// PRIMARY is always plaintext, and automatic copies update both X11 selections.
    pub(crate) fn copy_action(
        &mut self,
        tui: &mut Tui,
        cells: &[Arc<dyn HistoryCell>],
        action: &ViewAction,
        format: crate::clipboard_copy::CopyFormat,
    ) -> Option<(usize, CopyResult)> {
        use crate::clipboard_copy::CopyFormat;
        use crate::clipboard_copy::CopyStatus;

        let text = match action {
            ViewAction::Copy(text)
            | ViewAction::CopyOnSelect(text)
            | ViewAction::PrimarySelection(text)
            | ViewAction::CopyAndFollow(text) => text.as_str(),
            ViewAction::Changed | ViewAction::ExitSelectMode | ViewAction::OpenLink(_) => {
                return None;
            }
        };
        let selection_copy = self.selection_copy(action);
        if !selection_copy.copies_clipboard() {
            if selection_copy.publishes_primary() {
                self.publish_primary(tui, text);
            }
            return None;
        }
        let clear_selection = selection_copy.clear_selection();
        let mut primary_owner = None;
        let result =
            self.copy_selected_text_with(cells, text, clear_selection, |rich, rich_format| {
                let (copy_text, format) = match format {
                    CopyFormat::PlainText => (text, CopyFormat::PlainText),
                    CopyFormat::Markdown => (rich, rich_format),
                    CopyFormat::MarkdownSelection(source) => {
                        (text, CopyFormat::MarkdownSelection(source))
                    }
                };
                let (result, owner) = tui.copy_selection(copy_text, format, selection_copy, text);
                primary_owner = owner;
                result
            });
        if selection_copy.publishes_primary()
            && let Some(selection) = &mut self.selection
        {
            selection.primary_owner = primary_owner;
        }
        if matches!(action, ViewAction::CopyAndFollow(_))
            && matches!(result, Ok(CopyStatus::Pending(_)))
        {
            self.follow_pending_copy();
        }
        Some((text.chars().count(), result))
    }

    pub(crate) fn follow_pending_copy(&mut self) {
        if let Some(pending) = self
            .selection
            .as_mut()
            .and_then(|s| s.pending_copy.as_mut())
        {
            pending.follow = true;
        }
    }

    /// A replaced selection has no ticket; moved endpoints cannot consume an old completion.
    pub(crate) fn finish_copy(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        completion: &(u64, crate::clipboard_copy::worker::CopyResult),
        current: bool,
    ) -> Option<bool> {
        // Feedback also belongs to composer copies and survives selection changes. Complete
        // its matching ticket even when the selection can no longer consume the result.
        if let Some(feedback) = &self.copy_feedback
            && let Ok(crate::clipboard_copy::CopyStatus::Pending(id)) = feedback.result
        {
            if id == completion.0 {
                self.show_copy_feedback(&completion.1, feedback.characters);
            } else if id < completion.0 {
                // A picker can consume this result and start another copy while the view
                // is hidden. A newer completion proves the older request is no longer pending.
                self.copy_feedback = None;
            }
        }
        let selection = self.selection.as_mut()?;
        if selection.pending_copy.as_ref()?.id != completion.0 {
            return None;
        }
        let pending = selection.pending_copy.take()?;
        if !current
            || (pending.start, pending.end) != (selection.start, selection.end)
            || pending.position != self.position
            || pending.detailed != self.detailed
        {
            return None;
        }
        let characters = pending.characters;
        if pending.clear_selection
            && completion.1 == Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        {
            self.end_selection(cells);
        }
        self.show_copy_feedback(&completion.1, characters);
        Some(pending.follow && completion.1 == Ok(crate::clipboard_copy::CopyStatus::Confirmed))
    }

    /// A stationary click begun at Latest still belongs to the fresh screen until it selects text.
    pub(crate) fn has_pending_latest_selection(&self) -> bool {
        !self.has_selection_range()
            && self
                .selection
                .as_ref()
                .is_some_and(|selection| selection.dragging && selection.resume_on_empty)
    }

    /// End the pointer gesture without discarding selected text when input ownership changes.
    pub(crate) fn end_drag(&mut self) {
        self.follow_control = Default::default();
        if self.has_pending_latest_selection() {
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
            selection.clear_table_cell_scope();
            if selection.end != next {
                selection.primary_owner = None;
            }
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
            // Horizontal selection on an edge row must not move the text under the pointer.
            .filter(|selection| selection.dragging && selection.moved_vertically)
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
            if self.copy_mode.is_some() {
                // Full-entry selections include final padding; partial ranges stay half-open.
                let last = if begin == 0 && finish == visible.layout.text().len() {
                    finish
                } else {
                    finish.saturating_sub(/*rhs*/ 1)
                };
                let rows =
                    visible.layout.row_for_offset(begin)..=visible.layout.row_for_offset(last);
                if rows.contains(&visible.row) {
                    buf.set_style(area, crate::style::selection_style());
                }
                continue;
            }
            if let Some(table) = &selection.table_cell {
                if let Some(fragment) = visible.layout.table_cell_on_row(&table.ranges, visible.row)
                {
                    let start = fragment.start.max(begin);
                    let end = fragment.end.min(finish);
                    if start < end {
                        visible.layout.highlight(start..end, area, buf, visible.row);
                    }
                }
                continue;
            }
            let next = (visible.index + 1..=end.index)
                .filter_map(|index| {
                    selection
                        .snapshot
                        .pinned
                        .get(&self.entry_key(&selection.snapshot.cells, index))
                })
                .find(|layout| layout.row_count() > 0);
            visible.layout.highlight_selection(
                begin..finish,
                next.map(AsRef::as_ref),
                area,
                buf,
                visible.row,
            );
        }
    }

    fn hit_test(&self, column: u16, row: u16) -> Option<(Anchor, Arc<TextLayout>, usize)> {
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
            visible.row,
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

    pub(super) fn pin_selection_range(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        selection: &mut Selection,
    ) {
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

    pub(super) fn move_selection_endpoint(
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
