//! Lay out prepared text and nested rows within a terminal viewport.

mod prepare;
mod text;

use self::prepare::NodeKind;
use self::prepare::PreparedNode;
use self::text::Chunk;
use self::text::Paint;
use self::text::visit_chunks;
use super::MAX_TEXT_BYTES;
use super::RowWrap;
use super::TextProps;
use super::TextWeight;
use super::UiTree;
use super::theme::RenderTheme;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::mark_hyperlink_cells;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use std::ops::Range;
use syntect::highlighting::Theme;
use vtparse::CollectingVTActor;
use vtparse::VTAction;
use vtparse::VTParser;

#[derive(Debug, PartialEq)]
pub(crate) struct PreparedUiTree(Option<PreparedNode>);

impl UiTree {
    pub(crate) fn prepare(self) -> PreparedUiTree {
        PreparedUiTree(
            self.0
                .and_then(|node| PreparedNode::from_node(node, Paint::default())),
        )
    }
}

impl PreparedUiTree {
    pub(crate) fn measure_natural(&self) -> Size {
        self.0.as_ref().map_or_else(Size::default, |node| {
            let width = u16::try_from(node.widths.preferred).unwrap_or(u16::MAX);
            Size::new(width, node.natural_height)
        })
    }

    pub(crate) fn measure(&self, bounds: Size) -> Size {
        layout(self.0.as_ref(), bounds, /*draw*/ None)
    }

    /// A plain style paints the same layout with theme styling and hyperlinks suppressed.
    pub(crate) fn render(
        &self,
        area: Rect,
        theme: &Theme,
        buf: &mut Buffer,
        plain_style: Option<Style>,
    ) {
        let area = area.intersection(buf.area);
        let mut theme = RenderTheme::new(theme);
        layout(
            self.0.as_ref(),
            area.as_size(),
            Some(&mut |row, col, text, cells, paint| {
                let x = area.x + col as u16;
                let y = area.y + row as u16;
                buf.set_string(
                    x,
                    y,
                    text,
                    plain_style.unwrap_or_else(|| paint.style(&mut theme)),
                );
                if plain_style.is_none()
                    && let Some(href) = &paint.href
                {
                    mark_hyperlink_cells(buf, Rect::new(x, y, cells as u16, 1), href);
                }
            }),
        );
    }

    pub(crate) fn render_line(&self, width: u16, theme: &Theme) -> HyperlinkLine {
        let natural = self.measure_natural();
        if width == 1 && (natural.width > 1 || natural.height > 1) {
            return ratatui::text::Line::from("…").into();
        }
        let mut line = HyperlinkLine::default();
        let mut theme = RenderTheme::new(theme);
        layout(
            self.0.as_ref(),
            Size::new(width, /*height*/ 1),
            Some(&mut |_, col, text, cells, paint| {
                line.line
                    .push_span(Span::styled(text.to_owned(), paint.style(&mut theme)));
                if cells > 0
                    && let Some(href) = &paint.href
                {
                    push_link(&mut line, col..col + cells, href);
                }
            }),
        );
        line
    }
}

