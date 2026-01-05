//! Caches for transcript rendering in `codex-tui2`.
//!
//! The inline transcript view is drawn every frame. Two parts of that draw can
//! be expensive in steady state:
//!
//! - Building the *wrapped transcript* (`HistoryCell` → flattened `Line`s +
//!   per-line metadata). This work is needed for rendering and for scroll math.
//! - Rendering each visible `Line` into the frame buffer. Ratatui's rendering
//!   path performs grapheme segmentation and width/layout work; repeatedly
//!   rerendering the same visible lines can dominate CPU during streaming.
//!
//! This module provides a pair of caches:
//!
//! - [`WrappedTranscriptCache`] memoizes the wrapped transcript for a given
//!   terminal width and supports incremental append when new history cells are
//!   added.
//! - [`TranscriptRasterCache`] memoizes the *rasterized* representation of
//!   individual wrapped lines (a single terminal row of `Cell`s) so redraws can
//!   cheaply copy already-rendered cells instead of re-running grapheme
//!   segmentation for every frame.
//!
//! Notes:
//! - All caches are invalidated on width changes because wrapping and layout
//!   depend on the viewport width.
//! - Rasterization is cached for base transcript content only; selection
//!   highlight and copy affordances are applied after the rows are drawn, so
//!   they do not pollute the cache.
//!
//! ## Algorithm overview
//!
//! At a high level, transcript rendering is a two-stage pipeline:
//!
//! 1. **Build wrapped transcript lines**: flatten the logical `HistoryCell` list into a single
//!    vector of visual [`Line`]s and a parallel `meta` vector (`TranscriptLineMeta`) that maps each
//!    visual line back to `(cell_index, line_in_cell)` or `Spacer`.
//! 2. **Render visible lines into the frame buffer**: draw the subset of wrapped lines that are
//!    currently visible in the viewport.
//!
//! The cache mirrors that pipeline:
//!
//! - [`WrappedTranscriptCache`] memoizes stage (1) for the current `width` and supports incremental
//!   append when new cells are pushed during streaming.
//! - [`TranscriptRasterCache`] memoizes stage (2) per line by caching the final rendered row
//!   (`Vec<Cell>`) for a given `(line_index, is_user_row)` at the current `width`.
//!
//! ### Per draw tick
//!
//! Callers typically do the following during a draw tick:
//!
//! 1. Call [`TranscriptViewCache::ensure_wrapped`] with the current `cells` and viewport `width`.
//!    This may append new cells or rebuild from scratch (on width change/truncation/replacement).
//! 2. Use [`TranscriptViewCache::lines`] and [`TranscriptViewCache::line_meta`] for scroll math and
//!    to resolve the visible `line_index` range.
//! 3. Configure row caching via [`TranscriptViewCache::set_raster_capacity`] (usually a few
//!    viewports worth).
//! 4. For each visible `line_index`, call [`TranscriptViewCache::render_row_index_into`] to draw a
//!    single terminal row.
//!
//! ### Rasterization details
//!
//! `render_row_index_into` delegates to `TranscriptRasterCache::render_row_into`:
//!
//! - On a **cache hit**, it copies cached cells into the destination buffer (no grapheme
//!   segmentation, no span layout).
//! - On a **cache miss**, it renders the wrapped [`Line`] into a scratch `Buffer` with height 1,
//!   copies out the resulting cells, inserts them into the cache, and then copies them into the
//!   destination buffer.
//!
//! Cached rows are invalidated when:
//! - the wrapped transcript is rebuilt (line indices shift)
//! - the width changes (layout changes)
//!
//! The raster cache is bounded by `capacity` using an approximate LRU so it does not grow without
//! bound during long sessions.

use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::transcript_render::TranscriptLines;
use crate::tui::scrolling::TranscriptLineMeta;
use ratatui::buffer::Buffer;
use ratatui::prelude::Rect;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

/// Top-level cache for the inline transcript viewport.
///
/// This combines two caches that are used together during a draw tick:
///
/// - [`WrappedTranscriptCache`] produces the flattened wrapped transcript lines and metadata used
///   for rendering, scrolling, and selection/copy mapping.
/// - [`TranscriptRasterCache`] caches the expensive conversion from a wrapped [`Line`] into a row
///   of terminal [`ratatui::buffer::Cell`]s so repeated redraws can copy cells instead of redoing
///   grapheme segmentation.
///
/// The caches are intentionally coupled:
/// - width changes invalidate both layers
/// - wrapped transcript rebuilds invalidate the raster cache because line indices shift
pub(crate) struct TranscriptViewCache {
    /// Memoized flattened wrapped transcript content for the current width.
    wrapped: WrappedTranscriptCache,
    /// Per-line row rasterization cache for the current width.
    raster: TranscriptRasterCache,
}

