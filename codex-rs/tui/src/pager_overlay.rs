//! Overlay UIs rendered in an alternate screen.
//!
//! This module implements the pager-style overlays used by the TUI, including the transcript
//! overlay (`Ctrl+T`) that renders a full history view separate from the main viewport.
//!
//! The transcript overlay renders committed transcript cells plus an optional render-only live tail
//! derived from the current in-flight active cell. Because rebuilding wrapped `Line`s on every draw
//! can be expensive, that live tail is cached and only recomputed when its cache key changes, which
//! is derived from the terminal width (wrapping), an active-cell revision (in-place mutations), the
//! stream-continuation flag (spacing), and an animation tick (time-based spinner/shimmer output).
//!
//! The transcript overlay live tail is kept in sync by `App` during draws: `App` supplies an
//! `ActiveCellTranscriptKey` and a function to compute the active cell transcript lines, and
//! `TranscriptOverlay::sync_live_tail` uses the key to decide when the cached tail must be
//! recomputed. `ChatWidget` is responsible for producing a key that changes when the active cell
//! mutates in place or when its transcript output is time-dependent.

use std::any::TypeId;
use std::cell::Cell as StdCell;
use std::cell::RefCell;
use std::io::Result;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ActiveCellTranscriptKey;
use crate::chatwidget::CopyStatus;
use crate::footer_hints::FooterHint;
use crate::footer_hints::first_fitting_right_label;
use crate::footer_hints::footer_hint_line_for_row;
use crate::footer_hints::render_footer_line_with_optional_right;
use crate::footer_hints::render_footer_separator;
use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::key_hint::is_plain_text_key_event;
use crate::keymap::PagerKeymap;
use crate::pager_overlay_search::SearchCorpus;
use crate::pager_overlay_search::SearchCorpusKey;
use crate::pager_overlay_search::SearchMatch;
use crate::pager_overlay_search::transcript_search_lines;
use crate::render::Insets;
use crate::render::renderable::InsetRenderable;
use crate::render::renderable::Renderable;
use crate::style::accent_style;
use crate::style::user_message_style;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::mark_buffer_hyperlinks;
use crate::terminal_hyperlinks::visible_lines;
use crate::transcript_layout::TranscriptLayoutKey;
use crate::transcript_layout::TranscriptLayoutResult;
use crate::transcript_layout::spawn_transcript_layout_worker;
use crate::tui;
use crate::tui::TuiEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthChar;

pub(crate) enum Overlay {
    Transcript(Box<TranscriptOverlay>),
    Static(Box<StaticOverlay>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptOverlayState {
    pub(crate) scroll_offset: usize,
    pub(crate) highlight_cell: Option<usize>,
    pub(crate) render_mode: HistoryRenderMode,
}

impl TranscriptOverlayState {
    pub(crate) fn new(render_mode: HistoryRenderMode) -> Self {
        Self {
            scroll_offset: usize::MAX,
            highlight_cell: None,
            render_mode,
        }
    }
}

impl Overlay {
    pub(crate) fn new_transcript(
        cells: Vec<Arc<dyn HistoryCell>>,
        keymap: PagerKeymap,
        copy_keymap: Vec<KeyBinding>,
        toggle_raw_output_keymap: Vec<KeyBinding>,
        state: TranscriptOverlayState,
    ) -> Self {
        Self::Transcript(Box::new(TranscriptOverlay::new(
            cells,
            keymap,
            copy_keymap,
            toggle_raw_output_keymap,
            state,
        )))
    }

    pub(crate) fn new_static_with_lines(
        lines: Vec<Line<'static>>,
        title: String,
        keymap: PagerKeymap,
    ) -> Self {
        Self::Static(Box::new(StaticOverlay::with_title(lines, title, keymap)))
    }

    pub(crate) fn new_static_with_renderables(
        renderables: Vec<Box<dyn Renderable>>,
        title: String,
        keymap: PagerKeymap,
    ) -> Self {
        Self::Static(Box::new(StaticOverlay::with_renderables(
            renderables,
            title,
            keymap,
        )))
    }

    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match self {
            Overlay::Transcript(o) => o.handle_event(tui, event),
            Overlay::Static(o) => o.handle_event(tui, event),
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        match self {
            Overlay::Transcript(o) => o.is_done(),
            Overlay::Static(o) => o.is_done(),
        }
    }
}

fn first_or_empty(bindings: &[KeyBinding]) -> Vec<KeyBinding> {
    bindings.first().copied().into_iter().collect()
}

fn key_label(bindings: &[KeyBinding]) -> String {
    bindings
        .iter()
        .map(KeyBinding::display_label)
        .collect::<Vec<_>>()
        .join("/")
}

/// Generic widget for rendering a pager view.
struct PagerView {
    renderables: Vec<Box<dyn Renderable>>,
    scroll_offset: usize,
    title: String,
    footer_separator_label: String,
    show_header_progress: bool,
    keymap: PagerKeymap,
    last_content_height: Option<usize>,
    last_rendered_height: Option<usize>,
    layout: Option<PagerLayout>,
    /// If set, on next render position this row of the chunk near the upper third.
    pending_anchor_chunk: Option<(usize, usize)>,
    /// If set, on next render ensure this search-match line is visible.
    pending_search_line: Option<(usize, usize)>,
}

#[derive(Debug)]
struct PagerLayout {
    width: u16,
    offsets: Arc<[usize]>,
    heights: Arc<[usize]>,
    total_height: usize,
}

impl PagerView {
    fn new(
        renderables: Vec<Box<dyn Renderable>>,
        title: String,
        scroll_offset: usize,
        keymap: PagerKeymap,
    ) -> Self {
        Self {
            renderables,
            scroll_offset,
            title,
            footer_separator_label: String::new(),
            show_header_progress: true,
            keymap,
            last_content_height: None,
            last_rendered_height: None,
            layout: None,
            pending_anchor_chunk: None,
            pending_search_line: None,
        }
    }

    fn invalidate_layout(&mut self) {
        self.layout = None;
    }

    fn content_height(&mut self, width: u16) -> usize {
        self.layout(width).total_height
    }

    fn install_precomputed_layout(&mut self, width: u16, heights: Vec<usize>) {
        let mut offsets = Vec::with_capacity(heights.len());
        let mut total_height = 0usize;
        for height in &heights {
            offsets.push(total_height);
            total_height = total_height.saturating_add(*height);
        }
        self.layout = Some(PagerLayout {
            width,
            offsets: offsets.into(),
            heights: heights.into(),
            total_height,
        });
    }

    fn layout(&mut self, width: u16) -> &PagerLayout {
        let needs_rebuild = self.layout.as_ref().is_none_or(|layout| {
            layout.width != width || layout.heights.len() != self.renderables.len()
        });
        if needs_rebuild {
            let mut offsets = Vec::with_capacity(self.renderables.len());
            let mut heights = Vec::with_capacity(self.renderables.len());
            let mut total_height = 0usize;
            for renderable in &self.renderables {
                offsets.push(total_height);
                let height = renderable.desired_height(width) as usize;
                heights.push(height);
                total_height = total_height.saturating_add(height);
            }
            self.layout = Some(PagerLayout {
                width,
                offsets: offsets.into(),
                heights: heights.into(),
                total_height,
            });
        }
        match self.layout.as_ref() {
            Some(layout) => layout,
            None => unreachable!("pager layout missing after rebuild"),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let content_area = self.content_area(area);
        self.update_last_content_height(content_area.height);
        let content_height = self.content_height(content_area.width);
        self.last_rendered_height = Some(content_height);
        self.resolve_pending_scroll(content_area, content_height);
        self.render_header(area, content_area, buf, content_height);
        self.render_content(content_area, buf);

        self.render_bottom_bar(area, content_area, buf, content_height);
    }

    fn render_header(&self, area: Rect, content_area: Rect, buf: &mut Buffer, total_len: usize) {
        let header = Rect::new(area.x, area.y, area.width, 1);
        render_footer_separator(header, buf, String::new());
        let title = if self.show_header_progress {
            let percent = self.scroll_percent(content_area.height, total_len);
            format!(" {} · {percent}% ", self.title)
        } else {
            format!(" {} ", self.title)
        };
        title.dim().render_ref(header, buf);
    }

    fn render_content(&mut self, area: Rect, buf: &mut Buffer) {
        let (offsets, heights) = {
            let layout = self.layout(area.width);
            (layout.offsets.clone(), layout.heights.clone())
        };
        if offsets.is_empty() {
            for y in area.y..area.bottom() {
                if area.width == 0 {
                    break;
                }
                buf[(area.x, y)] = Cell::from('~');
                for x in area.x + 1..area.right() {
                    buf[(x, y)] = Cell::from(' ');
                }
            }
            return;
        }
        let first_visible = offsets
            .partition_point(|offset| *offset <= self.scroll_offset)
            .saturating_sub(1);
        let mut y = offsets[first_visible] as isize - self.scroll_offset as isize;
        let mut drawn_bottom = area.y;
        for (idx, renderable) in self.renderables.iter().enumerate().skip(first_visible) {
            let top = y;
            let height = heights[idx] as isize;
            y += height;
            let bottom = y;
            if bottom < area.y as isize {
                continue;
            }
            if top > area.y as isize + area.height as isize {
                break;
            }
            if top < 0 {
                let drawn = render_offset_content(area, buf, &**renderable, (-top) as u16);
                drawn_bottom = drawn_bottom.max(area.y + drawn);
            } else {
                let draw_height = (height as u16).min(area.height.saturating_sub(top as u16));
                let draw_area = Rect::new(area.x, area.y + top as u16, area.width, draw_height);
                renderable.render(draw_area, buf);
                drawn_bottom = drawn_bottom.max(draw_area.y.saturating_add(draw_area.height));
            }
        }

        for y in drawn_bottom..area.bottom() {
            if area.width == 0 {
                break;
            }
            buf[(area.x, y)] = Cell::from('~');
            for x in area.x + 1..area.right() {
                buf[(x, y)] = Cell::from(' ');
            }
        }
    }

    fn render_bottom_bar(
        &self,
        full_area: Rect,
        content_area: Rect,
        buf: &mut Buffer,
        _total_len: usize,
    ) {
        let sep_y = content_area.bottom();
        let sep_rect = Rect::new(full_area.x, sep_y, full_area.width, 1);

        render_footer_separator(sep_rect, buf, self.footer_separator_label.clone());
    }

    fn scroll_percent(&self, content_height: u16, total_len: usize) -> u8 {
        if total_len == 0 {
            return 100;
        }
        let max_scroll = total_len.saturating_sub(content_height as usize);
        if max_scroll == 0 {
            return 100;
        }
        (((self.scroll_offset.min(max_scroll)) as f32 / max_scroll as f32) * 100.0).round() as u8
    }

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) -> Result<bool> {
        match key_event {
            e if self.keymap.scroll_up.is_pressed(e) => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            e if self.keymap.scroll_down.is_pressed(e) => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            e if self.keymap.page_up.is_pressed(e) => {
                let page_height = self.page_height(tui.terminal.viewport_area);
                self.scroll_offset = self.scroll_offset.saturating_sub(page_height);
            }
            e if self.keymap.page_down.is_pressed(e) => {
                let page_height = self.page_height(tui.terminal.viewport_area);
                self.scroll_offset = self.scroll_offset.saturating_add(page_height);
            }
            e if self.keymap.half_page_down.is_pressed(e) => {
                let area = self.content_area(tui.terminal.viewport_area);
                let half_page = (area.height as usize).saturating_add(1) / 2;
                self.scroll_offset = self.scroll_offset.saturating_add(half_page);
            }
            e if self.keymap.half_page_up.is_pressed(e) => {
                let area = self.content_area(tui.terminal.viewport_area);
                let half_page = (area.height as usize).saturating_add(1) / 2;
                self.scroll_offset = self.scroll_offset.saturating_sub(half_page);
            }
            e if self.keymap.jump_top.is_pressed(e) => {
                self.scroll_offset = 0;
            }
            e if self.keymap.jump_bottom.is_pressed(e) => {
                self.scroll_offset = usize::MAX;
            }
            _ => {
                return Ok(false);
            }
        }
        tui.frame_requester()
            .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
        Ok(true)
    }

    /// Returns the height of one page in content rows.
    ///
    /// Prefers the last rendered content height (excluding header/footer chrome);
    /// if no render has occurred yet, falls back to the content area height
    /// computed from the given viewport.
    fn page_height(&self, viewport_area: Rect) -> usize {
        self.last_content_height
            .unwrap_or_else(|| self.content_area(viewport_area).height as usize)
    }

    fn update_last_content_height(&mut self, height: u16) {
        self.last_content_height = Some(height as usize);
    }

    fn content_area(&self, area: Rect) -> Rect {
        self.content_area_with_inset(area, /*top_inset*/ 0)
    }

    fn content_area_with_inset(&self, area: Rect, top_inset: u16) -> Rect {
        let mut area = area;
        area.y = area.y.saturating_add(1);
        area.height = area.height.saturating_sub(2);
        area.y = area.y.saturating_add(top_inset);
        area.height = area.height.saturating_sub(top_inset);
        area
    }
}

impl PagerView {
    fn is_scrolled_to_bottom(&self) -> bool {
        if self.scroll_offset == usize::MAX {
            return true;
        }
        let Some(height) = self.last_content_height else {
            return false;
        };
        if self.renderables.is_empty() {
            return true;
        }
        let Some(total_height) = self.last_rendered_height else {
            return false;
        };
        if total_height <= height {
            return true;
        }
        let max_scroll = total_height.saturating_sub(height);
        self.scroll_offset >= max_scroll
    }

    fn resolve_pending_scroll(&mut self, area: Rect, content_height: usize) {
        if let Some((idx, row_offset)) = self.pending_anchor_chunk.take() {
            self.position_chunk_at_upper_third(idx, row_offset, area);
            self.pending_search_line.take();
        } else if let Some((chunk_index, line_index)) = self.pending_search_line.take() {
            self.ensure_line_visible(chunk_index, line_index, area);
        }
        self.scroll_offset = self
            .scroll_offset
            .min(content_height.saturating_sub(area.height as usize));
    }

    fn scroll_line_into_view(&mut self, chunk_index: usize, line_index: usize) {
        self.pending_search_line = Some((chunk_index, line_index));
    }

    fn clamped_scroll_offset(&mut self, area: Rect) -> usize {
        self.scroll_offset.min(
            self.content_height(area.width)
                .saturating_sub(area.height as usize),
        )
    }

    /// Request that a row inside a selected chunk be anchored on next render.
    fn scroll_chunk_to_upper_third(&mut self, chunk_index: usize, row_offset: usize) {
        self.pending_anchor_chunk = Some((chunk_index, row_offset));
    }

    fn position_chunk_at_upper_third(&mut self, idx: usize, row_offset: usize, area: Rect) {
        if area.height == 0 || idx >= self.renderables.len() {
            return;
        }
        let layout = self.layout(area.width);
        let row = layout.offsets[idx].saturating_add(row_offset);
        let anchor = (area.height as usize) / 3;
        self.scroll_offset = row.saturating_sub(anchor);
    }

    fn ensure_line_visible(&mut self, chunk_index: usize, line_index: usize, area: Rect) {
        if area.height == 0 || chunk_index >= self.renderables.len() {
            return;
        }
        let layout = self.layout(area.width);
        let line = layout.offsets[chunk_index].saturating_add(line_index);
        let current_top = self.scroll_offset;
        let current_bottom = current_top.saturating_add(area.height.saturating_sub(1) as usize);
        if line < current_top {
            self.scroll_offset = line;
        } else if line > current_bottom {
            self.scroll_offset = line.saturating_sub(area.height.saturating_sub(1) as usize);
        }
    }
}

/// A renderable that caches its desired height.
struct CachedRenderable {
    renderable: Box<dyn Renderable>,
    height: std::cell::Cell<Option<u16>>,
    last_width: std::cell::Cell<Option<u16>>,
}

impl CachedRenderable {
    fn new(renderable: impl Into<Box<dyn Renderable>>) -> Self {
        Self {
            renderable: renderable.into(),
            height: std::cell::Cell::new(None),
            last_width: std::cell::Cell::new(None),
        }
    }
}

impl Renderable for CachedRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.renderable.render(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        if self.last_width.get() != Some(width) {
            let height = self.renderable.desired_height(width);
            self.height.set(Some(height));
            self.last_width.set(Some(width));
        }
        self.height.get().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RenderableMatch {
    renderable_index: usize,
    line_index: usize,
    start_col: u16,
    end_col: u16,
}

struct CellRenderable {
    cell: Arc<dyn HistoryCell>,
    renderable_index: usize,
    style: Style,
    selected_style: Option<Style>,
    highlight_cell: Rc<StdCell<Option<usize>>>,
    render_mode: HistoryRenderMode,
    active_match: Rc<RefCell<Option<RenderableMatch>>>,
}

impl Renderable for CellRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let style = if self.highlight_cell.get() == Some(self.renderable_index) {
            self.selected_style.unwrap_or(self.style)
        } else {
            self.style
        };
        let hyperlink_lines = self
            .cell
            .transcript_hyperlink_lines_for_mode(area.width, self.render_mode);
        let mut lines = visible_lines(hyperlink_lines.clone());
        if let Some(active_match) = *self.active_match.borrow()
            && active_match.renderable_index == self.renderable_index
        {
            highlight_match_in_lines(&mut lines, active_match);
        }
        let p = Paragraph::new(Text::from(lines))
            .style(style)
            .wrap(Wrap { trim: false });
        p.render(area, buf);
        mark_buffer_hyperlinks(buf, area, &hyperlink_lines, /*scroll_rows*/ 0);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.cell
            .desired_transcript_height_for_mode(width, self.render_mode)
    }
}

struct TailLinesRenderable {
    lines: Vec<HyperlinkLine>,
    renderable_index: usize,
    active_match: Rc<RefCell<Option<RenderableMatch>>>,
}

impl Renderable for TailLinesRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = visible_lines(self.lines.clone());
        if let Some(active_match) = *self.active_match.borrow()
            && active_match.renderable_index == self.renderable_index
        {
            highlight_match_in_lines(&mut lines, active_match);
        }
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .render(area, buf);
        mark_buffer_hyperlinks(buf, area, &self.lines, /*scroll_rows*/ 0);
    }

    fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(Text::from(visible_lines(self.lines.clone())))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(/*default*/ 0)
    }
}

