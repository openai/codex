use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::render::model::RenderColor;
use crate::render::model::RenderLine;
use crate::render::model::RenderParagraph;
use crate::render::model::RenderStyle;
use crate::render::renderable::Renderable;

/// Converts a render line into a ratatui line.
///
/// # Arguments
/// - `line` (&RenderLine): Backend-agnostic line to convert.
///
/// # Returns
/// - `Line<'static>`: Ratatui line with equivalent styling.
pub fn to_ratatui_line(line: &RenderLine) -> Line<'static> {
    let spans = line
        .cells
        .iter()
        .map(|cell| Span::styled(cell.text.clone(), to_ratatui_style(cell.style)))
        .collect::<Vec<_>>();
    Line::from(spans)
}

/// Converts a render paragraph into a ratatui paragraph.
///
/// # Arguments
/// - `paragraph` (&RenderParagraph): Backend-agnostic paragraph to convert.
///
/// # Returns
/// - `Paragraph<'static>`: Ratatui paragraph with equivalent styling and wrapping.
#[allow(dead_code)]
pub fn to_ratatui_paragraph(paragraph: &RenderParagraph) -> Paragraph<'static> {
    let lines = paragraph
        .lines
        .iter()
        .map(to_ratatui_line)
        .collect::<Vec<_>>();
    let mut out = Paragraph::new(lines);
    if paragraph.wrap {
        out = out.wrap(Wrap { trim: false });
    }
    out
}

/// Converts a render style into a ratatui style.
///
/// # Arguments
/// - `style` (RenderStyle): Backend-agnostic style to convert.
///
/// # Returns
/// - `Style`: Ratatui style with equivalent attributes.
fn to_ratatui_style(style: RenderStyle) -> Style {
    let mut out = Style::default();
    if let Some(fg) = style.fg {
        out = out.fg(to_ratatui_color(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(to_ratatui_color(bg));
    }
    if style.bold {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.dim {
        out = out.add_modifier(Modifier::DIM);
    }
    if style.italic {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if style.underline {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    out
}

/// Converts a render color into a ratatui color.
///
/// # Arguments
/// - `color` (RenderColor): Backend-agnostic color to convert.
///
/// # Returns
/// - `Color`: Ratatui color with equivalent value.
fn to_ratatui_color(color: RenderColor) -> Color {
    match color {
        RenderColor::Default => Color::Reset,
        RenderColor::Red => Color::Red,
        RenderColor::Green => Color::Green,
        RenderColor::Magenta => Color::Magenta,
        RenderColor::Cyan => Color::Cyan,
        RenderColor::Gray => Color::Gray,
    }
}

impl Renderable for RenderLine {
    /// Renders the line into the specified buffer area.
    ///
    /// # Arguments
    /// - `area` (Rect): Target region for rendering.
    /// - `buf` (&mut Buffer): Buffer to render into.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        to_ratatui_line(self).render_ref(area, buf);
    }

    /// Returns the desired height for rendering this line.
    ///
    /// # Arguments
    /// - `_width` (u16): Available width, unused for single-line rendering.
    ///
    /// # Returns
    /// - `u16`: Height in rows.
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

impl Renderable for RenderParagraph {
    /// Renders the paragraph into the specified buffer area.
    ///
    /// # Arguments
    /// - `area` (Rect): Target region for rendering.
    /// - `buf` (&mut Buffer): Buffer to render into.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        to_ratatui_paragraph(self).render_ref(area, buf);
    }

    /// Returns the desired height for rendering this paragraph at the given width.
    ///
    /// # Arguments
    /// - `width` (u16): Available width for wrapping.
    ///
    /// # Returns
    /// - `u16`: Height in rows after wrapping.
    fn desired_height(&self, width: u16) -> u16 {
        to_ratatui_paragraph(self).line_count(width) as u16
    }
}
