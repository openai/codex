//! Paragraph and prelaid-out row rendering that keeps visible text and links aligned.

use super::HyperlinkLine;
use super::annotate_web_urls;
use super::mark_buffer_hyperlinks;
use super::visible_lines_ref;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellWidth;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

/// Owns a word-wrapped paragraph whose web destinations survive clipping and wrapping.
pub(crate) struct HyperlinkText(Vec<HyperlinkLine>);

impl HyperlinkText {
    pub(crate) fn new(lines: Vec<Line<'static>>) -> Self {
        Self(annotate_web_urls(lines))
    }
}

impl Renderable for HyperlinkText {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        HyperlinkParagraph::new(&self.0, Style::default()).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        HyperlinkParagraph::new(&self.0, Style::default()).line_count(width) as u16
    }
}

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

/// Owns rows that have already been wrapped or clipped by their layout code.
/// Rendering clips to the viewport without changing row count or spacing.
pub(crate) struct HyperlinkRows(Vec<HyperlinkLine>);

impl From<Vec<HyperlinkLine>> for HyperlinkRows {
    fn from(lines: Vec<HyperlinkLine>) -> Self {
        Self(lines)
    }
}

impl Renderable for HyperlinkRows {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (row, line) in self.0.iter().take(usize::from(area.height)).enumerate() {
            line.render(Rect::new(area.x, area.y + row as u16, area.width, 1), buf);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        if width == 0 {
            0
        } else {
            self.0.len().min(usize::from(u16::MAX)) as u16
        }
    }
}

/// Render one already-laid-out row, clipping text and links without wrapping it again.
impl Widget for &HyperlinkLine {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.line.clone().render(area, buf);
        if area.is_empty() {
            return;
        }
        for link in &self.hyperlinks {
            let Some(destination) = link.terminal_destination() else {
                continue;
            };
            let mut trailing_columns = 0usize;
            for column in link.columns.start.min(usize::from(area.width))
                ..link.columns.end.min(usize::from(area.width))
            {
                if trailing_columns > 0 {
                    trailing_columns -= 1;
                    continue;
                }
                let cell = &mut buf[(area.x + column as u16, area.y)];
                if cell.diff_option == ratatui::buffer::CellDiffOption::Skip {
                    continue;
                }
                trailing_columns = usize::from(cell.cell_width()).saturating_sub(1);
                let symbol = format!("\x1b]8;;{destination}\x07{}\x1b]8;;\x07", cell.symbol());
                let width = std::num::NonZeroU16::new(cell.cell_width())
                    .unwrap_or(std::num::NonZeroU16::MIN);
                cell.set_symbol(&symbol)
                    .set_diff_option(ratatui::buffer::CellDiffOption::ForcedWidth(width));
            }
        }
    }
}
