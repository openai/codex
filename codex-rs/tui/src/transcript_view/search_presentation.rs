//! Search full sources without changing manual disclosure state. Only the current match supplies
//! an expanded display layout; retained reading snapshots keep their original display layouts.

use super::*;

impl TranscriptView {
    pub(in crate::transcript_view) fn is_search_expanded(&self, key: EntryKey) -> bool {
        self.search
            .match_anchor()
            .is_some_and(|anchor| anchor.key == key)
    }

    /// Retain the original presentation before revealing a match and its surrounding context.
    pub(super) fn install_search_match(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        anchor: Anchor,
        end: usize,
        layout: Arc<TextLayout>,
    ) {
        let current_cells = cells;
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        let base = self
            .base_layout(cells, anchor.index)
            .unwrap_or_else(|| Arc::new(TextLayout::new(Vec::new(), self.area.width)));
        self.position = Position::Reading(anchor);
        self.hold_live_reading(current_cells, base);
        self.search.current = Some(Match {
            anchor,
            end,
            layout: Arc::clone(&layout),
        });
        let row = layout.row_for_offset(anchor.offset);
        let match_rows = layout.row_for_offset(end - 1) - row + 1;
        let context_rows = usize::from(self.area.height / 3)
            .min(usize::from(self.area.height).saturating_sub(match_rows));
        // Keep the hit's retained revision while placing its lead-in above it, even
        // in the preceding entry, without clipping a match that fits in the viewport.
        let (index, row) = self.move_rows(cells, anchor.index, row, -(context_rows as isize));
        if let Some(context) = self.layout(cells, index) {
            let offset = context.position_at(row, /*column*/ 0);
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                offset,
                row_bias: context.row_for_offset(offset) as isize - row as isize,
            });
        }
        self.search.progress = Progress::Found;
        self.search.scanning_layout = None;
    }

    /// Cache the full live source only while finding, independently of the compact live display.
    pub(crate) fn sync_search_live_tail(
        &mut self,
        width: u16,
        key: Option<ActiveCellTranscriptKey>,
        lines: impl FnOnce(u16) -> Option<Vec<HyperlinkLine>>,
    ) -> bool {
        if !self.search.is_active() || self.detailed {
            return false;
        }
        let next = key.map(|key| (width, key));
        if key.is_some_and(|key| key.cacheable) && self.search.live_key == next {
            return false;
        }
        self.search.live_key = next;
        let live = lines(width).map(|lines| Arc::new(TextLayout::new(lines, width)));
        let changed = self.search.live.as_ref().map(|layout| layout.text())
            != live.as_ref().map(|layout| layout.text());
        if changed {
            self.invalidate_live_search();
        }
        self.search.live = live;
        changed
    }

    pub(super) fn search_layout(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        anchor: Anchor,
    ) -> Option<Arc<TextLayout>> {
        if let Some((key, layout)) = &self.search.scanning_layout
            && *key == anchor.key
        {
            return Some(Arc::clone(layout));
        }
        let layout = if let Some(layout) = self.search.match_layout(anchor.key) {
            layout
        } else if anchor.key == EntryKey::Live && !self.detailed {
            let live = self
                .snapshot()
                .and_then(|snapshot| snapshot.search_live.clone())
                .or_else(|| self.search.live.clone());
            match live {
                Some(live) if !cells.is_empty() && !self.live_continuation => {
                    Arc::new(live.rewrap(self.area.width).with_leading_separator())
                }
                Some(live) => live,
                None => self.layout(cells, anchor.index)?,
            }
        } else if self.detailed {
            self.layout(cells, anchor.index)?
        } else {
            self.entry_layout(cells, anchor.index, /*full_content*/ true)?
        };
        self.search.scanning_layout = Some((anchor.key, Arc::clone(&layout)));
        Some(layout)
    }
}