#[derive(Default)]
struct TranscriptSearchState {
    active: bool,
    editing: bool,
    query: String,
    matches: Vec<SearchMatch>,
    active_match: Option<usize>,
    dirty: bool,
    recompute_at: Option<Instant>,
}

impl TranscriptSearchState {
    fn activate(&mut self) {
        self.active = true;
        self.editing = true;
        self.query.clear();
        self.matches.clear();
        self.active_match = None;
        self.dirty = false;
        self.recompute_at = None;
    }

    fn deactivate(&mut self) {
        self.active = false;
        self.editing = false;
        self.query.clear();
        self.matches.clear();
        self.active_match = None;
        self.dirty = false;
        self.recompute_at = None;
    }
}

pub(crate) struct TranscriptOverlay {
    /// Pager UI state and the renderables currently displayed.
    ///
    /// The invariant is that `view.renderables` is `render_cells(cells)` plus an optional trailing
    /// live-tail renderable appended after the committed cells.
    view: PagerView,
    /// Committed transcript cells (does not include the live tail).
    cells: Vec<Arc<dyn HistoryCell>>,
    highlight_cell: Rc<StdCell<Option<usize>>>,
    user_prompt_positions: Vec<usize>,
    render_mode: HistoryRenderMode,
    copy_keymap: Vec<KeyBinding>,
    toggle_raw_output_keymap: Vec<KeyBinding>,
    copy_requested: bool,
    scroll_selected_user_cell: Option<usize>,
    footer_status: Option<FooterStatus>,
    search: TranscriptSearchState,
    search_corpus: Option<SearchCorpus>,
    search_corpus_revision: u64,
    active_search_match: Rc<RefCell<Option<RenderableMatch>>>,
    last_content_width: Option<u16>,
    live_tail_lines: Option<Vec<HyperlinkLine>>,
    /// Cache key for the render-only live tail appended after committed cells.
    live_tail_key: Option<LiveTailKey>,
    async_layout: AsyncTranscriptLayout,
    is_done: bool,
}

const FOOTER_STATUS_TTL: Duration = Duration::from_secs(2);
const ASYNC_TRANSCRIPT_LAYOUT_CELL_THRESHOLD: usize = 2_000;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(150);

#[derive(Clone)]
struct FooterStatus {
    line: Line<'static>,
    expires_at: Instant,
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

#[derive(Debug)]
struct AsyncTranscriptLayout {
    generation: u64,
    pending: Option<TranscriptLayoutKey>,
    measured: Option<TranscriptLayoutResult>,
}

impl AsyncTranscriptLayout {
    fn new() -> Self {
        Self {
            generation: 0,
            pending: None,
            measured: None,
        }
    }

    fn invalidate(&mut self) {
        self.generation = self.generation.saturating_add(/*rhs*/ 1);
        self.pending = None;
        self.measured = None;
    }
}

impl TranscriptOverlay {
    /// Creates a transcript overlay for a fixed set of committed cells.
    ///
    /// This overlay does not own the "active cell"; callers may optionally append a live tail via
    /// `sync_live_tail` during draws to reflect in-flight activity.
    pub(crate) fn new(
        transcript_cells: Vec<Arc<dyn HistoryCell>>,
        keymap: PagerKeymap,
        copy_keymap: Vec<KeyBinding>,
        toggle_raw_output_keymap: Vec<KeyBinding>,
        state: TranscriptOverlayState,
    ) -> Self {
        let highlight_cell = Rc::new(StdCell::new(state.highlight_cell));
        let active_search_match = Rc::new(RefCell::new(None));
        let renderables = Self::render_cells(
            &transcript_cells,
            Rc::clone(&highlight_cell),
            state.render_mode,
            Rc::clone(&active_search_match),
        );
        let user_prompt_positions = Self::user_prompt_positions(&transcript_cells);

        Self {
            view: {
                let mut view = PagerView::new(
                    renderables,
                    "Transcript".to_string(),
                    state.scroll_offset,
                    keymap,
                );
                view.show_header_progress = false;
                view
            },
            user_prompt_positions,
            cells: transcript_cells,
            highlight_cell,
            render_mode: state.render_mode,
            copy_keymap,
            toggle_raw_output_keymap,
            copy_requested: false,
            scroll_selected_user_cell: None,
            footer_status: None,
            search: TranscriptSearchState::default(),
            search_corpus: None,
            search_corpus_revision: 0,
            active_search_match,
            last_content_width: None,
            live_tail_lines: None,
            live_tail_key: None,
            async_layout: AsyncTranscriptLayout::new(),
            is_done: false,
        }
    }

    fn render_cells(
        cells: &[Arc<dyn HistoryCell>],
        highlight_cell: Rc<StdCell<Option<usize>>>,
        render_mode: HistoryRenderMode,
        active_match: Rc<RefCell<Option<RenderableMatch>>>,
    ) -> Vec<Box<dyn Renderable>> {
        cells
            .iter()
            .enumerate()
            .flat_map(|(i, c)| {
                let mut v: Vec<Box<dyn Renderable>> = Vec::new();
                let base_renderable = if c.as_any().is::<UserHistoryCell>() {
                    Box::new(CachedRenderable::new(CellRenderable {
                        cell: c.clone(),
                        renderable_index: i,
                        style: user_message_style(),
                        selected_style: Some(user_message_style().reversed()),
                        highlight_cell: Rc::clone(&highlight_cell),
                        render_mode,
                        active_match: Rc::clone(&active_match),
                    })) as Box<dyn Renderable>
                } else {
                    Box::new(CachedRenderable::new(CellRenderable {
                        cell: c.clone(),
                        renderable_index: i,
                        style: Style::default(),
                        selected_style: None,
                        highlight_cell: Rc::clone(&highlight_cell),
                        render_mode,
                        active_match: Rc::clone(&active_match),
                    })) as Box<dyn Renderable>
                };
                let mut cell_renderable = base_renderable;
                if !c.is_stream_continuation() && i > 0 {
                    cell_renderable = Box::new(InsetRenderable::new(
                        cell_renderable,
                        Insets::tlbr(
                            /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
                        ),
                    ));
                }
                v.push(cell_renderable);
                v
            })
            .collect()
    }

    fn user_prompt_positions(cells: &[Arc<dyn HistoryCell>]) -> Vec<usize> {
        let start = session_start_index(cells);
        cells
            .iter()
            .enumerate()
            .skip(start)
            .filter_map(|(idx, cell)| cell.is_user_prompt().then_some(idx))
            .collect()
    }

    /// Insert a committed history cell while keeping any cached live tail.
    ///
    /// The live tail is temporarily removed, the committed cells are rebuilt,
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
        self.cells.push(cell);
        self.invalidate_async_layout();
        self.user_prompt_positions = Self::user_prompt_positions(&self.cells);
        self.view.renderables = Self::render_cells(
            &self.cells,
            Rc::clone(&self.highlight_cell),
            self.render_mode,
            Rc::clone(&self.active_search_match),
        );
        self.view.invalidate_layout();
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
        self.invalidate_search_corpus();
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
    }

