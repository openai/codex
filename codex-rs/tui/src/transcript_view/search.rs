//! Incremental literal search over the current transcript presentation.
//!
//! Search retains one match and one entry being scanned. Each frame examines a bounded text
//! chunk; reaching the oldest loaded entry asks the app's existing history pager to continue.
//! The pager owns loading and failure status; search only remembers that it needs another page.

use crate::bottom_pane::TextArea;
use crate::bottom_pane::TextAreaState;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::text::Span;
use ratatui::widgets::StatefulWidgetRef;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::*;

const QUERY_BYTES: usize = 4096;
const SCAN_BYTES: usize = 16 * 1024;
const ENTRIES_PER_FRAME: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Older,
    Newer,
}

#[derive(Clone, Copy)]
struct Cursor {
    anchor: Anchor,
    direction: Direction,
}

#[derive(Clone, Copy, Default)]
enum Progress {
    #[default]
    Idle,
    Restart,
    Scanning(Cursor),
    AwaitingHistory,
    Found,
    Exhausted,
}

struct Match {
    anchor: Anchor,
    end: usize,
}

pub(super) struct Search {
    active: bool,
    editor: TextArea,
    folded_query: String,
    saved_position: Position,
    saved_snapshot: Option<ViewSnapshot>,
    saved_detailed: bool,
    current: Option<Match>,
    progress: Progress,
    scanning_layout: Option<(EntryKey, Arc<TextLayout>)>,
    page_start: Option<EntryKey>,
    query_truncated: bool,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            active: false,
            editor: TextArea::new(),
            folded_query: String::new(),
            saved_position: Position::default(),
            saved_snapshot: None,
            saved_detailed: false,
            current: None,
            progress: Progress::Idle,
            scanning_layout: None,
            page_start: None,
            query_truncated: false,
        }
    }
}

impl TranscriptView {
    pub(crate) fn begin_search(&mut self) {
        self.cancel_beginning();
        if self.search.active {
            return;
        }
        let saved_position = self.position;
        let saved_snapshot = self
            .selection
            .take()
            .map(|selection| selection.snapshot)
            .or(self.held_reading.take());
        let saved_detailed = self.detailed;
        self.search.active = true;
        self.disclosure.focused = None;
        self.set_presentation(/*detailed*/ true, self.mode);
        self.search.saved_position = saved_position;
        self.search.saved_snapshot = saved_snapshot;
        self.search.saved_detailed = saved_detailed;
        self.search.progress = Progress::Restart;
    }

    pub(crate) fn set_keymap_bindings(&mut self, keymap: &RuntimeKeymap) {
        // Resolved keymaps are immutable; config changes replace their shared chord map.
        if Arc::ptr_eq(&self.disclosure.keymap.chords, &keymap.chords) {
            return;
        }
        self.search.editor.set_keymap_bindings(keymap);
        self.disclosure.keymap = keymap.clone();
    }

    pub(crate) fn is_search_active(&self) -> bool {
        self.search.active
    }

    /// Project the editor and its caret from the same one-row viewport into the existing footer.
    pub(crate) fn search_footer(&self, width: u16) -> Option<(Line<'static>, u16)> {
        if !self.search.active || width == 0 {
            return None;
        }
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
        let mut buffer = Buffer::empty(area);
        let prefix_width = width.saturating_sub(/*rhs*/ 1).min(/*other*/ 6);
        Line::from("Find: ").dim().render(
            Rect::new(/*x*/ 0, /*y*/ 0, prefix_width, /*height*/ 1),
            &mut buffer,
        );
        let query_area = Rect::new(
            prefix_width,
            /*y*/ 0,
            width - prefix_width,
            /*height*/ 1,
        );
        let mut state = TextAreaState::default();
        StatefulWidgetRef::render_ref(&&self.search.editor, query_area, &mut buffer, &mut state);
        let (cursor_column, _) = self
            .search
            .editor
            .cursor_pos_with_state(query_area, state)?;
        let mut spans = Vec::new();
        let mut column = 0;
        while column < width {
            let cell = &buffer[(column, 0)];
            spans.push(Span::styled(cell.symbol().to_string(), cell.style()));
            column += cell.symbol().width().max(/*other*/ 1) as u16;
        }
        Some((Line::from(spans), cursor_column))
    }

