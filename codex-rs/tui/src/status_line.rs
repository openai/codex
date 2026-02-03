//! Status line value type used by the TUI footer.

use ratatui::style::Stylize;
use ratatui::text::Line;

#[derive(Debug, Clone)]
pub(crate) struct StatusLineValue {
    line: Line<'static>,
}

impl StatusLineValue {
    pub(crate) fn from_text(text: String) -> Self {
        Self {
            line: Line::from(text),
        }
    }

    pub(crate) fn from_line(line: Line<'static>) -> Self {
        Self { line }
    }

    pub(crate) fn as_line(&self) -> Line<'static> {
        self.line.clone().dim()
    }
}
