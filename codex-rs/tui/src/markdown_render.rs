use crate::citation_regex::CITATION_REGEX;
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
    suppress_on_pending_marker: bool,
}

impl IndentContext {
    fn new(prefix: Vec<Span<'static>>, suppress_on_pending_marker: bool) -> Self {
        Self {
            prefix,
            suppress_on_pending_marker,
        }
    }
}

#[allow(dead_code)]
pub(crate) fn render_markdown_text(input: &str) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut w = Writer::new(parser, None, None);
    w.run();
    w.text
}

pub(crate) fn render_markdown_text_with_citations(
    input: &str,
    scheme: Option<&str>,
    cwd: &Path,
) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut w = Writer::new(
        parser,
        scheme.map(|s| s.to_string()),
        Some(cwd.to_path_buf()),
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
    suppress_list_indent: bool,
    list_indices: Vec<Option<u64>>,
    link: Option<String>,
    needs_newline: bool,
    pending_marker_line: bool,
    in_paragraph: bool,
    scheme: Option<String>,
    cwd: Option<std::path::PathBuf>,
    in_code_block: bool,
}

impl<'a, I> Writer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn new(iter: I, scheme: Option<String>, cwd: Option<std::path::PathBuf>) -> Self {
        Self {
            iter,
            text: Text::default(),
            inline_styles: Vec::new(),
            indent_stack: Vec::new(),
            suppress_list_indent: false,
            list_indices: Vec::new(),
            link: None,
            needs_newline: false,
            pending_marker_line: false,
            in_paragraph: false,
            scheme,
            cwd,
            in_code_block: false,
        }
    }

    fn run(&mut self) {
        while let Some(ev) = self.iter.next() {
            self.handle_event(ev);
        }
    }

    fn handle_event(&mut self, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.code(code),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.hard_break(),
            Event::Html(html) => self.html(html),
            Event::InlineHtml(html) => self.html(html),
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
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    CodeBlockKind::Indented => None,
                };
                self.start_codeblock(lang)
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
            TagEnd::Item => {}
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
        let pending_marker_line = self.pending_marker_line;
        if self.needs_newline {
            self.push_blank_line();
            self.pending_marker_line = false;
        }
        if !pending_marker_line {
            self.push_line(Line::default());
        }
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
        self.indent_stack.push(IndentContext::new(
            vec![Span::from(">")],
            self.pending_marker_line,
        ));

        if self.pending_marker_line
            && let Some(last) = self.text.lines.last_mut()
        {
            for _ in 0..self.indent_stack.len() {
                last.push_span(Span::from(">"));
            }
            last.push_span(Span::from(" "));
        }
    }

    fn end_blockquote(&mut self) {
        self.indent_stack.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        self.pending_marker_line = false;
        if self.in_code_block
            && !self.needs_newline
            && self
                .text
                .lines
                .last()
                .map(|line| !line.spans.is_empty())
                .unwrap_or(false)
        {
            self.push_line(Line::default());
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
        let span = Span::from(code.into_string()).dim();
        self.push_span(span);
    }

    fn html(&mut self, html: CowStr<'a>) {
        self.pending_marker_line = false;
        let previous_suppress = self.suppress_list_indent;
        self.suppress_list_indent = true;
        for (i, line) in html.lines().enumerate() {
            if self.needs_newline {
                self.push_line(Line::default());
                self.needs_newline = false;
            }
            if i > 0 {
                self.push_line(Line::default());
            }
            self.strip_list_indent_for_html_line();
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_span(Span::styled(line.to_string(), style));
        }
        self.suppress_list_indent = previous_suppress;
        self.needs_newline = false;
    }

    fn strip_list_indent_for_html_line(&mut self) {
        if self.list_indices.is_empty() {
            return;
        }
        let Some(last) = self.text.lines.last_mut() else {
            return;
        };
        let has_content = last
            .spans
            .iter()
            .any(|span| span.content.chars().any(|ch| ch != ' ' && ch != '>'));
        if has_content {
            return;
        }
        while let Some(first) = last.spans.first() {
            if first.content.chars().all(|ch| ch == ' ') {
                last.spans.remove(0);
            } else {
                break;
            }
        }
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
        self.push_line(Line::default());
        let width = self.list_indices.len() * 4 - 3;
        if let Some(last_index) = self.list_indices.last_mut() {
            let span: Span = match last_index {
                None => Span::from(" ".repeat(width - 1) + "- "),
                Some(index) => {
                    *index += 1;
                    format!("{:width$}. ", *index - 1).light_blue()
                }
            };
            self.push_span(span);
        }
        self.needs_newline = false;
    }

    fn start_codeblock(&mut self, lang: Option<String>) {
        if !self.text.lines.is_empty() {
            self.push_blank_line();
        }
        self.in_code_block = true;
        let opener = match lang {
            Some(l) if !l.is_empty() => format!("```{l}"),
            _ => "```".to_string(),
        };
        self.push_line(opener.into());
        self.needs_newline = true;
    }

    fn end_codeblock(&mut self) {
        self.push_line("```".into());
        self.needs_newline = true;
        self.in_code_block = false;
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

    fn push_line(&mut self, line: Line<'static>) {
        let style = if self
            .indent_stack
            .iter()
            .any(|ctx| ctx.prefix.iter().any(|s| s.content == ">"))
        {
            Style::new().green()
        } else {
            Style::default()
        };
        self.text.lines.push(
            Line::from_iter([self.current_prefix_spans(), line.spans].concat()).set_style(style),
        );
    }

    fn push_span(&mut self, span: Span<'static>) {
        if let Some(last) = self.text.lines.last_mut() {
            last.push_span(span);
        } else {
            self.push_line(Line::from(vec![span]));
        }
    }

    fn push_blank_line(&mut self) {
        if self.indent_stack.is_empty() {
            self.text.lines.push(Line::default());
        } else {
            self.push_line(Line::default());
        }
    }

    fn current_prefix_spans(&self) -> Vec<Span<'static>> {
        let mut prefix: Vec<Span<'static>> = Vec::new();

        if !self.list_indices.is_empty() && !self.pending_marker_line && !self.suppress_list_indent
        {
            let depth = self.list_indices.len();
            let is_ordered = self.list_indices.last().and_then(|o| *o).is_some();
            prefix.extend(Self::list_indent_prefix(depth, is_ordered));
        }

        let mut blockquote_count = 0;
        for ctx in &self.indent_stack {
            if self.pending_marker_line && ctx.suppress_on_pending_marker {
                continue;
            }
            prefix.extend(ctx.prefix.iter().cloned());
            blockquote_count += 1;
        }
        if blockquote_count > 0 {
            prefix.push(Span::from(" "));
        }

        prefix
    }

    fn list_indent_prefix(depth: usize, is_ordered: bool) -> Vec<Span<'static>> {
        if depth == 0 {
            return Vec::new();
        }
        let width = depth * 4 - 3;
        let indent_len = if is_ordered { width + 2 } else { width + 1 };
        vec![Span::from(" ".repeat(indent_len))]
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
        let rendered = render_markdown_text_with_citations(markdown, None, cwd);
        // Convert lines back to string for comparison.
        let rendered: String = rendered
            .lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.clone())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(markdown, rendered);
        // Ensure helper rewrites.
        assert_ne!(markdown, unchanged);
    }
}
