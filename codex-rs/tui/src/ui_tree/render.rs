//! Lay out prepared text, rows, and columns within a terminal viewport.

mod flex;
mod prepare;
mod text;

use self::prepare::NodeKind;
use self::prepare::PreparedNode;
use self::text::Chunk;
use self::text::Paint;
use self::text::visit_chunks;
use super::MAX_TEXT_BYTES;
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
    /// The visible background of a flex item counts even on an otherwise blank line.
    Background(usize, &'a Paint),
    /// A vertical gap takes up a line without advancing the horizontal position.
    BlankLine,
    Ellipsis(&'a Paint),
}

impl<'a> Fragment<'a> {
    fn width(self) -> usize {
        match self {
            Self::Text(_, width)
            | Self::Padding(width, _)
            | Self::Gap(width, _)
            | Self::Background(width, _) => width,
            Self::BlankLine => 0,
            Self::Ellipsis(_) => 1,
        }
    }

    fn has_content(self) -> bool {
        self.width() > 0
            && !matches!(
                self,
                Self::Padding(..) | Self::Gap(..) | Self::Background(..)
            )
    }

    fn prefix(self, limit: usize, cut: text::Cut) -> Self {
        if self.width() <= limit {
            return self;
        }
        match self {
            Self::Text(chunks, _) => {
                Self::Text(chunks, visit_chunks(chunks, limit, cut, |_, _, _, _| {}))
            }
            Self::Padding(_, paint) => Self::Padding(limit, paint),
            Self::Gap(_, paint) => Self::Gap(limit, paint),
            Self::Background(_, paint) => Self::Background(limit, paint),
            Self::Ellipsis(_) | Self::BlankLine => Self::BlankLine,
        }
    }

    fn visit(
        self,
        limit: usize,
        cut: text::Cut,
        mut draw: impl FnMut(usize, &str, usize, &'a Paint),
    ) -> usize {
        match self {
            Self::Ellipsis(paint) => {
                if limit > 0 {
                    draw(0, "…", 1, paint);
                    1
                } else {
                    0
                }
            }
            Self::Text(chunks, width) => visit_chunks(chunks, limit.min(width), cut, draw),
            Self::BlankLine => 0,
            Self::Padding(width, paint)
            | Self::Gap(width, paint)
            | Self::Background(width, paint) => {
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
    ends_in_marker: bool,
    content_before_edge: bool,
    width: usize,
    edge: Option<(usize, Fragment<'a>)>,
    last_paint: Option<&'a Paint>,
}

type Draw<'a> = dyn FnMut(usize, usize, &str, usize, &Paint) + 'a;

#[derive(Clone, Copy)]
struct AvailableWidth {
    cells: usize,
    // Cells remaining in the viewport after ancestor offsets, before flex sizing.
    visible: usize,
    in_aligned_row: bool,
    // A row used as a flex item has an allocated width, unlike the root viewport.
    fill_background: bool,
    height: Height,
}

#[derive(Clone, Copy)]
enum Height {
    // Intrinsic height only. Count gaps arithmetically and skip nested stretch passes.
    Measure,
    Grow(usize),
}

impl Height {
    fn extra(self) -> usize {
        match self {
            Self::Grow(extra) => extra,
            Self::Measure => 0,
        }
    }

    fn with_extra(self, extra: usize) -> Self {
        match self {
            Self::Grow(_) => Self::Grow(extra),
            Self::Measure => Self::Measure,
        }
    }
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
        visible: width,
        in_aligned_row: false,
        fill_background: false,
        height: Height::Grow(0),
    };
    let total = layout_visible(
        root,
        available,
        height,
        /*marker*/ None,
        &mut |y, fragment| {
            if rows.len() <= y {
                rows.resize_with(y + 1, Row::default);
            }
            let row = &mut rows[y];
            let start = row.width;
            row.width += fragment.width();
            if fragment.width() > 0 {
                row.ends_in_marker = matches!(fragment, Fragment::Ellipsis(_));
            }
            if row.width < width {
                row.content_before_edge |= fragment.has_content();
            }
            if start < width {
                if row.width >= width {
                    row.edge = Some((start, fragment));
                } else if let Some(draw) = draw.as_deref_mut() {
                    fragment.visit(
                        usize::MAX,
                        text::Cut::Viewport,
                        |offset, text, cells, paint| {
                            draw(y, start + offset, text, cells, paint);
                            row.last_paint = Some(paint);
                        },
                    );
                }
            }
        },
    );
    let mut measured = Size::new(0, total.min(height) as u16);
    for (y, row) in rows.into_iter().enumerate() {
        let clipped =
            row.width > width || (y + 1 == height && total > height && !row.ends_in_marker);
        let mut end = row.width;
        let mut last_paint = row.last_paint;
        if clipped && width == 1 {
            end = 0;
            last_paint = None;
        } else if let Some((start, edge)) = row.edge
            && (clipped || draw.is_some())
        {
            let limit = width - usize::from(clipped) - start;
            let cut = if clipped {
                text::Cut::LineClamp {
                    prior_content: row.content_before_edge,
                }
            } else {
                text::Cut::Viewport
            };
            end = start
                + edge.visit(limit, cut, |offset, text, cells, paint| {
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
    marker: Option<&'a Paint>,
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
            if matches!(fragment, Fragment::Padding(cells, _) if cells > 0) {
                padding = Some(tail.len());
            }
            if marker.is_some() || padding.is_some() {
                tail.push(fragment);
                return;
            }
        }
        visit(y, fragment);
    });
    let marked = tail
        .iter()
        .rev()
        .find(|f| f.width() > 0)
        .is_some_and(|f| matches!(f, Fragment::Ellipsis(_)));
    let clipped = total > height && (!marked || last_width > width.cells);
    if clipped
        && last_width == width.cells
        && let Some(index) = padding
        && let Fragment::Padding(cells, _) = &mut tail[index]
    {
        *cells -= 1;
    }
    if let Some(marker) = marker.filter(|_| clipped && width.cells > 0) {
        let mut remaining = width.cells - 1;
        let mut prior_content = false;
        for fragment in tail {
            let prefix = fragment.prefix(remaining, text::Cut::LineClamp { prior_content });
            prior_content |= prefix.has_content();
            visit(height - 1, prefix);
            remaining -= prefix.width();
            if prefix.width() < fragment.width() {
                break;
            }
        }
        visit(height - 1, Fragment::Ellipsis(marker));
        return height;
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
        NodeKind::Empty | NodeKind::Spacer => 0,
        NodeKind::Clamped {
            content,
            max_lines,
            marker,
        } => {
            if *max_lines >= limit as u64 {
                layout_node(content, width, limit, visit)
            } else {
                let height = *max_lines as usize;
                if matches!(width.height, Height::Measure) {
                    layout_node(content, width, height, visit)
                } else {
                    layout_visible(content, width, height, Some(marker), visit).min(height)
                }
            }
        }
        NodeKind::Text(text) => text.layout(width, limit, visit),
        NodeKind::Col {
            children,
            gap,
            spacers,
        } => flex::column(children, *gap, *spacers, width, limit, visit),
        NodeKind::Row {
            children,
            props,
            paint,
        } => flex::row(children, props, paint, width, limit, visit),
    }
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
        paint.preserve_whitespace |= self.preserve_whitespace && self.max_lines.is_none();
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
