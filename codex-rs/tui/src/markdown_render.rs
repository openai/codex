use crate::render::line_utils::line_to_static;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use codex_ansi_escape::ansi_escape;
use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::HeadingLevel;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use termimad::MadSkin;
use termimad::crossterm::style::Attribute;
use termimad::crossterm::style::Color as TermColor;

struct MarkdownStyles {
    h1: Style,
    h2: Style,
    h3: Style,
    h4: Style,
    h5: Style,
    h6: Style,
    code: Style,
    emphasis: Style,
    strong: Style,
    strikethrough: Style,
    ordered_list_marker: Style,
    unordered_list_marker: Style,
    link: Style,
    blockquote: Style,
}

impl Default for MarkdownStyles {
    fn default() -> Self {
        use ratatui::style::Stylize;

        Self {
            h1: Style::new().bold().underlined(),
            h2: Style::new().bold(),
            h3: Style::new().bold().italic(),
            h4: Style::new().italic(),
            h5: Style::new().italic(),
            h6: Style::new().italic(),
            code: Style::new().cyan(),
            emphasis: Style::new().italic(),
            strong: Style::new().bold(),
            strikethrough: Style::new().crossed_out(),
            ordered_list_marker: Style::new().light_blue(),
            unordered_list_marker: Style::new(),
            link: Style::new().cyan().underlined(),
            blockquote: Style::new().green(),
        }
    }
}

#[derive(Clone, Debug)]
struct IndentContext {
    prefix: Vec<Span<'static>>,
    marker: Option<Vec<Span<'static>>>,
    is_list: bool,
}

impl IndentContext {
    fn new(prefix: Vec<Span<'static>>, marker: Option<Vec<Span<'static>>>, is_list: bool) -> Self {
        Self {
            prefix,
            marker,
            is_list,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MarkdownRenderer {
    Pulldown = 0,
    Termimad = 1,
}

static MARKDOWN_RENDERER: AtomicU8 = AtomicU8::new(MarkdownRenderer::Pulldown as u8);

pub(crate) fn set_markdown_renderer(renderer: MarkdownRenderer) {
    MARKDOWN_RENDERER.store(renderer as u8, Ordering::Relaxed);
}

fn markdown_renderer() -> MarkdownRenderer {
    match MARKDOWN_RENDERER.load(Ordering::Relaxed) {
        1 => MarkdownRenderer::Termimad,
        _ => MarkdownRenderer::Pulldown,
    }
}

fn termimad_skin() -> &'static MadSkin {
    static SKIN: OnceLock<MadSkin> = OnceLock::new();
    SKIN.get_or_init(|| {
        let mut skin = MadSkin::no_style();
        skin.bold.add_attr(Attribute::Bold);
        skin.italic.add_attr(Attribute::Italic);
        skin.strikeout.add_attr(Attribute::CrossedOut);
        skin.inline_code.set_fg(TermColor::Cyan);
        skin.code_block.set_fg(TermColor::Cyan);
        for header in &mut skin.headers {
            header.compound_style.add_attr(Attribute::Bold);
        }
        skin.quote_mark.set_char('>');
        skin.quote_mark.set_fg(TermColor::Green);
        skin.bullet.set_char('-');
        skin.horizontal_rule.set_char('-');
        skin
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FenceKind {
    Markdown,
    Other,
}

fn unwrap_markdown_fences(input: &str) -> std::borrow::Cow<'_, str> {
    let mut out = String::with_capacity(input.len());
    let mut in_fence: Option<(u8, usize, FenceKind)> = None;
    let mut changed = false;

    for line in input.split_inclusive('\n') {
        let line_trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed_start = line_trimmed.trim_start();
        if let Some((fence_char, fence_len, rest)) = parse_fence_line(trimmed_start) {
            if let Some((open_char, open_len, kind)) = in_fence {
                if fence_char == open_char && fence_len >= open_len && rest.trim().is_empty() {
                    in_fence = None;
                    if kind == FenceKind::Markdown {
                        changed = true;
                        continue;
                    }
                }
            } else if is_markdown_fence_info(rest) {
                in_fence = Some((fence_char, fence_len, FenceKind::Markdown));
                changed = true;
                continue;
            } else {
                in_fence = Some((fence_char, fence_len, FenceKind::Other));
            }
        }
        out.push_str(line);
    }

    if changed {
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(input)
    }
}

fn parse_fence_line(line: &str) -> Option<(u8, usize, &str)> {
    let bytes = line.as_bytes();
    let first = *bytes.first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let mut len = 1;
    while len < bytes.len() && bytes[len] == first {
        len += 1;
    }
    if len < 3 {
        return None;
    }
    Some((first, len, &line[len..]))
}

fn is_markdown_fence_info(info: &str) -> bool {
    for part in info.trim().split_whitespace() {
        let lower = part.to_ascii_lowercase();
        if matches!(lower.as_str(), "markdown" | "md")
            || lower.ends_with(".md")
            || lower.ends_with(".markdown")
            || lower.ends_with(".mdx")
        {
            return true;
        }
    }
    false
}

pub fn render_markdown_text(input: &str) -> Text<'static> {
    render_markdown_text_with_width(input, None)
}

pub(crate) fn render_markdown_text_with_width(input: &str, width: Option<usize>) -> Text<'static> {
    match markdown_renderer() {
        MarkdownRenderer::Pulldown => render_markdown_text_with_width_pulldown(input, width),
        MarkdownRenderer::Termimad => render_markdown_text_with_width_termimad(input, width),
    }
}

fn render_markdown_text_with_width_termimad(input: &str, width: Option<usize>) -> Text<'static> {
    let unwrapped = unwrap_markdown_fences(input);
    let rendered = termimad_skin().text(unwrapped.as_ref(), width).to_string();
    let rendered = reset_ansi_on_newlines(&rendered);
    ansi_escape(&rendered)
}

fn reset_ansi_on_newlines(input: &str) -> std::borrow::Cow<'_, str> {
    if !input.contains('\n') {
        return std::borrow::Cow::Borrowed(input);
    }
    std::borrow::Cow::Owned(input.replace('\n', "\u{1b}[0m\n"))
}

