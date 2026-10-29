//! The composer measures the natural width before allocating the left column, then renders
//! the selected content in the allotted rectangle.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::ui_tree::UiNode;

/// Owned content for the footer's negotiated left-hand area.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FooterContent {
    Line(Line<'static>),
    HyperlinkLine(HyperlinkLine),
    Extension { content: UiNode },
}

impl FooterContent {
    /// Natural display width, before the footer allocates space for the right indicator.
    pub(crate) fn width(&self) -> u16 {
        let width = match self {
            Self::Line(line) => line.width(),
            Self::HyperlinkLine(line) => line.width(),
            Self::Extension { content } => content.render_line(u16::MAX).width(),
        };
        u16::try_from(width).unwrap_or(u16::MAX)
    }

    /// Paint only inside the final area supplied by the parent.
    pub(crate) fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::Line(line) => Widget::render(line, area, buf),
            Self::Extension { content } => content.render_line(area.width).render_fitted(area, buf),
            Self::HyperlinkLine(mut line) => {
                let area = Rect {
                    height: area.height.min(1),
                    ..area
                };
                let width = usize::from(area.width);
                if line.width() > width {
                    line.line = truncate_line_with_ellipsis_if_overflow(line.line, width);
                    // The ellipsis is never part of the link.
                    line.clip_hyperlinks_to_width(line.width().saturating_sub(1));
                }
                line.render_fitted(area, buf);
            }
        }
    }

    /// Keep link destinations when handing the status line to the effort transition.
    pub(crate) fn into_hyperlink_line(self) -> HyperlinkLine {
        match self {
            Self::Line(line) => line.into(),
            Self::HyperlinkLine(line) => line,
            Self::Extension { content } => content.render_line(u16::MAX),
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