    /// Replace committed transcript cells while keeping any cached in-progress output that is
    /// currently shown at the end of the overlay.
    ///
    /// This is used when existing history is trimmed (for example after rollback) so the
    /// transcript overlay immediately reflects the same committed cells as the main transcript.
    pub(crate) fn replace_cells(&mut self, cells: Vec<Arc<dyn HistoryCell>>) {
        let follow_bottom = self.view.is_scrolled_to_bottom();
        self.cells = cells;
        self.invalidate_async_layout();
        self.user_prompt_positions = Self::user_prompt_positions(&self.cells);
        if self
            .highlight_cell
            .get()
            .is_some_and(|idx| idx >= self.cells.len())
        {
            self.highlight_cell.set(None);
        }
        self.rebuild_renderables();
        self.invalidate_search_corpus();
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
            let removed = clamped_end - clamped_start;
            if let Some(mut highlight_cell) = self.highlight_cell.get()
                && highlight_cell >= clamped_start
            {
                if highlight_cell < clamped_end {
                    highlight_cell = clamped_start;
                } else {
                    highlight_cell = highlight_cell.saturating_sub(removed.saturating_sub(1));
                }
                self.highlight_cell.set(Some(highlight_cell));
            }
            self.cells
                .splice(clamped_start..clamped_end, std::iter::once(consolidated));
            self.invalidate_async_layout();
            self.user_prompt_positions = Self::user_prompt_positions(&self.cells);
            if self
                .highlight_cell
                .get()
                .is_some_and(|highlight_cell| highlight_cell >= self.cells.len())
            {
                self.highlight_cell.set(None);
            }
            self.rebuild_renderables();
            self.invalidate_search_corpus();
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

        if self.live_tail_key == next_key {
            self.install_measured_layout_if_ready(width);
            return;
        }
        let follow_bottom = self.view.is_scrolled_to_bottom();

        self.take_live_tail_renderable();
        self.live_tail_key = next_key;
        self.live_tail_lines = None;

        if let Some(key) = next_key {
            let lines = compute_lines(width).unwrap_or_default();
            if !lines.is_empty() {
                self.live_tail_lines = Some(lines.clone());
                self.view.renderables.push(Self::live_tail_renderable(
                    lines,
                    !self.cells.is_empty(),
                    key.is_stream_continuation,
                    self.cells.len(),
                    Rc::clone(&self.active_search_match),
                ));
                self.view.invalidate_layout();
            }
        }
        self.invalidate_search_corpus();
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
        self.install_measured_layout_if_ready(width);
    }

    pub(crate) fn ensure_async_layout(&mut self, width: u16, app_event_tx: AppEventSender) {
        if !self.uses_async_layout() {
            return;
        }
        let key = self.layout_key(width);
        if self
            .async_layout
            .measured
            .as_ref()
            .is_some_and(|result| result.key == key)
        {
            self.install_measured_layout_if_ready(width);
            return;
        }
        if self.async_layout.pending == Some(key) {
            return;
        }
        self.async_layout.pending = Some(key);
        self.async_layout.measured = None;
        spawn_transcript_layout_worker(key, self.cells.clone(), app_event_tx);
    }

    pub(crate) fn apply_layout_result(&mut self, result: TranscriptLayoutResult) -> bool {
        if result.key != self.layout_key(result.key.width) {
            return false;
        }
        let width = result.key.width;
        self.async_layout.pending = None;
        self.async_layout.measured = Some(result);
        self.install_measured_layout_if_ready(width);
        true
    }

    fn uses_async_layout(&self) -> bool {
        self.cells.len() >= ASYNC_TRANSCRIPT_LAYOUT_CELL_THRESHOLD
    }

    fn layout_key(&self, width: u16) -> TranscriptLayoutKey {
        TranscriptLayoutKey {
            generation: self.async_layout.generation,
            width,
            render_mode: self.render_mode,
            cell_count: self.cells.len(),
        }
    }

    fn is_waiting_for_async_layout(&self, width: u16) -> bool {
        if !self.uses_async_layout() {
            return false;
        }
        let Some(layout) = self.view.layout.as_ref() else {
            return true;
        };
        layout.width != width || layout.heights.len() != self.view.renderables.len()
    }

    fn install_measured_layout_if_ready(&mut self, width: u16) -> bool {
        let Some(result) = self.async_layout.measured.as_ref() else {
            return false;
        };
        if result.key != self.layout_key(width) {
            return false;
        }
        let mut heights = result.heights.clone();
        for renderable in self.view.renderables.iter().skip(self.cells.len()) {
            heights.push(renderable.desired_height(width) as usize);
        }
        self.view.install_precomputed_layout(width, heights);
        true
    }

    fn invalidate_async_layout(&mut self) {
        self.async_layout.invalidate();
    }

    pub(crate) fn set_highlight_cell(&mut self, cell: Option<usize>) {
        self.set_highlight_cell_with_placement(cell, PromptSelectionPlacement::AnchorUpperThird);
    }

    fn set_highlight_cell_preserving_viewport(&mut self, cell: Option<usize>) {
        self.set_highlight_cell_with_placement(cell, PromptSelectionPlacement::PreserveViewport);
    }

    fn set_highlight_cell_with_placement(
        &mut self,
        cell: Option<usize>,
        placement: PromptSelectionPlacement,
    ) {
        let prompt_row_offset = cell.map(|idx| self.prompt_first_text_row_offset(idx));
        self.highlight_cell.set(cell);
        if let (Some(idx), Some(row_offset), PromptSelectionPlacement::AnchorUpperThird) =
            (cell, prompt_row_offset, placement)
        {
            self.view.scroll_chunk_to_upper_third(idx, row_offset);
        }
    }

    pub(crate) fn user_prompt_count(&self) -> usize {
        self.user_prompt_positions.len()
    }

    pub(crate) fn set_highlighted_user_prompt(&mut self, nth_user_message: usize) -> Option<usize> {
        let cell_idx = self.user_prompt_positions.get(nth_user_message).copied()?;
        self.set_highlight_cell(Some(cell_idx));
        Some(cell_idx)
    }

    /// Returns whether the underlying pager view is currently pinned to the bottom.
    ///
    /// The `App` draw loop uses this to decide whether to schedule animation frames for the live
    /// tail; if the user has scrolled up, we avoid driving animation work that they cannot see.
    pub(crate) fn is_scrolled_to_bottom(&self) -> bool {
        self.view.is_scrolled_to_bottom()
    }

    pub(crate) fn state(&self) -> TranscriptOverlayState {
        TranscriptOverlayState {
            scroll_offset: self.view.scroll_offset,
            highlight_cell: self.highlight_cell.get(),
            render_mode: self.render_mode,
        }
    }

    pub(crate) fn take_copy_requested(&mut self) -> bool {
        std::mem::take(&mut self.copy_requested)
    }

    pub(crate) fn take_scroll_selected_user_cell(&mut self) -> Option<usize> {
        self.scroll_selected_user_cell.take()
    }

    pub(crate) fn show_copy_status(&mut self, status: &CopyStatus, tui: &mut tui::Tui) {
        self.show_copy_status_at(status, Instant::now());
        tui.frame_requester().schedule_frame();
        tui.frame_requester().schedule_frame_in(FOOTER_STATUS_TTL);
    }

    pub(crate) fn search_is_active(&self) -> bool {
        self.search.active
    }

    pub(crate) fn is_start_search_key(&self, key_event: KeyEvent) -> bool {
        self.view.keymap.start_search.is_pressed(key_event)
    }

    pub(crate) fn selected_user_cell(&self) -> Option<usize> {
        self.highlight_cell.get().filter(|idx| {
            self.cells
                .get(*idx)
                .is_some_and(|cell| cell.is_user_prompt())
        })
    }

    pub(crate) fn cancel_search_mode(&mut self) {
        self.search.deactivate();
        self.active_search_match.borrow_mut().take();
    }

    fn toggle_render_mode(&mut self) {
        self.render_mode = match self.render_mode {
            HistoryRenderMode::Rich => HistoryRenderMode::Raw,
            HistoryRenderMode::Raw => HistoryRenderMode::Rich,
        };
        self.invalidate_async_layout();
        self.rebuild_renderables();
        self.invalidate_search_corpus();
    }

    fn append_search_text(&mut self, text: &str) {
        self.search.editing = true;
        self.search.query.push_str(text);
        self.mark_search_dirty();
    }

    fn search_backspace(&mut self) {
        self.search.editing = true;
        if self.search.query.pop().is_some() {
            self.mark_search_dirty();
        }
    }

    fn activate_search(&mut self) {
        self.search.activate();
        self.active_search_match.borrow_mut().take();
    }

    fn mark_search_dirty(&mut self) {
        self.search.dirty = true;
        self.search.active_match = None;
        self.active_search_match.borrow_mut().take();
        let debounce = if self.search.query.is_empty() {
            Duration::ZERO
        } else {
            SEARCH_DEBOUNCE
        };
        self.search.recompute_at = Some(Instant::now() + debounce);
    }

    fn invalidate_search_corpus(&mut self) {
        self.search_corpus = None;
        self.search_corpus_revision = self.search_corpus_revision.wrapping_add(1);
        if self.search.active && !self.search.query.is_empty() {
            self.search.dirty = true;
            self.search.recompute_at = Some(Instant::now());
        }
    }

    fn update_active_search_match(&mut self) {
        let active = self
            .search
            .active_match
            .and_then(|idx| self.search.matches.get(idx))
            .map(|search_match| RenderableMatch {
                renderable_index: search_match.renderable_index,
                line_index: search_match.line_index,
                start_col: search_match.start_col,
                end_col: search_match.end_col,
            });
        *self.active_search_match.borrow_mut() = active;
    }

    fn recompute_search_matches(&mut self, width: u16) {
        self.last_content_width = Some(width);
        self.search.dirty = false;
        self.search.recompute_at = None;
        self.search.matches.clear();
        self.search.active_match = None;
        self.active_search_match.borrow_mut().take();
        if !self.search.active || self.search.query.is_empty() || width == 0 {
            return;
        }

        let query = self.search.query.to_lowercase();
        self.ensure_search_corpus(width);
        self.search.matches = self
            .search_corpus
            .as_ref()
            .map_or_else(Vec::new, |corpus| corpus.find_matches(&query));

        if !self.search.matches.is_empty() {
            self.search.active_match = Some(0);
            self.update_active_search_match();
            self.scroll_to_active_match();
        }
    }

    fn scroll_to_active_match(&mut self) {
        let Some(active_idx) = self.search.active_match else {
            return;
        };
        let Some(active_match) = self.search.matches.get(active_idx) else {
            return;
        };
        self.view.scroll_line_into_view(
            active_match.renderable_index,
            active_match.scroll_line_index,
        );
    }

    fn select_next_match(&mut self) {
        self.search.editing = false;
        if self.search.matches.is_empty() {
            return;
        }
        let next = self
            .search
            .active_match
            .map(|idx| (idx + 1) % self.search.matches.len())
            .unwrap_or(0);
        self.search.active_match = Some(next);
        self.update_active_search_match();
        self.scroll_to_active_match();
    }

    fn select_previous_match(&mut self) {
        self.search.editing = false;
        if self.search.matches.is_empty() {
            return;
        }
        let prev = self
            .search
            .active_match
            .map(|idx| {
                if idx == 0 {
                    self.search.matches.len().saturating_sub(1)
                } else {
                    idx.saturating_sub(1)
                }
            })
            .unwrap_or_else(|| self.search.matches.len().saturating_sub(1));
        self.search.active_match = Some(prev);
        self.update_active_search_match();
        self.scroll_to_active_match();
    }

    fn ensure_search_corpus(&mut self, width: u16) {
        let key = SearchCorpusKey {
            width,
            render_mode: self.render_mode,
            revision: self.search_corpus_revision,
        };
        if self
            .search_corpus
            .as_ref()
            .is_some_and(|corpus| corpus.matches_key(key))
        {
            return;
        }
        let live_tail_has_top_padding = self
            .live_tail_key
            .is_some_and(|key| !key.is_stream_continuation && !self.cells.is_empty());
        let live_tail_lines = self.live_tail_lines.clone().map(visible_lines);
        self.search_corpus = Some(SearchCorpus::new(
            key,
            transcript_search_lines(
                &self.cells,
                live_tail_lines.as_deref(),
                self.render_mode,
                width,
                rendered_line_height,
                live_tail_has_top_padding,
            ),
        ));
    }

    fn flush_pending_search(&mut self) {
        if self.search.dirty
            && let Some(width) = self.last_content_width
        {
            self.recompute_search_matches(width);
        }
    }

    fn next_search_recompute_delay(&self) -> Duration {
        self.search
            .recompute_at
            .and_then(|recompute_at| recompute_at.checked_duration_since(Instant::now()))
            .unwrap_or(crate::tui::TARGET_FRAME_INTERVAL)
    }

    fn schedule_search_frames(&self, tui: &tui::Tui) {
        tui.frame_requester()
            .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
        if self.search.dirty {
            tui.frame_requester()
                .schedule_frame_in(self.next_search_recompute_delay());
        }
    }

    fn navigate_search_prompt(&mut self, direction: PromptSelectionDirection) {
        if self.highlight_cell.get().is_none() {
            let anchor = self
                .search
                .active_match
                .and_then(|idx| self.search.matches.get(idx))
                .and_then(|search_match| search_match.owning_user_prompt_cell);
            let Some(anchor) = anchor else {
                return;
            };
            self.set_highlight_cell(Some(anchor));
        }
        self.move_prompt_selection(direction);
    }

    fn move_prompt_selection(&mut self, direction: PromptSelectionDirection) {
        let Some(last_prompt) = self.user_prompt_positions.last().copied() else {
            return;
        };

        let next_prompt = match self.highlight_cell.get() {
            Some(current) => {
                let current_idx = self
                    .user_prompt_positions
                    .iter()
                    .position(|idx| *idx == current)
                    .unwrap_or(self.user_prompt_positions.len().saturating_sub(1));
                match direction {
                    PromptSelectionDirection::Previous => {
                        self.user_prompt_positions[current_idx.saturating_sub(1)]
                    }
                    PromptSelectionDirection::Next => self
                        .user_prompt_positions
                        .get(current_idx.saturating_add(1))
                        .copied()
                        .unwrap_or(last_prompt),
                }
            }
            None => last_prompt,
        };
        self.set_highlight_cell(Some(next_prompt));
    }

