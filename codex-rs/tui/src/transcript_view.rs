//! A conversation viewport over the app's retained cells and its current live tail.
//!
//! The app owns history and pagination. This view owns only reading/selection state and bounded
//! layout caches. Anchors address content inside an entry, so prepending history never renumbers
//! them and rewrapping does not turn a reading position into an unrelated screen row.
//! Recap tail spacing is reserved by layout and never enters selectable cell content.

mod activity;
mod bookmark;
mod composer_gap;
mod disclosure;
mod follow_control;
mod footer;
mod input;
mod layout;
mod mutations;
mod prompt_header;
mod search;
mod selection;
mod snapshot;
mod text;

use std::sync::Arc;

use crate::chatwidget::ActiveCellTranscriptKey;
use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;
use crate::pager_overlay::TranscriptHistoryState;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

use layout::LayoutCache;
use search::Search;
use selection::Selection;
use snapshot::ViewSnapshot;
use text::TextLayout;

pub(crate) use bookmark::TranscriptBookmark;
pub(crate) use input::JumpTarget;
pub(crate) use input::ViewAction;
pub(crate) use layout::ActivityTranscriptLines;

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
    offset: usize,
    // Synthetic rows share source offsets: positive biases precede source (separators),
    // while negative biases follow it (disclosure controls).
    row_bias: isize,
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
    activity_ids: Arc<[String]>,
}

/// Shared scrolling and interaction state for compact and detailed transcript presentations.
pub(crate) struct TranscriptView {
    position: Position,
    follow_control: follow_control::FollowControl,
    copy_feedback: Option<composer_gap::CopyFeedback>,
    composer_tip: Option<(Rect, HyperlinkLine)>,
    cache: LayoutCache,
    live: Option<Arc<TextLayout>>,
    live_separated: Option<Arc<TextLayout>>,
    live_key: Option<(u16, ActiveCellTranscriptKey)>,
    live_continuation: bool,
    area: Rect,
    suppressed_prompt_header: Option<prompt_header::SuppressedHeader>,
    visible: Vec<VisibleRow>,
    selection: Option<Selection>,
    held_reading: Option<ViewSnapshot>,
    search: Search,
    detailed: bool,
    mode: HistoryRenderMode,
    pub(crate) history: TranscriptHistoryState,
    highlight: Option<usize>,
    saved_position: Option<Position>,
    unseen_activity: bool,
    tail_visible: bool,
    last_tail: Option<EntryKey>,
    last_click: Option<(std::time::Instant, u16, u16, u8)>,
    disclosure: disclosure::Disclosure,
}

impl Default for TranscriptView {
    fn default() -> Self {
        Self {
            position: Position::Latest,
            follow_control: follow_control::FollowControl::default(),
            copy_feedback: None,
            composer_tip: None,
            cache: LayoutCache::default(),
            live: None,
            live_separated: None,
            live_key: None,
            live_continuation: false,
            area: Rect::default(),
            suppressed_prompt_header: None,
            visible: Vec::new(),
            selection: None,
            held_reading: None,
            search: Search::default(),
            detailed: false,
            mode: HistoryRenderMode::Rich,
            history: TranscriptHistoryState::Idle,
            highlight: None,
            saved_position: None,
            unseen_activity: false,
            tail_visible: true,
            last_tail: None,
            last_click: None,
            disclosure: disclosure::Disclosure::default(),
        }
    }
}

