//! Incremental literal search over full transcript content, independent of collapsed display.
//!
//! Search retains one match and one entry being scanned. Each frame examines a bounded text
//! chunk; reaching the oldest loaded entry asks the app's existing history pager to continue.
//! The pager owns loading and failure status; search only remembers that it needs another page.
//! Closing the query keeps the match readable; cancellation restores the original presentation.

use crate::bottom_pane::TextArea;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

use super::*;

#[path = "search_footer.rs"]
mod footer;
#[path = "search_presentation.rs"]
mod presentation;

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
    layout: Arc<TextLayout>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SearchMode {
    Closed,
    Editing,
    Reading,
}

pub(super) struct Search {
    mode: SearchMode,
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
    pub(super) live: Option<Arc<TextLayout>>,
    live_key: Option<(u16, ActiveCellTranscriptKey)>,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            mode: SearchMode::Closed,
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
            live: None,
            live_key: None,
        }
    }
}

impl TranscriptView {
    pub(crate) fn begin_search(&mut self) {
        self.cancel_beginning();
        if self.is_search_editing() {
            return;
        }
        self.copy_mode = None;
        let saved_position = self.position;
        let saved_snapshot = self
            .selection
            .take()
            .map(|selection| selection.snapshot)
            .or(self.held_reading.take());
        if self.search.is_reading() {
            self.held_reading = saved_snapshot;
            self.search.mode = SearchMode::Editing;
            if self.search.current.is_none() {
                self.search.progress = Progress::Restart;
            }
            return;
        }
        let saved_detailed = self.detailed;
        self.search.mode = SearchMode::Editing;
        self.disclosure.focused = None;
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
        self.cache.clear();
        self.live_key = None;
    }

    pub(crate) fn is_search_editing(&self) -> bool {
        self.search.mode == SearchMode::Editing
    }

    pub(super) fn handle_search_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> bool {
        if !self.search.is_active() {
            return false;
        }
        let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
        match (code, modifiers) {
            (KeyCode::Esc, _) if self.search.is_reading() => self.jump_to_latest(),
            (KeyCode::Esc, _) if !self.search.editor.is_empty() => {
                self.search.mode = SearchMode::Reading;
                self.search.progress = if matches!(self.search.progress, Progress::Exhausted) {
                    Progress::Exhausted
                } else if self.search.current.is_some() {
                    Progress::Found
                } else {
                    Progress::Idle
                };
                self.search.scanning_layout = None;
            }
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.cancel_search();
            }
            (KeyCode::Enter, _) if self.search.is_reading() => return false,
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.next_search_match(cells, Direction::Newer);
            }
            (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.next_search_match(cells, Direction::Older);
            }
            (KeyCode::PageUp | KeyCode::PageDown, KeyModifiers::NONE) => return false,
            _ if JumpTarget::from_key(key).is_some() || self.search.is_reading() => return false,
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
        // Query editing owns other keys; reading and page navigation use the transcript.
        true
    }

    /// Retain the pre-search revision when canonical history retires the cell anchoring it.
    pub(super) fn retain_search_origin(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        retiring: std::ops::Range<usize>,
    ) {
        if self.search.is_active()
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
    pub(crate) fn cancel_search(&mut self) {
        if !self.search.is_active() {
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
        self.search.mode = SearchMode::Closed;
        self.search.editor.set_text_clearing_elements("");
        self.search.query_changed();
        self.search.progress = Progress::Idle;
        self.search.live = None;
        self.search.live_key = None;
        self.position = position;
        self.held_reading = snapshot;
        self.rewrap_snapshot(self.area.width);
    }

    pub(crate) fn paste_search(&mut self, text: &str) -> bool {
        if !self.is_search_editing() {
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
        if self.search.is_active() {
            self.search.query_changed();
            // Reading should not jump when scrolling releases a retained live revision.
            if self.search.is_reading() {
                self.search.progress = Progress::Idle;
            }
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
            // Scrolling away from a retained match must not immediately find it again.
            self.search.progress = Progress::Idle;
        }
    }

    /// Prepare geometry before scanning: offsets belong to a particular source layout.
    pub(crate) fn prepare_width(&mut self, width: u16) {
        if width == self.area.width {
            return;
        }
        self.rewrap_snapshot(width);
        if let Some(found) = &mut self.search.current {
            found.layout = Arc::new(found.layout.rewrap(width));
        }
        self.area.width = width;
        if self.search.current.is_none()
            || !matches!(self.search.progress, Progress::Found | Progress::Exhausted)
        {
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
                self.install_search_match(current_cells, anchor, window.start + range.end, layout);
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
                && let Some(previous) = self.search.current.take()
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
                let layout = self.search_layout(
                    cells,
                    Anchor {
                        key: cells.get(index).map_or(EntryKey::Live, EntryKey::cell),
                        index,
                        ..anchor
                    },
                );
                let same_prefix = layout.as_ref().is_some_and(|layout| {
                    previous
                        .layout
                        .text()
                        .get(..offset)
                        .is_some_and(|prefix| layout.text().starts_with(prefix))
                });
                anchor = Anchor {
                    key: cells.get(index).map_or(EntryKey::Live, EntryKey::cell),
                    index,
                    offset: if same_prefix { offset } else { 0 },
                    row_bias: 0,
                };
                self.search.current = layout.filter(|_| same_prefix).map(|layout| Match {
                    anchor: Anchor {
                        offset: match_start,
                        ..anchor
                    },
                    end: match_end,
                    layout,
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
            self.start_search_scan(cells, direction);
        }
    }

    fn start_search_scan(&mut self, cells: &[Arc<dyn HistoryCell>], direction: Direction) {
        self.search.page_start = None;
        let count = cells.len() + usize::from(self.live.is_some() || self.search.live.is_some());
        let index = if self.search.is_reading() {
            // Without a retained match, resume from the visible entry, not a transcript end.
            self.start(cells).0
        } else {
            match direction {
                Direction::Older => count.saturating_sub(/*rhs*/ 1),
                Direction::Newer => 0,
            }
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
                < cells.len() + usize::from(self.live.is_some() || self.search.live.is_some()))
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
    pub(super) fn match_layout(&self, key: EntryKey) -> Option<Arc<TextLayout>> {
        self.current
            .as_ref()
            .filter(|found| found.anchor.key == key)
            .map(|found| Arc::clone(&found.layout))
    }

    pub(super) fn match_anchor(&self) -> Option<Anchor> {
        self.current.as_ref().map(|found| found.anchor)
    }

    pub(super) fn is_active(&self) -> bool {
        self.mode != SearchMode::Closed
    }

    pub(super) fn is_reading(&self) -> bool {
        self.mode == SearchMode::Reading
    }

    pub(super) fn has_active_query(&self) -> bool {
        self.is_active() && !self.editor.is_empty()
    }

    pub(super) fn needs_history(&self, history: TranscriptHistoryState) -> bool {
        matches!(self.progress, Progress::AwaitingHistory)
            && history == TranscriptHistoryState::Partial
    }

    pub(super) fn allows_viewport_paging(&self) -> bool {
        !self.is_active()
            || (self.has_active_query()
                && matches!(
                    self.progress,
                    Progress::Idle | Progress::Found | Progress::Exhausted
                ))
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