fn render_markdown_text_with_width_pulldown(input: &str, width: Option<usize>) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut w = Writer::new(parser, width);
    w.run();
    w.text
}

struct Writer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    iter: I,
    text: Text<'static>,
    styles: MarkdownStyles,
    inline_styles: Vec<Style>,
    indent_stack: Vec<IndentContext>,
    list_indices: Vec<Option<u64>>,
    link: Option<String>,
    needs_newline: bool,
    pending_marker_line: bool,
    in_paragraph: bool,
    in_code_block: bool,
    wrap_width: Option<usize>,
    current_line_content: Option<Line<'static>>,
    current_initial_indent: Vec<Span<'static>>,
    current_subsequent_indent: Vec<Span<'static>>,
    current_line_style: Style,
    current_line_in_code_block: bool,
}

impl<'a, I> Writer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn new(iter: I, wrap_width: Option<usize>) -> Self {
        Self {
            iter,
            text: Text::default(),
            styles: MarkdownStyles::default(),
            inline_styles: Vec::new(),
            indent_stack: Vec::new(),
            list_indices: Vec::new(),
            link: None,
            needs_newline: false,
            pending_marker_line: false,
            in_paragraph: false,
            in_code_block: false,
            wrap_width,
            current_line_content: None,
            current_initial_indent: Vec::new(),
            current_subsequent_indent: Vec::new(),
            current_line_style: Style::default(),
            current_line_in_code_block: false,
        }
    }

    fn run(&mut self) {
        while let Some(ev) = self.iter.next() {
            self.handle_event(ev);
        }
        self.flush_current_line();
    }

    fn handle_event(&mut self, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.code(code),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => {
                self.flush_current_line();
                if !self.text.lines.is_empty() {
                    self.push_blank_line();
                }
                self.push_line(Line::from("———"));
                self.needs_newline = true;
            }
            Event::Html(html) => self.html(html, false),
            Event::InlineHtml(html) => self.html(html, true),
            Event::FootnoteReference(_) => {}
            Event::TaskListMarker(_) => {}
        }
    }

    fn start_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => self.start_paragraph(),
            Tag::Heading { level, .. } => self.start_heading(level),
            Tag::BlockQuote => self.start_blockquote(),
            Tag::CodeBlock(kind) => {
                let indent = match kind {
                    CodeBlockKind::Fenced(_) => None,
                    CodeBlockKind::Indented => Some(Span::from(" ".repeat(4))),
                };
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    CodeBlockKind::Indented => None,
                };
                self.start_codeblock(lang, indent)
            }
            Tag::List(start) => self.start_list(start),
            Tag::Item => self.start_item(),
            Tag::Emphasis => self.push_inline_style(self.styles.emphasis),
            Tag::Strong => self.push_inline_style(self.styles.strong),
            Tag::Strikethrough => self.push_inline_style(self.styles.strikethrough),
            Tag::Link { dest_url, .. } => self.push_link(dest_url.to_string()),
            Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::Image { .. }
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::BlockQuote => self.end_blockquote(),
            TagEnd::CodeBlock => self.end_codeblock(),
            TagEnd::List(_) => self.end_list(),
            TagEnd::Item => {
                self.indent_stack.pop();
                self.pending_marker_line = false;
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.pop_inline_style(),
            TagEnd::Link => self.pop_link(),
            TagEnd::HtmlBlock
            | TagEnd::FootnoteDefinition
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::Image
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn start_paragraph(&mut self) {
        if self.needs_newline {
            self.push_blank_line();
        }
        self.push_line(Line::default());
        self.needs_newline = false;
        self.in_paragraph = true;
    }

    fn end_paragraph(&mut self) {
        self.needs_newline = true;
        self.in_paragraph = false;
        self.pending_marker_line = false;
    }

    fn start_heading(&mut self, level: HeadingLevel) {
        if self.needs_newline {
            self.push_line(Line::default());
            self.needs_newline = false;
        }
        let heading_style = match level {
            HeadingLevel::H1 => self.styles.h1,
            HeadingLevel::H2 => self.styles.h2,
            HeadingLevel::H3 => self.styles.h3,
            HeadingLevel::H4 => self.styles.h4,
            HeadingLevel::H5 => self.styles.h5,
            HeadingLevel::H6 => self.styles.h6,
        };
        let content = format!("{} ", "#".repeat(level as usize));
        self.push_line(Line::from(vec![Span::styled(content, heading_style)]));
        self.push_inline_style(heading_style);
        self.needs_newline = false;
    }

    fn end_heading(&mut self) {
        self.needs_newline = true;
        self.pop_inline_style();
    }

    fn start_blockquote(&mut self) {
        if self.needs_newline {
            self.push_blank_line();
            self.needs_newline = false;
        }
        self.indent_stack
            .push(IndentContext::new(vec![Span::from("> ")], None, false));
    }

    fn end_blockquote(&mut self) {
        self.indent_stack.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        if self.pending_marker_line {
            self.push_line(Line::default());
        }
        self.pending_marker_line = false;
        if self.in_code_block && !self.needs_newline {
            let has_content = self
                .current_line_content
                .as_ref()
                .map(|line| !line.spans.is_empty())
                .unwrap_or_else(|| {
                    self.text
                        .lines
                        .last()
                        .map(|line| !line.spans.is_empty())
                        .unwrap_or(false)
                });
            if has_content {
                self.push_line(Line::default());
            }
        }
        for (i, line) in text.lines().enumerate() {
            if self.needs_newline {
                self.push_line(Line::default());
                self.needs_newline = false;
            }
            if i > 0 {
                self.push_line(Line::default());
            }
            let content = line.to_string();
            let span = Span::styled(
                content,
                self.inline_styles.last().copied().unwrap_or_default(),
            );
            self.push_span(span);
        }
        self.needs_newline = false;
    }

    fn code(&mut self, code: CowStr<'a>) {
        if self.pending_marker_line {
            self.push_line(Line::default());
            self.pending_marker_line = false;
        }
        let span = Span::from(code.into_string()).style(self.styles.code);
        self.push_span(span);
    }

    fn html(&mut self, html: CowStr<'a>, inline: bool) {
        self.pending_marker_line = false;
        for (i, line) in html.lines().enumerate() {
            if self.needs_newline {
                self.push_line(Line::default());
                self.needs_newline = false;
            }
            if i > 0 {
                self.push_line(Line::default());
            }
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_span(Span::styled(line.to_string(), style));
        }
        self.needs_newline = !inline;
    }

    fn hard_break(&mut self) {
        self.push_line(Line::default());
    }

    fn soft_break(&mut self) {
        self.push_line(Line::default());
    }

    fn start_list(&mut self, index: Option<u64>) {
        if self.list_indices.is_empty() && self.needs_newline {
            self.push_line(Line::default());
        }
        self.list_indices.push(index);
    }

    fn end_list(&mut self) {
        self.list_indices.pop();
        self.needs_newline = true;
    }

    fn start_item(&mut self) {
        self.pending_marker_line = true;
        let depth = self.list_indices.len();
        let is_ordered = self
            .list_indices
            .last()
            .map(Option::is_some)
            .unwrap_or(false);
        let width = depth * 4 - 3;
        let marker = if let Some(last_index) = self.list_indices.last_mut() {
            match last_index {
                None => Some(vec![Span::styled(
                    " ".repeat(width - 1) + "- ",
                    self.styles.unordered_list_marker,
                )]),
                Some(index) => {
                    *index += 1;
                    Some(vec![Span::styled(
                        format!("{:width$}. ", *index - 1),
                        self.styles.ordered_list_marker,
                    )])
                }
            }
        } else {
            None
        };
        let indent_prefix = if depth == 0 {
            Vec::new()
        } else {
            let indent_len = if is_ordered { width + 2 } else { width + 1 };
            vec![Span::from(" ".repeat(indent_len))]
        };
        self.indent_stack
            .push(IndentContext::new(indent_prefix, marker, true));
        self.needs_newline = false;
    }

    fn start_codeblock(&mut self, _lang: Option<String>, indent: Option<Span<'static>>) {
        self.flush_current_line();
        if !self.text.lines.is_empty() {
            self.push_blank_line();
        }
        self.in_code_block = true;
        self.indent_stack.push(IndentContext::new(
            vec![indent.unwrap_or_default()],
            None,
            false,
        ));
        self.needs_newline = true;
    }

    fn end_codeblock(&mut self) {
        self.needs_newline = true;
        self.in_code_block = false;
        self.indent_stack.pop();
    }

    fn push_inline_style(&mut self, style: Style) {
        let current = self.inline_styles.last().copied().unwrap_or_default();
        let merged = current.patch(style);
        self.inline_styles.push(merged);
    }

    fn pop_inline_style(&mut self) {
        self.inline_styles.pop();
    }

    fn push_link(&mut self, dest_url: String) {
        self.link = Some(dest_url);
    }

    fn pop_link(&mut self) {
        if let Some(link) = self.link.take() {
            self.push_span(" (".into());
            self.push_span(Span::styled(link, self.styles.link));
            self.push_span(")".into());
        }
    }

    fn flush_current_line(&mut self) {
        if let Some(line) = self.current_line_content.take() {
            let style = self.current_line_style;
            // NB we don't wrap code in code blocks, in order to preserve whitespace for copy/paste.
            if !self.current_line_in_code_block
                && let Some(width) = self.wrap_width
            {
                let opts = RtOptions::new(width)
                    .initial_indent(self.current_initial_indent.clone().into())
                    .subsequent_indent(self.current_subsequent_indent.clone().into());
                for wrapped in word_wrap_line(&line, opts) {
                    let owned = line_to_static(&wrapped).style(style);
                    self.text.lines.push(owned);
                }
            } else {
                let mut spans = self.current_initial_indent.clone();
                let mut line = line;
                spans.append(&mut line.spans);
                self.text.lines.push(Line::from_iter(spans).style(style));
            }
            self.current_initial_indent.clear();
            self.current_subsequent_indent.clear();
            self.current_line_in_code_block = false;
        }
    }

    fn push_line(&mut self, line: Line<'static>) {
        self.flush_current_line();
        let blockquote_active = self
            .indent_stack
            .iter()
            .any(|ctx| ctx.prefix.iter().any(|s| s.content.contains('>')));
        let style = if blockquote_active {
            self.styles.blockquote
        } else {
            line.style
        };
        let was_pending = self.pending_marker_line;

        self.current_initial_indent = self.prefix_spans(was_pending);
        self.current_subsequent_indent = self.prefix_spans(false);
        self.current_line_style = style;
        self.current_line_content = Some(line);
        self.current_line_in_code_block = self.in_code_block;

        self.pending_marker_line = false;
    }

    fn push_span(&mut self, span: Span<'static>) {
        if let Some(line) = self.current_line_content.as_mut() {
            line.push_span(span);
        } else {
            self.push_line(Line::from(vec![span]));
        }
    }

    fn push_blank_line(&mut self) {
        self.flush_current_line();
        if self.indent_stack.iter().all(|ctx| ctx.is_list) {
            self.text.lines.push(Line::default());
        } else {
            self.push_line(Line::default());
            self.flush_current_line();
        }
    }

    fn prefix_spans(&self, pending_marker_line: bool) -> Vec<Span<'static>> {
        let mut prefix: Vec<Span<'static>> = Vec::new();
        let last_marker_index = if pending_marker_line {
            self.indent_stack
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, ctx)| if ctx.marker.is_some() { Some(i) } else { None })
        } else {
            None
        };
        let last_list_index = self.indent_stack.iter().rposition(|ctx| ctx.is_list);

        for (i, ctx) in self.indent_stack.iter().enumerate() {
            if pending_marker_line {
                if Some(i) == last_marker_index
                    && let Some(marker) = &ctx.marker
                {
                    prefix.extend(marker.iter().cloned());
                    continue;
                }
                if ctx.is_list && last_marker_index.is_some_and(|idx| idx > i) {
                    continue;
                }
            } else if ctx.is_list && Some(i) != last_list_index {
                continue;
            }
            prefix.extend(ctx.prefix.iter().cloned());
        }

        prefix
    }
}