    pub(super) fn handle_search_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> bool {
        if !self.search.active {
            return false;
        }
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        match (code, modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.cancel_search();
            }
            // Ctrl+N/P stay distinct when a legacy terminal encodes Shift+Enter as Enter.
            (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.next_search_match(cells, Direction::Newer);
            }
            (KeyCode::Enter, KeyModifiers::SHIFT) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.next_search_match(cells, Direction::Older);
            }
            _ => {
                let before = self.search.editor.text().to_string();
                let cursor = self.search.editor.cursor();
                self.search.editor.input(key);
                if self.search.editor.text().len() > QUERY_BYTES {
                    self.search.editor.set_text_clearing_elements(&before);
                    self.search.editor.set_cursor(cursor);
                    self.search.query_truncated = true;
                } else if self.search.editor.text() != before {
                    if self.held_reading.take().is_some() {
                        self.visible.clear();
                        self.position = Position::Latest;
                    }
                    self.search.query_changed();
                }
            }
        }
        // Find owns every key until it closes, including unbound keys and editor chords.
        true
    }

    /// Retain the pre-search revision when canonical history retires the cell anchoring it.
    pub(super) fn retain_search_origin(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        retiring: std::ops::Range<usize>,
    ) {
        if self.search.active
            && self.search.saved_snapshot.is_none()
            && let Position::Reading(anchor) = self.search.saved_position
            && cells[retiring]
                .iter()
                .any(|cell| EntryKey::cell(cell) == anchor.key)
        {
            let mut snapshot = self.capture_snapshot(cells);
            if self.detailed != self.search.saved_detailed {
                snapshot.pinned.clear();
                snapshot.activities.clear();
            }
            self.search.saved_snapshot = Some(snapshot);
        }
    }

    /// Restore the pre-search presentation before an explicit return or cancellation.
    pub(super) fn cancel_search(&mut self) {
        if !self.search.active {
            return;
        }
        let position = self.search.saved_position;
        let snapshot = self.search.saved_snapshot.take().or_else(|| {
            let Position::Reading(anchor) = position else {
                return None;
            };
            let mut snapshot = self.held_reading.take().filter(|snapshot| {
                snapshot
                    .cells
                    .iter()
                    .any(|cell| EntryKey::cell(cell) == anchor.key)
            })?;
            if self.detailed != self.search.saved_detailed {
                snapshot.pinned.clear();
                snapshot.activities.clear();
            }
            Some(snapshot)
        });
        let detailed = self.search.saved_detailed;
        self.set_presentation(detailed, self.mode);
        self.search.active = false;
        self.search.editor.set_text_clearing_elements("");
        self.search.query_changed();
        self.search.progress = Progress::Idle;
        self.position = position;
        self.held_reading = snapshot;
        self.rewrap_snapshot(self.area.width);
    }

    pub(crate) fn paste_search(&mut self, text: &str) -> bool {
        if !self.search.active {
            return false;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let remaining = QUERY_BYTES.saturating_sub(self.search.editor.text().len());
        let end = normalized
            .grapheme_indices(/*is_extended*/ true)
            .map(|(offset, grapheme)| offset + grapheme.len())
            .take_while(|end| *end <= remaining)
            .last()
            .unwrap_or_default();
        if end > 0 {
            if self.held_reading.take().is_some() {
                self.visible.clear();
                self.position = Position::Latest;
            }
            self.search.editor.insert_str(&normalized[..end]);
            self.search.query_changed();
        }
        self.search.query_truncated = end < normalized.len();
        true
    }

    /// Restart after a presentation change invalidates the searched text and its source offsets.
    pub(crate) fn restart_search(&mut self) {
        if self.search.active {
            self.search.query_changed();
        }
    }

    /// Drop an in-progress scan of a live revision when the visible source changes. A match
    /// already being read has its own snapshot and remains valid until navigation leaves it.
    pub(super) fn invalidate_live_search(&mut self) {
        if self.held_reading.is_none()
            && matches!(
                self.search.progress,
                Progress::Scanning(Cursor {
                    anchor: Anchor {
                        key: EntryKey::Live,
                        ..
                    },
                    ..
                })
            )
        {
            self.restart_search();
        }
    }

    pub(super) fn invalidate_held_search(&mut self) {
        if self.search.has_active_query() {
            self.restart_search();
        }
    }

    /// Prepare geometry before scanning: offsets belong to a particular source layout.
    pub(crate) fn prepare_width(&mut self, width: u16) {
        if width == self.area.width {
            return;
        }
        self.rewrap_snapshot(width);
        self.area.width = width;
        if self.held_reading.is_none() || !matches!(self.search.progress, Progress::Found) {
            self.restart_search();
        }
    }

    /// Return true only while another local scanning frame can make progress.
    pub(crate) fn advance_search(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        if self.selection.is_some() {
            return false;
        }
        let current_cells = cells;
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        if matches!(self.search.progress, Progress::Restart) {
            self.search.progress = Progress::Idle;
            // A refined query must not leave an obsolete match at the top while history loads.
            self.position = self.search.saved_position;
            if self.search.editor.is_empty() {
                self.held_reading = self.search.saved_snapshot.clone();
                self.rewrap_snapshot(self.area.width);
                return false;
            }
            self.start_search_scan(cells, Direction::Older);
        }
        if matches!(self.search.progress, Progress::AwaitingHistory)
            && matches!(
                self.history,
                TranscriptHistoryState::Complete | TranscriptHistoryState::Idle
            )
        {
            self.search.progress = Progress::Exhausted;
        }
        let mut scanned_bytes = 0;
        for _ in 0..ENTRIES_PER_FRAME {
            let Progress::Scanning(mut cursor) = self.search.progress else {
                return false;
            };
            cursor.anchor.index = self.resolve(cells, cursor.anchor);
            let Some(layout) = self.search_layout(cells, cursor.anchor) else {
                self.advance_search_cursor(cells, cursor, 0..0, /*text_len*/ 0);
                return false;
            };
            let text = layout.text();
            cursor.anchor.offset = text.floor_char_boundary(cursor.anchor.offset.min(text.len()));
            let window = scan_window(text, cursor, self.search.folded_query.len());
            if let Some(range) = find_literal(
                &text[window.clone()],
                &self.search.folded_query,
                cursor.direction,
            ) {
                let anchor = Anchor {
                    offset: window.start + range.start,
                    ..cursor.anchor
                };
                self.position = Position::Reading(anchor);
                self.hold_live_reading(current_cells, Arc::clone(&layout));
                self.search.current = Some(Match {
                    anchor,
                    end: window.start + range.end,
                });
                self.search.progress = Progress::Found;
                self.search.scanning_layout = None;
                return false;
            }
            scanned_bytes += window.len();
            self.advance_search_cursor(cells, cursor, window, text.len());
            // Batch short entries without letting a frame scan an unbounded transcript.
            // The last window can overshoot the budget; its size and query overlap are bounded.
            if scanned_bytes >= SCAN_BYTES {
                return matches!(self.search.progress, Progress::Scanning(_));
            }
        }
        matches!(self.search.progress, Progress::Scanning(_))
    }

    /// Scan only the inserted page; retained session headers may precede its splice location.
    pub(crate) fn history_loaded(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        inserted: Range<usize>,
    ) {
        // With only a header and live output, the first older page extends the canonical tail.
        if !inserted.is_empty()
            && inserted.end == cells.len()
            && self.last_tail == cells[..inserted.start].last().map(EntryKey::cell)
        {
            self.last_tail = cells.last().map(EntryKey::cell);
        }
        self.prepend_snapshot_history(cells, inserted.clone());
        if let Position::Reading(anchor) = self.position {
            let index = self.resolve(cells, anchor);
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                ..anchor
            });
        }
        if !matches!(self.search.progress, Progress::AwaitingHistory) || inserted.is_empty() {
            return;
        }
        let index = inserted.end - 1;
        self.search.progress = Progress::Scanning(Cursor {
            anchor: Anchor {
                key: self.entry_key(cells, index),
                index,
                offset: usize::MAX,
                row_bias: 0,
            },
            direction: Direction::Older,
        });
        self.search.scanning_layout = None;
        self.search.page_start = Some(self.entry_key(cells, inserted.start));
    }

    pub(super) fn render_search(&self, buf: &mut Buffer) {
        // The copy range takes precedence until selection ends.
        if self.selection.is_some() {
            return;
        }
        let Some(found) = &self.search.current else {
            return;
        };
        for (y, visible) in self.visible.iter().enumerate() {
            if visible.key == found.anchor.key {
                let area = Rect::new(
                    self.area.x,
                    self.area.y + y as u16,
                    self.area.width,
                    /*height*/ 1,
                );
                visible
                    .layout
                    .highlight(found.anchor.offset..found.end, area, buf, visible.row);
            }
        }
    }

    /// Navigate from a hit; confirmation must not reverse the initial scan of older history.
    fn next_search_match(&mut self, cells: &[Arc<dyn HistoryCell>], direction: Direction) {
        if self.search.editor.is_empty() {
            return;
        }
        if matches!(self.search.progress, Progress::AwaitingHistory)
            && self.history == TranscriptHistoryState::Failed
            && direction == Direction::Older
        {
            self.history = TranscriptHistoryState::Partial;
        } else if let Some(found) = &self.search.current {
            self.search.scanning_layout = None;
            let offset = match direction {
                Direction::Older => found.anchor.offset,
                Direction::Newer => found.end,
            };
            let match_start = found.anchor.offset;
            let match_end = found.end;
            let mut anchor = Anchor {
                offset,
                ..found.anchor
            };
            if direction == Direction::Newer
                && let Some(snapshot) = self.held_reading.take()
            {
                // Continue through newly committed output, or rejoin a surviving cell.
                let index = if anchor.key == EntryKey::Live {
                    snapshot
                        .cells
                        .iter()
                        .rev()
                        .find_map(|previous| {
                            cells.iter().rposition(|cell| Arc::ptr_eq(cell, previous))
                        })
                        .map_or(0, |index| index + 1)
                } else if let Some(index) = cells
                    .iter()
                    .position(|cell| EntryKey::cell(cell) == anchor.key)
                {
                    index
                } else {
                    self.restart_search();
                    self.visible.clear();
                    return;
                };
                let prefix = snapshot
                    .pinned
                    .get(&anchor.key)
                    .and_then(|layout| layout.text().get(..offset));
                let same_prefix = self.current_layout(cells, index).is_some_and(|layout| {
                    prefix.is_some_and(|prefix| layout.text().starts_with(prefix))
                });
                anchor = Anchor {
                    key: cells.get(index).map_or(EntryKey::Live, EntryKey::cell),
                    index,
                    offset: if same_prefix { offset } else { 0 },
                    row_bias: 0,
                };
                self.search.current = same_prefix.then_some(Match {
                    anchor: Anchor {
                        offset: match_start,
                        ..anchor
                    },
                    end: match_end,
                });
                self.visible.clear();
                self.position = Position::Reading(
                    self.search
                        .current
                        .as_ref()
                        .map_or(anchor, |found| found.anchor),
                );
                if same_prefix && let Some(layout) = self.current_layout(cells, index) {
                    self.hold_live_reading(cells, layout);
                }
            }
            self.search.progress = Progress::Scanning(Cursor { anchor, direction });
        } else if matches!(self.search.progress, Progress::Idle | Progress::Exhausted) {
            self.start_search_scan(cells, Direction::Older);
        }
    }

    fn start_search_scan(&mut self, cells: &[Arc<dyn HistoryCell>], direction: Direction) {
        self.search.page_start = None;
        let count = cells.len() + usize::from(self.live.is_some());
        let index = match direction {
            Direction::Older => count.saturating_sub(/*rhs*/ 1),
            Direction::Newer => 0,
        };
        let offset = match direction {
            Direction::Older => usize::MAX,
            Direction::Newer => 0,
        };
        self.search.progress = Progress::Scanning(Cursor {
            anchor: Anchor {
                key: self.entry_key(cells, index),
                index,
                offset,
                row_bias: 0,
            },
            direction,
        });
    }

    fn search_layout(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        anchor: Anchor,
    ) -> Option<Arc<TextLayout>> {
        if let Some((key, layout)) = &self.search.scanning_layout
            && *key == anchor.key
        {
            return Some(Arc::clone(layout));
        }
        let layout = self.layout(cells, anchor.index)?;
        self.search.scanning_layout = Some((anchor.key, Arc::clone(&layout)));
        Some(layout)
    }

    fn advance_search_cursor(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        cursor: Cursor,
        window: Range<usize>,
        text_len: usize,
    ) {
        let remaining = match cursor.direction {
            Direction::Older => window.start > 0,
            Direction::Newer => window.end < text_len,
        };
        if remaining {
            let offset = match cursor.direction {
                Direction::Older => cursor.anchor.offset.saturating_sub(SCAN_BYTES),
                Direction::Newer => cursor.anchor.offset.saturating_add(SCAN_BYTES),
            };
            self.search.progress = Progress::Scanning(Cursor {
                anchor: Anchor {
                    offset,
                    ..cursor.anchor
                },
                ..cursor
            });
            return;
        }
        self.search.scanning_layout = None;
        let next = match cursor.direction {
            Direction::Older if self.search.page_start == Some(cursor.anchor.key) => None,
            Direction::Older => cursor.anchor.index.checked_sub(/*rhs*/ 1),
            Direction::Newer => (cursor.anchor.index + 1
                < cells.len() + usize::from(self.live.is_some()))
            .then_some(cursor.anchor.index + 1),
        };
        self.search.progress = match next {
            Some(index) => Progress::Scanning(Cursor {
                anchor: Anchor {
                    key: self.entry_key(cells, index),
                    index,
                    offset: if cursor.direction == Direction::Older {
                        usize::MAX
                    } else {
                        0
                    },
                    row_bias: 0,
                },
                direction: cursor.direction,
            }),
            None if cursor.direction == Direction::Older
                && matches!(
                    self.history,
                    TranscriptHistoryState::LoadingOlder
                        | TranscriptHistoryState::LoadingBeginning
                        | TranscriptHistoryState::Partial
                        | TranscriptHistoryState::Failed
                ) =>
            {
                Progress::AwaitingHistory
            }
            None => Progress::Exhausted,
        };
    }
}

