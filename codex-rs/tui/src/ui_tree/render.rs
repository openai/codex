//! Lay out prepared text and nested rows within a terminal viewport.

mod prepare;
mod text;

use self::prepare::NodeKind;
use self::prepare::PreparedNode;
use self::prepare::RowPaint;
use self::text::Chunk;
use self::text::Paint;
use self::text::visit_chunks;
use super::MAX_TEXT_BYTES;
use super::RowJustify;
use super::RowProps;
use super::RowWrap;
use super::TextProps;
use super::TextWeight;
use super::UiTree;
use super::theme::RenderTheme;
use crate::terminal_hyperlinks::mark_hyperlink_cells;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use ratatui::style::Style;
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
}

#[derive(Clone, Copy)]
enum Fragment<'a> {
    Text(&'a [Chunk], usize),
    Padding(usize, &'a Paint),
    Gap(usize, &'a Paint),
}

impl<'a> Fragment<'a> {
    fn width(self) -> usize {
        match self {
            Self::Text(_, width) | Self::Padding(width, _) | Self::Gap(width, _) => width,
        }
    }

    fn visit(self, limit: usize, mut draw: impl FnMut(usize, &str, usize, &'a Paint)) -> usize {
        match self {
            Self::Text(chunks, _) => visit_chunks(chunks, limit, draw),
            Self::Padding(width, paint) | Self::Gap(width, paint) => {
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

#[derive(Clone, Copy)]
struct AvailableWidth {
    cells: usize,
    // A centered or end-aligned group leaves wholly blank lines unpainted, even in nested rows.
    align_content_only: bool,
}

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
    let available = AvailableWidth {
        cells: width,
        align_content_only: false,
    };
    let total = layout_visible(root, available, height, &mut |y, fragment| {
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

// On the last visible line, alignment padding can give up a cell for the vertical
// overflow marker. Defer the suffix starting at that padding until we see the next line.
fn layout_visible<'a>(
    root: &'a PreparedNode,
    width: AvailableWidth,
    height: usize,
    visit: &mut Visit<'a, '_>,
) -> usize {
    let mut tail = Vec::new();
    let mut padding = None;
    let mut last_width = 0;
    let total = layout_node(root, width, height + 1, &mut |y, fragment| {
        if y >= height {
            return;
        }
        if y + 1 == height {
            last_width += fragment.width();
            if matches!(fragment, Fragment::Padding(..)) {
                padding = Some(tail.len());
            }
            if padding.is_some() {
                tail.push(fragment);
                return;
            }
        }
        visit(y, fragment);
    });
    if total > height
        && last_width == width.cells
        && let Some(index) = padding
        && let Fragment::Padding(cells, _) = &mut tail[index]
    {
        *cells -= 1;
    }
    for fragment in tail {
        visit(height - 1, fragment);
    }
    total
}

fn layout_node<'a>(
    node: &'a PreparedNode,
    width: AvailableWidth,
    limit: usize,
    visit: &mut Visit<'a, '_>,
) -> usize {
    match &node.kind {
        NodeKind::Empty => 0,
        NodeKind::Text(text) => text.layout(width, limit, visit),
        NodeKind::Row {
            children,
            props,
            paint,
        } => {
            let width = AvailableWidth {
                align_content_only: width.align_content_only
                    || matches!(props.justify, Some(RowJustify::Center | RowJustify::End)),
                ..width
            };
            let gap = props.gap.min((width.cells + 1) as u64) as usize;
            let mut place_band = |children: &'a [PreparedNode], used, y| {
                layout_band(
                    children,
                    width,
                    limit - y,
                    props,
                    width.cells.saturating_sub(used),
                    paint,
                    &mut |row, part| visit(y + row, part),
                )
            };
            let mut y = 0;
            let mut start = 0;
            let mut used = 0;
            for (i, child) in children.iter().enumerate() {
                let preferred = child.widths.preferred.min((width.cells + 1) as u64) as usize;
                if props.wrap == RowWrap::Wrap && i > start && used + gap + preferred > width.cells
                {
                    y += place_band(&children[start..i], used, y);
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
            y + place_band(&children[start..], used, y)
        }
    }
}

fn layout_band<'a>(
    children: &'a [PreparedNode],
    width: AvailableWidth,
    limit: usize,
    props: &RowProps,
    spare: usize,
    paint: &'a RowPaint,
    visit: &mut Visit<'a, '_>,
) -> usize {
    let bounded = |cells: u64| cells.min((width.cells + 1) as u64) as usize;
    let gap = bounded(props.gap);
    let (leading, between) = match props.justify.unwrap_or_default() {
        RowJustify::Start => (0, 0),
        RowJustify::Center => (spare / 2, 0),
        RowJustify::End => (spare, 0),
        RowJustify::Between => (0, spare),
    };
    let gaps = children.len().saturating_sub(1);
    // Round each item's cumulative share to the nearest cell, keeping both edges fixed.
    let share = |i: usize| {
        (i * between + gaps / 2)
            .checked_div(gaps)
            .unwrap_or_default()
    };
    let mut reserved = children
        .iter()
        .map(|c| bounded(c.widths.minimum))
        .sum::<usize>()
        + gap * gaps
        + share(gaps);
    let mut offset = leading;
    let mut ends = Vec::new();
    for (i, child) in children.iter().enumerate() {
        let minimum = bounded(child.widths.minimum);
        reserved -= minimum;
        let available = bounded(child.widths.preferred)
            .min(width.cells.saturating_sub(offset + reserved))
            .max(minimum);
        let child_width = AvailableWidth {
            cells: available.clamp(1, width.cells),
            ..width
        };
        layout_node(child, child_width, limit, &mut |y, part| {
            if y >= ends.len() {
                ends.resize(y + 1, 0);
            }
            let end = &mut ends[y];
            let start = offset.min(width.cells + 1);
            if part.width() > 0 && *end == 0 && leading > 0 {
                visit(y, Fragment::Padding(leading, &paint.leading));
                *end = leading;
            }
            if start > *end && (!width.align_content_only || *end > 0 || part.width() > 0) {
                visit(y, Fragment::Gap(start - *end, &paint.gap));
                *end = start;
            }
            // Forward blank lines so ancestors count them toward the height limit.
            if offset <= width.cells || part.width() == 0 {
                visit(y, part);
                *end += part.width();
            }
        });
        let next_gap = if i < gaps {
            gap + share(i + 1) - share(i)
        } else {
            0
        };
        offset += available + next_gap;
        reserved = reserved.saturating_sub(next_gap);
    }
    ends.len()
}

pub(crate) fn sanitize_leaf(mut raw: String) -> String {
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
