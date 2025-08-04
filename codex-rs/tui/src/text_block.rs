use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::history_cell::DynamicHeightWidgetRef;

/// A simple widget that just displays a list of `Line`s via a `Paragraph`.
/// This is the default rendering backend for most `HistoryCell` variants.
#[derive(Clone)]
pub(crate) struct TextBlock {
    pub(crate) lines: Vec<Line<'static>>,
}

impl TextBlock {
    pub(crate) fn new(lines: Vec<Line<'static>>) -> Self {
        Self { lines }
    }
}

impl DynamicHeightWidgetRef for &TextBlock {
    fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(Text::from(self.lines.clone()))
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(0)
    }
}

impl WidgetRef for &TextBlock {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Text::from(self.lines.clone()))
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
