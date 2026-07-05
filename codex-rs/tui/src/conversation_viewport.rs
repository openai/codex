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

use crate::chatwidget::ActiveCellDisplaySnapshot;
use crate::chatwidget::ActiveCellRenderKey;
use crate::conversation_selection::CellSelectionProjection;
use crate::conversation_selection::ConversationSelection;
use crate::conversation_selection::SelectionCellLayout;
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
    live_cells: Vec<ActiveCellDisplaySnapshot>,
    deferred_cells: Option<Vec<Arc<dyn HistoryCell>>>,
    deferred_render_mode: Option<HistoryRenderMode>,
    selection: ConversationSelection,
    selection_projection_cache: Option<SelectionProjectionCache>,
}

struct SelectionProjectionCache {
    width: u16,
    render_mode: HistoryRenderMode,
    projections: Vec<Option<CellSelectionProjection>>,
    computed: Vec<bool>,
    layout: Vec<SelectionCellLayout>,
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
            live_cells: Vec::new(),
            deferred_cells: None,
            deferred_render_mode: None,
            selection: ConversationSelection::default(),
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

    pub(crate) fn handle_selection_mouse_scroll(
        &mut self,
        area: Rect,
        direction: MouseScrollDirection,
        position: Position,
    ) {
        self.content.handle_mouse_scroll(direction);
        self.update_selection(area, position);
    }

    pub(crate) fn begin_selection(&mut self, area: Rect, position: Position) -> bool {
        let Some(point) = self.selection_point(area, position, /*clamp*/ false) else {
            return false;
        };
        self.selection.start(point);
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
        let selected = if let Some(cache) = self.selection_projection_cache.as_ref() {
            self.selection
                .finish(/*point*/ None, &cache.projections, &cache.layout)
        } else {
            self.selection.finish(/*point*/ None, &[], &[])
        };
        self.apply_deferred_state();
        selected
    }

    pub(crate) fn cancel_selection(&mut self) {
        self.selection.cancel();
        self.apply_deferred_state();
    }

    pub(crate) fn selection_is_active(&self) -> bool {
        self.selection.is_active()
    }

