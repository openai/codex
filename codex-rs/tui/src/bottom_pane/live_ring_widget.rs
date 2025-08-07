use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::insert_history::word_wrap_lines;

/// Minimal rendering-only widget for the transient ring rows.
pub(crate) struct LiveRingWidget {
    max_rows: u16,
    rows: Vec<Line<'static>>, // newest at the end
}

impl LiveRingWidget {
    pub fn new() -> Self {
        Self {
            max_rows: 3,
            rows: Vec::new(),
        }
    }

    pub fn set_max_rows(&mut self, n: u16) {
        self.max_rows = n.max(1);
    }

    pub fn set_rows(&mut self, rows: Vec<Line<'static>>) {
        self.rows = rows;
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        if self.rows.is_empty() {
            return 0;
        }
        let wrapped = word_wrap_lines(&self.rows, width);
        (wrapped.len() as u16).min(self.max_rows)
    }
}

impl WidgetRef for LiveRingWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let wrapped = word_wrap_lines(&self.rows, area.width);
        let visible = wrapped.len().saturating_sub(self.max_rows as usize);
        let slice = &wrapped[visible..];
        let para = Paragraph::new(slice.to_vec());
        para.render_ref(area, buf);
    }
}

#[cfg(test)]
impl LiveRingWidget {
    pub fn test_rows(&self) -> Vec<Line<'static>> {
        self.rows.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::widgets::WidgetRef;

    #[test]
    fn live_ring_word_wrap_no_mid_word_split() {
        let mut ring = LiveRingWidget::new();
        ring.set_max_rows(10);

        let sample = "Years passed, and Willowmere thrived in peace and friendship. Mira’s herb garden flourished with both ordinary and enchanted plants, and travelers spoke of the kindness of the woman who tended them.";
        ring.set_rows(vec![Line::from(sample)]);

        let area = ratatui::layout::Rect::new(0, 0, 40, 6);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        (&ring).render_ref(area, &mut buf);

        let mut lines: Vec<String> = Vec::new();
        for row in 0..area.height {
            let mut s = String::new();
            for col in 0..area.width {
                let cell = buf.get(col, row);
                let ch = cell.symbol().chars().next().unwrap_or(' ');
                s.push(ch);
            }
            lines.push(s.trim_end().to_string());
        }
        let joined = lines.join("\n");
        assert!(
            !joined.contains("bo\nth"),
            "word 'both' should not be split in live ring:\n{joined}"
        );
    }
}
