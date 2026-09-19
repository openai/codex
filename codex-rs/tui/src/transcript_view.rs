//! A conversation viewport over the app's retained cells and its current live tail.
//!
//! The app owns history and pagination. This view owns reading state and bounded
//! layout caches. Reading positions track entry identity and a display-row offset.
//! Resize and replacement clamp that offset; live content always uses its current revision.

mod layout;
mod text;

use std::sync::Arc;

use crate::chatwidget::ActiveCellTranscriptKey;
use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::TranscriptHistoryState;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

use layout::LayoutCache;
use text::TextLayout;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum EntryKey {
    Cell(usize),
    Live,
}

impl EntryKey {
    fn cell(cell: &Arc<dyn HistoryCell>) -> Self {
        Self::Cell(Arc::as_ptr(cell).cast::<()>() as usize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Anchor {
    key: EntryKey,
    index: usize,
    row: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Position {
    #[default]
    Latest,
    Reading(Anchor),
}

struct VisibleRow {
    index: usize,
    row: usize,
    layout: Arc<TextLayout>,
    key: EntryKey,
}

/// Entry-based scrolling for the detailed transcript overlay.
#[derive(Default)]
pub(crate) struct TranscriptView {
    position: Position,
    cache: LayoutCache,
    live: Option<Arc<TextLayout>>,
    live_separated: Option<Arc<TextLayout>>,
    live_key: Option<(u16, ActiveCellTranscriptKey)>,
    live_continuation: bool,
    area: Rect,
    visible: Vec<VisibleRow>,
    pub(crate) history: TranscriptHistoryState,
}

impl TranscriptView {
    pub(crate) fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cells: &[Arc<dyn HistoryCell>],
        highlight: Option<usize>,
    ) -> u16 {
        self.cache.begin_frame();
        Clear.render(area, buf);
        self.area = area;
        self.visible.clear();
        if area.is_empty() {
            return 0;
        }
        let mut rendered = 0;
        let (mut index, mut row) = self.start(cells);
        for y in area.top()..area.bottom() {
            let Some(layout) = self.layout(cells, index) else {
                break;
            };
            if row >= layout.row_count() {
                index += 1;
                row = 0;
                let Some(next) = self.next_nonempty(cells, index) else {
                    break;
                };
                index = next;
            }
            let Some(layout) = self.layout(cells, index) else {
                break;
            };
            let key = self.entry_key(cells, index);
            let row_area = Rect::new(area.x, y, area.width, /*height*/ 1);
            if (index == 0 || row > 0)
                && cells
                    .get(index)
                    .is_some_and(|cell| cell.as_any().is::<UserHistoryCell>())
            {
                buf.set_style(row_area, crate::style::history_prompt_style());
            }
            self.visible.push(VisibleRow {
                index,
                row,
                layout,
                key,
            });
            row += 1;
            rendered += 1;
        }
        let mut y = area.y;
        for rows in self
            .visible
            .chunk_by(|a, b| Arc::ptr_eq(&a.layout, &b.layout))
        {
            let first = &rows[0];
            let height = rows.len() as u16;
            first
                .layout
                .render(Rect::new(area.x, y, area.width, height), buf, first.row);
            if highlight == Some(first.index) {
                for (offset, row) in rows.iter().enumerate() {
                    row.layout.highlight(
                        Rect::new(area.x, y + offset as u16, area.width, /*height*/ 1),
                        buf,
                        row.row,
                    );
                }
            }
            y += height;
        }
        rendered
    }

    pub(crate) fn sync_live_tail(
        &mut self,
        width: u16,
        key: Option<ActiveCellTranscriptKey>,
        lines: impl FnOnce(u16) -> Option<Vec<HyperlinkLine>>,
    ) {
        let next = key.map(|key| (width, key));
        if key.is_some_and(|key| key.cacheable) && self.live_key == next {
            return;
        }
        self.live_key = next;
        self.live_separated = None;
        self.live_continuation = key.is_some_and(|key| key.is_stream_continuation);
        self.live = lines(width).map(|lines| Arc::new(TextLayout::new(lines, width)));
    }

    pub(crate) fn is_following(&self) -> bool {
        self.position == Position::Latest
    }

    pub(crate) fn live_tail_visible(&self) -> bool {
        self.visible.iter().any(|row| row.key == EntryKey::Live)
    }

    /// Hold the current reading position until every older page has arrived.
    pub(crate) fn jump_to_beginning(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        if self.history == TranscriptHistoryState::LoadingBeginning {
            return;
        }
        if self.history.has_unloaded_history() {
            self.scroll(cells, /*rows*/ 0);
            self.history = TranscriptHistoryState::LoadingBeginning;
        } else {
            self.jump_to_entry(cells, /*index*/ 0);
        }
    }

    pub(crate) fn jump_to_latest(&mut self) {
        self.position = Position::Latest;
    }

    pub(crate) fn scroll(&mut self, cells: &[Arc<dyn HistoryCell>], rows: isize) {
        let start = self.start(cells);
        let (index, row) = self.move_rows(cells, start.0, start.1, rows);
        if rows < 0
            && (index, row) == start
            && self.is_following()
            && !self.history.has_unloaded_history()
        {
            return;
        }
        // Anchor visible content, not a hidden header that stays ahead of every older page.
        let index = self.next_nonempty(cells, index).unwrap_or(index);
        let bottom = self.bottom_start(cells);
        if rows > 0 && (index, row) >= bottom {
            self.jump_to_latest();
            return;
        }
        if self.layout(cells, index).is_some() {
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                row,
            });
        }
    }

    pub(crate) fn jump_to_entry(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) {
        self.position = Position::Reading(Anchor {
            key: self.entry_key(cells, index),
            index,
            row: 0,
        });
    }

    /// Reveal an entire entry when prompt backtracking changes the highlight.
    pub(crate) fn ensure_entry_visible(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) {
        let key = cells.get(index).map_or(EntryKey::Live, EntryKey::cell);
        let mut visible = self
            .visible
            .iter()
            .filter(|row| row.index == index && row.key == key);
        let fully_visible = visible
            .next()
            .is_some_and(|first| first.row == 0 && visible.count() + 1 == first.layout.row_count());
        if !fully_visible {
            self.jump_to_entry(cells, index);
        }
    }

    /// Keep readers on the replacement entry without translating source offsets.
    pub(crate) fn replace_range(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        range: std::ops::Range<usize>,
        replacement: &Arc<dyn HistoryCell>,
    ) {
        if let Position::Reading(anchor) = self.position
            && range.contains(&self.resolve(cells, anchor))
        {
            self.position = Position::Reading(Anchor {
                key: EntryKey::cell(replacement),
                index: range.start,
                ..anchor
            });
        }
    }

    /// Preserve display rows when retained entries gain or lose a leading separator.
    pub(crate) fn history_changed(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        if let Position::Reading(anchor) = self.position {
            let index = self.resolve(cells, anchor);
            let separated = cells.get(index).is_some_and(|cell| {
                EntryKey::cell(cell) == anchor.key && !cell.is_stream_continuation()
            });
            let row = if separated && anchor.index == 0 && index > 0 {
                anchor.row.saturating_add(/*rhs*/ 1)
            } else if separated && anchor.index > 0 && index == 0 {
                anchor.row.saturating_sub(/*rhs*/ 1)
            } else {
                anchor.row
            };
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                row,
            });
        }
    }

    pub(crate) fn near_start(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        if self.is_following() && self.area.is_empty() {
            return false;
        }
        let (index, row) = self.start(cells);
        self.move_rows(cells, index, row, -(self.area.height as isize)) == (0, 0)
    }

    fn start(&mut self, cells: &[Arc<dyn HistoryCell>]) -> (usize, usize) {
        match self.position {
            Position::Latest => self.bottom_start(cells),
            Position::Reading(anchor) => {
                let index = self.resolve(cells, anchor);
                let row = self.layout(cells, index).map_or(/*default*/ 0, |layout| {
                    anchor.row.min(layout.row_count().saturating_sub(/*rhs*/ 1))
                });
                self.position = Position::Reading(Anchor {
                    key: self.entry_key(cells, index),
                    index,
                    row,
                });
                (index, row)
            }
        }
    }

    fn bottom_start(&mut self, cells: &[Arc<dyn HistoryCell>]) -> (usize, usize) {
        let count = cells.len() + usize::from(self.live.is_some());
        let Some(last) = count.checked_sub(/*rhs*/ 1) else {
            return (0, 0);
        };
        let height = self
            .layout(cells, last)
            .map_or(/*default*/ 0, |l| l.row_count());
        self.move_rows(cells, last, height, -(self.area.height as isize))
    }

    fn move_rows(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        mut index: usize,
        mut row: usize,
        rows: isize,
    ) -> (usize, usize) {
        if rows < 0 {
            let mut remaining = rows.unsigned_abs();
            while remaining > row && index > 0 {
                remaining -= row;
                index -= 1;
                row = self
                    .layout(cells, index)
                    .map_or(/*default*/ 0, |l| l.row_count());
            }
            return (index, row.saturating_sub(remaining));
        }
        let mut remaining = row.saturating_add(rows as usize);
        while let Some(layout) = self.layout(cells, index) {
            if remaining < layout.row_count() {
                break;
            }
            remaining -= layout.row_count();
            index += 1;
        }
        (index, remaining)
    }

    fn next_nonempty(&mut self, cells: &[Arc<dyn HistoryCell>], mut index: usize) -> Option<usize> {
        while let Some(layout) = self.layout(cells, index) {
            if layout.row_count() > 0 {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn resolve(&self, cells: &[Arc<dyn HistoryCell>], anchor: Anchor) -> usize {
        if anchor.key == EntryKey::Live {
            return if self.live.is_some() {
                cells.len()
            } else {
                cells.len().saturating_sub(/*rhs*/ 1)
            };
        }
        if cells
            .get(anchor.index)
            .is_some_and(|cell| EntryKey::cell(cell) == anchor.key)
        {
            return anchor.index;
        }
        cells
            .iter()
            .position(|cell| EntryKey::cell(cell) == anchor.key)
            .unwrap_or_else(|| anchor.index.min(cells.len().saturating_sub(/*rhs*/ 1)))
    }

    fn entry_key(&self, cells: &[Arc<dyn HistoryCell>], index: usize) -> EntryKey {
        cells.get(index).map_or(EntryKey::Live, EntryKey::cell)
    }
}

impl TranscriptView {
    fn layout(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) -> Option<Arc<TextLayout>> {
        let Some(cell) = cells.get(index) else {
            if index != cells.len() {
                return None;
            }
            let live = self.live.as_ref()?;
            if cells.is_empty() || self.live_continuation {
                return Some(Arc::clone(live));
            }
            return Some(Arc::clone(self.live_separated.get_or_insert_with(|| {
                Arc::new(live.as_ref().clone().with_leading_separator())
            })));
        };
        let width = self.area.width.max(/*other*/ 1);
        if cell.as_any().is::<crate::history_cell::SessionInfoCell>()
            && let Some(placeholder) = self.history.session_header_placeholder()
        {
            return Some(Arc::new(TextLayout::new(
                vec![HyperlinkLine::from(Line::from(placeholder).dim())],
                width,
            )));
        }
        Some(
            self.cache
                .get(cell, width, index > 0 && !cell.is_stream_continuation()),
        )
    }
}

#[cfg(test)]
#[path = "transcript_view_tests.rs"]
mod tests;
