use ratatui::prelude::*;

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
