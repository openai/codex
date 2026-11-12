//! Native and extension status content measured and painted inside the status rectangle.

use crate::line_truncation::truncate_line_with_ellipsis;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use std::borrow::Cow;
use std::sync::Arc;

use crate::render::highlight::with_syntax_theme;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::ui_tree::PreparedUiTree;
use crate::ui_tree::UiTree;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusLineFocus {
    Focused,
    Unfocused,
}

/// Owned status content displayed below the composer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FooterContent {
    HyperlinkLine(Box<HyperlinkLine>),
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
    /// Natural display width in terminal cells.
    pub(crate) fn width(&self) -> u16 {
        let width = match self {
            Self::HyperlinkLine(line) => line.width(),
            Self::Extension { content } => return content.measure_natural().width,
        };
        u16::try_from(width).unwrap_or(u16::MAX)
    }

    /// Paint a focused preview in its allotted rectangle.
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_with_focus(area, buf, StatusLineFocus::Focused);
    }

    pub(in crate::bottom_pane) fn render_with_focus(
        &self,
        area: Rect,
        buf: &mut Buffer,
        focus: StatusLineFocus,
    ) {
        match self {
            Self::Extension { content } => {
                let plain_style =
                    (focus == StatusLineFocus::Unfocused).then(|| Style::reset().dim());
                with_syntax_theme(|theme| content.render(area, theme, buf, plain_style));
            }
            Self::HyperlinkLine(line) => render_status_row(
                line,
                Rect {
                    height: area.height.min(1),
                    ..area
                },
                buf,
                focus,
                /*more_rows*/ false,
            ),
        }
    }

    pub(crate) fn into_hyperlink_line(self) -> HyperlinkLine {
        match self {
            Self::HyperlinkLine(line) => *line,
            Self::Extension { content } => {
                with_syntax_theme(|theme| content.render_line(u16::MAX, theme))
            }
        }
    }
}

pub(in crate::bottom_pane) fn render_status_row(
    line: &HyperlinkLine,
    area: Rect,
    buf: &mut Buffer,
    focus: StatusLineFocus,
    more_rows: bool,
) {
    if area.is_empty() {
        return;
    }
    let mut line = Cow::Borrowed(line);
    let width = usize::from(area.width);
    if more_rows || line.width() > width {
        let line = line.to_mut();
        line.line = truncate_line_with_ellipsis(std::mem::take(&mut line.line), width);
        line.clip_hyperlinks_to_width(line.width().saturating_sub(1));
    }
    if focus == StatusLineFocus::Unfocused {
        let line = line.to_mut();
        line.hyperlinks.clear();
        line.line.style = Style::reset().dim();
        for span in &mut line.line.spans {
            span.style = Style::default();
        }
    }
    line.render_fitted(area, buf);
}

impl From<HyperlinkLine> for FooterContent {
    fn from(line: HyperlinkLine) -> Self {
        Self::HyperlinkLine(Box::new(line))
    }
}

impl From<Line<'static>> for FooterContent {
    fn from(line: Line<'static>) -> Self {
        Self::from(HyperlinkLine::from(line))
    }
}

impl From<&'static str> for FooterContent {
    fn from(text: &'static str) -> Self {
        Self::from(Line::from(text))
    }
}

#[cfg(test)]
#[path = "footer_content_tests.rs"]
mod tests;
