//! Lay out prepared text and nested rows within a terminal viewport.

mod prepare;
mod text;

use self::prepare::NodeKind;
use self::prepare::PreparedNode;
use self::text::Paint;
use super::RowWrap;

use super::MAX_TEXT_BYTES;
use super::TextProps;
use super::TextWeight;
use super::UiTree;
use super::theme::RenderTheme;
use crate::line_truncation::truncate_line_with_ellipsis;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use std::borrow::Cow;
use std::ops::Range;
use syntect::highlighting::Theme;
use vtparse::CollectingVTActor;
use vtparse::VTAction;
use vtparse::VTParser;

pub(crate) struct PreparedUiTree<'a>(Option<PreparedNode<'a>>);

impl UiTree {
    pub(crate) fn prepare(&self, theme: &Theme) -> PreparedUiTree<'_> {
        let theme = RenderTheme::new(theme);
        PreparedUiTree(self.0.as_ref().and_then(|node| {
            PreparedNode::from_node(node, Style::default(), /*href*/ None, &theme)
        }))
    }

    pub(crate) fn render_line(&self, width: u16, theme: &Theme) -> HyperlinkLine {
        self.prepare(theme).render_line(width)
    }
}

impl PreparedUiTree<'_> {
    pub(crate) fn render_line(&self, width: u16) -> HyperlinkLine {
        self.render_lines(Size::new(width, /*height*/ 1))
            .pop()
            .unwrap_or_default()
    }

    /// Fit the tree to the viewport, marking clipped rows and vertical overflow with an unlinked ellipsis.
    pub(crate) fn render_lines(&self, size: Size) -> Vec<HyperlinkLine> {
        let Some(item) = &self.0 else {
            return Vec::new();
        };
        if size.width == 0 || size.height == 0 {
            return Vec::new();
        }
        let width = usize::from(size.width);
        let height = usize::from(size.height);
        // The extra row distinguishes an exact fit from vertical overflow.
        let mut lines = layout_node(item, width, height + 1);
        let more = lines.len() > height;
        lines.truncate(height);
        for (index, line) in lines.iter_mut().enumerate() {
            if line.width() > width || (more && index + 1 == height) {
                line.line = truncate_line_with_ellipsis(std::mem::take(&mut line.line), width);
                line.clip_hyperlinks_to_width(line.width() - 1);
            }
        }
        lines
    }
}

fn layout_node(node: &PreparedNode<'_>, width: usize, limit: usize) -> Vec<HyperlinkLine> {
    match &node.kind {
        NodeKind::Text(text) => text.layout(width, limit),
        NodeKind::Row {
            children,
            gap,
            wrap,
            paint,
        } => {
            let gap = (*gap).min((width + 1) as u64) as usize;
            let mut lines = Vec::new();
            let mut start = 0;
            let mut used = 0;
            for (i, child) in children.iter().enumerate() {
                let preferred = child.widths.preferred.min((width + 1) as u64) as usize;
                if *wrap == RowWrap::Wrap && i > start && used + gap + preferred > width {
                    lines.extend(layout_band(
                        &children[start..i],
                        width,
                        limit - lines.len(),
                        gap,
                        *paint,
                    ));
                    if lines.len() == limit {
                        return lines;
                    }
                    start = i;
                    used = 0;
                }
                if i > start {
                    used += gap;
                }
                used += preferred;
            }
            lines.extend(layout_band(
                &children[start..],
                width,
                limit - lines.len(),
                gap,
                *paint,
            ));
            lines
        }
    }
}

fn layout_band(
    children: &[PreparedNode<'_>],
    width: usize,
    limit: usize,
    gap: usize,
    paint: Paint<'_>,
) -> Vec<HyperlinkLine> {
    let bounded = |cells: u64| cells.min((width + 1) as u64) as usize;
    let mut reserved = children
        .iter()
        .map(|c| bounded(c.widths.minimum))
        .sum::<usize>()
        + gap * children.len().saturating_sub(1);
    let mut offset = 0;
    let mut rows: Vec<(HyperlinkLine, usize)> = Vec::new();
    for child in children {
        let minimum = bounded(child.widths.minimum);
        reserved -= minimum;
        let available = bounded(child.widths.preferred)
            .min(width.saturating_sub(offset + reserved))
            .max(minimum);
        let fragments = layout_node(child, available.clamp(1, width), limit);
        if rows.len() < fragments.len() {
            rows.resize_with(fragments.len(), Default::default);
        }
        for ((line, end), fragment) in rows.iter_mut().zip(fragments) {
            // One extra column records overflow without materializing a provider-sized gap.
            let start = offset.min(width + 1);
            if start > *end {
                line.line
                    .push_span(Span::styled(" ".repeat(start - *end), paint.style));
                if let Some(href) = paint.href {
                    push_link(line, *end..start, href);
                }
                *end = start;
            }
            if offset > width {
                continue;
            }
            let start = *end;
            *end += fragment.width();
            line.line.spans.extend(fragment.line.spans);
            for link in fragment.hyperlinks {
                push_link(
                    line,
                    start + link.columns.start..start + link.columns.end,
                    &link.destination,
                );
            }
        }
        offset += available + gap;
        reserved = reserved.saturating_sub(gap);
    }
    rows.into_iter().map(|(line, _)| line).collect()
}

fn sanitize_leaf(raw: &str) -> Cow<'_, str> {
    // Built-in values can bypass the wire decoder's text limit.
    let raw = &raw[..raw.floor_char_boundary(MAX_TEXT_BYTES)];
    if raw
        .chars()
        .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
    {
        return Cow::Borrowed(raw);
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
    fn style(&self, mut style: Style, theme: &RenderTheme<'_>) -> Style {
        if let Some(color) = &self.color {
            style = style.fg(color.resolve(theme));
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
                Some(true) => style = style.add_modifier(modifier),
                Some(false) => style = style.remove_modifier(modifier),
                None => {}
            }
        }
        style
    }
}
