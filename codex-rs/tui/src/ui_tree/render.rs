//! Render the visible prefix of a horizontal display tree into one terminal line.

use super::MAX_TEXT_BYTES;
use super::TextProps;
use super::TextWeight;
use super::UiNode;
use super::UiTree;
use super::theme::RenderTheme;
use crate::line_truncation::truncate_line_with_ellipsis;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use std::borrow::Cow;
use syntect::highlighting::Theme;
use vtparse::CollectingVTActor;
use vtparse::VTAction;
use vtparse::VTParser;

impl UiTree {
    /// Render one row. On overflow, fit an unlinked ellipsis inside the supplied width.
    pub(crate) fn render_line(&self, width: u16, theme: &Theme) -> HyperlinkLine {
        let width = usize::from(width);
        let Some(root) = self.0.as_ref().filter(|_| width > 0) else {
            return HyperlinkLine::default();
        };
        let mut line = node_line(root, width, Style::default(), &RenderTheme::new(theme));
        if line.width() > width {
            line.line = truncate_line_with_ellipsis(line.line, width);
            line.clip_hyperlinks_to_width(line.width() - 1);
        }
        line
    }
}

fn node_line(node: &UiNode, width: usize, style: Style, theme: &RenderTheme<'_>) -> HyperlinkLine {
    let (children, gap, style) = match node {
        UiNode::String(raw) => {
            let text = sanitize_leaf(raw);
            let first = text.split('\n').next().unwrap_or_default();
            let visible = first.replace('\t', "    ");
            return HyperlinkLine::new(Span::styled(visible, style).into());
        }
        UiNode::Link { props, children } => {
            let mut line = row_line(children, /*gap*/ 0, width, style.underlined(), theme);
            let width = line.width();
            if width > 0 {
                line.hyperlinks = vec![TerminalHyperlink::web(0..width, props.href.0.clone())];
            }
            return line;
        }
        UiNode::Row { props, children } => {
            let style = props
                .background
                .as_ref()
                .map_or(style, |color| style.bg(color.resolve(theme)));
            (children, props.gap, style)
        }
        UiNode::Text { props, children } => (children, 0, props.style(style, theme)),
        UiNode::Bold { children } => (children, 0, style.bold()),
        UiNode::Italic { children } => (children, 0, style.italic()),
        UiNode::Underline { children } => (children, 0, style.underlined()),
    };
    row_line(children, gap, width, style, theme)
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

fn row_line(
    children: &[Option<UiNode>],
    gap: u64,
    width: usize,
    style: Style,
    theme: &RenderTheme<'_>,
) -> HyperlinkLine {
    let mut result = HyperlinkLine::default();
    let mut used = 0;
    for child in children.iter().flatten() {
        let child = node_line(child, width, style, theme);
        let child_width = child.width();
        if child_width == 0 {
            continue;
        }
        if used > 0 && gap > 0 {
            // One column past the viewport is sufficient to detect overflow.
            let spacing = gap.min((width + 1 - used) as u64) as usize;
            result
                .line
                .push_span(Span::styled(" ".repeat(spacing), style));
            used += spacing;
        }
        let offset = used;
        result.line.spans.extend(child.line.spans);
        result
            .hyperlinks
            .extend(child.hyperlinks.into_iter().map(|mut link| {
                link.columns = link.columns.start + offset..link.columns.end + offset;
                link
            }));
        used += child_width;
        if used > width {
            break;
        }
    }
    result
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
