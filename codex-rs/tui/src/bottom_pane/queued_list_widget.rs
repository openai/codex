use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, WidgetRef};

pub(crate) struct QueuedListWidget {
    rows: Vec<Line<'static>>,
}

impl QueuedListWidget {
    pub(crate) fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub(crate) fn set_rows(&mut self, rows: Vec<Line<'static>>) {
        self.rows = rows;
    }

    pub(crate) fn desired_height(&self, _width: u16) -> u16 {
        self.rows.len() as u16
    }
}

impl WidgetRef for QueuedListWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || self.rows.is_empty() {
            return;
        }
        let max_rows = area.height as usize;
        let start = if self.rows.len() > max_rows {
            self.rows.len() - max_rows
        } else {
            0
        };
        let visible = self.rows[start..].to_vec();
        Paragraph::new(visible).render_ref(area, buf);
    }
}