    pub(crate) fn push_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        if self.selection.is_active() {
            self.deferred_cells
                .get_or_insert_with(|| self.cells.clone())
                .push(cell);
            return;
        }
        self.invalidate_selection_projections();
        let follow_bottom = self.content.is_following_bottom();
        let had_prior_cells = !self.cells.is_empty();
        self.take_live_tail_renderables();
        let renderable = Self::cell_renderable(
            cell.clone(),
            self.render_mode,
            /*has_prior_cells*/ had_prior_cells,
        );
        self.cells.push(cell);
        self.content.push(renderable);
        self.push_live_tail_renderables();
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    pub(crate) fn replace_cells(&mut self, cells: Vec<Arc<dyn HistoryCell>>) {
        if self.selection.is_active() {
            self.deferred_cells = Some(cells);
            return;
        }
        self.invalidate_selection_projections();
        let follow_bottom = self.content.is_following_bottom();
        self.take_live_tail_renderables();
        self.live_tail_key = None;
        self.live_cells.clear();
        self.cells = cells;
        self.content
            .replace(Self::render_cells(&self.cells, self.render_mode));
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    pub(crate) fn set_render_mode(&mut self, render_mode: HistoryRenderMode) {
        let effective_render_mode = self.deferred_render_mode.unwrap_or(self.render_mode);
        if effective_render_mode == render_mode {
            return;
        }
        if self.selection.is_active() {
            self.deferred_render_mode = Some(render_mode);
            return;
        }
        self.invalidate_selection_projections();
        let follow_bottom = self.content.is_following_bottom();
        self.take_live_tail_renderables();
        self.live_tail_key = None;
        self.live_cells.clear();
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
        compute_cells: impl FnOnce(u16) -> Option<Vec<ActiveCellDisplaySnapshot>>,
    ) {
        // A drag's screen coordinates and source projections must describe the same immutable
        // content. Active cells can mutate on every output delta or animation tick, so defer all
        // live-tail changes until the drag completes or is cancelled.
        if self.selection.is_active() {
            let width_is_stable = self
                .selection_projection_cache
                .as_ref()
                .is_some_and(|cache| cache.width == width);
            if width_is_stable {
                return;
            }
            self.cancel_selection();
        }
        let next_key = active_key.map(|key| LiveTailKey {
            width,
            revision: key.revision,
            is_stream_continuation: key.is_stream_continuation,
            animation_tick: key.animation_tick,
        });
        if self.live_tail_key == next_key {
            return;
        }

        let follow_bottom = self.content.is_following_bottom();
        self.take_live_tail_renderables();
        self.selection_projection_cache = None;
        self.live_tail_key = next_key;
        self.live_cells.clear();
        if next_key.is_some() {
            self.live_cells = compute_cells(width).unwrap_or_default();
            self.push_live_tail_renderables();
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

    fn push_live_tail_renderables(&mut self) {
        let mut has_prior_cells = !self.cells.is_empty();
        for cell in &self.live_cells {
            self.content.push(Self::live_tail_renderable(
                cell.lines.clone(),
                has_prior_cells,
                cell.is_stream_continuation,
            ));
            has_prior_cells = true;
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

    fn take_live_tail_renderables(&mut self) {
        while self.content.len() > self.cells.len() {
            self.content.pop();
        }
    }

    fn invalidate_selection_projections(&mut self) {
        self.cancel_selection();
        self.selection_projection_cache = None;
    }

    fn apply_deferred_state(&mut self) {
        let deferred_cells = self.deferred_cells.take();
        let deferred_render_mode = self.deferred_render_mode.take();
        if deferred_cells.is_none() && deferred_render_mode.is_none() {
            return;
        }

        let follow_bottom = self.content.is_following_bottom();
        self.take_live_tail_renderables();
        self.live_tail_key = None;
        self.live_cells.clear();
        if let Some(cells) = deferred_cells {
            self.cells = cells;
        }
        if let Some(render_mode) = deferred_render_mode {
            self.render_mode = render_mode;
        }
        self.content
            .replace(Self::render_cells(&self.cells, self.render_mode));
        self.selection_projection_cache = None;
        if follow_bottom {
            self.content.scroll_to_bottom();
        }
    }

    fn ensure_selection_projections(&mut self, width: u16) {
        let current_cell_count = self.cells.len().saturating_add(self.live_cells.len());
        let cache_matches = self
            .selection_projection_cache
            .as_ref()
            .is_some_and(|cache| {
                cache.width == width
                    && cache.render_mode == self.render_mode
                    && cache.projections.len() == current_cell_count
            });
        if cache_matches {
            return;
        }
        self.cancel_selection();
        let selection_cell_count = self.cells.len().saturating_add(self.live_cells.len());
        let renderable_heights = self.content.renderable_heights(width);
        let mut content_top = 0usize;
        let mut layout = Vec::with_capacity(selection_cell_count);
        for index in 0..selection_cell_count {
            let (is_stream_continuation, desired_height) = if let Some(cell) = self.cells.get(index)
            {
                (
                    cell.is_stream_continuation(),
                    cell.desired_height_for_mode(width, self.render_mode),
                )
            } else {
                let Some(cell) = self.live_cells.get(index.saturating_sub(self.cells.len())) else {
                    return;
                };
                (
                    cell.is_stream_continuation,
                    HyperlinkLinesRenderable {
                        lines: cell.lines.clone(),
                    }
                    .desired_height(width),
                )
            };
            let leading_spacing = usize::from(index > 0 && !is_stream_continuation);
            let renderable_height = renderable_heights
                .get(index)
                .copied()
                .unwrap_or(desired_height);
            layout.push(SelectionCellLayout {
                top: content_top.saturating_add(leading_spacing),
                height: usize::from(renderable_height).saturating_sub(leading_spacing),
            });
            content_top = content_top.saturating_add(usize::from(renderable_height));
        }
        let mut projections = vec![None; selection_cell_count];
        let mut computed = vec![false; selection_cell_count];
        for (live_index, cell) in self.live_cells.iter().enumerate() {
            let index = self.cells.len().saturating_add(live_index);
            let separator = if index > 0 && cell.is_stream_continuation {
                "\n"
            } else {
                "\n\n"
            };
            projections[index] = cell
                .selection_projection
                .clone()
                .map(|projection| projection.with_default_separator_before(separator));
            computed[index] = true;
        }
        self.selection_projection_cache = Some(SelectionProjectionCache {
            width,
            render_mode: self.render_mode,
            projections,
            computed,
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
            .selection_contribution(cache.width, cache.render_mode)
            .into_projection()
            .map(|projection| projection.with_default_separator_before(separator));
        if let Some(cache) = self.selection_projection_cache.as_mut() {
            cache.projections[index] = projection;
            cache.computed[index] = true;
        }
    }

    fn ensure_selected_projections(&mut self) {
        let Some(layout) = self
            .selection_projection_cache
            .as_ref()
            .map(|cache| cache.layout.as_slice())
        else {
            return;
        };
        let Some(selected_cells) = self.selection.selected_cell_span(layout) else {
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
        Some(SelectionPoint {
            row: content_row,
            column,
        })
    }

    fn render_selection(&self, area: Rect, buf: &mut Buffer) {
        let Some(cache) = self.selection_projection_cache.as_ref() else {
            return;
        };
        let Some(selected_cells) = self.selection.selected_cell_span(&cache.layout) else {
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
                    if !self
                        .selection
                        .segment_is_selected(layout, row_index, &segment.columns)
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
