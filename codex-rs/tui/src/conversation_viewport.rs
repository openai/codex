//! Retained conversation rendering for an application-owned terminal viewport.
//!
//! This module intentionally owns only transcript projection and bottom-follow state. The app
//! decides when the terminal is owned and how much space remains after the bottom pane is laid
//! out. Committed cells use their main-viewport representation; the more detailed `Ctrl+T`
//! representation remains owned by `pager_overlay`.

use std::cell::Cell;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use crate::chatwidget::ActiveCellRenderKey;
use crate::conversation_selection::CellSelectionProjection;
use crate::conversation_selection::ConversationSelection;
use crate::conversation_selection::SelectionPoint;
use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;
use crate::keymap::PagerKeymap;
use crate::pager_overlay::BottomFollowMode;
use crate::pager_overlay::PagerContent;
use crate::render::Insets;
use crate::render::renderable::InsetRenderable;
use crate::render::renderable::Renderable;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::mark_buffer_hyperlinks;
use crate::terminal_hyperlinks::visible_lines;
use crate::tui::MouseScrollDirection;

pub(crate) struct ConversationViewport {
    content: PagerContent,
    cells: Vec<Arc<dyn HistoryCell>>,
    render_mode: HistoryRenderMode,
    live_tail_key: Option<LiveTailKey>,
    selection: ConversationSelection,
    selection_scroll_offset: Option<usize>,
    selection_projection_cache: Option<SelectionProjectionCache>,
}

struct SelectionProjectionCache {
    width: u16,
    render_mode: HistoryRenderMode,
    projections: Vec<Option<CellSelectionProjection>>,
    computed: Vec<bool>,
    layout: Vec<SelectionCellLayout>,
}

#[derive(Clone, Copy)]
struct SelectionCellLayout {
    top: usize,
    height: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiveTailKey {
    width: u16,
    revision: u64,
    is_stream_continuation: bool,
    animation_tick: Option<u64>,
}

impl ConversationViewport {
    pub(crate) fn new(
        cells: Vec<Arc<dyn HistoryCell>>,
        render_mode: HistoryRenderMode,
        keymap: PagerKeymap,
    ) -> Self {
        let renderables = Self::render_cells(&cells, render_mode);
        Self {
            content: PagerContent::new(renderables, keymap),
            cells,
            render_mode,
            live_tail_key: None,
            selection: ConversationSelection::default(),
            selection_scroll_offset: None,
            selection_projection_cache: None,
        }
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let bottom_follow = if self.selection.is_active() {
            BottomFollowMode::Frozen
        } else {
            BottomFollowMode::Enabled
        };
        self.content.render(area, buf, bottom_follow);
        if self.selection.is_active()
            && self
                .selection_scroll_offset
                .is_some_and(|offset| offset != self.content.scroll_offset())
        {
            self.cancel_selection();
        }
        if self.selection_projection_cache.is_some() {
            self.ensure_selection_projections(area.width);
            self.ensure_selected_projections();
            self.render_selection(area, buf);
        }
    }

    pub(crate) fn handle_navigation_key(&mut self, area: Rect, key_event: KeyEvent) -> bool {
        self.content.handle_navigation_key(area, key_event)
    }

    pub(crate) fn set_keymap(&mut self, keymap: PagerKeymap) {
        self.content.set_keymap(keymap);
    }

    pub(crate) fn handle_mouse_scroll(&mut self, direction: MouseScrollDirection) {
        self.content.handle_mouse_scroll(direction);
    }

    pub(crate) fn begin_selection(&mut self, area: Rect, position: Position) -> bool {
        let Some(point) = self.selection_point(area, position, /*clamp*/ false) else {
            return false;
        };
        self.selection.start(point);
        self.selection_scroll_offset = Some(self.content.scroll_offset());
        true
    }

    pub(crate) fn update_selection(&mut self, area: Rect, position: Position) -> bool {
        let Some(point) = self.selection_point(area, position, /*clamp*/ true) else {
            return false;
        };
        self.selection.update(point)
    }

    pub(crate) fn finish_selection(&mut self, area: Rect, position: Position) -> Option<String> {
        let point = self.selection_point(area, position, /*clamp*/ true);
        self.selection.set_release_point(point);
        self.ensure_selected_projections();
        let projections = self
            .selection_projection_cache
            .as_ref()
            .map(|cache| cache.projections.as_slice())
            .unwrap_or_default();
        let selected = self.selection.finish(/*point*/ None, projections);
        self.selection_scroll_offset = None;
        selected
    }

