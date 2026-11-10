//! Native and extension status content measured and painted inside the status rectangle.

use crate::line_truncation::line_width;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::sync::Arc;

use crate::render::highlight::with_syntax_theme;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::ui_tree::PreparedUiTree;
use crate::ui_tree::UiTree;

/// Owned status content displayed below the composer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FooterContent {
    Line(Line<'static>),
    HyperlinkLine(HyperlinkLine),
    Extension { content: Arc<PreparedUiTree> },
}

impl From<UiTree> for FooterContent {
    fn from(tree: UiTree) -> Self {
        Self::Extension {
            content: Arc::new(tree.prepare()),
        }
    }
}

impl FooterContent {
    pub(crate) fn native_parts(&self) -> Option<(&Line<'static>, &[TerminalHyperlink])> {
        match self {
            Self::Line(line) => Some((line, &[])),
            Self::HyperlinkLine(line) => Some((&line.line, &line.hyperlinks)),
            Self::Extension { .. } => None,
        }
    }

    /// Natural display width in terminal cells.
    pub(crate) fn width(&self) -> u16 {
        let width = match self {
            Self::Line(line) => line_width(line),
            Self::HyperlinkLine(line) => line.width(),
            Self::Extension { content } => return content.measure_natural().width,
        };
        u16::try_from(width).unwrap_or(u16::MAX)
    }

    /// Paint only inside the final area supplied by the parent.
    pub(crate) fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::Extension { content } => {
                with_syntax_theme(|theme| content.render(area, theme, buf));
            }
            native => {
                let line = native.into_hyperlink_line();
                let area = Rect {
                    height: area.height.min(1),
                    ..area
                };
                let width = usize::from(area.width);
                if line.width() > width {
                    line.render_with_ellipsis(area, buf);
                } else {
                    line.render_fitted(area, buf);
                }
            }
        }
    }

    pub(crate) fn into_hyperlink_line(self) -> HyperlinkLine {
        match self {
            Self::Line(line) => line.into(),
            Self::HyperlinkLine(line) => line,
            Self::Extension { content } => {
                with_syntax_theme(|theme| content.render_line(u16::MAX, theme))
            }
        }
    }
}

impl From<HyperlinkLine> for FooterContent {
    fn from(line: HyperlinkLine) -> Self {
        Self::HyperlinkLine(line)
    }
}

impl From<Line<'static>> for FooterContent {
    fn from(line: Line<'static>) -> Self {
        Self::Line(line)
    }
}

impl From<&'static str> for FooterContent {
    fn from(text: &'static str) -> Self {
        Self::Line(Line::from(text))
    }
}

#[cfg(test)]
#[path = "footer_content_tests.rs"]
mod tests;