    fn prompt_first_text_row_offset(&self, idx: usize) -> usize {
        let inter_cell_spacing = usize::from(
            idx > 0
                && self
                    .cells
                    .get(idx)
                    .is_some_and(|cell| !cell.is_stream_continuation()),
        );
        let rich_prompt_padding = usize::from(matches!(self.render_mode, HistoryRenderMode::Rich));
        inter_cell_spacing.saturating_add(rich_prompt_padding)
    }

    fn prompt_entering_viewport(
        &mut self,
        width: u16,
        height: u16,
        before: usize,
        after: usize,
    ) -> Option<usize> {
        if before == after || height == 0 {
            return None;
        }
        let offsets = self.view.layout(width).offsets.clone();
        let prompts = self.user_prompt_positions.iter().copied().map(|idx| {
            (
                idx,
                offsets[idx].saturating_add(self.prompt_first_text_row_offset(idx)),
            )
        });
        if after > before {
            let previous_bottom = before.saturating_add(height as usize);
            let current_bottom = after.saturating_add(height as usize);
            prompts
                .filter(|(_, row)| previous_bottom <= *row && *row < current_bottom)
                .map(|(idx, _)| idx)
                .next_back()
        } else {
            prompts
                .filter(|(_, row)| after <= *row && *row < before)
                .map(|(idx, _)| idx)
                .next()
        }
    }

    fn handle_viewport_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) -> Result<()> {
        let top_h = tui.terminal.viewport_area.height.saturating_sub(3);
        let top = Rect::new(
            tui.terminal.viewport_area.x,
            tui.terminal.viewport_area.y,
            tui.terminal.viewport_area.width,
            top_h,
        );
        let search_row_height = u16::from(self.search.active);
        let content_area = self.view.content_area_with_inset(top, search_row_height);
        if self.is_waiting_for_async_layout(content_area.width) {
            return Ok(());
        }
        let before = self.view.clamped_scroll_offset(content_area);
        if !self.view.handle_key_event(tui, key_event)? {
            return Ok(());
        }
        let after = self.view.clamped_scroll_offset(content_area);
        if let Some(cell_idx) =
            self.prompt_entering_viewport(content_area.width, content_area.height, before, after)
        {
            self.set_highlight_cell_preserving_viewport(Some(cell_idx));
            self.scroll_selected_user_cell = Some(cell_idx);
        }
        Ok(())
    }

    fn header_title(&self) -> String {
        "Transcript".to_string()
    }

    fn footer_progress_label(&self, content_height: u16, total_len: usize, width: u16) -> String {
        let total = self.user_prompt_positions.len();
        let selected = self
            .highlight_cell
            .get()
            .and_then(|highlight_cell| {
                let selected = self
                    .user_prompt_positions
                    .partition_point(|idx| *idx <= highlight_cell);
                (selected > 0).then_some(selected)
            })
            .unwrap_or(total);
        let percent = self.view.scroll_percent(content_height, total_len);
        let labels = [
            format!(" {selected} / {total} · {percent}% "),
            format!(" {selected}/{total} · {percent}% "),
            format!(" {percent}% "),
        ];
        first_fitting_right_label(width, &labels)
    }

    fn loading_progress_label(&self, width: u16) -> String {
        let total = self.user_prompt_positions.len();
        let selected = self
            .highlight_cell
            .get()
            .and_then(|highlight_cell| {
                let selected = self
                    .user_prompt_positions
                    .partition_point(|idx| *idx <= highlight_cell);
                (selected > 0).then_some(selected)
            })
            .unwrap_or(total);
        let labels = [
            format!(" {selected} / {total} · loading... "),
            format!(" {selected}/{total} · loading... "),
            " loading... ".to_string(),
        ];
        first_fitting_right_label(width, &labels)
    }

    fn rebuild_renderables(&mut self) {
        let tail_renderable = self.take_live_tail_renderable();
        self.view.renderables = Self::render_cells(
            &self.cells,
            Rc::clone(&self.highlight_cell),
            self.render_mode,
            Rc::clone(&self.active_search_match),
        );
        if let Some(tail) = tail_renderable {
            self.view.renderables.push(tail);
        }
        self.view.invalidate_layout();
    }

    /// Removes and returns the cached live-tail renderable, if present.
    ///
    /// The live tail is represented as a single optional renderable appended after the committed
    /// cell renderables, so this relies on the live tail always being the final entry in
    /// `view.renderables` when present.
    fn take_live_tail_renderable(&mut self) -> Option<Box<dyn Renderable>> {
        let tail = (self.view.renderables.len() > self.cells.len())
            .then(|| self.view.renderables.pop())?;
        if tail.is_some() {
            self.view.invalidate_layout();
        }
        tail
    }

    fn live_tail_renderable(
        lines: Vec<HyperlinkLine>,
        has_prior_cells: bool,
        is_stream_continuation: bool,
        renderable_index: usize,
        active_match: Rc<RefCell<Option<RenderableMatch>>>,
    ) -> Box<dyn Renderable> {
        let mut renderable: Box<dyn Renderable> =
            Box::new(CachedRenderable::new(TailLinesRenderable {
                lines,
                renderable_index,
                active_match,
            }));
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

    fn show_copy_status_at(&mut self, status: &CopyStatus, now: Instant) {
        let line = if status.is_success() {
            Line::from(status.message().to_string().green())
        } else {
            Line::from(status.message().to_string().red())
        };
        self.footer_status = Some(FooterStatus {
            line,
            expires_at: now + FOOTER_STATUS_TTL,
        });
    }

    fn clear_footer_status(&mut self) -> bool {
        self.footer_status.take().is_some()
    }

    fn clear_expired_footer_status_at(&mut self, now: Instant) -> bool {
        if self
            .footer_status
            .as_ref()
            .is_some_and(|status| status.expires_at <= now)
        {
            return self.clear_footer_status();
        }
        false
    }

    fn search_status_text(&self) -> String {
        if self.search.dirty {
            return "searching…".to_string();
        }
        let current = self.search.active_match.map(|idx| idx + 1).unwrap_or(0);
        format!("{current}/{}", self.search.matches.len())
    }

    fn search_row(&self, area: Rect, buf: &mut Buffer) {
        let label = format!("Search: {}", self.search.query);
        Line::from(label).render_ref(area, buf);
        let status = self.search_status_text();
        let status_width = u16::try_from(status.chars().count()).unwrap_or(u16::MAX);
        if status_width < area.width {
            Line::from(status).dim().render_ref(
                Rect::new(
                    area.right().saturating_sub(status_width),
                    area.y,
                    status_width,
                    1,
                ),
                buf,
            );
        }
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let line1 = Rect::new(area.x, area.y, area.width, 1);
        let line2 = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        let scroll_keys = first_or_empty(&self.view.keymap.scroll_up)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.scroll_down))
            .collect::<Vec<_>>();
        let page_keys = first_or_empty(&self.view.keymap.page_up)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.page_down))
            .collect::<Vec<_>>();
        let jump_keys = first_or_empty(&self.view.keymap.jump_top)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.jump_bottom))
            .collect::<Vec<_>>();
        let prompt_keys = first_or_empty(&self.view.keymap.previous_user_prompt)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.next_user_prompt))
            .collect::<Vec<_>>();
        let navigation_hints = vec![
            FooterHint::new(
                key_label(&scroll_keys),
                "scroll",
                "scroll",
                /*priority*/ 1,
            ),
            FooterHint::new(
                key_label(&prompt_keys),
                "prompts",
                "prompts",
                /*priority*/ 2,
            ),
            FooterHint::new(key_label(&page_keys), "page", "page", /*priority*/ 6),
            FooterHint::new(key_label(&jump_keys), "jump", "jump", /*priority*/ 7),
        ];
        render_footer_line_with_optional_right(
            line1,
            buf,
            footer_hint_line_for_row(&navigation_hints, area.width),
            self.footer_status
                .as_ref()
                .map(|status| status.line.clone()),
        );

        let mut action_hints = Vec::new();
        if self.search.active {
            action_hints.push(FooterHint::new(
                key_label(&first_or_empty(&self.view.keymap.close_transcript)),
                "quit",
                "quit",
                /*priority*/ 0,
            ));
            action_hints.push(FooterHint::new(
                key_label(&[key_hint::plain(KeyCode::Esc)]),
                "exit search",
                "exit",
                /*priority*/ 1,
            ));
            action_hints.push(FooterHint::new(
                key_label(&first_or_empty(&self.view.keymap.next_search_match)),
                "next hit",
                "next",
                /*priority*/ 2,
            ));
            action_hints.push(FooterHint::new(
                key_label(&first_or_empty(&self.view.keymap.previous_search_match)),
                "prev hit",
                "prev",
                /*priority*/ 3,
            ));
            action_hints.push(FooterHint::new(
                key_label(&first_or_empty(&self.view.keymap.start_search)),
                "restart",
                "restart",
                /*priority*/ 4,
            ));
        } else {
            action_hints.push(FooterHint::new(
                key_label(&first_or_empty(&self.view.keymap.close)),
                "quit",
                "quit",
                /*priority*/ 0,
            ));
            if !self.copy_keymap.is_empty() {
                action_hints.push(FooterHint::new(
                    key_label(&first_or_empty(&self.copy_keymap)),
                    "copy",
                    "copy",
                    /*priority*/ 3,
                ));
            }
            if !self.toggle_raw_output_keymap.is_empty() {
                let mode_label = match self.render_mode {
                    HistoryRenderMode::Rich => "raw",
                    HistoryRenderMode::Raw => "rich",
                };
                action_hints.push(FooterHint::new(
                    key_label(&first_or_empty(&self.toggle_raw_output_keymap)),
                    mode_label,
                    mode_label,
                    /*priority*/ 4,
                ));
            }
            if self.highlight_cell.get().is_some() {
                let previous_edit_keys = std::iter::once(key_hint::plain(KeyCode::Esc))
                    .chain(first_or_empty(&self.view.keymap.previous_user_prompt))
                    .collect::<Vec<_>>();
                action_hints.push(FooterHint::new(
                    key_label(&previous_edit_keys),
                    "edit prev",
                    "prev",
                    /*priority*/ 8,
                ));
                action_hints.push(FooterHint::new(
                    key_label(&first_or_empty(&self.view.keymap.next_user_prompt)),
                    "edit next",
                    "next",
                    /*priority*/ 9,
                ));
                action_hints.push(FooterHint::new(
                    key_label(&[key_hint::plain(KeyCode::Enter)]),
                    "edit message",
                    "edit",
                    /*priority*/ 10,
                ));
            } else {
                let previous_edit_keys = std::iter::once(key_hint::plain(KeyCode::Esc))
                    .chain(first_or_empty(&self.view.keymap.previous_user_prompt))
                    .collect::<Vec<_>>();
                action_hints.push(FooterHint::new(
                    key_label(&previous_edit_keys),
                    "edit prev",
                    "prev",
                    /*priority*/ 8,
                ));
                action_hints.push(FooterHint::new(
                    key_label(&first_or_empty(&self.view.keymap.start_search)),
                    "search",
                    "search",
                    /*priority*/ 11,
                ));
            }
        }
        footer_hint_line_for_row(&action_hints, area.width).render_ref(line2, buf);
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.clear_expired_footer_status_at(Instant::now());
        let top_h = area.height.saturating_sub(3);
        let top = Rect::new(area.x, area.y, area.width, top_h);
        let bottom = Rect::new(area.x, area.y + top_h, area.width, 3);
        self.view.title = self.header_title();
        Clear.render(top, buf);
        let search_row_height = u16::from(self.search.active);
        let content_area = self.view.content_area_with_inset(top, search_row_height);
        self.install_measured_layout_if_ready(content_area.width);
        if self.is_waiting_for_async_layout(content_area.width) {
            self.render_loading(top, bottom, buf);
            return;
        }
        self.view.update_last_content_height(content_area.height);
        let width_changed = self.last_content_width != Some(content_area.width);
        self.last_content_width = Some(content_area.width);
        let now = Instant::now();
        let search_recompute_due = self
            .search
            .recompute_at
            .is_some_and(|recompute_at| recompute_at <= now);
        if width_changed {
            self.search_corpus = None;
            if self.search.active && !self.search.query.is_empty() {
                self.recompute_search_matches(content_area.width);
            }
        } else if self.search.dirty && search_recompute_due {
            self.recompute_search_matches(content_area.width);
        }
        if self.search.active {
            self.search_row(Rect::new(top.x, top.y.saturating_add(1), top.width, 1), buf);
        }
        let content_height = self.view.content_height(content_area.width);
        self.view.last_rendered_height = Some(content_height);
        self.view
            .resolve_pending_scroll(content_area, content_height);
        self.view.footer_separator_label =
            self.footer_progress_label(content_area.height, content_height, top.width);
        self.view
            .render_header(top, content_area, buf, content_height);
        self.view.render_content(content_area, buf);
        self.view
            .render_bottom_bar(top, content_area, buf, content_height);
        self.render_hints(bottom, buf);
    }

    fn render_loading(&mut self, top: Rect, bottom: Rect, buf: &mut Buffer) {
        Clear.render(top, buf);
        let header = Rect::new(top.x, top.y, top.width, /*height*/ 1);
        render_footer_separator(header, buf, String::new());
        format!(" {} ", self.header_title())
            .dim()
            .render_ref(header, buf);

        let content_area = self.view.content_area(top);
        if content_area.height > 0 {
            Paragraph::new("Loading transcript...".dim())
                .wrap(Wrap { trim: false })
                .render(content_area, buf);
        }

        let separator = Rect::new(top.x, content_area.bottom(), top.width, /*height*/ 1);
        render_footer_separator(separator, buf, self.loading_progress_label(top.width));
        self.render_hints(bottom, buf);
    }
}