impl Search {
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    pub(super) fn has_active_query(&self) -> bool {
        self.active && !self.editor.is_empty()
    }

    pub(super) fn needs_history(&self, history: TranscriptHistoryState) -> bool {
        matches!(self.progress, Progress::AwaitingHistory)
            && history == TranscriptHistoryState::Partial
    }

    pub(super) fn status_line(&self, width: u16, history: TranscriptHistoryState) -> Line<'static> {
        let (status, compact) = match self.progress {
            Progress::Idle => ("Type to find", "Type to find"),
            Progress::Restart | Progress::Scanning(_) => ("Searching…", "Searching…"),
            Progress::AwaitingHistory if history == TranscriptHistoryState::Failed => {
                ("History unavailable · ctrl+p retry", "ctrl+p retry")
            }
            Progress::AwaitingHistory => ("Searching earlier history…", "Loading…"),
            Progress::Found => ("enter next · ctrl+p previous", "enter next"),
            Progress::Exhausted if self.current.is_some() => (
                "No more matches · enter next · ctrl+p previous",
                "enter next",
            ),
            Progress::Exhausted => ("No matches", "No matches"),
        };
        let limit = if self.query_truncated {
            " · query limited to 4 KiB"
        } else {
            ""
        };
        crate::footer_hint::first_fitting_line(
            [
                format!("{status} · full transcript · esc close{limit}"),
                format!("{status} · esc close{limit}"),
                format!("{compact} · esc close"),
                format!("{compact} · esc"),
                "esc close".to_owned(),
            ]
            .map(|hint| super::footer::navigation_line(&hint)),
            width,
        )
    }

    fn query_changed(&mut self) {
        self.folded_query = self
            .editor
            .text()
            .chars()
            .flat_map(char::to_lowercase)
            .collect();
        self.current = None;
        self.scanning_layout = None;
        self.page_start = None;
        self.progress = Progress::Restart;
        self.query_truncated = false;
    }
}