    pub(crate) fn cancel_selection(&mut self) {
        self.selection.cancel();
        self.selection_scroll_offset = None;
    }

    pub(crate) fn selection_is_active(&self) -> bool {
        self.selection.is_active()
    }

    pub(crate) fn push_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        self.invalidate_selection_projections();
        let follow_bottom = self.content.is_following_bottom();
        let had_prior_cells = !self.cells.is_empty();
        let tail_renderable = self.take_live_tail_renderable();
        let renderable = Self::cell_renderable(
            cell.clone(),
            self.render_mode,
            /*has_prior_cells*/ had_prior_cells,
        );
        self.cells.push(cell);
        self.content.push(renderable);

        if let Some(tail) = tail_renderable {
            let tail = if !had_prior_cells
                && self
                    .live_tail_key
                    .is_some_and(|key| !key.is_stream_continuation)
            {
                Self::with_leading_spacing(tail)
            } else {
                tail
            };
            self.content.push(tail);
        }
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    pub(crate) fn replace_cells(&mut self, cells: Vec<Arc<dyn HistoryCell>>) {
        self.invalidate_selection_projections();
        let follow_bottom = self.content.is_following_bottom();
        self.take_live_tail_renderable();
        self.live_tail_key = None;
        self.cells = cells;
        self.content
            .replace(Self::render_cells(&self.cells, self.render_mode));
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    pub(crate) fn set_render_mode(&mut self, render_mode: HistoryRenderMode) {
        if self.render_mode == render_mode {
            return;
        }
        self.invalidate_selection_projections();
        let follow_bottom = self.content.is_following_bottom();
        self.take_live_tail_renderable();
        self.live_tail_key = None;
        self.render_mode = render_mode;
        self.content
            .replace(Self::render_cells(&self.cells, self.render_mode));
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    pub(crate) fn sync_live_tail(
        &mut self,
        width: u16,
        active_key: Option<ActiveCellRenderKey>,
        compute_lines: impl FnOnce(u16) -> Option<Vec<HyperlinkLine>>,
    ) {
        let next_key = active_key.map(|key| LiveTailKey {
            width,
            revision: key.revision,
            is_stream_continuation: key.is_stream_continuation,
            animation_tick: key.animation_tick,
        });
        if self.live_tail_key == next_key {
            return;
        }

        let follow_bottom = !self.selection.is_active() && self.content.is_following_bottom();
        self.take_live_tail_renderable();
        self.live_tail_key = next_key;
        if let Some(key) = next_key {
            let lines = compute_lines(width).unwrap_or_default();
            if !lines.is_empty() {
                self.content.push(Self::live_tail_renderable(
                    lines,
                    !self.cells.is_empty(),
                    key.is_stream_continuation,
                ));
            }
        }
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    #[cfg(test)]
    pub(crate) fn is_following_bottom(&self) -> bool {
        self.content.is_following_bottom()
    }

    #[cfg(test)]
    pub(crate) fn committed_cell_count(&self) -> usize {
        self.cells.len()
    }

    fn render_cells(
        cells: &[Arc<dyn HistoryCell>],
        render_mode: HistoryRenderMode,
    ) -> Vec<Box<dyn Renderable>> {
        cells
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                Self::cell_renderable(
                    cell.clone(),
                    render_mode,
                    /*has_prior_cells*/ index > 0,
                )
            })
            .collect()
    }

    fn cell_renderable(
        cell: Arc<dyn HistoryCell>,
        render_mode: HistoryRenderMode,
        has_prior_cells: bool,
    ) -> Box<dyn Renderable> {
        let is_stream_continuation = cell.is_stream_continuation();
        let renderable: Box<dyn Renderable> = Box::new(ConversationCellRenderable {
            cell,
            render_mode,
            cached_height: Cell::new(None),
        });
        if has_prior_cells && !is_stream_continuation {
            Self::with_leading_spacing(renderable)
        } else {
            renderable
        }
    }

    fn live_tail_renderable(
        lines: Vec<HyperlinkLine>,
        has_prior_cells: bool,
        is_stream_continuation: bool,
    ) -> Box<dyn Renderable> {
        let renderable: Box<dyn Renderable> = Box::new(HyperlinkLinesRenderable { lines });
        if has_prior_cells && !is_stream_continuation {
            Self::with_leading_spacing(renderable)
        } else {
            renderable
        }
    }

    fn with_leading_spacing(renderable: Box<dyn Renderable>) -> Box<dyn Renderable> {
        Box::new(InsetRenderable::new(
            renderable,
            Insets::tlbr(
                /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
            ),
        ))
    }

    fn take_live_tail_renderable(&mut self) -> Option<Box<dyn Renderable>> {
        (self.content.len() > self.cells.len()).then(|| self.content.pop())?
    }

    fn invalidate_selection_projections(&mut self) {
        self.cancel_selection();
        self.selection_projection_cache = None;
    }

    fn ensure_selection_projections(&mut self, width: u16) {
        let cache_matches = self
            .selection_projection_cache
            .as_ref()
            .is_some_and(|cache| {
                cache.width == width
                    && cache.render_mode == self.render_mode
                    && cache.projections.len() == self.cells.len()
            });
        if cache_matches {
            return;
        }
        self.cancel_selection();
        let renderable_heights = self.content.renderable_heights(width);
        let mut content_top = 0usize;
        let layout = self
            .cells
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                let leading_spacing = usize::from(index > 0 && !cell.is_stream_continuation());
                let renderable_height = renderable_heights
                    .get(index)
                    .copied()
                    .map(usize::from)
                    .unwrap_or_else(|| {
                        leading_spacing.saturating_add(usize::from(
                            cell.desired_height_for_mode(width, self.render_mode),
                        ))
                    });
                let cell_layout = SelectionCellLayout {
                    top: content_top.saturating_add(leading_spacing),
                    height: renderable_height.saturating_sub(leading_spacing),
                };
                content_top = content_top.saturating_add(renderable_height);
                cell_layout
            })
            .collect();
        self.selection_projection_cache = Some(SelectionProjectionCache {
            width,
            render_mode: self.render_mode,
            projections: vec![None; self.cells.len()],
            computed: vec![false; self.cells.len()],
            layout,
        });
    }