impl TranscriptViewCache {
    /// Create an empty transcript view cache.
    pub(crate) fn new() -> Self {
        Self {
            wrapped: WrappedTranscriptCache::new(),
            raster: TranscriptRasterCache::new(),
        }
    }

    /// Ensure the wrapped transcript cache is up to date for `cells` at `width`.
    ///
    /// This is the shared entrypoint for the transcript renderer and scroll math. It ensures the
    /// cache reflects the current transcript and viewport width while preserving scroll/copy
    /// invariants (`lines`, `meta`, and `joiner_before` remain aligned).
    ///
    /// Rebuild conditions:
    /// - `width` changes (wrapping/layout is width-dependent)
    /// - the transcript is truncated (fewer `cells` than last time), which means the previously
    ///   cached suffix may refer to cells that no longer exist and the cached `(cell_index,
    ///   line_in_cell)` mapping is no longer valid. In `tui2` today, this happens when the user
    ///   backtracks/forks a conversation: `app_backtrack` trims `App::transcript_cells` to preserve
    ///   only content up to the selected user message.
    /// - the transcript is replaced (detected by a change in the first cell pointer), which
    ///   commonly happens when history is rotated/dropped from the front while keeping a similar
    ///   length (e.g. to cap history size) or when switching to a different transcript. We don't
    ///   currently replace the transcript list in the main render loop, but we keep this guard so
    ///   future history-capping or transcript-reload features can't accidentally treat a shifted
    ///   list as an append. In that case, treating the new list as an append would misattribute
    ///   line origins and break scroll anchors and selection/copy mapping.
    ///
    /// The raster cache is invalidated whenever the wrapped transcript is rebuilt or the width no
    /// longer matches.
    pub(crate) fn ensure_wrapped(&mut self, cells: &[Arc<dyn HistoryCell>], width: u16) {
        let update = self.wrapped.ensure(cells, width);
        if update == WrappedTranscriptUpdate::Rebuilt {
            self.raster.width = width;
            self.raster.clear();
        } else if width != self.raster.width {
            // Keep the invariant that raster cache always matches the active wrapped width.
            self.raster.clear();
            self.raster.width = width;
        }
    }

    /// Return the cached flattened wrapped transcript lines.
    ///
    /// This is primarily used for:
    /// - computing `total_lines` for scroll/viewport logic
    /// - any code that needs a read-only view of the current flattened transcript
    ///
    /// Callers should generally avoid iterating these lines to render them in the draw hot path;
    /// use [`Self::render_row_index_into`] so redraws can take advantage of the raster cache.
    pub(crate) fn lines(&self) -> &[Line<'static>] {
        &self.wrapped.transcript.lines
    }

    /// Return per-line origin metadata aligned with [`Self::lines`].
    ///
    /// This mapping is what makes scroll/selection stable as the transcript grows and reflows:
    /// each visible line index can be mapped back to the originating `(cell_index, line_in_cell)`
    /// pair (or to a `Spacer` row).
    ///
    /// Typical uses:
    /// - scroll anchoring (`TranscriptScroll` resolves/anchors using this metadata)
    /// - determining whether a visible row is a user-authored row (`cell_index → is_user_cell`)
    pub(crate) fn line_meta(&self) -> &[TranscriptLineMeta] {
        &self.wrapped.transcript.meta
    }

    /// Configure the per-line raster cache capacity.
    ///
    /// When `capacity == 0`, raster caching is disabled and rows are rendered directly into the
    /// destination buffer (but wrapped transcript caching still applies).
    pub(crate) fn set_raster_capacity(&mut self, capacity: usize) {
        self.raster.set_capacity(capacity);
    }

    /// Whether a flattened transcript line belongs to a user-authored history cell.
    ///
    /// User rows apply a row-wide base style (background). This is a property of the originating
    /// cell, not of the line content, so it is derived from the cached `line_meta` mapping.
    pub(crate) fn is_user_row(&self, line_index: usize) -> bool {
        let Some(cell_index) = self
            .wrapped
            .transcript
            .meta
            .get(line_index)
            .and_then(TranscriptLineMeta::cell_index)
        else {
            return false;
        };

        self.wrapped
            .is_user_cell
            .get(cell_index)
            .copied()
            .unwrap_or(false)
    }

