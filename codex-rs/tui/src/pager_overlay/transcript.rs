//! Detailed transcript overlay over committed history and the current live tail.

use super::*;
use crate::transcript_view::LayoutCache;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) struct TranscriptOverlay {
    /// Pager UI state and the renderables currently displayed.
    ///
    /// The invariant is that `view.renderables` is `render_cells(cells)` plus an optional trailing
    /// live-tail renderable appended after the committed cells.
    view: PagerView,
    cache: Rc<RefCell<LayoutCache>>,
    /// Committed transcript cells (does not include the live tail).
    cells: Vec<Arc<dyn HistoryCell>>,
    highlight_cell: Option<usize>,
    /// Cache key for the render-only live tail appended after committed cells.
    live_tail_key: Option<LiveTailKey>,
    history_state: TranscriptHistoryState,
    is_done: bool,
}

/// Cache key for the active-cell "live tail" appended to the transcript overlay.
///
/// Changing any field implies a different rendered tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LiveTailKey {
    /// Current terminal width, which affects wrapping.
    width: u16,
    /// Revision that changes on in-place active cell transcript updates.
    revision: u64,
    /// Whether the tail should be treated as a continuation for spacing.
    is_stream_continuation: bool,
    /// Optional animation tick to refresh spinners/progress indicators.
    animation_tick: Option<u64>,
}

impl TranscriptOverlay {
    /// Creates a transcript overlay for a fixed set of committed cells.
    ///
    /// This overlay does not own the "active cell"; callers may optionally append a live tail via
    /// `sync_live_tail` during draws to reflect in-flight activity.
    pub(crate) fn new(transcript_cells: Vec<Arc<dyn HistoryCell>>, keymap: PagerKeymap) -> Self {
        let cache = Rc::new(RefCell::new(LayoutCache::default()));
        Self {
            view: PagerView::new(
                Self::render_cells(
                    &cache,
                    &transcript_cells,
                    /*highlight_cell*/ None,
                    TranscriptHistoryState::Idle,
                ),
                "T R A N S C R I P T".to_string(),
                usize::MAX,
                keymap,
            ),
            cache,
            cells: transcript_cells,
            highlight_cell: None,
            live_tail_key: None,
            history_state: TranscriptHistoryState::Idle,
            is_done: false,
        }
    }

    pub(crate) fn set_history_state(
        &mut self,
        state: TranscriptHistoryState,
    ) -> TranscriptHistoryState {
        let previous = self.history_state;
        if previous == state {
            return previous;
        }
        if previous == TranscriptHistoryState::LoadingBeginning
            && state == TranscriptHistoryState::Complete
        {
            self.view.scroll_offset = 0;
        }
        self.history_state = state;
        self.view.scroll_percentage_visible = !state.has_unloaded_history();
        if self
            .cells
            .iter()
            .any(|cell| cell.as_any().is::<SessionInfoCell>())
        {
            let live_tail = self.take_live_tail_renderable();
            self.rebuild_renderables(live_tail);
        }
        previous
    }

    fn render_cells(
        cache: &Rc<RefCell<LayoutCache>>,
        cells: &[Arc<dyn HistoryCell>],
        highlight_cell: Option<usize>,
        history_state: TranscriptHistoryState,
    ) -> Vec<Box<dyn Renderable>> {
        cells
            .iter()
            .enumerate()
            .map(|(i, cell)| Self::render_cell(cache, cell, i, highlight_cell, history_state))
            .collect()
    }

    /// Build the renderable for a committed cell, caching its height when the cell is stable.
    fn render_cell(
        cache: &Rc<RefCell<LayoutCache>>,
        cell: &Arc<dyn HistoryCell>,
        index: usize,
        highlight_cell: Option<usize>,
        history_state: TranscriptHistoryState,
    ) -> Box<dyn Renderable> {
        if cell.as_any().is::<SessionInfoCell>()
            && let Some(placeholder) = history_state.session_header_placeholder()
        {
            return Box::new(Line::from(placeholder).dim());
        }
        let cell_renderable = CellRenderable {
            cache: Rc::clone(cache),
            cell: cell.clone(),
            highlighted: highlight_cell == Some(index),
        };
        let mut cell_renderable: Box<dyn Renderable> = if cell.has_stable_transcript_height() {
            Box::new(CachedRenderable::new(cell_renderable))
        } else {
            Box::new(cell_renderable)
        };
        if !cell.is_stream_continuation() && index > 0 {
            cell_renderable = Box::new(InsetRenderable::new(
                cell_renderable,
                Insets::tlbr(
                    /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
                ),
            ));
        }
        cell_renderable
    }