fn scan_window(text: &str, cursor: Cursor, query_len: usize) -> Range<usize> {
    // A source character can occupy four bytes while its lowercase form occupies only one.
    let overlap = query_len.saturating_mul(/*rhs*/ 4);
    let extent = SCAN_BYTES.saturating_add(overlap);
    match cursor.direction {
        Direction::Older => {
            text.floor_char_boundary(cursor.anchor.offset.saturating_sub(extent))
                ..cursor.anchor.offset
        }
        Direction::Newer => {
            cursor.anchor.offset
                ..text.floor_char_boundary(
                    cursor.anchor.offset.saturating_add(extent).min(text.len()),
                )
        }
    }
}

fn find_literal(text: &str, query: &str, direction: Direction) -> Option<Range<usize>> {
    let mut folded = String::new();
    let mut spans = Vec::new();
    for (start, character) in text.char_indices() {
        for lowercase in character.to_lowercase() {
            folded.push(lowercase);
            spans.push((folded.len(), start..start + character.len_utf8()));
        }
    }
    let start = match direction {
        Direction::Older => folded.rfind(query),
        Direction::Newer => folded.find(query),
    }?;
    let first = spans.partition_point(|(end, _)| *end <= start);
    let last = spans.partition_point(|(end, _)| *end < start + query.len());
    Some(spans.get(first)?.1.start..spans.get(last)?.1.end)
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