impl TranscriptView {
    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer, cells: &[Arc<dyn HistoryCell>]) {
        self.composer_tip = None;
        self.cache.begin_frame();
        self.sync_history_tail(cells);
        let current_cells = cells;
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let suppressed_prompt_header = self.suppressed_prompt_header.take();
        Clear.render(area, buf);
        self.prepare_width(area.width);
        self.area = area;
        self.normalize_selection(cells);
        self.visible.clear();
        if area.is_empty() {
            self.tail_visible = false;
            return;
        }
        let initial_start = self.start(cells);
        let mut start = initial_start;
        let mut body = area;
        let header_position = prompt_header::SuppressedHeader {
            viewport: area,
            key: self.entry_key(cells, start.0),
            row: start.1,
        };
        if matches!(self.position, Position::Reading(_))
            && suppressed_prompt_header.as_ref() == Some(&header_position)
        {
            // Freezing follow must preserve a header that yielded to the next visible prompt.
            self.suppressed_prompt_header = Some(header_position);
        } else if area.height >= 4 && prompt_header::line(cells, start.0, area.width).is_some() {
            body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
            self.area = body;
            // Following may advance into the next turn after reserving the header row.
            start = self.start(cells);
            if let Some(header) = prompt_header::line(cells, start.0, area.width) {
                header.render(Rect::new(area.x, area.y, area.width, /*height*/ 1), buf);
            } else {
                body = area;
                self.area = area;
                start = initial_start;
                self.suppressed_prompt_header = Some(header_position);
            }
        }
        let area = body;
        let (mut index, mut row) = start;
        let mut entry_activity_ids: Option<(usize, Arc<[String]>)> = None;
        for y in area.top()..area.bottom() {
            let Some(layout) = self.layout(cells, index) else {
                break;
            };
            if row >= layout.row_count() {
                index += 1;
                row = 0;
                // Empty entries are legal (for example hidden reasoning).
                let Some(next) = self.next_nonempty(cells, index) else {
                    break;
                };
                index = next;
            }
            let Some(layout) = self.layout(cells, index) else {
                break;
            };
            let key = self.entry_key(cells, index);
            let activity_ids = match &entry_activity_ids {
                Some((previous, ids)) if *previous == index => Arc::clone(ids),
                _ => {
                    let ids = self.displayed_activity_ids(cells, index);
                    entry_activity_ids = Some((index, Arc::clone(&ids)));
                    ids
                }
            };
            let row_area = Rect::new(area.x, y, area.width, /*height*/ 1);
            layout.render(row_area, buf, row);
            self.render_disclosure(&activity_ids, &layout, row, row_area, buf);
            if self.highlight == Some(index) && self.selection.is_none() && !self.search.is_active()
            {
                layout.highlight(0..layout.text().len(), row_area, buf, row);
            }
            self.visible.push(VisibleRow {
                index,
                row,
                layout,
                key,
                activity_ids,
            });
            row += 1;
        }
        if let Some(pointer) = self
            .selection
            .as_ref()
            .filter(|selection| selection.dragging && selection.moved)
            .and_then(|selection| selection.pointer)
        {
            self.extend_selection(pointer.x, pointer.y);
        }
        self.tail_visible = self.current_tail_is_visible(current_cells);
        if self.tail_visible {
            self.unseen_activity = false;
        }
        self.render_selection(buf);
        if self.history == TranscriptHistoryState::LoadingBeginning && self.is_following() {
            self.scroll(current_cells, /*rows*/ 0);
        }
        self.render_search(buf);
    }

    pub(crate) fn sync_live_tail(
        &mut self,
        width: u16,
        key: Option<ActiveCellTranscriptKey>,
        lines: impl FnOnce(u16) -> Option<Vec<HyperlinkLine>>,
    ) -> bool {
        self.sync_live_layout(width, key, |width| {
            lines(width).map(|lines| TextLayout::new(lines, width))
        })
    }

    pub(crate) fn sync_live_activity_tail(
        &mut self,
        width: u16,
        key: Option<ActiveCellTranscriptKey>,
        expanded: bool,
        lines: impl FnOnce(u16) -> Option<ActivityTranscriptLines>,
    ) -> bool {
        self.sync_live_layout(width, key, |width| {
            lines(width).map(|lines| layout::activity_layout(lines, width, expanded))
        })
    }

    fn sync_live_layout(
        &mut self,
        width: u16,
        key: Option<ActiveCellTranscriptKey>,
        layout: impl FnOnce(u16) -> Option<TextLayout>,
    ) -> bool {
        let next = key.map(|key| (width, key));
        if key.is_some_and(|key| key.cacheable) && self.live_key == next {
            return false;
        }
        let revision_changed = self.live_key.is_none()
            || self.live_key.map(|(_, key)| key.revision) != next.map(|(_, key)| key.revision);
        self.live_key = next;
        self.live_separated = None;
        self.live_continuation = key.is_some_and(|key| key.is_stream_continuation);
        let live = layout(width).map(Arc::new);
        if self.live.as_ref().map(|layout| layout.text())
            != live.as_ref().map(|layout| layout.text())
        {
            if !self.is_following() && revision_changed && live.is_some() {
                self.unseen_activity = true;
            }
            self.invalidate_live_search();
        }
        let changed = self.live.is_some() || live.is_some();
        self.live = live;
        changed
    }

    pub(crate) fn set_presentation(&mut self, detailed: bool, mode: HistoryRenderMode) {
        if self.detailed == detailed && self.mode == mode {
            return;
        }
        self.selection = None;
        self.release_live_reading();
        self.cache.clear();
        self.suppressed_prompt_header = None;
        self.live_key = None;
        // Search temporarily expands content without changing either presentation's position.
        if self.detailed != detailed && !self.search.is_active() {
            let previous = self.position;
            self.position = self.saved_position.take().unwrap_or(previous);
            self.saved_position = Some(previous);
        }
        self.restart_search();
        self.detailed = detailed;
        self.mode = mode;
    }

    pub(crate) fn has_active_interaction(&self) -> bool {
        self.selection.is_some() || self.search.is_active() || self.is_activity_focused()
    }

    pub(crate) fn is_detailed(&self) -> bool {
        self.detailed
    }

    pub(crate) fn is_following(&self) -> bool {
        self.selection.is_none() && self.position == Position::Latest
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
        self.cancel_search();
        self.cancel_beginning();
        self.position = Position::Latest;
        self.selection = None;
        self.release_live_reading();
        self.unseen_activity = false;
        self.disclosure.focused = None;
    }

    pub(crate) fn scroll(&mut self, cells: &[Arc<dyn HistoryCell>], rows: isize) {
        if rows != 0 {
            self.last_click = None;
            self.cancel_beginning();
        }
        if self.area.is_empty() {
            return;
        }
        let current_cells = cells;
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let start = self.start(cells);
        let (index, row) = self.move_rows(cells, start.0, start.1, rows);
        if rows < 0
            && (index, row) == start
            && self.is_following()
            && !self.history.has_unloaded_history()
        {
            return;
        }
        if (index, row) != start
            && let Some(selection) = &mut self.selection
        {
            selection.resume_on_empty = false;
        }
        // Anchor visible content, not a hidden header that stays ahead of every older page.
        let index = self.next_nonempty(cells, index).unwrap_or(index);
        let bottom = self.bottom_start(cells);
        if rows > 0 && (index, row) >= bottom {
            if self.selection.is_none() {
                self.jump_to_latest();
            } else {
                self.position = Position::Latest;
            }
            return;
        }
        if let Some(layout) = self.layout(cells, index) {
            let offset = layout.position_at(row, /*column*/ 0);
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                offset,
                row_bias: layout.row_for_offset(offset) as isize - row as isize,
            });
            self.hold_live_reading(current_cells, layout);
        }
    }

    pub(crate) fn jump_to_entry(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) {
        self.cancel_beginning();
        self.release_live_reading();
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        self.position = Position::Reading(Anchor {
            key: self.entry_key(cells, index),
            index,
            offset: 0,
            row_bias: 0,
        });
    }

    pub(crate) fn cancel_beginning(&mut self) {
        if self.history == TranscriptHistoryState::LoadingBeginning {
            // Keep the in-flight page, but let the user's new navigation supersede the jump.
            self.history = TranscriptHistoryState::LoadingOlder;
        }
    }

    pub(crate) fn ensure_entry_visible(&mut self, cells: &[Arc<dyn HistoryCell>], index: usize) {
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        if !self.visible.iter().any(|row| {
            let start = row.layout.position_at(row.row, /*column*/ 0);
            let end = row.layout.position_at(row.row, self.area.width);
            row.index == index && !row.layout.text()[start..end].trim().is_empty()
        }) {
            // Restoring a highlight after pagination must not cancel an explicit Home jump.
            let history = self.history;
            self.jump_to_entry(cells, index);
            self.history = history;
        }
    }

    pub(crate) fn set_highlight(&mut self, index: Option<usize>) {
        self.highlight = index;
    }

    pub(crate) fn needs_history(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        self.search.needs_history(self.history)
            || (!self.search.is_active() && self.near_start(cells))
    }

    pub(crate) fn near_start(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        if self.area.is_empty() && self.is_following() {
            return false;
        }
        let (index, row) = self.start(cells);
        let threshold = usize::from(self.area.height);
        let mut distance = row;
        if distance > threshold {
            return false;
        }
        for previous in (0..index).rev() {
            distance = distance.saturating_add(
                self.layout(cells, previous)
                    .map_or(/*default*/ 0, |layout| layout.row_count()),
            );
            if distance > threshold {
                return false;
            }
        }
        true
    }

    fn start(&mut self, cells: &[Arc<dyn HistoryCell>]) -> (usize, usize) {
        match self.position {
            Position::Latest => self.bottom_start(cells),
            Position::Reading(anchor) => {
                let index = self.resolve(cells, anchor);
                let row = self.layout(cells, index).map_or(/*default*/ 0, |layout| {
                    layout
                        .row_for_offset(anchor.offset)
                        .saturating_add_signed(anchor.row_bias.saturating_neg())
                        .min(layout.row_count().saturating_sub(/*rhs*/ 1))
                });
                self.position = Position::Reading(Anchor {
                    key: self.entry_key(cells, index),
                    index,
                    ..anchor
                });
                (index, row)
            }
        }
    }

    fn bottom_start(&mut self, cells: &[Arc<dyn HistoryCell>]) -> (usize, usize) {
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let has_live = self.snapshot().map_or(self.live.is_some(), |snapshot| {
            snapshot.pinned.contains_key(&EntryKey::Live)
        });
        let count = cells.len() + usize::from(has_live);
        let Some(last) = count.checked_sub(/*rhs*/ 1) else {
            return (0, 0);
        };
        let height = self
            .layout(cells, last)
            .map_or(/*default*/ 0, |l| l.row_count());
        let recap_gap = usize::from(
            self.area.height > 1
                && !has_live
                && cells.last().is_some_and(|cell| {
                    cell.as_any()
                        .is::<crate::history_cell::ThreadRecapHistoryCell>()
                }),
        );
        self.move_rows(
            cells,
            last,
            height + recap_gap,
            -(self.area.height as isize),
        )
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
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        if anchor.key == EntryKey::Live {
            return cells.len();
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
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        cells.get(index).map_or(EntryKey::Live, EntryKey::cell)
    }
}

#[cfg(test)]
#[path = "transcript_view_tests.rs"]
mod tests;