impl TranscriptOverlay {
    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => {
                if matches!(key_event.kind, KeyEventKind::Release) {
                    return Ok(());
                }
                self.clear_footer_status();
                if self.view.keymap.close_transcript.is_pressed(key_event) {
                    self.is_done = true;
                    return Ok(());
                }
                if self.search.active {
                    if self.view.keymap.close.is_pressed(key_event)
                        && !is_plain_text_key_event(key_event)
                    {
                        self.is_done = true;
                        return Ok(());
                    }
                    let next_match_pressed =
                        self.view.keymap.next_search_match.is_pressed(key_event);
                    let previous_match_pressed =
                        self.view.keymap.previous_search_match.is_pressed(key_event);
                    if self.view.keymap.start_search.is_pressed(key_event) {
                        self.activate_search();
                    } else if matches!(key_event.code, KeyCode::Esc) {
                        self.cancel_search_mode();
                    } else if matches!(key_event.code, KeyCode::Backspace) {
                        self.search_backspace();
                    } else if self.view.keymap.previous_user_prompt.is_pressed(key_event) {
                        self.navigate_search_prompt(PromptSelectionDirection::Previous);
                    } else if self.view.keymap.next_user_prompt.is_pressed(key_event) {
                        self.navigate_search_prompt(PromptSelectionDirection::Next);
                    } else if next_match_pressed
                        && (!self.search.editing || matches!(key_event.code, KeyCode::Enter))
                    {
                        self.flush_pending_search();
                        self.select_next_match();
                    } else if previous_match_pressed && !self.search.editing {
                        self.flush_pending_search();
                        self.select_previous_match();
                    } else if is_plain_text_key_event(key_event)
                        && (self.search.editing || (!next_match_pressed && !previous_match_pressed))
                    {
                        if let KeyCode::Char(ch) = key_event.code {
                            let mut text = String::new();
                            text.push(ch);
                            self.append_search_text(&text);
                        }
                    } else {
                        return self.handle_viewport_key_event(tui, key_event);
                    }
                    self.schedule_search_frames(tui);
                    Ok(())
                } else {
                    match key_event {
                        e if self.view.keymap.close.is_pressed(e) => {
                            self.is_done = true;
                            Ok(())
                        }
                        e if self.view.keymap.start_search.is_pressed(e) => {
                            self.activate_search();
                            tui.frame_requester()
                                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                            Ok(())
                        }
                        e if self.view.keymap.previous_user_prompt.is_pressed(e) => {
                            self.move_prompt_selection(PromptSelectionDirection::Previous);
                            tui.frame_requester()
                                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                            Ok(())
                        }
                        e if self.view.keymap.next_user_prompt.is_pressed(e) => {
                            self.move_prompt_selection(PromptSelectionDirection::Next);
                            tui.frame_requester()
                                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                            Ok(())
                        }
                        e if self.toggle_raw_output_keymap.is_pressed(e) => {
                            self.toggle_render_mode();
                            tui.frame_requester()
                                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                            Ok(())
                        }
                        e if self.copy_keymap.is_pressed(e) => {
                            self.copy_requested = true;
                            Ok(())
                        }
                        other => self.handle_viewport_key_event(tui, other),
                    }
                }
            }
            TuiEvent::Paste(pasted) if self.search.active => {
                let normalized = normalize_search_paste(&pasted);
                if !normalized.is_empty() {
                    self.append_search_text(&normalized);
                    self.schedule_search_frames(tui);
                }
                Ok(())
            }
            TuiEvent::Draw | TuiEvent::Resize => {
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

    #[cfg(test)]
    pub(crate) fn committed_cell_count(&self) -> usize {
        self.cells.len()
    }
}

enum PromptSelectionDirection {
    Previous,
    Next,
}

#[derive(Clone, Copy)]
enum PromptSelectionPlacement {
    AnchorUpperThird,
    PreserveViewport,
}

pub(crate) struct StaticOverlay {
    view: PagerView,
    is_done: bool,
}

impl StaticOverlay {
    pub(crate) fn with_title(
        lines: Vec<Line<'static>>,
        title: String,
        keymap: PagerKeymap,
    ) -> Self {
        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        Self::with_renderables(
            vec![Box::new(CachedRenderable::new(paragraph))],
            title,
            keymap,
        )
    }

    pub(crate) fn with_renderables(
        renderables: Vec<Box<dyn Renderable>>,
        title: String,
        keymap: PagerKeymap,
    ) -> Self {
        Self {
            view: PagerView::new(renderables, title, /*scroll_offset*/ 0, keymap),
            is_done: false,
        }
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let line1 = Rect::new(area.x, area.y, area.width, 1);
        let line2 = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        let scroll_keys = first_or_empty(&self.view.keymap.scroll_up)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.scroll_down))
            .collect::<Vec<_>>();
        let page_keys = first_or_empty(&self.view.keymap.page_up)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.page_down))
            .collect::<Vec<_>>();
        let jump_keys = first_or_empty(&self.view.keymap.jump_top)
            .into_iter()
            .chain(first_or_empty(&self.view.keymap.jump_bottom))
            .collect::<Vec<_>>();
        let navigation_hints = vec![
            FooterHint::new(
                key_label(&scroll_keys),
                "scroll",
                "scroll",
                /*priority*/ 1,
            ),
            FooterHint::new(key_label(&page_keys), "page", "page", /*priority*/ 6),
            FooterHint::new(key_label(&jump_keys), "jump", "jump", /*priority*/ 7),
        ];
        footer_hint_line_for_row(&navigation_hints, area.width).render_ref(line1, buf);

        let action_hints = vec![FooterHint::new(
            key_label(&first_or_empty(&self.view.keymap.close)),
            "quit",
            "quit",
            /*priority*/ 0,
        )];
        footer_hint_line_for_row(&action_hints, area.width).render_ref(line2, buf);
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let top_h = area.height.saturating_sub(3);
        let top = Rect::new(area.x, area.y, area.width, top_h);
        let bottom = Rect::new(area.x, area.y + top_h, area.width, 3);
        self.view.render(top, buf);
        self.render_hints(bottom, buf);
    }
}

