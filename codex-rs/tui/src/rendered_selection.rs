//! Read-only selection for regions registered by the last owned-screen render.
//! Widgets register painted rectangles, and may invalidate their own region when its meaning
//! changes. Only the clicked rectangle is captured from the completed terminal frame; other
//! regions remain live. Gestures, keyboard selection and clipboard tickets use TranscriptView.

use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::transcript_view::TranscriptView;
use crate::transcript_view::ViewAction;
use crate::tui::TuiEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellWidth;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct RenderedSelection {
    pub(crate) view: TranscriptView,
    pub(crate) cells: Vec<Arc<dyn HistoryCell>>,
    pub(crate) areas: Vec<Rect>,
    snapshot: Option<Buffer>,
    pressed: Option<MouseEvent>,
}

pub(crate) enum RegionAction {
    Selection(ViewAction),
    // Replay clickable decorations only after a stationary release.
    Click(MouseEvent),
}

impl RenderedSelection {
    fn clear(&mut self) {
        self.view.jump_to_latest();
        self.snapshot = None;
        self.pressed = None;
    }

    /// Register a painted, passive region. Invalidation affects only a selection in this region.
    pub(crate) fn register(&mut self, area: Rect, invalidate: bool) {
        if invalidate
            && self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.area == area)
        {
            self.clear();
        }
        if !area.is_empty() {
            self.areas.push(area);
        }
    }

    pub(crate) fn render(&mut self, buffer: &mut Buffer) {
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !self.areas.contains(&snapshot.area))
        {
            self.clear();
        }
        if !self.view.has_active_interaction() {
            self.snapshot = None;
        }
        if let Some(snapshot) = &self.snapshot {
            self.view.render(snapshot.area, buffer, &self.cells);
            // Keep native hyperlinks and forced glyph widths from the original cells.
            for position in snapshot.area.positions() {
                buffer[position].set_symbol(snapshot[position].symbol());
                buffer[position].diff_option = snapshot[position].diff_option;
            }
        }
    }

    pub(crate) fn needs_refresh(&self, event: MouseEvent) -> bool {
        event.kind != MouseEventKind::Moved
            && self
                .areas
                .iter()
                .any(|area| area.contains(Position::new(event.column, event.row)))
    }

    pub(crate) fn handle_event(
        &mut self,
        event: &TuiEvent,
        completed: &Buffer,
    ) -> Option<RegionAction> {
        match event {
            TuiEvent::Mouse(mouse) => return self.handle_mouse(*mouse, completed),
            TuiEvent::Key(key) if key.kind == KeyEventKind::Release => return None,
            TuiEvent::Key(key) if self.view.has_active_interaction() => {
                if let Some(action) = self.view.handle_selection_key(*key, &self.cells) {
                    return Some(RegionAction::Selection(action));
                }
            }
            TuiEvent::Key(_)
            | TuiEvent::Paste(_)
            | TuiEvent::Resize(_)
            | TuiEvent::FocusLost
            | TuiEvent::Resume => {}
            TuiEvent::Draw | TuiEvent::FocusGained => return None,
        }
        self.clear();
        None
    }

    fn handle_mouse(&mut self, mut event: MouseEvent, completed: &Buffer) -> Option<RegionAction> {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let position = Position::new(event.column, event.row);
                let Some(area) = self
                    .areas
                    .iter()
                    .copied()
                    .find(|area| area.contains(position))
                else {
                    self.clear();
                    return None;
                };
                if self
                    .snapshot
                    .as_ref()
                    .is_none_or(|snapshot| snapshot.area != area)
                {
                    self.view.jump_to_latest();
                    let area = area.intersection(completed.area);
                    let mut surface = Buffer::empty(area);
                    for position in area.positions() {
                        surface[position] = completed[position].clone();
                    }
                    self.cells = vec![Arc::new(PlainHistoryCell::new(surface_lines(&surface)))];
                    self.view.render(area, &mut surface.clone(), &self.cells);
                    self.snapshot = Some(surface);
                }
                self.pressed = Some(event);
            }
            MouseEventKind::Drag(MouseButton::Left) => self.pressed = None,
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some(pressed) = self.pressed.take()
                    && (pressed.column, pressed.row) == (event.column, event.row)
                    && !self.view.has_selection_range()
                {
                    self.view.end_selection(&self.cells);
                    return Some(RegionAction::Click(pressed));
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.clear();
                return None;
            }
            _ => {}
        }
        let snapshot = self.snapshot.as_ref()?;
        if matches!(event.kind, MouseEventKind::Drag(_) | MouseEventKind::Up(_)) {
            event.column = event.column.clamp(snapshot.area.x, snapshot.area.right());
            event.row = event.row.clamp(snapshot.area.y, snapshot.area.bottom() - 1);
        }
        self.view
            .handle_mouse(event, &self.cells)
            .map(RegionAction::Selection)
    }
}

fn surface_lines(surface: &Buffer) -> Vec<Line<'static>> {
    (surface.area.y..surface.area.bottom())
        .map(|row| {
            let mut spans = Vec::new();
            let mut column = surface.area.x;
            while column < surface.area.right() {
                let cell = &surface[(column, row)];
                spans.push(Span::styled(
                    codex_ansi_escape::ansi_escape_line(cell.symbol()).to_string(),
                    cell.style(),
                ));
                column += cell.cell_width().max(/*other*/ 1);
            }
            while spans.last().is_some_and(|span| span.content == " ") {
                spans.pop();
            }
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
#[path = "footer_selection_tests.rs"]
mod tests;