    /// Render a single cached line index into the destination `buf`.
    ///
    /// This is the draw hot-path helper: it looks up the wrapped `Line` for `line_index`, applies
    /// user-row styling if needed, and then either rasterizes the line or copies cached cells into
    /// place.
    ///
    /// Callers are expected to have already ensured the cache via [`Self::ensure_wrapped`].
    pub(crate) fn render_row_index_into(
        &mut self,
        line_index: usize,
        row_area: Rect,
        buf: &mut Buffer,
    ) {
        let is_user_row = self.is_user_row(line_index);
        let line = &self.wrapped.transcript.lines[line_index];
        self.raster
            .render_row_into(line_index, is_user_row, line, row_area, buf);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WrappedTranscriptUpdate {
    /// The cache already represented the provided `cells` and `width`.
    Unchanged,
    /// The cache appended additional cells without rebuilding.
    Appended,
    /// The cache rebuilt from scratch (width change, truncation, or replacement).
    Rebuilt,
}

/// Incremental memoization of wrapped transcript lines for a given width.
///
/// This cache exists so callers doing tight-loop scroll math (mouse wheel, PgUp/PgDn) and render
/// ticks do not repeatedly rebuild the wrapped transcript (`HistoryCell` → flattened `Line`s).
///
/// It assumes the transcript is append-mostly: when new cells arrive, they are appended to the end
/// of `cells` and existing cells do not mutate. If the underlying cell list is replaced or
/// truncated, the cache rebuilds from scratch.
struct WrappedTranscriptCache {
    /// Width this cache was last built for.
    width: u16,
    /// Number of leading cells already incorporated into [`Self::transcript`].
    cell_count: usize,
    /// Pointer identity of the first cell at the time the cache was built.
    ///
    /// This is a cheap replacement/truncation detector: if the caller swaps the transcript list
    /// (for example, drops old cells from the front to cap history length), the length may remain
    /// the same while the content shifts. In that case, we must rebuild because `(cell_index,
    /// line_in_cell)` mappings and scroll anchors would otherwise become inconsistent.
    first_cell_ptr: Option<*const dyn HistoryCell>,
    /// Cached flattened wrapped transcript output.
    ///
    /// Invariant: `lines.len() == meta.len() == joiner_before.len()`.
    transcript: TranscriptLines,
    /// Whether the flattened transcript has emitted at least one non-spacer line.
    ///
    /// This is used to decide whether to insert a spacer line between non-continuation cells.
    has_emitted_lines: bool,
    /// Per-cell marker indicating whether a logical cell is a [`UserHistoryCell`].
    ///
    /// We store this alongside the wrapped transcript so user-row styling can be derived cheaply
    /// from `TranscriptLineMeta::cell_index()` without re-inspecting the cell type every frame.
    is_user_cell: Vec<bool>,
}

impl WrappedTranscriptCache {
    /// Create an empty wrapped transcript cache.
    ///
    /// The cache is inert until the first [`Self::ensure`] call; until then it contains no
    /// rendered transcript state.
    fn new() -> Self {
        Self {
            width: 0,
            cell_count: 0,
            first_cell_ptr: None,
            transcript: TranscriptLines {
                lines: Vec::new(),
                meta: Vec::new(),
                joiner_before: Vec::new(),
            },
            has_emitted_lines: false,
            is_user_cell: Vec::new(),
        }
    }

    /// Ensure the wrapped transcript represents `cells` at `width`.
    ///
    /// This cache is intentionally single-entry and width-scoped:
    /// - when `width` is unchanged and `cells` has grown, append only the new cells
    /// - when `width` changes or the transcript is replaced/truncated, rebuild from scratch
    ///
    /// The cache assumes history cells are append-only and immutable once inserted. If existing
    /// cell contents can change without changing identity, callers must treat that as a rebuild.
    fn ensure(&mut self, cells: &[Arc<dyn HistoryCell>], width: u16) -> WrappedTranscriptUpdate {
        if width == 0 {
            self.width = width;
            self.cell_count = cells.len();
            self.first_cell_ptr = cells.first().map(Arc::as_ptr);
            self.transcript.lines.clear();
            self.transcript.meta.clear();
            self.transcript.joiner_before.clear();
            self.has_emitted_lines = false;
            self.is_user_cell.clear();
            return WrappedTranscriptUpdate::Rebuilt;
        }

        let current_first_ptr = cells.first().map(Arc::as_ptr);
        if self.width != width
            || self.cell_count > cells.len()
            || (self.cell_count > 0
                && current_first_ptr.is_some()
                && self.first_cell_ptr != current_first_ptr)
        {
            self.rebuild(cells, width);
            return WrappedTranscriptUpdate::Rebuilt;
        }

        if self.cell_count == cells.len() {
            return WrappedTranscriptUpdate::Unchanged;
        }

        let old_cell_count = self.cell_count;
        self.cell_count = cells.len();
        self.first_cell_ptr = current_first_ptr;
        let base_opts: crate::wrapping::RtOptions<'_> =
            crate::wrapping::RtOptions::new(width.max(1) as usize);
        for (cell_index, cell) in cells.iter().enumerate().skip(old_cell_count) {
            self.is_user_cell
                .push(cell.as_any().is::<UserHistoryCell>());
            crate::transcript_render::append_wrapped_transcript_cell(
                &mut self.transcript,
                &mut self.has_emitted_lines,
                cell_index,
                cell,
                width,
                &base_opts,
            );
        }

        WrappedTranscriptUpdate::Appended
    }

    /// Rebuild the wrapped transcript cache from scratch.
    ///
    /// This is used when width changes, the transcript is truncated, or the caller provides a new
    /// cell list that cannot be treated as an append to the previous one.
    fn rebuild(&mut self, cells: &[Arc<dyn HistoryCell>], width: u16) {
        self.width = width;
        self.cell_count = cells.len();
        self.first_cell_ptr = cells.first().map(Arc::as_ptr);
        self.transcript.lines.clear();
        self.transcript.meta.clear();
        self.transcript.joiner_before.clear();
        self.has_emitted_lines = false;
        self.is_user_cell.clear();
        self.is_user_cell.reserve(cells.len());

        let base_opts: crate::wrapping::RtOptions<'_> =
            crate::wrapping::RtOptions::new(width.max(1) as usize);
        for (cell_index, cell) in cells.iter().enumerate() {
            self.is_user_cell
                .push(cell.as_any().is::<UserHistoryCell>());
            crate::transcript_render::append_wrapped_transcript_cell(
                &mut self.transcript,
                &mut self.has_emitted_lines,
                cell_index,
                cell,
                width,
                &base_opts,
            );
        }
    }
}

/// Bounded cache of rasterized transcript rows.
///
/// Each cached entry stores the final rendered [`ratatui::buffer::Cell`] values for a single
/// transcript line rendered into a 1-row buffer.
///
/// Keying:
/// - The cache key includes `(line_index, is_user_row)`.
/// - Width is stored out-of-band and any width change clears the cache.
///
/// Eviction:
/// - The cache uses an approximate LRU implemented with a monotonic stamp (`clock`) and an
///   `(key, stamp)` queue.
/// - This avoids per-access list manipulation while still keeping memory bounded.
struct TranscriptRasterCache {
    /// Width this cache's rasterized rows were rendered at.
    width: u16,
    /// Maximum number of rasterized rows to retain.
    capacity: usize,
    /// Monotonic counter used to stamp accesses for eviction.
    clock: u64,
    /// Version of the terminal palette used for the cached rows.
    palette_version: u64,
    /// Access log used for approximate LRU eviction.
    lru: VecDeque<(u64, u64)>,
    /// Cached rasterized rows by key.
    rows: HashMap<u64, RasterizedRow>,
}

/// Cached raster for a single transcript line at a particular width.
#[derive(Clone)]
struct RasterizedRow {
    /// The last access stamp recorded for this row.
    ///
    /// Eviction only removes a row when a popped `(key, stamp)` matches this value.
    last_used: u64,
    /// The full row of rendered cells (length is `width` at the time of rasterization).
    cells: Vec<ratatui::buffer::Cell>,
}

impl TranscriptRasterCache {
    /// Create an empty raster cache (caching disabled until a non-zero capacity is set).
    fn new() -> Self {
        Self {
            width: 0,
            capacity: 0,
            clock: 0,
            palette_version: crate::terminal_palette::palette_version(),
            lru: VecDeque::new(),
            rows: HashMap::new(),
        }
    }

    /// Drop all cached rasterized rows and reset access tracking.
    ///
    /// This is used on width changes and when disabling caching so we don't retain stale rows or
    /// unbounded memory.
    fn clear(&mut self) {
        self.lru.clear();
        self.rows.clear();
        self.clock = 0;
    }

    /// Set the maximum number of cached rasterized rows.
    ///
    /// When set to 0, caching is disabled and any existing cached rows are dropped.
    fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        self.evict_if_needed();
    }

    /// Render a single wrapped transcript line into `buf`, using a cached raster when possible.
    ///
    /// The cache key includes `is_user_row` because user rows apply a row-wide base style, so the
    /// final raster differs even when the text spans are identical.
    fn render_row_into(
        &mut self,
        line_index: usize,
        is_user_row: bool,
        line: &Line<'static>,
        row_area: Rect,
        buf: &mut Buffer,
    ) {
        if row_area.width == 0 || row_area.height == 0 {
            return;
        }

        let palette_version = crate::terminal_palette::palette_version();
        if palette_version != self.palette_version {
            self.palette_version = palette_version;
            self.clear();
        }

        if self.width != row_area.width {
            self.width = row_area.width;
            self.clear();
        }

        if self.capacity == 0 {
            let cells = rasterize_line(line, row_area.width, is_user_row);
            copy_row(row_area, buf, &cells);
            return;
        }

        let key = raster_key(line_index, is_user_row);
        let stamp = self.bump_clock();
        if let Some(row) = self.rows.get_mut(&key) {
            row.last_used = stamp;
            self.lru.push_back((key, stamp));
            copy_row(row_area, buf, &row.cells);
            return;
        }

        let cells = rasterize_line(line, row_area.width, is_user_row);
        copy_row(row_area, buf, &cells);
        self.rows.insert(
            key,
            RasterizedRow {
                last_used: stamp,
                cells,
            },
        );
        self.lru.push_back((key, stamp));
        self.evict_if_needed();
    }

    /// Return a new access stamp.
    ///
    /// The stamp is used only for equality checks ("is this the latest access for this key?") so a
    /// wrapping counter is sufficient; `u64` wraparound is effectively unreachable in practice for
    /// a UI cache.
    fn bump_clock(&mut self) -> u64 {
        let stamp = self.clock;
        self.clock = self.clock.wrapping_add(1);
        stamp
    }

    /// Evict old cached rows until `rows.len() <= capacity`.
    ///
    /// The cache uses an approximate LRU: we push `(key, stamp)` on every access, and only evict a
    /// row when the popped entry matches the row's current `last_used` stamp.
    fn evict_if_needed(&mut self) {
        if self.capacity == 0 {
            self.clear();
            return;
        }
        while self.rows.len() > self.capacity {
            let Some((key, stamp)) = self.lru.pop_front() else {
                break;
            };
            if self
                .rows
                .get(&key)
                .is_some_and(|row| row.last_used == stamp)
            {
                self.rows.remove(&key);
            }
        }
    }
}

/// Compute the cache key for a rasterized transcript row.
///
/// We key by `line_index` (not by hashing line content) because:
/// - it is effectively free in the draw loop
/// - the wrapped transcript cache defines a stable `(index → Line)` mapping until the next rebuild
/// - rebuilds clear the raster cache, so indices cannot alias across different transcripts
///
/// `is_user_row` is included because user rows apply a row-wide base style that affects every cell.
fn raster_key(line_index: usize, is_user_row: bool) -> u64 {
    (line_index as u64) << 1 | u64::from(is_user_row)
}

/// Rasterize a single wrapped transcript [`Line`] into a 1-row cell vector.
///
/// This is the expensive step we want to avoid repeating on every redraw: it runs Ratatui's
/// rendering for the line (including grapheme segmentation) into a scratch buffer and then copies
/// out the rendered cells.
///
/// For user rows, we pre-fill the row with the base user style so the cached raster includes the
/// full-width background, matching the viewport behavior.
fn rasterize_line(
    line: &Line<'static>,
    width: u16,
    is_user_row: bool,
) -> Vec<ratatui::buffer::Cell> {
    let scratch_area = Rect::new(0, 0, width, 1);
    let mut scratch = Buffer::empty(scratch_area);

    if is_user_row {
        let base_style = crate::style::user_message_style();
        for x in 0..width {
            scratch[(x, 0)].set_style(base_style);
        }
    }

    line.render_ref(scratch_area, &mut scratch);

    let mut out = Vec::with_capacity(width as usize);
    for x in 0..width {
        out.push(scratch[(x, 0)].clone());
    }
    out
}

/// Copy a cached rasterized row into a destination buffer at `area`.
///
/// This is the "fast path" for redraws: once a row is cached, a redraw copies the pre-rendered
/// cells into the frame buffer without re-running span layout/grapheme segmentation.
fn copy_row(area: Rect, buf: &mut Buffer, cells: &[ratatui::buffer::Cell]) {
    let y = area.y;
    for (dx, cell) in cells.iter().enumerate() {
        let x = area.x.saturating_add(dx as u16);
        if x >= area.right() {
            break;
        }
        buf[(x, y)] = cell.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::TranscriptLinesWithJoiners;
    use crate::history_cell::UserHistoryCell;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use ratatui::style::Style;
    use ratatui::style::Stylize;
    use ratatui::text::Span;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[derive(Debug)]
    struct FakeCell {
        lines: Vec<Line<'static>>,
        joiner_before: Vec<Option<String>>,
        is_stream_continuation: bool,
        transcript_calls: Arc<AtomicUsize>,
    }

    impl FakeCell {
        fn new(
            lines: Vec<Line<'static>>,
            joiner_before: Vec<Option<String>>,
            is_stream_continuation: bool,
            transcript_calls: Arc<AtomicUsize>,
        ) -> Self {
            Self {
                lines,
                joiner_before,
                is_stream_continuation,
                transcript_calls,
            }
        }
    }

    impl HistoryCell for FakeCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn transcript_lines_with_joiners(&self, _width: u16) -> TranscriptLinesWithJoiners {
            self.transcript_calls.fetch_add(1, Ordering::Relaxed);
            TranscriptLinesWithJoiners {
                lines: self.lines.clone(),
                joiner_before: self.joiner_before.clone(),
            }
        }

        fn is_stream_continuation(&self) -> bool {
            self.is_stream_continuation
        }
    }

    #[test]
    fn wrapped_cache_matches_build_wrapped_transcript_lines() {
        let calls0 = Arc::new(AtomicUsize::new(0));
        let calls1 = Arc::new(AtomicUsize::new(0));
        let calls2 = Arc::new(AtomicUsize::new(0));

        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            // Wrapping case: expect a soft-wrap joiner for the continuation segment.
            Arc::new(FakeCell::new(
                vec![Line::from("• hello world")],
                vec![None],
                false,
                calls0,
            )),
            // Preformatted (cyan) lines are not wrapped by the viewport wrapper.
            Arc::new(FakeCell::new(
                vec![Line::from("    let x = 12345;").cyan()],
                vec![None],
                true,
                calls1,
            )),
            // New non-continuation cell inserts a spacer.
            Arc::new(FakeCell::new(
                vec![Line::from("• foo bar")],
                vec![None],
                false,
                calls2,
            )),
        ];

        let width = 8;
        let expected = crate::transcript_render::build_wrapped_transcript_lines(&cells, width);

        let mut cache = TranscriptViewCache::new();
        cache.ensure_wrapped(&cells, width);

        assert_eq!(cache.lines(), expected.lines.as_slice());
        assert_eq!(cache.line_meta(), expected.meta.as_slice());
        assert_eq!(
            cache.wrapped.transcript.joiner_before,
            expected.joiner_before
        );
        assert_eq!(cache.lines().len(), cache.line_meta().len());
        assert_eq!(
            cache.lines().len(),
            cache.wrapped.transcript.joiner_before.len()
        );
    }

