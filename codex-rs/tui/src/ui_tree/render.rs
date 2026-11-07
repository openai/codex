//! Render the visible prefix of a horizontal display tree into one terminal line.

mod text;

use self::text::InlineText;
use self::text::Paint;
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
use std::ops::Range;
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
        let output = LineBuilder::from_node(
            root,
            width,
            Style::default(),
            /*href*/ None,
            &RenderTheme::new(theme),
        );
        let mut line = output.line;
        if output.used > width {
            line.line = truncate_line_with_ellipsis(line.line, width);
            line.clip_hyperlinks_to_width(line.width() - 1);
        }
        line
    }
}

struct LineBuilder<'a> {
    inline: InlineText<'a>,
    line: HyperlinkLine,
    used: usize,
    width: usize,
    saw_row: bool,
}

impl<'a> LineBuilder<'a> {
    fn from_node(
        node: &'a UiNode,
        width: usize,
        style: Style,
        href: Option<&'a str>,
        theme: &RenderTheme<'_>,
    ) -> Self {
        let mut output = Self {
            inline: InlineText::default(),
            line: HyperlinkLine::default(),
            used: 0,
            width,
            saw_row: false,
        };
        output.visit(node, style, href, theme);
        output.flush_runs();
        output
    }

    fn visit(
        &mut self,
        node: &'a UiNode,
        mut style: Style,
        mut href: Option<&'a str>,
        theme: &RenderTheme<'_>,
    ) {
        let children = match node {
            UiNode::String(raw) => {
                self.inline.push_leaf(raw, Paint { style, href });
                return;
            }
            UiNode::Row { props, children } => {
                self.saw_row = true;
                self.flush_runs();
                if let Some(color) = &props.background {
                    style = style.bg(color.resolve(theme));
                }
                let start = self.used;
                for child in children.iter().flatten() {
                    if self.used > self.width {
                        break;
                    }
                    let child = Self::from_node(child, self.width - self.used, style, href, theme);
                    if child.used == 0 {
                        continue;
                    }
                    if self.used > start && props.gap > 0 {
                        let gap = props.gap.min((self.width + 1 - self.used) as u64) as usize;
                        self.line
                            .line
                            .push_span(Span::styled(" ".repeat(gap), style));
                        if let Some(url) = href {
                            push_link(&mut self.line, self.used..self.used + gap, url);
                        }
                        self.used += gap;
                    }
                    self.line.line.spans.extend(child.line.line.spans);
                    for mut link in child.line.hyperlinks {
                        link.columns = self.used + link.columns.start..self.used + link.columns.end;
                        push_link(&mut self.line, link.columns, &link.destination);
                    }
                    self.used += child.used;
                }
                return;
            }
            UiNode::Text { props, children } => {
                style = props.style(style, theme);
                children
            }
            UiNode::Link { props, children } => {
                href = Some(&props.href.0);
                style = style.underlined();
                children
            }
            UiNode::Bold { children } => {
                style = style.bold();
                children
            }
            UiNode::Italic { children } => {
                style = style.italic();
                children
            }
            UiNode::Underline { children } => {
                style = style.underlined();
                children
            }
        };
        for node in children.iter().flatten() {
            if self.used > self.width {
                break;
            }
            self.visit(node, style, href, theme);
        }
    }

    fn flush_runs(&mut self) {
        let start = self.used;
        let first_span = self.line.line.spans.len();
        let text = std::mem::take(&mut self.inline).finish();
        if let Some(first) = text.lines.first() {
            self.used += text::paint_chunks(first, &mut self.line, self.used);
        }
        // A zero-cell fragment between row children must not style a later ellipsis.
        if self.saw_row && self.used == start {
            self.line.line.spans.truncate(first_span);
        }
    }
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
