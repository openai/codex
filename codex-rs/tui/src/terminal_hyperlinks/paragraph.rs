//! Paragraph rendering that keeps visible text and hyperlink annotations aligned.

use super::HyperlinkLine;
use super::mark_buffer_hyperlinks;
use super::visible_lines_ref;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

/// Word-wraps without trimming and applies the same vertical scroll to text and links.
pub(crate) struct HyperlinkParagraph<'a> {
    lines: &'a [HyperlinkLine],
    paragraph: Paragraph<'a>,
    scroll_rows: u16,
}

impl<'a> HyperlinkParagraph<'a> {
    pub(crate) fn new(lines: &'a [HyperlinkLine], style: Style) -> Self {
        Self {
            lines,
            paragraph: Paragraph::new(Text::from(visible_lines_ref(lines)))
                .style(style)
                .wrap(Wrap { trim: false }),
            scroll_rows: 0,
        }
    }

    pub(crate) fn line_count(&self, width: u16) -> usize {
        self.paragraph.line_count(width)
    }

    pub(crate) fn scroll(mut self, rows: u16) -> Self {
        self.scroll_rows = rows;
        self
    }
}

impl Widget for HyperlinkParagraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.paragraph
            .scroll((self.scroll_rows, 0))
            .render(area, buf);
        mark_buffer_hyperlinks(buf, area, self.lines, usize::from(self.scroll_rows));
    }
}