    /// Insert a committed history cell while keeping any cached live tail.
    ///
    /// The live tail is temporarily removed, the new committed cell is appended,
    /// then the tail is reattached. If the tail previously had no leading
    /// spacing because it was the only renderable, we add the missing inset
    /// when the first committed cell arrives.
    ///
    /// This expects `cell` to be a committed transcript cell (not the in-flight active cell). If
    /// the overlay was scrolled to bottom before insertion, it remains pinned to bottom after the
    /// insertion to preserve the "follow along" behavior.
    pub(crate) fn insert_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        let follow_bottom = self.view.is_scrolled_to_bottom();
        let had_prior_cells = !self.cells.is_empty();
        let tail_renderable = self.take_live_tail_renderable();
        let cell_renderable = Self::render_cell(
            &self.cache,
            &cell,
            self.cells.len(),
            self.highlight_cell,
            self.history_state,
        );
        self.cells.push(cell);
        self.view.renderables.push(cell_renderable);
        if let Some(tail) = tail_renderable {
            let tail = if !had_prior_cells
                && self
                    .live_tail_key
                    .is_some_and(|key| !key.is_stream_continuation)
            {
                // The tail was rendered as the only entry, so it lacks a top
                // inset; add one now that it follows a committed cell.
                Box::new(InsetRenderable::new(
                    tail,
                    Insets::tlbr(
                        /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
                    ),
                )) as Box<dyn Renderable>
            } else {
                tail
            };
            self.view.renderables.push(tail);
        }
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
    }

    /// Returns whether an upward navigation is close enough to request older history.
    pub(crate) fn should_load_older(&self, key_event: KeyEvent) -> bool {
        self.should_load_from_start(key_event)
            || (self.view.scroll_offset
                <= self.view.last_content_height.unwrap_or(/*default*/ 0)
                && (self.view.keymap.scroll_up.is_pressed(key_event)
                    || self.view.keymap.page_up.is_pressed(key_event)
                    || self.view.keymap.half_page_up.is_pressed(key_event)))
    }

    pub(crate) fn should_load_from_start(&self, key_event: KeyEvent) -> bool {
        self.view.keymap.jump_top.is_pressed(key_event)
    }

    /// Prepends history without moving visible content and returns its insertion index.
    pub(crate) fn prepend(&mut self, cells: Vec<Arc<dyn HistoryCell>>, width: u16) -> usize {
        if cells.is_empty() {
            return 0;
        }
        let follow_bottom = self.view.is_scrolled_to_bottom();
        let previous_height = self.view.content_height(width);
        let live_tail = self.take_live_tail_renderable();
        let added_cells = cells.len();
        let insert_at = self
            .cells
            .iter()
            .rposition(|cell| cell.as_any().is::<SessionInfoCell>())
            .map_or(/*default*/ 0, |index| index.saturating_add(/*rhs*/ 1));
        self.cells.splice(insert_at..insert_at, cells);
        for index in [
            &mut self.highlight_cell,
            &mut self.view.pending_scroll_chunk,
        ] {
            if let Some(index) = index.as_mut()
                && *index >= insert_at
            {
                *index = index.saturating_add(added_cells);
            }
        }
        self.rebuild_renderables(live_tail);
        let content_height = self.view.content_height(width);
        self.view.scroll_offset = if follow_bottom {
            usize::MAX
        } else {
            self.view
                .scroll_offset
                .saturating_add(content_height.saturating_sub(previous_height))
        };
        self.view.last_rendered_height = Some(content_height);
        insert_at
    }

    /// Replace committed transcript cells while keeping any cached in-progress output that is
    /// currently shown at the end of the overlay.
    ///
    /// This is used when existing history is replaced or trimmed so the
    /// transcript overlay immediately reflects the same committed cells as the main transcript.
    pub(crate) fn replace_cells(&mut self, cells: Vec<Arc<dyn HistoryCell>>) {
        let follow_bottom = self.view.is_scrolled_to_bottom();
        let live_tail = self.take_live_tail_renderable();
        self.cells = cells;
        if self
            .highlight_cell
            .is_some_and(|idx| idx >= self.cells.len())
        {
            self.highlight_cell = None;
        }
        self.rebuild_renderables(live_tail);
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
    }

    /// Replace a range of committed cells with a single consolidated cell.
    ///
    /// Mirrors the splice performed on `App::transcript_cells` during
    /// `ConsolidateAgentMessage` so the Ctrl+T overlay stays in sync with the
    /// main transcript. The range is clamped defensively: cells may have been
    /// inserted after the overlay opened, leaving it with fewer entries than
    /// the main transcript.
    pub(crate) fn consolidate_cells(
        &mut self,
        range: std::ops::Range<usize>,
        consolidated: Arc<dyn HistoryCell>,
    ) {
        let follow_bottom = self.view.is_scrolled_to_bottom();
        // Clamp the range to the overlay's cell count to avoid panic if the overlay has fewer
        // cells than the main transcript (e.g. cells were inserted after the overlay has opened).
        let clamped_end = range.end.min(self.cells.len());
        let clamped_start = range.start.min(clamped_end);
        if clamped_start < clamped_end {
            let live_tail = self.take_live_tail_renderable();
            let removed = clamped_end - clamped_start;
            if let Some(highlight_cell) = self.highlight_cell.as_mut()
                && *highlight_cell >= clamped_start
            {
                if *highlight_cell < clamped_end {
                    *highlight_cell = clamped_start;
                } else {
                    *highlight_cell = highlight_cell.saturating_sub(removed.saturating_sub(1));
                }
            }
            self.cells
                .splice(clamped_start..clamped_end, std::iter::once(consolidated));
            if self
                .highlight_cell
                .is_some_and(|highlight_cell| highlight_cell >= self.cells.len())
            {
                self.highlight_cell = None;
            }
            self.rebuild_renderables(live_tail);
        }
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
    }

    /// Sync the active-cell live tail with the current width and cell state.
    ///
    /// Recomputes the tail only when the cache key changes, preserving scroll
    /// position and dropping the tail if there is nothing to render.
    ///
    /// The overlay owns committed transcript cells while the live tail is derived from the current
    /// active cell, which can mutate in place while streaming. `App` calls this during
    /// `TuiEvent::Draw` for `Overlay::Transcript`, passing a key that changes when the active cell
    /// mutates or animates so the cached tail stays fresh.
    ///
    /// Passing a key that does not change on in-place active-cell mutations will freeze the tail in
    /// `Ctrl+T` while the main viewport continues to update.
    pub(crate) fn sync_live_tail(
        &mut self,
        width: u16,
        active_key: Option<ActiveCellTranscriptKey>,
        compute_lines: impl FnOnce(u16) -> Option<Vec<HyperlinkLine>>,
    ) {
        let next_key = active_key.map(|key| LiveTailKey {
            width,
            revision: key.revision,
            is_stream_continuation: key.is_stream_continuation,
            animation_tick: key.animation_tick,
        });

        if active_key.is_some_and(|key| key.cacheable) && self.live_tail_key == next_key {
            return;
        }
        let follow_bottom = self.view.is_scrolled_to_bottom();

        self.take_live_tail_renderable();
        self.live_tail_key = next_key;

        if let Some(key) = next_key {
            let lines = compute_lines(width).unwrap_or_default();
            if !lines.is_empty() {
                self.view.renderables.push(Self::live_tail_renderable(
                    lines,
                    !self.cells.is_empty(),
                    key.is_stream_continuation,
                ));
            }
        }
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
    }

    pub(crate) fn set_highlight_cell(&mut self, cell: Option<usize>) {
        let previous = self.highlight_cell;
        self.highlight_cell = cell;
        // Highlighting changes only these cells' styling. Keep the other renderables and their
        // cached heights so moving between prompts does not lay out the entire transcript again.
        if previous != cell {
            for index in [previous, cell].into_iter().flatten() {
                if let Some(history_cell) = self.cells.get(index) {
                    self.view.renderables[index] = Self::render_cell(
                        &self.cache,
                        history_cell,
                        index,
                        self.highlight_cell,
                        self.history_state,
                    );
                }
            }
        }
        if let Some(idx) = self.highlight_cell {
            self.view.scroll_chunk_into_view(idx);
        }
    }

    /// Returns whether the underlying pager view is currently pinned to the bottom.
    ///
    /// The `App` draw loop uses this to decide whether to schedule animation frames for the live
    /// tail; if the user has scrolled up, we avoid driving animation work that they cannot see.
    pub(crate) fn is_scrolled_to_bottom(&self) -> bool {
        self.view.is_scrolled_to_bottom()
    }

    // Detach the live tail before changing cells: their old count identifies the tail renderable.
    fn rebuild_renderables(&mut self, tail_renderable: Option<Box<dyn Renderable>>) {
        self.view.renderables = Self::render_cells(
            &self.cache,
            &self.cells,
            self.highlight_cell,
            self.history_state,
        );
        if let Some(tail) = tail_renderable {
            self.view.renderables.push(tail);
        }
    }

    /// Removes and returns the cached live-tail renderable, if present.
    ///
    /// The live tail is represented as a single optional renderable appended after the committed
    /// cell renderables, so this relies on the live tail always being the final entry in
    /// `view.renderables` when present.
    fn take_live_tail_renderable(&mut self) -> Option<Box<dyn Renderable>> {
        (self.view.renderables.len() > self.cells.len()).then(|| self.view.renderables.pop())?
    }

    fn live_tail_renderable(
        lines: Vec<HyperlinkLine>,
        has_prior_cells: bool,
        is_stream_continuation: bool,
    ) -> Box<dyn Renderable> {
        let mut renderable: Box<dyn Renderable> =
            Box::new(CachedRenderable::new(HyperlinkLinesRenderable { lines }));
        if has_prior_cells && !is_stream_continuation {
            renderable = Box::new(InsetRenderable::new(
                renderable,
                Insets::tlbr(
                    /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
                ),
            ));
        }
        renderable
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let line1 = Rect::new(area.x, area.y, area.width, 1);
        let line2 = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        render_navigation_hints(line1, buf, &self.view.keymap);

        let mut pairs: Vec<(Vec<ShortcutHint>, &str)> = vec![(
            first_or_empty(&self.view.keymap, "close", &self.view.keymap.close),
            "close",
        )];
        if self.highlight_cell.is_some() {
            pairs.push((
                vec![
                    key_hint::plain(KeyCode::Esc).into(),
                    key_hint::plain(KeyCode::Left).into(),
                ],
                "to edit prev",
            ));
            pairs.push((vec![key_hint::plain(KeyCode::Right).into()], "to edit next"));
            pairs.push((
                vec![key_hint::plain(KeyCode::Enter).into()],
                "to edit message",
            ));
        } else {
            pairs.push((vec![key_hint::plain(KeyCode::Esc).into()], "to edit prev"));
        }
        render_key_hints(line2, buf, &pairs);
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.cache.borrow_mut().begin_frame();
        // Preserve following the tail before the composer changes the available height.
        if self.view.is_scrolled_to_bottom() {
            self.view.scroll_offset = usize::MAX;
        }
        let top_h = area.height.saturating_sub(3);
        let top = Rect::new(area.x, area.y, area.width, top_h);
        let bottom = Rect::new(area.x, area.y + top_h, area.width, 3);
        self.view.render(top, buf);
        self.render_history_state(top, buf);
        self.render_hints(bottom, buf);
    }

    fn render_history_state(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let label = match self.history_state {
            TranscriptHistoryState::Idle => return,
            TranscriptHistoryState::LoadingOlder | TranscriptHistoryState::LoadingBeginning => {
                " loading older history... "
            }
            TranscriptHistoryState::Partial => " partial history | PgUp for earlier ",
            TranscriptHistoryState::Failed => " history unavailable | PgUp to retry ",
            TranscriptHistoryState::Complete => " start of history ",
        };
        let width = (label.chars().count() as u16).min(area.width);
        let status_area = Rect::new(
            area.right().saturating_sub(width),
            area.y,
            width,
            /*height*/ 1,
        );
        Span::from(label).dim().render(status_area, buf);
    }
}

impl TranscriptOverlay {
    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => match key_event {
                e if self.view.keymap.close.is_pressed(e)
                    || self.view.keymap.close_transcript.is_pressed(e) =>
                {
                    self.is_done = true;
                    Ok(())
                }
                other => self.view.handle_key_event(tui, other),
            },
            TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
                tui.draw(u16::MAX, |frame| {
                    self.render(frame.area(), frame.buffer);
                })?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod highlight_tests;