    fn ensure_cell_projection(&mut self, index: usize) {
        let Some(cache) = self.selection_projection_cache.as_ref() else {
            return;
        };
        if cache
            .computed
            .get(index)
            .copied()
            .unwrap_or(/*default*/ true)
        {
            return;
        }
        let Some(cell) = self.cells.get(index) else {
            return;
        };
        let separator = if index > 0 && cell.is_stream_continuation() {
            "\n"
        } else {
            "\n\n"
        };
        let projection = cell
            .selection_projection(cache.width, cache.render_mode)
            .map(|projection| projection.with_separator_before(separator));
        if let Some(cache) = self.selection_projection_cache.as_mut() {
            cache.projections[index] = projection;
            cache.computed[index] = true;
        }
    }

    fn ensure_selected_projections(&mut self) {
        let Some(selected_cells) = self.selection.selected_cell_span() else {
            return;
        };
        for index in selected_cells {
            self.ensure_cell_projection(index);
        }
    }

    fn selection_point(
        &mut self,
        area: Rect,
        position: Position,
        clamp: bool,
    ) -> Option<SelectionPoint> {
        if area.is_empty() {
            return None;
        }
        self.ensure_selection_projections(area.width);
        let column = position
            .x
            .clamp(area.x, area.right().saturating_sub(1))
            .saturating_sub(area.x);
        let screen_row = position
            .y
            .clamp(area.y, area.bottom().saturating_sub(1))
            .saturating_sub(area.y);
        if !clamp && !area.contains(position) {
            return None;
        }
        let content_row = self
            .content
            .scroll_offset()
            .saturating_add(usize::from(screen_row));
        let target = self
            .selection_projection_cache
            .as_ref()?
            .layout
            .iter()
            .position(|layout| {
                layout.height > 0
                    && content_row >= layout.top
                    && content_row < layout.top.saturating_add(layout.height)
            });
        if let Some(index) = target {
            self.ensure_cell_projection(index);
            let cache = self.selection_projection_cache.as_ref()?;
            let layout = cache.layout[index];
            let projection = cache.projections[index].as_ref();
            let local_row = content_row.saturating_sub(layout.top);
            let bytes = projection.and_then(|projection| {
                if clamp {
                    projection
                        .closest_hit(local_row, column)
                        .or_else(|| projection.closest_hit_in_any_row(local_row, column))
                } else {
                    projection.hit(local_row, column)
                }
            });
            if let Some(bytes) = bytes {
                return Some(SelectionPoint { cell: index, bytes });
            }
        }
        if !clamp {
            return None;
        }

        let mut candidates = self
            .selection_projection_cache
            .as_ref()?
            .layout
            .iter()
            .enumerate()
            .filter(|(index, layout)| Some(*index) != target && layout.height > 0)
            .map(|(index, layout)| {
                let distance = if content_row < layout.top {
                    layout.top - content_row
                } else {
                    content_row
                        .saturating_sub(layout.top.saturating_add(layout.height).saturating_sub(1))
                };
                (distance, index)
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        for (_, index) in candidates {
            self.ensure_cell_projection(index);
            let cache = self.selection_projection_cache.as_ref()?;
            let layout = cache.layout[index];
            let local_row = content_row
                .saturating_sub(layout.top)
                .min(layout.height.saturating_sub(1));
            if let Some(bytes) = cache.projections[index]
                .as_ref()
                .and_then(|projection| projection.closest_hit_in_any_row(local_row, column))
            {
                return Some(SelectionPoint { cell: index, bytes });
            }
        }
        None
    }

    fn render_selection(&self, area: Rect, buf: &mut Buffer) {
        let Some(cache) = self.selection_projection_cache.as_ref() else {
            return;
        };
        let Some(selected_cells) = self.selection.selected_cell_span() else {
            return;
        };
        let scroll_offset = self.content.scroll_offset();
        for cell_index in selected_cells {
            let Some(layout) = cache.layout.get(cell_index).copied() else {
                continue;
            };
            let Some(projection) = cache.projections.get(cell_index).and_then(Option::as_ref)
            else {
                continue;
            };
            let Some(selected_bytes) = self
                .selection
                .selected_bytes_for_cell(cell_index, projection.text().len())
            else {
                continue;
            };
            for (row_index, row) in projection.rows().iter().enumerate() {
                let content_row = layout.top.saturating_add(row_index);
                let Some(screen_row) = content_row.checked_sub(scroll_offset) else {
                    continue;
                };
                let Ok(screen_row) = u16::try_from(screen_row) else {
                    continue;
                };
                if screen_row >= area.height {
                    continue;
                }
                for segment in &row.segments {
                    if segment.bytes.start >= selected_bytes.end
                        || segment.bytes.end <= selected_bytes.start
                    {
                        continue;
                    }
                    for column in segment.columns.clone() {
                        if column < area.width {
                            buf[(
                                area.x.saturating_add(column),
                                area.y.saturating_add(screen_row),
                            )]
                                .modifier
                                .insert(Modifier::REVERSED);
                        }
                    }
                }
            }
        }
    }
}

struct ConversationCellRenderable {
    cell: Arc<dyn HistoryCell>,
    render_mode: HistoryRenderMode,
    cached_height: Cell<Option<(u16, u16)>>,
}

impl Renderable for ConversationCellRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let hyperlink_lines = self
            .cell
            .display_hyperlink_lines_for_mode(area.width, self.render_mode);
        let block_style = match self.render_mode {
            HistoryRenderMode::Rich => self.cell.rich_block_style().unwrap_or_default(),
            HistoryRenderMode::Raw => Default::default(),
        };
        Paragraph::new(Text::from(visible_lines(hyperlink_lines.clone())))
            .style(block_style)
            .wrap(Wrap { trim: false })
            .render(area, buf);
        mark_buffer_hyperlinks(buf, area, &hyperlink_lines, /*scroll_rows*/ 0);
    }

    fn desired_height(&self, width: u16) -> u16 {
        if let Some((cached_width, height)) = self.cached_height.get()
            && cached_width == width
        {
            return height;
        }
        let height = self.cell.desired_height_for_mode(width, self.render_mode);
        self.cached_height.set(Some((width, height)));
        height
    }
}

struct HyperlinkLinesRenderable {
    lines: Vec<HyperlinkLine>,
}

impl Renderable for HyperlinkLinesRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Text::from(visible_lines(self.lines.clone())))
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

#[cfg(test)]
#[path = "conversation_viewport_tests.rs"]
mod tests;
