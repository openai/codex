use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Alignment;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::render::model::RenderAlignment;
use crate::render::model::RenderCell;
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
/// - `Line`: Ratatui line with equivalent styling.
pub fn to_ratatui_line(line: &RenderLine) -> Line<'static> {
    let spans = line
        .spans
        .iter()
        .map(|cell| Span::styled(cell.content.clone(), to_ratatui_style(cell.style)))
        .collect::<Vec<_>>();
    let mut out = Line::from(spans);
    if let Some(alignment) = line.alignment {
        out.alignment = Some(to_ratatui_alignment(alignment));
    }
    out
}

pub fn to_ratatui_lines(lines: &[RenderLine]) -> Vec<Line<'static>> {
    lines.iter().map(to_ratatui_line).collect()
}

impl From<RenderLine> for Line<'static> {
    fn from(value: RenderLine) -> Self {
        to_ratatui_line(&value)
    }
}

pub fn to_ratatui_text(lines: &[RenderLine]) -> Text<'static> {
    Text::from(to_ratatui_lines(lines))
}

pub fn to_ratatui_span(cell: &RenderCell) -> Span<'static> {
    Span::styled(cell.content.clone(), to_ratatui_style(cell.style))
}

/// Converts a render paragraph into a ratatui paragraph.
///
/// # Arguments
/// - `paragraph` (&RenderParagraph): Backend-agnostic paragraph to convert.
///
/// # Returns
/// - `Paragraph<'static>`: Ratatui paragraph with equivalent styling and wrapping.
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
    if style.strikethrough {
        out = out.add_modifier(Modifier::CROSSED_OUT);
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
        RenderColor::Yellow => Color::Yellow,
        RenderColor::Blue => Color::Blue,
        RenderColor::Magenta => Color::Magenta,
        RenderColor::Cyan => Color::Cyan,
        RenderColor::LightBlue => Color::LightBlue,
        RenderColor::DarkGray => Color::DarkGray,
        RenderColor::Rgb(_, _, _) | RenderColor::Indexed(_) => Color::Reset,
    }
}

/// Converts a ratatui line into a render line.
///
/// # Arguments
/// - `line` (&Line): Ratatui line to convert.
///
/// # Returns
/// - `RenderLine`: Backend-agnostic line.
pub fn from_ratatui_line(line: &Line) -> RenderLine {
    let mut cells = Vec::with_capacity(line.spans.len());
    for span in &line.spans {
        let style = from_ratatui_style(span.style.patch(line.style));
        cells.push(crate::render::model::RenderCell::new(
            span.content.to_string(),
            style,
        ));
    }
    let mut out = RenderLine::new(cells);
    out.alignment = line.alignment.map(from_ratatui_alignment);
    out
}

/// Converts a list of ratatui lines into render lines.
///
/// # Arguments
/// - `lines` (&[Line]): Ratatui lines to convert.
///
/// # Returns
/// - `Vec<RenderLine>`: Converted render lines.
pub fn from_ratatui_lines(lines: &[Line]) -> Vec<RenderLine> {
    lines.iter().map(from_ratatui_line).collect()
}

pub fn from_ratatui_style(style: Style) -> RenderStyle {
    RenderStyle {
        fg: style.fg.and_then(from_ratatui_color),
        bg: style.bg.and_then(from_ratatui_color),
        bold: style.add_modifier.contains(Modifier::BOLD),
        dim: style.add_modifier.contains(Modifier::DIM),
        italic: style.add_modifier.contains(Modifier::ITALIC),
        underline: style.add_modifier.contains(Modifier::UNDERLINED),
        strikethrough: style.add_modifier.contains(Modifier::CROSSED_OUT),
    }
}

fn from_ratatui_color(color: Color) -> Option<RenderColor> {
    match color {
        Color::Reset => Some(RenderColor::Default),
        Color::Red => Some(RenderColor::Red),
        Color::Green => Some(RenderColor::Green),
        Color::Yellow => Some(RenderColor::Yellow),
        Color::Blue => Some(RenderColor::Blue),
        Color::Magenta => Some(RenderColor::Magenta),
        Color::Cyan => Some(RenderColor::Cyan),
        Color::LightBlue => Some(RenderColor::LightBlue),
        Color::DarkGray => Some(RenderColor::DarkGray),
        Color::Rgb(r, g, b) => Some(RenderColor::Rgb(r, g, b)),
        Color::Indexed(index) => Some(RenderColor::Indexed(index)),
        _ => None,
    }
}

fn to_ratatui_alignment(alignment: RenderAlignment) -> Alignment {
    match alignment {
        RenderAlignment::Left => Alignment::Left,
        RenderAlignment::Center => Alignment::Center,
        RenderAlignment::Right => Alignment::Right,
    }
}

fn from_ratatui_alignment(alignment: Alignment) -> RenderAlignment {
    match alignment {
        Alignment::Left => RenderAlignment::Left,
        Alignment::Center => RenderAlignment::Center,
        Alignment::Right => RenderAlignment::Right,
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