    #[test]
    fn wrapped_cache_ensure_appends_only_new_cells_when_width_is_unchanged() {
        let calls0 = Arc::new(AtomicUsize::new(0));
        let calls1 = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(FakeCell::new(
                vec![Line::from("• hello world")],
                vec![None],
                false,
                calls0.clone(),
            )),
            Arc::new(FakeCell::new(
                vec![Line::from("• foo bar")],
                vec![None],
                false,
                calls1.clone(),
            )),
        ];

        let mut cache = TranscriptViewCache::new();
        cache.ensure_wrapped(&cells[..1], 8);
        cache.ensure_wrapped(&cells, 8);

        assert_eq!(calls0.load(Ordering::Relaxed), 1);
        assert_eq!(calls1.load(Ordering::Relaxed), 1);

        assert_eq!(
            cache.lines(),
            &[
                Line::from("• hello"),
                Line::from("world"),
                Line::from(""),
                Line::from("• foo"),
                Line::from("bar")
            ]
        );
        assert_eq!(
            cache.line_meta(),
            &[
                TranscriptLineMeta::CellLine {
                    cell_index: 0,
                    line_in_cell: 0
                },
                TranscriptLineMeta::CellLine {
                    cell_index: 0,
                    line_in_cell: 1
                },
                TranscriptLineMeta::Spacer,
                TranscriptLineMeta::CellLine {
                    cell_index: 1,
                    line_in_cell: 0
                },
                TranscriptLineMeta::CellLine {
                    cell_index: 1,
                    line_in_cell: 1
                },
            ]
        );
        assert_eq!(
            cache.wrapped.transcript.joiner_before.as_slice(),
            &[
                None,
                Some(" ".to_string()),
                None,
                None,
                Some(" ".to_string()),
            ]
        );
    }

    #[test]
    fn wrapped_cache_ensure_rebuilds_on_width_change() {
        let calls0 = Arc::new(AtomicUsize::new(0));
        let calls1 = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(FakeCell::new(
                vec![Line::from("• hello world")],
                vec![None],
                false,
                calls0.clone(),
            )),
            Arc::new(FakeCell::new(
                vec![Line::from("• foo bar")],
                vec![None],
                false,
                calls1.clone(),
            )),
        ];

        let mut cache = TranscriptViewCache::new();
        cache.ensure_wrapped(&cells, 8);
        cache.ensure_wrapped(&cells, 10);

        assert_eq!(calls0.load(Ordering::Relaxed), 2);
        assert_eq!(calls1.load(Ordering::Relaxed), 2);

        let expected = crate::transcript_render::build_wrapped_transcript_lines(&cells, 10);
        assert_eq!(cache.lines(), expected.lines.as_slice());
        assert_eq!(cache.line_meta(), expected.meta.as_slice());
        assert_eq!(
            cache.wrapped.transcript.joiner_before,
            expected.joiner_before
        );
    }

    #[test]
    fn wrapped_cache_ensure_rebuilds_on_truncation() {
        let calls0 = Arc::new(AtomicUsize::new(0));
        let calls1 = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(FakeCell::new(
                vec![Line::from("• hello world")],
                vec![None],
                false,
                calls0.clone(),
            )),
            Arc::new(FakeCell::new(
                vec![Line::from("• foo bar")],
                vec![None],
                false,
                calls1.clone(),
            )),
        ];

        let mut cache = TranscriptViewCache::new();
        cache.ensure_wrapped(&cells, 8);
        cache.ensure_wrapped(&cells[..1], 8);

        // The second ensure is a rebuild of the truncated prefix; only the first cell is rendered.
        assert_eq!(calls0.load(Ordering::Relaxed), 2);
        assert_eq!(calls1.load(Ordering::Relaxed), 1);

        let expected = crate::transcript_render::build_wrapped_transcript_lines(&cells[..1], 8);
        assert_eq!(cache.lines(), expected.lines.as_slice());
        assert_eq!(cache.line_meta(), expected.meta.as_slice());
    }

    #[test]
    fn wrapped_cache_ensure_with_zero_width_clears_without_calling_cell_render() {
        let calls = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(FakeCell::new(
            vec![Line::from("• hello world")],
            vec![None],
            false,
            calls.clone(),
        ))];

        let mut cache = TranscriptViewCache::new();
        cache.ensure_wrapped(&cells, 0);

        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(cache.lines(), &[]);
        assert_eq!(cache.line_meta(), &[]);
        assert_eq!(
            cache.wrapped.transcript.joiner_before,
            Vec::<Option<String>>::new()
        );
    }

    #[test]
    fn wrapped_cache_ensure_rebuilds_when_first_cell_pointer_changes() {
        let calls_a = Arc::new(AtomicUsize::new(0));
        let calls_b = Arc::new(AtomicUsize::new(0));

        let cell_a0: Arc<dyn HistoryCell> = Arc::new(FakeCell::new(
            vec![Line::from("• a")],
            vec![None],
            false,
            calls_a.clone(),
        ));
        let cell_a1: Arc<dyn HistoryCell> = Arc::new(FakeCell::new(
            vec![Line::from("• b")],
            vec![None],
            false,
            calls_b.clone(),
        ));

        let mut cache = TranscriptViewCache::new();
        cache.ensure_wrapped(&[cell_a0.clone(), cell_a1.clone()], 10);
        assert_eq!(calls_a.load(Ordering::Relaxed), 1);
        assert_eq!(calls_b.load(Ordering::Relaxed), 1);

        // Replace the transcript with a different first cell but keep the length the same.
        let calls_c = Arc::new(AtomicUsize::new(0));
        let cell_b0: Arc<dyn HistoryCell> = Arc::new(FakeCell::new(
            vec![Line::from("• c")],
            vec![None],
            false,
            calls_c.clone(),
        ));

        cache.ensure_wrapped(&[cell_b0.clone(), cell_a1.clone()], 10);

        // This should be treated as a replacement and rebuilt from scratch.
        assert_eq!(calls_c.load(Ordering::Relaxed), 1);
        assert_eq!(calls_b.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn raster_cache_reuses_rows_and_clears_on_width_change() {
        let mut cache = TranscriptViewCache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(FakeCell::new(
            vec![Line::from(vec![
                Span::from("• hello").style(Style::default().fg(Color::Magenta)),
            ])],
            vec![None],
            false,
            calls,
        ))];

        cache.ensure_wrapped(&cells, 20);
        cache.set_raster_capacity(8);

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);

        cache.render_row_index_into(0, area, &mut buf);
        assert_eq!(cache.raster.rows.len(), 1);

        cache.render_row_index_into(0, area, &mut buf);
        assert_eq!(cache.raster.rows.len(), 1);

        let mut buf_wide = Buffer::empty(Rect::new(0, 0, 12, 1));
        cache.render_row_index_into(0, Rect::new(0, 0, 12, 1), &mut buf_wide);
        assert_eq!(cache.raster.width, 12);
        assert_eq!(cache.raster.rows.len(), 1);
    }

    fn direct_render_cells(
        line: &Line<'static>,
        width: u16,
        is_user_row: bool,
    ) -> Vec<ratatui::buffer::Cell> {
        let area = Rect::new(0, 0, width, 1);
        let mut scratch = Buffer::empty(area);
        if is_user_row {
            let base_style = crate::style::user_message_style();
            for x in 0..width {
                scratch[(x, 0)].set_style(base_style);
            }
        }
        line.render_ref(area, &mut scratch);
        (0..width).map(|x| scratch[(x, 0)].clone()).collect()
    }

    #[test]
    fn rasterize_line_matches_direct_render_for_user_and_non_user_rows() {
        let width = 12;
        let line = Line::from(vec!["hello".into(), " ".into(), "world".magenta()]);

        let non_user = rasterize_line(&line, width, false);
        assert_eq!(non_user, direct_render_cells(&line, width, false));

        let user = rasterize_line(&line, width, true);
        assert_eq!(user, direct_render_cells(&line, width, true));
    }

    #[test]
    fn raster_cache_evicts_old_rows_when_over_capacity() {
        let mut cache = TranscriptViewCache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(FakeCell::new(
            vec![Line::from("first"), Line::from("second")],
            vec![None, None],
            false,
            calls,
        ))];

        cache.ensure_wrapped(&cells, 10);
        cache.set_raster_capacity(1);

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);

        cache.render_row_index_into(0, area, &mut buf);
        assert_eq!(cache.raster.rows.len(), 1);
        assert!(cache.raster.rows.contains_key(&raster_key(0, false)));

        cache.render_row_index_into(1, area, &mut buf);
        assert_eq!(cache.raster.rows.len(), 1);
        assert!(cache.raster.rows.contains_key(&raster_key(1, false)));
    }

    #[test]
    fn raster_cache_resets_when_palette_version_changes() {
        let mut cache = TranscriptViewCache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(FakeCell::new(
            vec![Line::from("palette")],
            vec![None],
            false,
            calls,
        ))];

        cache.ensure_wrapped(&cells, 20);
        cache.set_raster_capacity(1);

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);

        cache.render_row_index_into(0, area, &mut buf);
        assert_eq!(cache.raster.clock, 1);

        cache.render_row_index_into(0, area, &mut buf);
        assert_eq!(cache.raster.clock, 2);

        crate::terminal_palette::requery_default_colors();
        cache.render_row_index_into(0, area, &mut buf);
        assert_eq!(cache.raster.clock, 1);
    }

    #[test]
    fn render_row_index_into_treats_user_history_cells_as_user_rows() {
        let mut cache = TranscriptViewCache::new();
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(UserHistoryCell {
            message: "hello".to_string(),
        })];

        cache.ensure_wrapped(&cells, 20);
        cache.set_raster_capacity(8);

        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);

        cache.render_row_index_into(0, area, &mut buf);
        assert!(cache.is_user_row(0));
        assert!(cache.raster.rows.contains_key(&raster_key(0, true)));
    }
}
