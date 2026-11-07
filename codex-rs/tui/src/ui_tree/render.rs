//! Render the visible prefix of a horizontal display tree into one terminal line.

mod prepare;
mod text;

use self::prepare::NodeKind;
use self::prepare::PreparedNode;

use super::MAX_TEXT_BYTES;
use super::TextProps;
use super::TextWeight;
use super::UiTree;
use super::theme::RenderTheme;
use crate::line_truncation::truncate_line_with_ellipsis;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
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
        let Some(root) = self.0.as_ref().filter(|_| width > 0) else {
            return HyperlinkLine::default();
        };
        let width = usize::from(width);
        let mut line = first_line(root, width);
        if line.width() > width {
            line.line = truncate_line_with_ellipsis(line.line, width);
            line.clip_hyperlinks_to_width(line.width() - 1);
        }
        line
    }
}

fn first_line(node: &PreparedNode<'_>, width: usize) -> HyperlinkLine {
    let mut line = HyperlinkLine::default();
    match &node.kind {
        NodeKind::Text(text) => {
            if let Some(first) = text.lines.first() {
                text::paint_chunks(first, &mut line, /*start*/ 0);
            }
        }
        NodeKind::Row {
            children,
            gap,
            paint,
        } => {
            let mut used = 0;
            for child in children {
                let fragment = first_line(child, width - used);
                let child_width = fragment.width();
                if child_width == 0 {
                    continue;
                }
                if used > 0 {
                    let gap = (*gap).min((width + 1 - used) as u64) as usize;
                    if gap != 0 {
                        line.line
                            .push_span(Span::styled(" ".repeat(gap), paint.style));
                        if let Some(href) = paint.href {
                            push_link(&mut line, used..used + gap, href);
                        }
                        used += gap;
                    }
                }
                line.line.spans.extend(fragment.line.spans);
                for link in fragment.hyperlinks {
                    push_link(
                        &mut line,
                        used + link.columns.start..used + link.columns.end,
                        &link.destination,
                    );
                }
                used += child_width;
                if used > width {
                    break;
                }
            }
        }
    }
    line
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