impl StaticOverlay {
    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => match key_event {
                e if self.view.keymap.close.is_pressed(e) => {
                    self.is_done = true;
                    Ok(())
                }
                other => self.view.handle_key_event(tui, other).map(|_| ()),
            },
            TuiEvent::Draw | TuiEvent::Resize => {
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

fn render_offset_content(
    area: Rect,
    buf: &mut Buffer,
    renderable: &dyn Renderable,
    scroll_offset: u16,
) -> u16 {
    let height = renderable.desired_height(area.width);
    let mut tall_buf = Buffer::empty(Rect::new(
        0,
        0,
        area.width,
        height.min(area.height + scroll_offset),
    ));
    renderable.render(*tall_buf.area(), &mut tall_buf);
    let copy_height = area
        .height
        .min(tall_buf.area().height.saturating_sub(scroll_offset));
    for y in 0..copy_height {
        let src_y = y + scroll_offset;
        for x in 0..area.width {
            buf[(area.x + x, area.y + y)] = tall_buf[(x, src_y)].clone();
        }
    }

    copy_height
}

fn normalize_search_paste(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn highlight_match_in_lines(lines: &mut [Line<'static>], active_match: RenderableMatch) {
    if active_match.end_col <= active_match.start_col {
        return;
    }
    let Some(line) = lines.get_mut(active_match.line_index) else {
        return;
    };

    let mut col = 0u16;
    let mut spans = Vec::new();
    for span in &line.spans {
        for ch in span.content.chars() {
            let char_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(0);
            let next_col = col.saturating_add(char_width);
            let is_match = next_col > active_match.start_col && col < active_match.end_col;
            let style = if is_match {
                accent_style().add_modifier(Modifier::REVERSED)
            } else {
                span.style
            };
            spans.push(Span::styled(ch.to_string(), style));
            col = next_col;
        }
    }

    let mut highlighted = line.clone();
    highlighted.spans = spans;
    *line = highlighted;
}

fn rendered_line_height(line: &Line<'_>, width: u16) -> usize {
    Paragraph::new(Text::from(vec![line.clone()]))
        .wrap(Wrap { trim: false })
        .line_count(width)
}

fn session_start_index(cells: &[Arc<dyn HistoryCell>]) -> usize {
    let session_start_type = TypeId::of::<SessionInfoCell>();
    let type_of = |cell: &Arc<dyn HistoryCell>| cell.as_any().type_id();

    cells
        .iter()
        .rposition(|cell| type_of(cell) == session_start_type)
        .map_or(0, |idx| idx + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::ReviewDecision;
    use codex_app_server_protocol::AskForApproval;
    use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
    use codex_protocol::ThreadId;
    use codex_protocol::config_types::ApprovalsReviewer;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use std::time::Instant;
    use tempfile::TempDir;

    use crate::diff_model::FileChange;
    use crate::exec_cell::CommandOutput;
    use crate::history_cell;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use crate::history_cell::new_patch_event;
    use crate::history_cell::new_session_info;
    use crate::legacy_core::config::ConfigBuilder;
    use crate::session_state::ThreadSessionState;
    use codex_protocol::parse_command::ParsedCommand;
    use ratatui::Terminal as RatatuiTerminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    use ratatui::text::Text;

    #[derive(Debug)]
    struct TestCell {
        lines: Vec<Line<'static>>,
    }

    #[derive(Debug)]
    struct CountingTestCell {
        lines: Vec<Line<'static>>,
        transcript_render_count: Arc<AtomicUsize>,
    }

    impl crate::history_cell::HistoryCell for TestCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn raw_lines(&self) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }
    }

    impl crate::history_cell::HistoryCell for CountingTestCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn raw_lines(&self) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.transcript_render_count.fetch_add(1, Ordering::SeqCst);
            self.lines.clone()
        }
    }

    fn paragraph_block(label: &str, lines: usize) -> Box<dyn Renderable> {
        let text = Text::from(
            (0..lines)
                .map(|i| Line::from(format!("{label}{i}")))
                .collect::<Vec<_>>(),
        );
        Box::new(Paragraph::new(text)) as Box<dyn Renderable>
    }

    fn default_pager_keymap() -> crate::keymap::PagerKeymap {
        crate::keymap::RuntimeKeymap::defaults().pager
    }

    fn transcript_overlay(cells: Vec<Arc<dyn HistoryCell>>) -> TranscriptOverlay {
        let keymap = crate::keymap::RuntimeKeymap::defaults();
        TranscriptOverlay::new(
            cells,
            keymap.pager,
            keymap.app.copy,
            keymap.app.toggle_raw_output,
            TranscriptOverlayState::new(HistoryRenderMode::Rich),
        )
    }

    fn user_cell(message: &str) -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            message: message.to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        })
    }

    fn synthetic_transcript(prompt_count: usize) -> Vec<Arc<dyn HistoryCell>> {
        let mut cells = Vec::with_capacity(prompt_count.saturating_mul(2));
        for i in 0..prompt_count {
            cells.push(user_cell(&format!("prompt {i}")));
            cells.push(Arc::new(AgentMessageCell::new(
                vec![
                    Line::from(format!("assistant response {i}")),
                    Line::from(
                        "additional detail to make wrapping and height measurement realistic",
                    ),
                ],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>);
        }
        cells
    }

    fn large_test_cells() -> Vec<Arc<dyn HistoryCell>> {
        (0..ASYNC_TRANSCRIPT_LAYOUT_CELL_THRESHOLD)
            .map(|idx| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("line-{idx}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect()
    }

    fn measured_layout_for(
        overlay: &TranscriptOverlay,
        width: u16,
        height: usize,
    ) -> TranscriptLayoutResult {
        let cell_count = overlay.committed_cell_count();
        TranscriptLayoutResult {
            key: overlay.layout_key(width),
            heights: vec![height; cell_count],
            total_height: cell_count.saturating_mul(height),
        }
    }

    fn synthetic_search_transcript(prompt_count: usize) -> Vec<Arc<dyn HistoryCell>> {
        let mut cells = Vec::with_capacity(prompt_count.saturating_mul(2));
        for i in 0..prompt_count {
            cells.push(user_cell(&format!("prompt {i} main branch")));
            cells.push(Arc::new(AgentMessageCell::new(
                vec![
                    Line::from(format!("assistant response {i} main branch")),
                    Line::from(
                        "additional main detail to make wrapping and height measurement realistic",
                    ),
                ],
                /*is_first_line*/ true,
            )) as Arc<dyn HistoryCell>);
        }
        cells
    }

    fn session_info_cell(cwd: &str) -> Arc<dyn HistoryCell> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let temp_dir = TempDir::new().expect("tempdir");
        let config = runtime
            .block_on(
                ConfigBuilder::default()
                    .codex_home(temp_dir.path().to_path_buf())
                    .build(),
            )
            .expect("config");
        let session = ThreadSessionState {
            thread_id: ThreadId::new(),
            forked_from_id: None,
            fork_parent_title: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            permission_profile: PermissionProfile::read_only(),
            active_permission_profile: None,
            cwd: test_path_buf(cwd).abs(),
            runtime_workspace_roots: Vec::new(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: Some(ReasoningEffortConfig::High),
            collaboration_mode: None,
            personality: None,
            message_history: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        };
        Arc::new(new_session_info(
            &config, "gpt-test", &session, /*is_first_event*/ false,
            /*tooltip_override*/ None, /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )) as Arc<dyn HistoryCell>
    }

    fn static_overlay(lines: Vec<Line<'static>>, title: &str) -> StaticOverlay {
        StaticOverlay::with_title(lines, title.to_string(), default_pager_keymap())
    }

    fn pager_view(
        renderables: Vec<Box<dyn Renderable>>,
        title: &str,
        scroll_offset: usize,
    ) -> PagerView {
        PagerView::new(
            renderables,
            title.to_string(),
            scroll_offset,
            default_pager_keymap(),
        )
    }

    #[test]
    fn edit_prev_hint_is_visible() {
        let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
            lines: vec![Line::from("hello")],
        })]);

        // Render into a wide buffer so the footer hints aren't truncated.
        let area = Rect::new(0, 0, 120, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let s = buffer_to_text(&buf, area);
        assert!(
            s.contains("edit prev"),
            "expected 'edit prev' hint in overlay footer, got: {s:?}"
        );
    }

    #[test]
    fn edit_next_hint_is_visible_when_highlighted() {
        let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
            lines: vec![Line::from("hello")],
        })]);
        overlay.set_highlight_cell(Some(0));

        // Render into a wide buffer so the footer hints aren't truncated.
        let area = Rect::new(0, 0, 120, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let s = buffer_to_text(&buf, area);
        assert!(
            s.contains("edit next"),
            "expected 'edit next' hint in overlay footer, got: {s:?}"
        );
    }

    #[test]
    fn transcript_overlay_snapshot_basic() {
        // Prepare a transcript overlay with a few lines
        let mut overlay = transcript_overlay(vec![
            Arc::new(TestCell {
                lines: vec![Line::from("alpha")],
            }),
            Arc::new(TestCell {
                lines: vec![Line::from("beta")],
            }),
            Arc::new(TestCell {
                lines: vec![Line::from("gamma")],
            }),
        ]);
        let mut term = RatatuiTerminal::new(TestBackend::new(40, 10)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    #[test]
    fn transcript_overlay_preserves_semantic_web_links() {
        let destination = "https://example.com/a/very/long/path";
        let mut overlay = transcript_overlay(vec![Arc::new(history_cell::AgentMarkdownCell::new(
            destination.to_string(),
            std::path::Path::new("/tmp"),
        ))]);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 10,
        );
        let mut buf = Buffer::empty(area);

        overlay.render(area, &mut buf);

        assert!(area.positions().any(|position| {
            buf[position]
                .symbol()
                .contains(&format!("\x1b]8;;{destination}\x07"))
        }));
    }

    #[test]
    #[ignore = "local performance probe for transcript prompt selection"]
    fn transcript_prompt_selection_perf() {
        const PROMPTS: usize = 1_500;
        const STEPS: usize = 300;
        const WIDTH: u16 = 120;
        const HEIGHT: u16 = 40;

        let cells = synthetic_transcript(PROMPTS);
        let cell_count = cells.len();
        let mut selection_only = transcript_overlay(cells.clone());
        selection_only.set_highlight_cell(Some(cell_count.saturating_sub(2)));

        let selection_start = Instant::now();
        for _ in 0..STEPS {
            selection_only.move_prompt_selection(PromptSelectionDirection::Previous);
            selection_only.move_prompt_selection(PromptSelectionDirection::Next);
        }
        let selection_elapsed = selection_start.elapsed();

        let mut selection_plus_render = transcript_overlay(cells);
        selection_plus_render.set_highlight_cell(Some(cell_count.saturating_sub(2)));
        let mut term = RatatuiTerminal::new(TestBackend::new(WIDTH, HEIGHT)).expect("term");

        let render_start = Instant::now();
        for _ in 0..STEPS {
            selection_plus_render.move_prompt_selection(PromptSelectionDirection::Previous);
            term.draw(|f| selection_plus_render.render(f.area(), f.buffer_mut()))
                .expect("draw previous");
            selection_plus_render.move_prompt_selection(PromptSelectionDirection::Next);
            term.draw(|f| selection_plus_render.render(f.area(), f.buffer_mut()))
                .expect("draw next");
        }
        let render_elapsed = render_start.elapsed();

        let operations = STEPS.saturating_mul(2);
        let mut stdout = std::io::stdout().lock();
        writeln!(
            stdout,
            "transcript_prompt_selection_perf prompts={PROMPTS} cells={cell_count} steps={operations} selection_only_ms={:.3} selection_only_avg_us={:.3} selection_plus_render_ms={:.3} selection_plus_render_avg_us={:.3}",
            selection_elapsed.as_secs_f64() * 1_000.0,
            selection_elapsed.as_secs_f64() * 1_000_000.0 / operations as f64,
            render_elapsed.as_secs_f64() * 1_000.0,
            render_elapsed.as_secs_f64() * 1_000_000.0 / operations as f64,
        )
        .expect("write perf output");
    }

    #[test]
    #[ignore = "local performance probe for transcript search"]
    fn transcript_search_perf() {
        const PROMPTS: usize = 1_500;
        const WIDTH: u16 = 120;
        let mut output = std::io::stdout();
        let mut overlay = transcript_overlay(synthetic_search_transcript(PROMPTS));
        overlay.activate_search();
        overlay.last_content_width = Some(WIDTH);

        let corpus_start = Instant::now();
        overlay.ensure_search_corpus(WIDTH);
        let corpus_elapsed = corpus_start.elapsed();

        overlay.append_search_text("m");
        let first_query_start = Instant::now();
        overlay.flush_pending_search();
        let first_query_elapsed = first_query_start.elapsed();

        overlay.append_search_text("ain");
        let refine_start = Instant::now();
        overlay.flush_pending_search();
        let refine_elapsed = refine_start.elapsed();

        overlay.search_backspace();
        let backspace_start = Instant::now();
        overlay.flush_pending_search();
        let backspace_elapsed = backspace_start.elapsed();

        let navigation_start = Instant::now();
        for _ in 0..300 {
            overlay.select_next_match();
        }
        let navigation_elapsed = navigation_start.elapsed();

        writeln!(
            output,
            "transcript_search_perf prompts={PROMPTS} corpus_ms={:.3} first_query_ms={:.3} refine_ms={:.3} backspace_ms={:.3} navigation_ms={:.3} navigation_avg_us={:.3} matches={}",
            corpus_elapsed.as_secs_f64() * 1_000.0,
            first_query_elapsed.as_secs_f64() * 1_000.0,
            refine_elapsed.as_secs_f64() * 1_000.0,
            backspace_elapsed.as_secs_f64() * 1_000.0,
            navigation_elapsed.as_secs_f64() * 1_000.0,
            navigation_elapsed.as_secs_f64() * 1_000_000.0 / 300.0,
            overlay.search.matches.len(),
        )
        .expect("write perf output");
    }

    #[test]
    fn transcript_overlay_renders_live_tail() {
        let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
            lines: vec![Line::from("alpha")],
        })]);
        overlay.sync_live_tail(
            /*width*/ 40,
            Some(ActiveCellTranscriptKey {
                revision: 1,
                is_stream_continuation: false,
                animation_tick: None,
            }),
            |_| Some(vec![HyperlinkLine::from("tail")]),
        );

        let mut term = RatatuiTerminal::new(TestBackend::new(40, 10)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    #[test]
    fn transcript_overlay_live_tail_preserves_semantic_web_links() {
        let destination = "https://example.com/a/streamed/path";
        let cell = history_cell::AgentMarkdownCell::new(
            destination.to_string(),
            std::path::Path::new("/tmp"),
        );
        let mut overlay = transcript_overlay(Vec::new());
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 10,
        );
        let mut buf = Buffer::empty(area);

        overlay.sync_live_tail(
            area.width,
            Some(ActiveCellTranscriptKey {
                revision: 1,
                is_stream_continuation: false,
                animation_tick: None,
            }),
            |width| Some(cell.transcript_hyperlink_lines(width)),
        );
        overlay.render(area, &mut buf);

        assert!(area.positions().any(|position| {
            buf[position]
                .symbol()
                .contains(&format!("\x1b]8;;{destination}\x07"))
        }));
    }

    #[test]
    fn transcript_overlay_search_matches_all_text_case_insensitively() {
        let mut overlay = transcript_overlay(vec![
            user_cell("Prompt with NeedLe"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant needle")],
                /*is_first_line*/ true,
            )),
        ]);

        overlay.activate_search();
        overlay.append_search_text("needle");
        overlay.recompute_search_matches(/*width*/ 80);

        assert_eq!(overlay.search.matches.len(), 2);
        assert_eq!(overlay.search.active_match, Some(0));
    }

    #[test]
    fn transcript_overlay_search_reuses_corpus_for_query_edits() {
        let transcript_render_count = Arc::new(AtomicUsize::new(0));
        let mut overlay = transcript_overlay(vec![Arc::new(CountingTestCell {
            lines: vec![Line::from("main branch")],
            transcript_render_count: Arc::clone(&transcript_render_count),
        })]);

        overlay.activate_search();
        overlay.append_search_text("m");
        overlay.recompute_search_matches(/*width*/ 80);
        overlay.append_search_text("ain");
        overlay.recompute_search_matches(/*width*/ 80);

        assert_eq!(transcript_render_count.load(Ordering::SeqCst), 1);
        assert_eq!(overlay.search.matches.len(), 1);
    }

    #[test]
    fn transcript_overlay_search_rebuilds_corpus_after_width_change() {
        let transcript_render_count = Arc::new(AtomicUsize::new(0));
        let mut overlay = transcript_overlay(vec![Arc::new(CountingTestCell {
            lines: vec![Line::from("main branch")],
            transcript_render_count: Arc::clone(&transcript_render_count),
        })]);

        overlay.activate_search();
        overlay.append_search_text("main");
        overlay.recompute_search_matches(/*width*/ 80);
        overlay.recompute_search_matches(/*width*/ 40);

        assert_eq!(transcript_render_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn transcript_overlay_search_debounces_query_recompute() {
        let mut overlay = transcript_overlay(vec![user_cell("main branch")]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        overlay.activate_search();
        overlay.append_search_text("main");

        overlay.render(area, &mut buf);
        assert!(buffer_to_text(&buf, area).contains("searching…"));
        assert!(overlay.search.matches.is_empty());

        overlay.search.recompute_at = Some(Instant::now());
        overlay.render(area, &mut buf);

        assert_eq!(overlay.search.matches.len(), 1);
        assert!(!buffer_to_text(&buf, area).contains("searching…"));
    }

    #[test]
    fn transcript_overlay_search_prompt_navigation_uses_match_owner() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant one")],
                /*is_first_line*/ true,
            )),
            user_cell("second prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant needle")],
                /*is_first_line*/ true,
            )),
        ]);

        overlay.activate_search();
        overlay.append_search_text("needle");
        overlay.recompute_search_matches(/*width*/ 80);
        overlay.navigate_search_prompt(PromptSelectionDirection::Previous);

        assert_eq!(overlay.selected_user_cell(), Some(0));
    }

    #[test]
    fn transcript_overlay_search_highlights_active_match_and_renders_search_row() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant needle here")],
                /*is_first_line*/ true,
            )),
        ]);
        overlay.activate_search();
        overlay.append_search_text("needle");
        overlay.recompute_search_matches(/*width*/ 60);

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        let rendered = buffer_to_text(&buf, area);

        assert!(rendered.contains("Search: needle"));
        assert!(
            (area.y..area.bottom())
                .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
                .any(|(x, y)| {
                    buf[(x, y)].symbol() == "n"
                        && buf[(x, y)]
                            .style()
                            .add_modifier
                            .contains(Modifier::REVERSED)
                }),
            "expected a reversed search match cell"
        );
    }

    #[test]
    fn transcript_overlay_search_highlights_wrapped_later_match_text() {
        let mut overlay = transcript_overlay(vec![user_cell(
            "main first hit with enough padding words to push the second main hit onto a wrapped line",
        )]);
        overlay.activate_search();
        overlay.append_search_text("main");
        overlay.recompute_search_matches(/*width*/ 32);
        overlay.select_next_match();

        let area = Rect::new(0, 0, 32, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let highlighted = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let cell = &buf[(x, y)];
                cell.style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
                    .then(|| cell.symbol().to_string())
            })
            .collect::<String>();

        assert_eq!(highlighted, "main");
    }

    #[test]
    fn transcript_overlay_search_highlights_match_after_viewport_wrapping() {
        let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
            lines: vec![Line::from(format!(
                "main first hit {}main second hit",
                "padding ".repeat(16)
            ))],
        })]);
        overlay.activate_search();
        overlay.append_search_text("main");
        overlay.recompute_search_matches(/*width*/ 24);
        overlay.select_next_match();

        let area = Rect::new(0, 0, 24, 8);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let highlighted = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let cell = &buf[(x, y)];
                cell.style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
                    .then(|| cell.symbol().to_string())
            })
            .collect::<String>();

        assert_eq!(highlighted, "main");
    }

    #[test]
    fn transcript_overlay_search_scrolls_initial_match_into_view() {
        let mut overlay = transcript_overlay(vec![
            user_cell("main first hit"),
            Arc::new(TestCell {
                lines: (0..24)
                    .map(|line| Line::from(format!("tail line {line}")))
                    .collect(),
            }),
        ]);
        overlay.activate_search();
        overlay.append_search_text("main");
        overlay.recompute_search_matches(/*width*/ 40);

        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let highlighted = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let cell = &buf[(x, y)];
                cell.style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
                    .then(|| cell.symbol().to_string())
            })
            .collect::<String>();

        assert_eq!(highlighted, "main");
    }

    #[test]
    fn transcript_overlay_search_snapshot() {
        let mut overlay = transcript_overlay(vec![
            user_cell("find this prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant needle result")],
                /*is_first_line*/ true,
            )),
        ]);
        overlay.activate_search();
        overlay.append_search_text("needle");
        overlay.recompute_search_matches(/*width*/ 50);

        let mut term = RatatuiTerminal::new(TestBackend::new(50, 10)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    #[test]
    fn transcript_overlay_state_round_trips_scroll_selection_and_mode() {
        let keymap = crate::keymap::RuntimeKeymap::defaults();
        let state = TranscriptOverlayState {
            scroll_offset: 3,
            highlight_cell: Some(0),
            render_mode: HistoryRenderMode::Raw,
        };
        let overlay = TranscriptOverlay::new(
            vec![user_cell("prompt")],
            keymap.pager,
            keymap.app.copy,
            keymap.app.toggle_raw_output,
            state,
        );

        assert_eq!(overlay.state(), state);
    }

    #[test]
    fn prompt_navigation_moves_between_user_prompts() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first"),
            Arc::new(TestCell {
                lines: vec![Line::from("assistant")],
            }),
            user_cell("second"),
        ]);

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(overlay.selected_user_cell(), Some(2));

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(overlay.selected_user_cell(), Some(0));

        overlay.move_prompt_selection(PromptSelectionDirection::Next);
        assert_eq!(overlay.selected_user_cell(), Some(2));
    }

    #[test]
    fn transcript_header_title_is_stable() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant")],
                /*is_first_line*/ true,
            )),
            user_cell("second"),
        ]);

        assert_eq!(overlay.header_title(), "Transcript");

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(overlay.header_title(), "Transcript");

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(overlay.header_title(), "Transcript");
    }

    #[test]
    fn transcript_footer_progress_label_counts_selected_user_prompt() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant")],
                /*is_first_line*/ true,
            )),
            user_cell("second"),
        ]);

        assert_eq!(
            overlay.footer_progress_label(
                /*content_height*/ 5, /*total_len*/ 12, /*width*/ 80
            ),
            " 2 / 2 · 100% "
        );

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(
            overlay.footer_progress_label(
                /*content_height*/ 5, /*total_len*/ 12, /*width*/ 80
            ),
            " 2 / 2 · 100% "
        );

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(
            overlay.footer_progress_label(
                /*content_height*/ 5, /*total_len*/ 12, /*width*/ 80
            ),
            " 1 / 2 · 100% "
        );
    }

    #[test]
    fn transcript_scroll_selects_prompt_when_its_first_line_enters_from_below() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first"),
            Arc::new(TestCell {
                lines: (0..8)
                    .map(|idx| Line::from(format!("answer-{idx}")))
                    .collect(),
            }),
            user_cell("second"),
            Arc::new(TestCell {
                lines: vec![Line::from("after second")],
            }),
        ]);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 12,
        );
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        let top = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(3));
        let content_area = overlay.view.content_area(top);
        let second_row = {
            let layout = overlay.view.layout(content_area.width);
            layout.offsets[2].saturating_add(overlay.prompt_first_text_row_offset(/*idx*/ 2))
        };

        let before = second_row.saturating_sub(content_area.height as usize);
        let after = before.saturating_add(1);
        let selected = overlay.prompt_entering_viewport(
            content_area.width,
            content_area.height,
            before,
            after,
        );
        overlay.view.scroll_offset = after;
        overlay.set_highlight_cell_preserving_viewport(selected);
        overlay.render(area, &mut buf);

        assert_eq!(selected, Some(2));
        assert_eq!(overlay.selected_user_cell(), Some(2));
        assert_eq!(overlay.view.scroll_offset, after);
        assert_snapshot!(
            "transcript_scroll_selects_prompt_entering_from_below",
            buffer_to_text(&buf, area)
        );
    }

    #[test]
    fn transcript_scroll_selects_prompt_when_its_first_line_enters_from_above() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first"),
            Arc::new(TestCell {
                lines: (0..16)
                    .map(|idx| Line::from(format!("answer-{idx}")))
                    .collect(),
            }),
        ]);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 12,
        );
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        let top = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(3));
        let content_area = overlay.view.content_area(top);

        let selected = overlay.prompt_entering_viewport(
            content_area.width,
            content_area.height,
            /*before*/ 2,
            /*after*/ 1,
        );
        overlay.view.scroll_offset = 1;
        overlay.set_highlight_cell_preserving_viewport(selected);

        assert_eq!(selected, Some(0));
        assert_eq!(overlay.selected_user_cell(), Some(0));
        assert_eq!(overlay.view.scroll_offset, 1);
    }

    #[test]
    fn explicit_prompt_selection_anchors_prompt_in_upper_third() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first"),
            Arc::new(TestCell {
                lines: (0..12)
                    .map(|idx| Line::from(format!("before-{idx}")))
                    .collect(),
            }),
            user_cell("second"),
            Arc::new(TestCell {
                lines: (0..12)
                    .map(|idx| Line::from(format!("after-{idx}")))
                    .collect(),
            }),
        ]);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 15,
        );
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        overlay.set_highlight_cell(Some(2));
        overlay.render(area, &mut buf);

        let top = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(3));
        let content_area = overlay.view.content_area(top);
        let selected_row = {
            let layout = overlay.view.layout(content_area.width);
            layout.offsets[2].saturating_add(overlay.prompt_first_text_row_offset(/*idx*/ 2))
        };
        assert_eq!(
            selected_row.saturating_sub(overlay.view.scroll_offset),
            (content_area.height as usize) / 3,
        );
        assert_snapshot!(
            "explicit_prompt_selection_anchors_prompt_in_upper_third",
            buffer_to_text(&buf, area)
        );
    }

    #[test]
    fn transcript_prompt_selection_ignores_prompts_before_latest_session_header() {
        let mut overlay = transcript_overlay(vec![
            user_cell("old prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("old assistant")],
                /*is_first_line*/ true,
            )),
            session_info_cell("/tmp/project"),
            user_cell("current first"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("current assistant")],
                /*is_first_line*/ true,
            )),
            user_cell("current second"),
        ]);

        assert_eq!(overlay.user_prompt_count(), 2);
        assert_eq!(overlay.header_title(), "Transcript");
        assert_eq!(
            overlay.footer_progress_label(
                /*content_height*/ 5, /*total_len*/ 12, /*width*/ 80
            ),
            " 2 / 2 · 100% "
        );

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(overlay.selected_user_cell(), Some(5));
        assert_eq!(
            overlay.footer_progress_label(
                /*content_height*/ 5, /*total_len*/ 12, /*width*/ 80
            ),
            " 2 / 2 · 100% "
        );

        overlay.move_prompt_selection(PromptSelectionDirection::Previous);
        assert_eq!(overlay.selected_user_cell(), Some(3));
        assert_eq!(
            overlay.footer_progress_label(
                /*content_height*/ 5, /*total_len*/ 12, /*width*/ 80
            ),
            " 1 / 2 · 100% "
        );

        assert_eq!(
            overlay.set_highlighted_user_prompt(/*nth_user_message*/ 2),
            None
        );
    }

    #[test]
    fn selected_user_prompt_keeps_reversed_style_without_role_gutter() {
        let mut overlay = transcript_overlay(vec![user_cell("selected prompt")]);
        overlay.set_highlight_cell(Some(0));
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 8,
        );
        let mut buf = Buffer::empty(area);

        overlay.render(area, &mut buf);
        let rendered = buffer_to_text(&buf, area);

        assert!(!rendered.contains('▌'));
        assert!(!rendered.contains('│'));
        let prompt_marker = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .find(|(x, y)| buf[(*x, *y)].symbol() == "›")
            .expect("expected selected prompt marker");
        assert!(
            buf[prompt_marker]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn prompt_selection_updates_style_without_rebuilding_layout() {
        let mut overlay = transcript_overlay(vec![
            user_cell("first prompt"),
            Arc::new(AgentMessageCell::new(
                vec![Line::from("assistant")],
                /*is_first_line*/ true,
            )),
            user_cell("second prompt"),
        ]);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 14,
        );
        let mut buf = Buffer::empty(area);

        overlay.render(area, &mut buf);
        assert!(overlay.view.layout.is_some());
        let renderable_count = overlay.view.renderables.len();

        overlay.set_highlight_cell(Some(0));
        assert!(overlay.view.layout.is_some());
        assert_eq!(overlay.view.renderables.len(), renderable_count);
        overlay.render(area, &mut buf);
        let first_selection = prompt_marker_reversed_states(&buf, area);
        assert_eq!(first_selection, vec![true, false]);

        overlay.set_highlight_cell(Some(2));
        assert!(overlay.view.layout.is_some());
        assert_eq!(overlay.view.renderables.len(), renderable_count);
        overlay.render(area, &mut buf);
        let second_selection = prompt_marker_reversed_states(&buf, area);
        assert_eq!(second_selection, vec![false, true]);
    }

    #[test]
    fn large_transcript_renders_loading_until_layout_is_ready() {
        let mut overlay = transcript_overlay(large_test_cells());
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
        );
        let mut buf = Buffer::empty(area);

        overlay.render(area, &mut buf);

        assert!(overlay.view.layout.is_none());
        assert_snapshot!(
            "large_transcript_renders_loading_until_layout_is_ready",
            buffer_to_text(&buf, area)
        );
    }

    #[test]
    fn large_transcript_applies_matching_precomputed_layout() {
        let mut overlay = transcript_overlay(large_test_cells());
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
        );
        let result = measured_layout_for(&overlay, /*width*/ 80, /*height*/ 1);
        let committed_cell_count = overlay.committed_cell_count();

        assert!(overlay.apply_layout_result(result));

        let layout = overlay.view.layout(/*width*/ 80);
        assert_eq!(layout.heights.len(), committed_cell_count);
        assert_eq!(layout.total_height, committed_cell_count);

        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        assert!(!buffer_to_text(&buf, area).contains("Loading transcript"));
    }

    #[test]
    fn large_transcript_ignores_stale_layout_result() {
        let mut overlay = transcript_overlay(large_test_cells());
        let mut result = measured_layout_for(&overlay, /*width*/ 80, /*height*/ 1);
        result.key.generation = result.key.generation.saturating_add(/*rhs*/ 1);

        assert!(!overlay.apply_layout_result(result));
        assert!(overlay.view.layout.is_none());
    }

    #[test]
    fn prompt_selection_anchors_after_async_layout_arrives() {
        let cells: Vec<Arc<dyn HistoryCell>> = (0..ASYNC_TRANSCRIPT_LAYOUT_CELL_THRESHOLD)
            .map(|idx| user_cell(&format!("prompt {idx}")))
            .collect();
        let mut overlay = transcript_overlay(cells);
        let selected = overlay.committed_cell_count() / 2;
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 15,
        );
        let top = Rect::new(
            area.x,
            area.y,
            area.width,
            area.height.saturating_sub(/*rhs*/ 3),
        );
        let content_area = overlay.view.content_area(top);

        overlay.set_highlight_cell(Some(selected));
        overlay.render(area, &mut Buffer::empty(area));
        assert!(overlay.view.layout.is_none());

        let result = measured_layout_for(&overlay, /*width*/ 80, /*height*/ 3);
        assert!(overlay.apply_layout_result(result));
        overlay.render(area, &mut Buffer::empty(area));

        let selected_row = {
            let layout = overlay.view.layout(content_area.width);
            layout.offsets[selected].saturating_add(overlay.prompt_first_text_row_offset(selected))
        };
        assert_eq!(
            selected_row.saturating_sub(overlay.view.scroll_offset),
            (content_area.height as usize) / 3,
        );
    }

    fn prompt_marker_reversed_states(buf: &Buffer, area: Rect) -> Vec<bool> {
        let mut states = Vec::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if buf[(x, y)].symbol() == "›" {
                    states.push(
                        buf[(x, y)]
                            .style()
                            .add_modifier
                            .contains(Modifier::REVERSED),
                    );
                }
            }
        }
        states
    }

    #[test]
    fn transcript_overlay_sync_live_tail_is_noop_for_identical_key() {
        let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
            lines: vec![Line::from("alpha")],
        })]);

        let calls = std::cell::Cell::new(0usize);
        let key = ActiveCellTranscriptKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        };

        overlay.sync_live_tail(/*width*/ 40, Some(key), |_| {
            calls.set(calls.get() + 1);
            Some(vec![HyperlinkLine::from("tail")])
        });
        overlay.sync_live_tail(/*width*/ 40, Some(key), |_| {
            calls.set(calls.get() + 1);
            Some(vec![HyperlinkLine::from("tail2")])
        });

        assert_eq!(calls.get(), 1);
    }

    fn buffer_to_text(buf: &Buffer, area: Rect) -> String {
        let mut out = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let symbol = buf[(x, y)].symbol();
                if symbol.is_empty() {
                    out.push(' ');
                } else {
                    out.push(symbol.chars().next().unwrap_or(' '));
                }
            }
            // Trim trailing spaces for stability.
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
        }
        out
    }

    fn render_snapshot(overlay: &mut TranscriptOverlay, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        buffer_to_text(&buf, area)
    }

    #[test]
    fn transcript_overlay_apply_patch_scroll_vt100_clears_previous_page() {
        let cwd = PathBuf::from("/repo");
        let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();

        let mut approval_changes = HashMap::new();
        approval_changes.insert(
            PathBuf::from("foo.txt"),
            FileChange::Add {
                content: "hello\nworld\n".to_string(),
            },
        );
        let approval_cell: Arc<dyn HistoryCell> = Arc::new(new_patch_event(approval_changes, &cwd));
        cells.push(approval_cell);

        let mut apply_changes = HashMap::new();
        apply_changes.insert(
            PathBuf::from("foo.txt"),
            FileChange::Add {
                content: "hello\nworld\n".to_string(),
            },
        );
        let apply_begin_cell: Arc<dyn HistoryCell> = Arc::new(new_patch_event(apply_changes, &cwd));
        cells.push(apply_begin_cell);

        let apply_end_cell: Arc<dyn HistoryCell> = history_cell::new_approval_decision_cell(
            history_cell::ApprovalDecisionSubject::Command(vec!["ls".into()]),
            ReviewDecision::Approved,
            history_cell::ApprovalDecisionActor::User,
        )
        .into();
        cells.push(apply_end_cell);

        let mut exec_cell = crate::exec_cell::new_active_exec_command(
            "exec-1".into(),
            vec!["bash".into(), "-lc".into(), "ls".into()],
            vec![ParsedCommand::Unknown { cmd: "ls".into() }],
            ExecCommandSource::Agent,
            /*interaction_input*/ None,
            /*animations_enabled*/ true,
        );
        exec_cell.complete_call(
            "exec-1",
            CommandOutput {
                exit_code: 0,
                aggregated_output: "src\nREADME.md\n".into(),
                formatted_output: "src\nREADME.md\n".into(),
            },
            Duration::from_millis(420),
        );
        let exec_cell: Arc<dyn HistoryCell> = Arc::new(exec_cell);
        cells.push(exec_cell);

        let mut overlay = transcript_overlay(cells);
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);

        overlay.render(area, &mut buf);
        overlay.view.scroll_offset = 0;
        overlay.render(area, &mut buf);

        let snapshot = buffer_to_text(&buf, area);
        assert_snapshot!("transcript_overlay_apply_patch_scroll_vt100", snapshot);
    }

    #[test]
    fn transcript_overlay_footer_status_snapshot() {
        let mut overlay = transcript_overlay(vec![user_cell("prompt")]);
        overlay.show_copy_status_at(
            &CopyStatus::Success("Copied selected turn to clipboard".into()),
            Instant::now(),
        );

        assert_snapshot!(
            "transcript_overlay_footer_status",
            render_snapshot(
                &mut overlay,
                Rect::new(
                    /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8
                ),
            )
        );
    }

    #[test]
    fn transcript_overlay_footer_status_snapshot_narrow() {
        let mut overlay = transcript_overlay(vec![user_cell("prompt")]);
        overlay.show_copy_status_at(
            &CopyStatus::Error("No agent response to copy for selected prompt".into()),
            Instant::now(),
        );

        assert_snapshot!(
            "transcript_overlay_footer_status_narrow",
            render_snapshot(
                &mut overlay,
                Rect::new(
                    /*x*/ 0, /*y*/ 0, /*width*/ 28, /*height*/ 8
                ),
            )
        );
    }

    #[test]
    fn transcript_overlay_footer_status_can_be_cleared_immediately() {
        let mut overlay = transcript_overlay(vec![user_cell("prompt")]);
        overlay.show_copy_status_at(
            &CopyStatus::Success("Copied selected turn to clipboard".into()),
            Instant::now(),
        );
        assert!(overlay.clear_footer_status());

        assert!(overlay.footer_status.is_none());
    }

    #[test]
    fn transcript_overlay_footer_status_clears_after_expiry() {
        let mut overlay = transcript_overlay(vec![user_cell("prompt")]);
        overlay.show_copy_status_at(
            &CopyStatus::Success("Copied selected turn to clipboard".into()),
            Instant::now() - FOOTER_STATUS_TTL,
        );

        let _ = render_snapshot(
            &mut overlay,
            Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 8,
            ),
        );

        assert!(overlay.footer_status.is_none());
    }

    #[test]
    fn transcript_overlay_footer_status_replaces_previous_message() {
        let mut overlay = transcript_overlay(vec![user_cell("prompt")]);
        overlay.show_copy_status_at(
            &CopyStatus::Error("Copy failed: blocked".into()),
            Instant::now(),
        );
        overlay.show_copy_status_at(
            &CopyStatus::Success("Copied selected turn to clipboard".into()),
            Instant::now(),
        );

        let status = overlay.footer_status.as_ref().expect("status").line.clone();
        assert_eq!(status.spans.len(), 1);
        assert_eq!(
            status.spans[0].content.as_ref(),
            "Copied selected turn to clipboard"
        );
    }

    #[test]
    fn transcript_overlay_keeps_scroll_pinned_at_bottom() {
        let mut overlay = transcript_overlay(
            (0..20)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        let mut term = RatatuiTerminal::new(TestBackend::new(40, 12)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");

        assert!(
            overlay.view.is_scrolled_to_bottom(),
            "expected initial render to leave view at bottom"
        );

        overlay.insert_cell(Arc::new(TestCell {
            lines: vec!["tail".into()],
        }));

        assert_eq!(overlay.view.scroll_offset, usize::MAX);
    }

    #[test]
    fn transcript_overlay_preserves_manual_scroll_position() {
        let mut overlay = transcript_overlay(
            (0..20)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        let mut term = RatatuiTerminal::new(TestBackend::new(40, 12)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");

        overlay.view.scroll_offset = 0;

        overlay.insert_cell(Arc::new(TestCell {
            lines: vec!["tail".into()],
        }));

        assert_eq!(overlay.view.scroll_offset, 0);
    }

    #[test]
    fn transcript_overlay_consolidation_remaps_highlight_inside_range() {
        let mut overlay = transcript_overlay(
            (0..6)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        overlay.set_highlight_cell(Some(3));

        overlay.consolidate_cells(
            2..5,
            Arc::new(TestCell {
                lines: vec![Line::from("consolidated")],
            }),
        );

        assert_eq!(
            overlay.highlight_cell.get(),
            Some(2),
            "highlight inside consolidated range should point to replacement cell",
        );
    }

    #[test]
    fn transcript_overlay_consolidation_remaps_highlight_after_range() {
        let mut overlay = transcript_overlay(
            (0..7)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        overlay.set_highlight_cell(Some(6));

        overlay.consolidate_cells(
            2..5,
            Arc::new(TestCell {
                lines: vec![Line::from("consolidated")],
            }),
        );

        assert_eq!(
            overlay.highlight_cell.get(),
            Some(4),
            "highlight after consolidated range should shift left by removed cells",
        );
    }

    #[test]
    fn static_overlay_snapshot_basic() {
        // Prepare a static overlay with a few lines and a title
        let mut overlay = static_overlay(
            vec!["one".into(), "two".into(), "three".into()],
            "S T A T I C",
        );
        let mut term = RatatuiTerminal::new(TestBackend::new(40, 10)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    /// Render transcript overlay and return visible line numbers (`line-NN`) in order.
    fn transcript_line_numbers(overlay: &mut TranscriptOverlay, area: Rect) -> Vec<usize> {
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        let top_h = area.height.saturating_sub(3);
        let top = Rect::new(area.x, area.y, area.width, top_h);
        let content_area = overlay.view.content_area(top);

        let mut nums = Vec::new();
        for y in content_area.y..content_area.bottom() {
            let mut line = String::new();
            for x in content_area.x..content_area.right() {
                line.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            if let Some(n) = line
                .split_whitespace()
                .find_map(|w| w.strip_prefix("line-"))
                .and_then(|s| s.parse().ok())
            {
                nums.push(n);
            }
        }
        nums
    }

    #[test]
    fn transcript_overlay_paging_is_continuous_and_round_trips() {
        let mut overlay = transcript_overlay(
            (0..50)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line-{i:02}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        let area = Rect::new(0, 0, 40, 15);

        // Prime layout so last_content_height is populated and paging uses the real content height.
        let mut buf = Buffer::empty(area);
        overlay.view.scroll_offset = 0;
        overlay.render(area, &mut buf);
        let page_height = overlay.view.page_height(area);

        // Scenario 1: starting from the top, PageDown should show the next page of content.
        overlay.view.scroll_offset = 0;
        let page1 = transcript_line_numbers(&mut overlay, area);
        let page1_len = page1.len();
        let expected_page1: Vec<usize> = (0..page1_len).collect();
        assert_eq!(
            page1, expected_page1,
            "first page should start at line-00 and show a full page of content"
        );

        overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_add(page_height);
        let page2 = transcript_line_numbers(&mut overlay, area);
        assert_eq!(
            page2.len(),
            page1_len,
            "second page should have the same number of visible lines as the first page"
        );
        let expected_page2_first = *page1.last().unwrap() + 1;
        assert_eq!(
            page2[0], expected_page2_first,
            "second page after PageDown should immediately follow the first page"
        );

        // Scenario 2: from an interior offset (start=3), PageDown then PageUp should round-trip.
        let interior_offset = 3usize;
        overlay.view.scroll_offset = interior_offset;
        let before = transcript_line_numbers(&mut overlay, area);
        overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_add(page_height);
        let _ = transcript_line_numbers(&mut overlay, area);
        overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_sub(page_height);
        let after = transcript_line_numbers(&mut overlay, area);
        assert_eq!(
            before, after,
            "PageDown+PageUp from interior offset ({interior_offset}) should round-trip"
        );

        // Scenario 3: from the top of the second page, PageUp then PageDown should round-trip.
        overlay.view.scroll_offset = page_height;
        let before2 = transcript_line_numbers(&mut overlay, area);
        overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_sub(page_height);
        let _ = transcript_line_numbers(&mut overlay, area);
        overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_add(page_height);
        let after2 = transcript_line_numbers(&mut overlay, area);
        assert_eq!(
            before2, after2,
            "PageUp+PageDown from the top of the second page should round-trip"
        );
    }

    #[test]
    fn static_overlay_wraps_long_lines() {
        let mut overlay = static_overlay(
            vec!["a very long line that should wrap when rendered within a narrow pager overlay width".into()],
            "S T A T I C",
        );
        let mut term = RatatuiTerminal::new(TestBackend::new(24, 8)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    #[test]
    fn pager_view_content_height_counts_renderables() {
        let mut pv = pager_view(
            vec![
                paragraph_block("a", /*lines*/ 2),
                paragraph_block("b", /*lines*/ 3),
            ],
            "T",
            /*scroll_offset*/ 0,
        );

        assert_eq!(pv.content_height(/*width*/ 80), 5);
    }

    #[test]
    fn pager_view_positions_selected_chunk_in_upper_third() {
        let mut pv = pager_view(
            vec![
                paragraph_block("a", /*lines*/ 1),
                paragraph_block("b", /*lines*/ 3),
                paragraph_block("c", /*lines*/ 3),
            ],
            "T",
            /*scroll_offset*/ 0,
        );
        let area = Rect::new(0, 0, 20, 8);

        let content_area = pv.content_area(area);
        pv.position_chunk_at_upper_third(/*idx*/ 2, /*row_offset*/ 0, content_area);

        assert_eq!(pv.scroll_offset, 2);
    }

    #[test]
    fn pager_view_upper_third_position_clamps_at_start() {
        let mut pv = pager_view(
            vec![
                paragraph_block("a", /*lines*/ 2),
                paragraph_block("b", /*lines*/ 3),
                paragraph_block("c", /*lines*/ 3),
            ],
            "T",
            /*scroll_offset*/ 0,
        );
        let area = Rect::new(0, 0, 20, 3);

        pv.scroll_offset = 6;
        pv.position_chunk_at_upper_third(/*idx*/ 0, /*row_offset*/ 0, area);

        assert_eq!(pv.scroll_offset, 0);
    }

    #[test]
    fn pager_view_is_scrolled_to_bottom_accounts_for_wrapped_height() {
        let mut pv = pager_view(
            vec![paragraph_block("a", /*lines*/ 10)],
            "T",
            /*scroll_offset*/ 0,
        );
        let area = Rect::new(0, 0, 20, 8);
        let mut buf = Buffer::empty(area);

        pv.render(area, &mut buf);

        assert!(
            !pv.is_scrolled_to_bottom(),
            "expected view to report not at bottom when offset < max"
        );

        pv.scroll_offset = usize::MAX;
        pv.render(area, &mut buf);

        assert!(
            pv.is_scrolled_to_bottom(),
            "expected view to report at bottom after scrolling to end"
        );
    }
}