#[derive(Clone, Copy)]
enum Fragment<'a> {
    Text(&'a [Chunk], usize),
    Gap(usize, &'a Paint),
}

impl<'a> Fragment<'a> {
    fn width(self) -> usize {
        match self {
            Self::Text(_, width) | Self::Gap(width, _) => width,
        }
    }

    fn visit(self, limit: usize, mut draw: impl FnMut(usize, &str, usize, &'a Paint)) -> usize {
        match self {
            Self::Text(chunks, _) => visit_chunks(chunks, limit, draw),
            Self::Gap(width, paint) => {
                const SPACES: &str = "                                ";
                let end = width.min(limit);
                for start in (0..end).step_by(SPACES.len()) {
                    let n = (end - start).min(SPACES.len());
                    draw(start, &SPACES[..n], n, paint);
                }
                end
            }
        }
    }
}

type Visit<'a, 'b> = dyn FnMut(usize, Fragment<'a>) + 'b;

/// Only the fragment that reaches the right edge must wait until overflow is known.
#[derive(Default)]
struct Row<'a> {
    width: usize,
    edge: Option<(usize, Fragment<'a>)>,
    last_paint: Option<&'a Paint>,
}

type Draw<'a> = dyn FnMut(usize, usize, &str, usize, &Paint) + 'a;

fn layout(root: Option<&PreparedNode>, bounds: Size, mut draw: Option<&mut Draw<'_>>) -> Size {
    let Some(root) = root else {
        return Size::default();
    };
    let width = usize::from(bounds.width);
    let height = usize::from(bounds.height);
    if width == 0 || height == 0 {
        return Size::default();
    }
    let mut rows = Vec::<Row>::new();
    let total = layout_node(root, width, height + 1, &mut |y, fragment| {
        if y >= height {
            return;
        }
        if rows.len() <= y {
            rows.resize_with(y + 1, Row::default);
        }
        let row = &mut rows[y];
        let start = row.width;
        row.width += fragment.width();
        if start < width {
            if row.width >= width {
                row.edge = Some((start, fragment));
            } else if let Some(draw) = draw.as_deref_mut() {
                fragment.visit(usize::MAX, |offset, text, cells, paint| {
                    draw(y, start + offset, text, cells, paint);
                    row.last_paint = Some(paint);
                });
            }
        }
    });
    let mut measured = Size::new(0, total.min(height) as u16);
    for (y, row) in rows.into_iter().enumerate() {
        let clipped = row.width > width || (y + 1 == height && total > height);
        let mut end = row.width;
        let mut last_paint = row.last_paint;
        if clipped && width == 1 {
            end = 0;
            last_paint = None;
        } else if let Some((start, edge)) = row.edge
            && (clipped || draw.is_some())
        {
            let limit = width - usize::from(clipped) - start;
            end = start
                + edge.visit(limit, |offset, text, cells, paint| {
                    if let Some(draw) = draw.as_deref_mut() {
                        draw(y, start + offset, text, cells, paint);
                    }
                    last_paint = Some(paint);
                });
        }
        if clipped {
            if let Some(draw) = draw.as_deref_mut() {
                let mut paint = last_paint.cloned().unwrap_or_default();
                paint.href = None;
                draw(y, end, "…", 1, &paint);
            }
            end += 1;
        }
        measured.width = measured.width.max(end as u16);
    }
    measured
}

fn layout_node<'a>(
    node: &'a PreparedNode,
    width: usize,
    limit: usize,
    visit: &mut Visit<'a, '_>,
) -> usize {
    match &node.kind {
        NodeKind::Text(text) => text.layout(width, limit, visit),
        NodeKind::Row {
            children,
            gap,
            wrap,
            paint,
        } => {
            let gap = (*gap).min((width + 1) as u64) as usize;
            let mut y = 0;
            let mut start = 0;
            let mut used = 0;
            for (i, child) in children.iter().enumerate() {
                let preferred = child.widths.preferred.min((width + 1) as u64) as usize;
                if *wrap == RowWrap::Wrap && i > start && used + gap + preferred > width {
                    y += layout_band(
                        &children[start..i],
                        width,
                        limit - y,
                        gap,
                        paint,
                        &mut |row, part| visit(y + row, part),
                    );
                    if y == limit {
                        return y;
                    }
                    start = i;
                    used = 0;
                }
                if i > start {
                    used += gap;
                }
                used += preferred;
            }
            y + layout_band(
                &children[start..],
                width,
                limit - y,
                gap,
                paint,
                &mut |row, part| visit(y + row, part),
            )
        }
    }
}

fn layout_band<'a>(
    children: &'a [PreparedNode],
    width: usize,
    limit: usize,
    gap: usize,
    paint: &'a Paint,
    visit: &mut Visit<'a, '_>,
) -> usize {
    let bounded = |cells: u64| cells.min((width + 1) as u64) as usize;
    let mut reserved = children
        .iter()
        .map(|c| bounded(c.widths.minimum))
        .sum::<usize>()
        + gap * children.len().saturating_sub(1);
    let mut offset = 0usize;
    let mut ends = Vec::new();
    for child in children {
        let minimum = bounded(child.widths.minimum);
        reserved -= minimum;
        let available = bounded(child.widths.preferred)
            .min(width.saturating_sub(offset + reserved))
            .max(minimum);
        layout_node(child, available.clamp(1, width), limit, &mut |y, part| {
            if y >= ends.len() {
                ends.resize(y + 1, 0);
            }
            let end = &mut ends[y];
            let start = offset.min(width + 1);
            if start > *end {
                visit(y, Fragment::Gap(start - *end, paint));
                *end = start;
            }
            if offset <= width {
                visit(y, part);
                *end += part.width();
            }
        });
        offset += available + gap;
        reserved = reserved.saturating_sub(gap);
    }
    ends.len()
}

fn sanitize_leaf(mut raw: String) -> String {
    // Directly constructed trees can bypass the wire decoder's text limit.
    if raw.len() > MAX_TEXT_BYTES {
        raw.truncate(raw.floor_char_boundary(MAX_TEXT_BYTES));
        raw.shrink_to_fit();
    }
    if raw
        .chars()
        .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
    {
        return raw;
    }
    let mut actions = CollectingVTActor::default();
    VTParser::new().parse(raw.as_bytes(), &mut actions);
    actions
        .into_iter()
        .filter_map(|action| match action {
            VTAction::Print(ch) if !ch.is_control() => Some(ch),
            VTAction::ExecuteC0orC1(ch @ (b'\n' | b'\t')) => Some(char::from(ch)),
            _ => None,
        })
        .collect()
}

fn push_link(line: &mut HyperlinkLine, columns: Range<usize>, href: &str) {
    if let Some(last) = line.hyperlinks.last_mut()
        && last.columns.end == columns.start
        && last.destination == href
    {
        last.columns.end = columns.end;
    } else {
        line.hyperlinks
            .push(TerminalHyperlink::web(columns, href.to_owned()));
    }
}

impl TextProps {
    fn apply_to(&self, paint: &mut Paint) {
        if let Some(color) = self.color {
            paint.foreground = Some(color);
        }
        for (enabled, modifier) in [
            (
                self.weight.map(|weight| weight == TextWeight::Bold),
                Modifier::BOLD,
            ),
            (self.italic, Modifier::ITALIC),
            (self.underline, Modifier::UNDERLINED),
        ] {
            match enabled {
                Some(true) => paint.modifiers = paint.modifiers.add_modifier(modifier),
                Some(false) => paint.modifiers = paint.modifiers.remove_modifier(modifier),
                None => {}
            }
        }
    }
}
