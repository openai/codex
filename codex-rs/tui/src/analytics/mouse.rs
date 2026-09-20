//! Mouse actions use the most recently painted Usage layout and semantic report controls.
//! Wheel input belongs only to the body, with bounded independent report/help/dashboard offsets.

use super::AnalyticsView;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Position;

impl AnalyticsView {
    pub(super) fn invalidate_mouse_targets(&mut self) {
        self.mouse_context = None;
        self.tab_hits.clear();
        self.control_hits.clear();
    }

    pub(super) fn handle_mouse(&mut self, event: MouseEvent) {
        // A report/help/dashboard transition can happen before its scheduled redraw. Old
        // rectangles must not activate controls belonging to the surface being replaced.
        if self.mouse_context != Some((self.section, self.show_help, self.zoomed)) {
            return;
        }
        let point = Position::new(event.column, event.row);
        match event.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                if self.body_area.contains(point) =>
            {
                let offset = self.scroll_offset().min(self.max_scroll);
                *self.scroll_offset_mut() = match event.kind {
                    MouseEventKind::ScrollDown => {
                        offset.saturating_add(/*rhs*/ 3).min(self.max_scroll)
                    }
                    _ => offset.saturating_sub(/*rhs*/ 3),
                };
                self.follow_selection = false;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((section, _)) =
                    self.tab_hits.iter().find(|(_, rect)| rect.contains(point))
                {
                    self.select_section(*section);
                } else if let Some((control, _)) = self
                    .control_hits
                    .iter()
                    .find(|(_, rect)| rect.contains(point))
                {
                    let control = *control;
                    self.follow_selection = false;
                    self.activate_control(control);
                }
            }
            _ => {}
        }
    }
}
