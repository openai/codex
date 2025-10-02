use crate::citation_regex::CITATION_REGEX;
use crate::render::line_utils::line_to_static;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::HeadingLevel;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use std::borrow::Cow;
use std::path::Path;

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

pub fn render_markdown_text(input: &str) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut w = Writer::new(parser, None, None, None);
    w.run();
    w.text
}

pub(crate) fn render_markdown_text_with_citations(
    input: &str,
    width: Option<usize>,
    scheme: Option<&str>,
    cwd: &Path,
) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut w = Writer::new(
        parser,
        scheme.map(str::to_string),
        Some(cwd.to_path_buf()),
        width,
    );
    w.run();
    w.text
}

struct Writer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    iter: I,
    text: Text<'static>,
    inline_styles: Vec<Style>,
    indent_stack: Vec<IndentContext>,
    list_indices: Vec<Option<u64>>,
    link: Option<String>,
    needs_newline: bool,
    pending_marker_line: bool,
    in_paragraph: bool,
    scheme: Option<String>,
    cwd: Option<std::path::PathBuf>,
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
    fn new(
        iter: I,
        scheme: Option<String>,
        cwd: Option<std::path::PathBuf>,
        wrap_width: Option<usize>,
    ) -> Self {
        Self {
            iter,
            text: Text::default(),
            inline_styles: Vec::new(),
            indent_stack: Vec::new(),
            list_indices: Vec::new(),
            link: None,
            needs_newline: false,
            pending_marker_line: false,
            in_paragraph: false,
            scheme,
            cwd,
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
            Tag::Emphasis => self.push_inline_style(Style::new().italic()),
            Tag::Strong => self.push_inline_style(Style::new().bold()),
            Tag::Strikethrough => self.push_inline_style(Style::new().crossed_out()),
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
            HeadingLevel::H1 => Style::new().bold().underlined(),
            HeadingLevel::H2 => Style::new().bold(),
            HeadingLevel::H3 => Style::new().bold().italic(),
            HeadingLevel::H4 => Style::new().italic(),
            HeadingLevel::H5 => Style::new().italic(),
            HeadingLevel::H6 => Style::new().italic(),
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
            let mut content = line.to_string();
            if !self.in_code_block
                && let (Some(scheme), Some(cwd)) = (&self.scheme, &self.cwd)
            {
                let cow = rewrite_file_citations_with_scheme(&content, Some(scheme.as_str()), cwd);
                if let std::borrow::Cow::Owned(s) = cow {
                    content = s;
                }
            }
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
        let span = Span::from(code.into_string()).dim();
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
                None => Some(vec![Span::from(" ".repeat(width - 1) + "- ")]),
                Some(index) => {
                    *index += 1;
                    Some(vec![format!("{:width$}. ", *index - 1).light_blue()])
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
            self.push_span(link.cyan().underlined());
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
            Style::new().green()
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

pub(crate) fn rewrite_file_citations_with_scheme<'a>(
    src: &'a str,
    scheme_opt: Option<&str>,
    cwd: &Path,
) -> Cow<'a, str> {
    let scheme: &str = match scheme_opt {
        Some(s) => s,
        None => return Cow::Borrowed(src),
    };

    CITATION_REGEX.replace_all(src, |caps: &regex_lite::Captures<'_>| {
        let file = &caps[1];
        let start_line = &caps[2];

        // Resolve the path against `cwd` when it is relative.
        let absolute_path = {
            let p = Path::new(file);
            let absolute_path = if p.is_absolute() {
                path_clean::clean(p)
            } else {
                path_clean::clean(cwd.join(p))
            };
            // VS Code expects forward slashes even on Windows because URIs use
            // `/` as the path separator.
            absolute_path.to_string_lossy().replace('\\', "/")
        };

        // Render as a normal markdown link so the downstream renderer emits
        // the hyperlink escape sequence (when supported by the terminal).
        //
        // In practice, sometimes multiple citations for the same file, but with a
        // different line number, are shown sequentially, so we:
        // - include the line number in the label to disambiguate them
        // - add a space after the link to make it easier to read
        format!("[{file}:{start_line}]({scheme}://file{absolute_path}:{start_line}) ")
    })
}

#[cfg(test)]
mod markdown_render_tests {
    include!("markdown_render_tests.rs");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
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
    fn citation_is_rewritten_with_absolute_path() {
        let markdown = "See 【F:/src/main.rs†L42-L50】 for details.";
        let cwd = Path::new("/workspace");
        let result = rewrite_file_citations_with_scheme(markdown, Some("vscode"), cwd);

        assert_eq!(
            "See [/src/main.rs:42](vscode://file/src/main.rs:42)  for details.",
            result
        );
    }

    #[test]
    fn citation_followed_by_space_so_they_do_not_run_together() {
        let markdown = "References on lines 【F:src/foo.rs†L24】【F:src/foo.rs†L42】";
        let cwd = Path::new("/home/user/project");
        let result = rewrite_file_citations_with_scheme(markdown, Some("vscode"), cwd);

        assert_eq!(
            "References on lines [src/foo.rs:24](vscode://file/home/user/project/src/foo.rs:24) [src/foo.rs:42](vscode://file/home/user/project/src/foo.rs:42) ",
            result
        );
    }

    #[test]
    fn citation_unchanged_without_file_opener() {
        let markdown = "Look at 【F:file.rs†L1】.";
        let cwd = Path::new("/");
        let unchanged = rewrite_file_citations_with_scheme(markdown, Some("vscode"), cwd);
        // The helper itself always rewrites – this test validates behaviour of
        // append_markdown when `file_opener` is None.
        let rendered = render_markdown_text_with_citations(markdown, None, None, cwd);
        // Convert lines back to string for comparison.
        let rendered: String = lines_to_strings(&rendered).join("");
        assert_eq!(markdown, rendered);
        // Ensure helper rewrites.
        assert_ne!(markdown, unchanged);
    }

    #[test]
    fn wraps_plain_text_when_width_provided() {
        let markdown = "This is a simple sentence that should wrap.";
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(16), None, cwd);
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
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(14), None, cwd);
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
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(20), None, cwd);
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec![
                "- outer item with".to_string(),
                "  several words".to_string(),
                "  to wrap".to_string(),
                "    - inner item".to_string(),
                "      that also".to_string(),
                "      needs wrapping".to_string(),
            ]
        );
    }

    #[test]
    fn wraps_ordered_lists() {
        let markdown = "1. ordered item contains many words for wrapping";
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(18), None, cwd);
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
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(22), None, cwd);
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
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(24), None, cwd);
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
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(24), None, cwd);
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
        let cwd = Path::new("/");
        let rendered = render_markdown_text_with_citations(markdown, Some(10), None, cwd);
        let lines = lines_to_strings(&rendered);
        assert_eq!(
            lines,
            vec!["fn main() { println!(\"hi from a long line\"); }".to_string(),]
        );
    }
}