#[cfg(test)]
mod markdown_render_tests {
    include!("markdown_render_tests.rs");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use ratatui::style::Modifier;
    use ratatui::text::Text;

    fn lines_to_strings(text: &Text<'_>) -> Vec<String> {
        text.lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn wraps_plain_text_when_width_provided() {
        let markdown = "This is a simple sentence that should wrap.";
        let rendered = render_markdown_text_with_width(markdown, Some(16));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "This is a simple".to_string(),
                "sentence that".to_string(),
                "should wrap.".to_string(),
            ]
        );
    }

    #[test]
    fn wraps_list_items_preserving_indent() {
        let markdown = "- first second third fourth";
        let rendered = render_markdown_text_with_width(markdown, Some(14));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec!["- first second".to_string(), "  third fourth".to_string(),]
        );
    }

    #[test]
    fn wraps_nested_lists() {
        let markdown =
            "- outer item with several words to wrap\n  - inner item that also needs wrapping";
        let rendered = render_markdown_text_with_width(markdown, Some(20));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "- outer item with".to_string(),
                "  several words to".to_string(),
                "  wrap".to_string(),
                "    - inner item".to_string(),
                "      that also".to_string(),
                "      needs wrapping".to_string(),
            ]
        );
    }

    #[test]
    fn wraps_ordered_lists() {
        let markdown = "1. ordered item contains many words for wrapping";
        let rendered = render_markdown_text_with_width(markdown, Some(18));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "1. ordered item".to_string(),
                "   contains many".to_string(),
                "   words for".to_string(),
                "   wrapping".to_string(),
            ]
        );
    }

    #[test]
    fn wraps_blockquotes() {
        let markdown = "> block quote with content that should wrap nicely";
        let rendered = render_markdown_text_with_width(markdown, Some(22));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "> block quote with".to_string(),
                "> content that should".to_string(),
                "> wrap nicely".to_string(),
            ]
        );
    }

    #[test]
    fn wraps_blockquotes_inside_lists() {
        let markdown = "- list item\n  > block quote inside list that wraps";
        let rendered = render_markdown_text_with_width(markdown, Some(24));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "- list item".to_string(),
                "  > block quote inside".to_string(),
                "  > list that wraps".to_string(),
            ]
        );
    }

    #[test]
    fn wraps_list_items_containing_blockquotes() {
        let markdown = "1. item with quote\n   > quoted text that should wrap";
        let rendered = render_markdown_text_with_width(markdown, Some(24));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "1. item with quote".to_string(),
                "   > quoted text that".to_string(),
                "   > should wrap".to_string(),
            ]
        );
    }

    #[test]
    fn does_not_wrap_code_blocks() {
        let markdown = "````\nfn main() { println!(\"hi from a long line\"); }\n````";
        let rendered = render_markdown_text_with_width(markdown, Some(10));
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec!["fn main() { println!(\"hi from a long line\"); }".to_string(),]
        );
    }

    fn render_termimad(input: &str) -> Text<'static> {
        render_markdown_text_with_width_termimad(input, None)
    }

    fn is_default_color(color: Option<Color>) -> bool {
        matches!(color, None | Some(Color::Reset))
    }

    fn is_cyan(color: Option<Color>) -> bool {
        matches!(color, Some(Color::Cyan) | Some(Color::Indexed(14)))
    }

    fn is_green(color: Option<Color>) -> bool {
        matches!(color, Some(Color::Green) | Some(Color::Indexed(10)))
    }

    #[test]
    fn termimad_plain_text_uses_default_color() {
        let rendered = render_termimad("Just plain text.");
        let has_non_default = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|s| !is_default_color(s.style.fg) || !is_default_color(s.style.bg));
        assert_eq!(has_non_default, false);
    }

    #[test]
    fn termimad_inline_code_is_cyan() {
        let rendered = render_termimad("Use `code` here.");
        let code_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content == "code")
            .expect("expected inline code span");
        assert_eq!(is_cyan(code_span.style.fg), true);
    }

    #[test]
    fn termimad_inline_code_does_not_color_whole_line() {
        let rendered = render_termimad("Use `code` here.");
        let mut saw_plain_prefix = false;
        let mut saw_plain_suffix = false;
        for line in &rendered.lines {
            for span in &line.spans {
                if span.content == "Use " {
                    saw_plain_prefix = true;
                    assert_eq!(is_default_color(span.style.fg), true);
                }
                if span.content == " here." {
                    saw_plain_suffix = true;
                    assert_eq!(is_default_color(span.style.fg), true);
                }
            }
        }
        assert_eq!(saw_plain_prefix, true);
        assert_eq!(saw_plain_suffix, true);
    }

    #[test]
    fn termimad_headers_are_bold() {
        let rendered = render_termimad("# Heading");
        let has_bold = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(has_bold, true);
    }

    #[test]
    fn termimad_quote_mark_is_green() {
        let rendered = render_termimad("> quoted");
        let quote_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.contains('>'))
            .expect("expected quote marker span");
        assert_eq!(is_green(quote_span.style.fg), true);
    }

    #[test]
    fn termimad_unwraps_markdown_fence() {
        let rendered = render_termimad("```markdown\n**bold**\n```\n");
        let bold_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content == "bold")
            .expect("expected bold span");
        assert_eq!(bold_span.style.add_modifier.contains(Modifier::BOLD), true);
        assert_eq!(is_default_color(bold_span.style.fg), true);
    }

    #[test]
    fn termimad_unwraps_markdown_fence_with_filename() {
        let rendered = render_termimad("```markdown README.md\n**bold**\n```\n");
        let bold_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content == "bold")
            .expect("expected bold span");
        assert_eq!(bold_span.style.add_modifier.contains(Modifier::BOLD), true);
        assert_eq!(is_default_color(bold_span.style.fg), true);
    }

    #[test]
    fn termimad_unwraps_markdown_fence_with_md_filename_only() {
        let rendered = render_termimad("```README.md\n**bold**\n```\n");
        let bold_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content == "bold")
            .expect("expected bold span");
        assert_eq!(bold_span.style.add_modifier.contains(Modifier::BOLD), true);
        assert_eq!(is_default_color(bold_span.style.fg), true);
    }

    #[test]
    fn termimad_style_does_not_bleed_across_lines() {
        let rendered = render_termimad("`code`\nplain\n");
        let plain_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content == "plain")
            .expect("expected plain span");
        assert_eq!(is_default_color(plain_span.style.fg), true);
    }

    #[test]
    fn termimad_keeps_non_markdown_fence_as_code() {
        let rendered = render_termimad("```text\nbold\n```\n");
        let code_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.trim() == "bold")
            .expect("expected code span");
        assert_eq!(is_cyan(code_span.style.fg), true);
        assert_eq!(code_span.style.add_modifier.contains(Modifier::BOLD), false);
    }

    #[test]
    fn unwrap_markdown_fences_preserves_nested_fences() {
        let src = "```text\n```markdown\n**bold**\n```\n```\n";
        let unwrapped = unwrap_markdown_fences(src);
        assert_eq!(unwrapped.contains("```text"), true);
        assert_eq!(unwrapped.contains("```markdown"), true);
        assert_eq!(unwrapped.contains("**bold**"), true);
    }

    #[test]
    fn termimad_readme_like_snippet_colors_code_only() {
        let markdown = "## Installing Codex\n\nToday, the easiest way to install Codex is via `npm`:\n\n```shell\nnpm i -g @openai/codex\ncodex\n```\n\nYou can also install via Homebrew (`brew install --cask codex`) or download a platform-specific release.\n";
        let rendered = render_termimad(markdown);

        let header_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.contains("Installing Codex"))
            .expect("expected header span");
        assert_eq!(is_default_color(header_span.style.fg), true);

        let npm_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content == "npm")
            .expect("expected inline code span for npm");
        assert_eq!(is_cyan(npm_span.style.fg), true);

        let code_line = rendered
            .lines
            .iter()
            .find(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.clone())
                    .collect::<String>()
                    .contains("npm i -g @openai/codex")
            })
            .expect("expected code block line");
        let code_line_is_cyan = code_line.spans.iter().all(|span| is_cyan(span.style.fg));
        assert_eq!(code_line_is_cyan, true);

        let after_code_span = rendered
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.contains("You can also install via Homebrew"))
            .expect("expected paragraph span after code block");
        assert_eq!(is_default_color(after_code_span.style.fg), true);
    }
}
