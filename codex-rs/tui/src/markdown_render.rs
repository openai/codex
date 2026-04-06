//! Markdown rendering for the TUI transcript.
//!
//! This renderer intentionally treats local file links differently from normal web links. For
//! local paths, the displayed text comes from the destination, not the markdown label, so
//! transcripts show the real file target (including normalized location suffixes) and can shorten
//! absolute paths relative to a known working directory.

use crate::render::highlight::highlight_code_to_lines;
use crate::render::line_utils::line_to_static;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use codex_utils_string::normalize_markdown_hash_location_suffix;
use dirs::home_dir;
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
use regex_lite::Regex;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use url::Url;

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

pub fn render_markdown_text(input: &str) -> Text<'static> {
    render_markdown_text_with_width(input, /*width*/ None)
}

/// Render markdown using the current process working directory for local file-link display.
pub(crate) fn render_markdown_text_with_width(input: &str, width: Option<usize>) -> Text<'static> {
    let cwd = std::env::current_dir().ok();
    render_markdown_text_with_width_and_cwd(input, width, cwd.as_deref())
}

/// Render markdown with an explicit working directory for local file links.
///
/// The `cwd` parameter controls how absolute local targets are shortened before display. Passing
/// the session cwd keeps full renders, history cells, and streamed deltas visually aligned even
/// when rendering happens away from the process cwd.
pub(crate) fn render_markdown_text_with_width_and_cwd(
    input: &str,
    width: Option<usize>,
    cwd: Option<&Path>,
) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut w = Writer::new(parser, width, cwd);
    w.run();
    w.text
}

#[derive(Clone, Debug)]
struct LinkState {
    destination: String,
    show_destination: bool,
    /// Pre-rendered display text for local file links.
    ///
    /// When this is present, the markdown label is intentionally suppressed so the rendered
    /// transcript always reflects the real target path.
    local_target_display: Option<String>,
}

fn should_render_link_destination(dest_url: &str) -> bool {
    !is_local_path_like_link(dest_url)
}

static COLON_LOCATION_SUFFIX_RE: LazyLock<Regex> =
    LazyLock::new(
        || match Regex::new(r":\d+(?::\d+)?(?:[-–]\d+(?::\d+)?)?$") {
            Ok(regex) => regex,
            Err(error) => panic!("invalid location suffix regex: {error}"),
        },
    );

// Covered by load_location_suffix_regexes.
static HASH_LOCATION_SUFFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| match Regex::new(r"^L\d+(?:C\d+)?(?:-L\d+(?:C\d+)?)?$") {
        Ok(regex) => regex,
        Err(error) => panic!("invalid hash location regex: {error}"),
    });

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
    link: Option<LinkState>,
    needs_newline: bool,
    pending_marker_line: bool,
    in_paragraph: bool,
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_buffer: String,
    wrap_width: Option<usize>,
    cwd: Option<PathBuf>,
    line_ends_with_local_link_target: bool,
    pending_local_link_soft_break: bool,
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
    fn new(iter: I, wrap_width: Option<usize>, cwd: Option<&Path>) -> Self {
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
            code_block_lang: None,
            code_block_buffer: String::new(),
            wrap_width,
            cwd: cwd.map(Path::to_path_buf),
            line_ends_with_local_link_target: false,
            pending_local_link_soft_break: false,
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
        self.prepare_for_event(&event);
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
            Event::Html(html) => self.html(html, /*inline*/ false),
            Event::InlineHtml(html) => self.html(html, /*inline*/ true),
            Event::FootnoteReference(_) => {}
            Event::TaskListMarker(_) => {}
        }
    }

    fn prepare_for_event(&mut self, event: &Event<'a>) {
        if !self.pending_local_link_soft_break {
            return;
        }

        // Local file links render from the destination at `TagEnd::Link`, so a Markdown soft break
        // immediately before a descriptive `: ...` should stay inline instead of splitting the
        // list item across two lines.
        if matches!(event, Event::Text(text) if text.trim_start().starts_with(':')) {
            self.pending_local_link_soft_break = false;
            return;
        }

        self.pending_local_link_soft_break = false;
        self.push_line(Line::default());
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
        self.indent_stack.push(IndentContext::new(
            vec![Span::from("> ")],
            /*marker*/ None,
            /*is_list*/ false,
        ));
    }

    fn end_blockquote(&mut self) {
        self.indent_stack.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        if self.suppressing_local_link_label() {
            return;
        }
        self.line_ends_with_local_link_target = false;
        if self.pending_marker_line {
            self.push_line(Line::default());
        }
        self.pending_marker_line = false;

        // When inside a fenced code block with a known language, accumulate
        // text into the buffer for batch highlighting in end_codeblock().
        // Append verbatim — pulldown-cmark text events already contain the
        // original line breaks, so inserting separators would double them.
        if self.in_code_block && self.code_block_lang.is_some() {
            self.code_block_buffer.push_str(&text);
            return;
        }

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
        if self.suppressing_local_link_label() {
            return;
        }
        self.line_ends_with_local_link_target = false;
        if self.pending_marker_line {
            self.push_line(Line::default());
            self.pending_marker_line = false;
        }
        let span = Span::from(code.into_string()).style(self.styles.code);
        self.push_span(span);
    }

    fn html(&mut self, html: CowStr<'a>, inline: bool) {
        if self.suppressing_local_link_label() {
            return;
        }
        self.line_ends_with_local_link_target = false;
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
        if self.suppressing_local_link_label() {
            return;
        }
        self.line_ends_with_local_link_target = false;
        self.push_line(Line::default());
    }

    fn soft_break(&mut self) {
        if self.suppressing_local_link_label() {
            return;
        }
        if self.line_ends_with_local_link_target {
            self.pending_local_link_soft_break = true;
            self.line_ends_with_local_link_target = false;
            return;
        }
        self.line_ends_with_local_link_target = false;
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
        self.indent_stack.push(IndentContext::new(
            indent_prefix,
            marker,
            /*is_list*/ true,
        ));
        self.needs_newline = false;
    }

    fn start_codeblock(&mut self, lang: Option<String>, indent: Option<Span<'static>>) {
        self.flush_current_line();
        if !self.text.lines.is_empty() {
            self.push_blank_line();
        }
        self.in_code_block = true;

        // Extract the language token from the info string.  CommonMark info
        // strings can contain metadata after the language, separated by commas,
        // spaces, or other delimiters (e.g. "rust,no_run", "rust title=demo").
        // Take only the first token so the syntax lookup succeeds.
        let lang = lang
            .as_deref()
            .and_then(|s| s.split([',', ' ', '\t']).next())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        self.code_block_lang = lang;
        self.code_block_buffer.clear();

        self.indent_stack.push(IndentContext::new(
            vec![indent.unwrap_or_default()],
            /*marker*/ None,
            /*is_list*/ false,
        ));
        self.needs_newline = true;
    }

    fn end_codeblock(&mut self) {
        // If we buffered code for a known language, syntax-highlight it now.
        if let Some(lang) = self.code_block_lang.take() {
            let code = std::mem::take(&mut self.code_block_buffer);
            if !code.is_empty() {
                let highlighted = highlight_code_to_lines(&code, &lang);
                for hl_line in highlighted {
                    self.push_line(Line::default());
                    for span in hl_line.spans {
                        self.push_span(span);
                    }
                }
            }
        }

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
        let show_destination = should_render_link_destination(&dest_url);
        self.link = Some(LinkState {
            show_destination,
            local_target_display: if is_local_path_like_link(&dest_url) {
                render_local_link_target(&dest_url, self.cwd.as_deref())
            } else {
                None
            },
            destination: dest_url,
        });
    }

    fn pop_link(&mut self) {
        if let Some(link) = self.link.take() {
            if link.show_destination {
                self.push_span(" (".into());
                self.push_span(Span::styled(link.destination, self.styles.link));
                self.push_span(")".into());
            } else if let Some(local_target_display) = link.local_target_display {
                if self.pending_marker_line {
                    self.push_line(Line::default());
                }
                // Local file links are rendered as code-like path text so the transcript shows the
                // resolved target instead of arbitrary caller-provided label text.
                let style = self
                    .inline_styles
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .patch(self.styles.code);
                self.push_span(Span::styled(local_target_display, style));
                self.line_ends_with_local_link_target = true;
            }
        }
    }

    fn suppressing_local_link_label(&self) -> bool {
        self.link
            .as_ref()
            .and_then(|link| link.local_target_display.as_ref())
            .is_some()
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
                for wrapped in adaptive_wrap_line(&line, opts) {
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
            self.line_ends_with_local_link_target = false;
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
        self.current_subsequent_indent = self.prefix_spans(/*pending_marker_line*/ false);
        self.current_line_style = style;
        self.current_line_content = Some(line);
        self.current_line_in_code_block = self.in_code_block;
        self.line_ends_with_local_link_target = false;

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

fn is_local_path_like_link(dest_url: &str) -> bool {
    dest_url.starts_with("file://")
        || dest_url.starts_with('/')
        || dest_url.starts_with("~/")
        || dest_url.starts_with("./")
        || dest_url.starts_with("../")
        || dest_url.starts_with("\\\\")
        || matches!(
            dest_url.as_bytes(),
            [drive, b':', separator, ..]
                if drive.is_ascii_alphabetic() && matches!(separator, b'/' | b'\\')
        )
}

/// Parse a local link target into normalized path text plus an optional location suffix.
///
/// This accepts the path shapes Codex emits today: `file://` URLs, absolute and relative paths,
/// `~/...`, Windows paths, and `#L..C..` or `:line:col` suffixes.
fn render_local_link_target(dest_url: &str, cwd: Option<&Path>) -> Option<String> {
    let (path_text, location_suffix) = parse_local_link_target(dest_url)?;
    let mut rendered = display_local_link_path(&path_text, cwd);
    if let Some(location_suffix) = location_suffix {
        rendered.push_str(&location_suffix);
    }
    Some(rendered)
}

/// Split a local-link destination into `(normalized_path_text, location_suffix)`.
///
/// The returned path text never includes a trailing `#L..` or `:line[:col]` suffix. Path
/// normalization expands `~/...` when possible and rewrites path separators into display-stable
/// forward slashes. The suffix, when present, is returned separately in normalized markdown form.
///
/// Returns `None` only when the destination looks like a `file://` URL but cannot be parsed into a
/// local path. Plain path-like inputs always return `Some(...)` even if they are relative.
fn parse_local_link_target(dest_url: &str) -> Option<(String, Option<String>)> {
    if dest_url.starts_with("file://") {
        let url = Url::parse(dest_url).ok()?;
        let path_text = file_url_to_local_path_text(&url)?;
        let location_suffix = url
            .fragment()
            .and_then(normalize_hash_location_suffix_fragment);
        return Some((path_text, location_suffix));
    }

    let mut path_text = dest_url;
    let mut location_suffix = None;
    // Prefer `#L..` style fragments when both forms are present so URLs like `path#L10` do not
    // get misparsed as a plain path ending in `:10`.
    if let Some((candidate_path, fragment)) = dest_url.rsplit_once('#')
        && let Some(normalized) = normalize_hash_location_suffix_fragment(fragment)
    {
        path_text = candidate_path;
        location_suffix = Some(normalized);
    }
    if location_suffix.is_none()
        && let Some(suffix) = extract_colon_location_suffix(path_text)
    {
        let path_len = path_text.len().saturating_sub(suffix.len());
        path_text = &path_text[..path_len];
        location_suffix = Some(suffix);
    }

    let decoded_path_text = decode_percent_encoded_path_text(path_text);
    Some((expand_local_link_path(&decoded_path_text), location_suffix))
}

/// Normalize a hash fragment like `L12` or `L12C3-L14C9` into the display suffix we render.
///
/// Returns `None` for fragments that are not location references. This deliberately ignores other
/// `#...` fragments so non-location hashes stay part of the path text.
fn normalize_hash_location_suffix_fragment(fragment: &str) -> Option<String> {
    HASH_LOCATION_SUFFIX_RE
        .is_match(fragment)
        .then(|| format!("#{fragment}"))
        .and_then(|suffix| normalize_markdown_hash_location_suffix(&suffix))
}

/// Extract a trailing `:line`, `:line:col`, or range suffix from a plain path-like string.
///
/// The suffix must occur at the end of the input; embedded colons elsewhere in the path are left
/// alone. This is what keeps Windows drive letters like `C:/...` from being misread as locations.
fn extract_colon_location_suffix(path_text: &str) -> Option<String> {
    COLON_LOCATION_SUFFIX_RE
        .find(path_text)
        .filter(|matched| matched.end() == path_text.len())
        .map(|matched| matched.as_str().to_string())
}

/// Expand home-relative paths and normalize separators for display.
///
/// If `~/...` cannot be expanded because the home directory is unavailable, the original text still
/// goes through separator normalization and is returned as-is otherwise.
fn expand_local_link_path(path_text: &str) -> String {
    // Expand `~/...` eagerly so home-relative links can participate in the same normalization and
    // cwd-relative shortening path as absolute links.
    if let Some(rest) = path_text.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return normalize_local_link_path_text(&home.join(rest).to_string_lossy());
    }

    normalize_local_link_path_text(path_text)
}

/// Decode `%XX` path escapes for non-`file://` local links.
///
/// If any escape is malformed or decoded bytes are not UTF-8, the original text is returned
/// unchanged so rendering stays lossless.
fn decode_percent_encoded_path_text(path_text: &str) -> String {
    let bytes = path_text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    let mut saw_escape = false;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return path_text.to_string();
            }
            let hi = bytes[i + 1];
            let lo = bytes[i + 2];
            let Some(hi_val) = (hi as char).to_digit(16) else {
                return path_text.to_string();
            };
            let Some(lo_val) = (lo as char).to_digit(16) else {
                return path_text.to_string();
            };
            let decoded_byte = ((hi_val << 4) | lo_val) as u8;
            if decoded_byte.is_ascii() {
                return path_text.to_string();
            }
            decoded.push(decoded_byte);
            saw_escape = true;
            i += 3;
            continue;
        }

        decoded.push(bytes[i]);
        i += 1;
    }

    if !saw_escape {
        return path_text.to_string();
    }

    String::from_utf8(decoded).unwrap_or_else(|_| path_text.to_string())
}

/// Convert a `file://` URL into the normalized local-path text used for transcript rendering.
///
/// This prefers `Url::to_file_path()` for standard file URLs. When that rejects Windows-oriented
/// encodings, we reconstruct a display path from the host/path parts so UNC paths and drive-letter
/// URLs still render sensibly.
fn file_url_to_local_path_text(url: &Url) -> Option<String> {
    if let Ok(path) = url.to_file_path() {
        return Some(normalize_local_link_path_text(&path.to_string_lossy()));
    }

    // Fall back to string reconstruction for cases `to_file_path()` rejects, especially UNC-style
    // hosts and Windows drive paths encoded in URL form.
    let mut path_text = url.path().to_string();
    if let Some(host) = url.host_str()
        && !host.is_empty()
        && host != "localhost"
    {
        path_text = format!("//{host}{path_text}");
    } else if matches!(
        path_text.as_bytes(),
        [b'/', drive, b':', b'/', ..] if drive.is_ascii_alphabetic()
    ) {
        path_text.remove(0);
    }

    Some(normalize_local_link_path_text(&path_text))
}

/// Normalize local-path text into the transcript display form.
///
/// Display normalization is intentionally lexical: it does not touch the filesystem, resolve
/// symlinks, or collapse `.` / `..`. It only converts separators to forward slashes and rewrites
/// UNC-style `\\\\server\\share` inputs into `//server/share` so later prefix checks operate on a
/// stable representation.
fn normalize_local_link_path_text(path_text: &str) -> String {
    // Render all local link paths with forward slashes so display and prefix stripping are stable
    // across mixed Windows and Unix-style inputs.
    if let Some(rest) = path_text.strip_prefix("\\\\") {
        format!("//{}", rest.replace('\\', "/").trim_start_matches('/'))
    } else {
        path_text.replace('\\', "/")
    }
}

fn is_absolute_local_link_path(path_text: &str) -> bool {
    path_text.starts_with('/')
        || path_text.starts_with("//")
        || matches!(
            path_text.as_bytes(),
            [drive, b':', b'/', ..] if drive.is_ascii_alphabetic()
        )
}

/// Remove trailing separators from a local path without destroying root semantics.
///
/// Roots like `/`, `//`, and `C:/` stay intact so callers can still distinguish "the root itself"
/// from "a path under the root".
fn trim_trailing_local_path_separator(path_text: &str) -> &str {
    if path_text == "/" || path_text == "//" {
        return path_text;
    }
    if matches!(path_text.as_bytes(), [drive, b':', b'/'] if drive.is_ascii_alphabetic()) {
        return path_text;
    }
    path_text.trim_end_matches('/')
}

/// Strip `cwd_text` from the start of `path_text` when `path_text` is strictly underneath it.
///
/// Returns the relative remainder without a leading slash. If the path equals the cwd exactly, this
/// returns `None` so callers can keep rendering the full path instead of collapsing it to an empty
/// string.
fn strip_local_path_prefix<'a>(path_text: &'a str, cwd_text: &str) -> Option<&'a str> {
    let path_text = trim_trailing_local_path_separator(path_text);
    let cwd_text = trim_trailing_local_path_separator(cwd_text);
    if path_text == cwd_text {
        return None;
    }

    // Treat filesystem roots specially so `/tmp/x` under `/` becomes `tmp/x` instead of being
    // left unchanged by the generic prefix-stripping branch.
    if cwd_text == "/" || cwd_text == "//" {
        return path_text.strip_prefix('/');
    }

    path_text
        .strip_prefix(cwd_text)
        .and_then(|rest| rest.strip_prefix('/'))
}

/// Choose the visible path text for a local link after normalization.
///
/// Relative paths stay relative. Absolute paths are shortened against `cwd` only when they are
/// lexically underneath it; otherwise the absolute path is preserved. This is display logic only,
/// not filesystem canonicalization.
fn display_local_link_path(path_text: &str, cwd: Option<&Path>) -> String {
    let path_text = normalize_local_link_path_text(path_text);
    if !is_absolute_local_link_path(&path_text) {
        return path_text;
    }

    if let Some(cwd) = cwd {
        // Only shorten absolute paths that are under the provided session cwd; otherwise preserve
        // the original absolute target for clarity.
        let cwd_text = normalize_local_link_path_text(&cwd.to_string_lossy());
        if let Some(stripped) = strip_local_path_prefix(&path_text, &cwd_text) {
            return stripped.to_string();
        }
    }

    path_text
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

    #[test]
    fn does_not_split_long_url_like_token_without_scheme() {
        let url_like =
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890";
        let rendered = render_markdown_text_with_width(url_like, Some(24));
        let lines = lines_to_strings(&rendered);

        assert_eq!(
            lines.iter().filter(|line| line.contains(url_like)).count(),
            1,
            "expected full URL-like token in one rendered line, got: {lines:?}"
        );
    }

    #[test]
    fn fenced_code_info_string_with_metadata_highlights() {
        // CommonMark info strings like "rust,no_run" or "rust title=demo"
        // contain metadata after the language token.  The language must be
        // extracted (first word / comma-separated token) so highlighting works.
        for info in &["rust,no_run", "rust no_run", "rust title=\"demo\""] {
            let markdown = format!("```{info}\nfn main() {{}}\n```\n");
            let rendered = render_markdown_text(&markdown);
            let has_rgb = rendered.lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|s| matches!(s.style.fg, Some(ratatui::style::Color::Rgb(..))))
            });
            assert!(
                has_rgb,
                "info string \"{info}\" should still produce syntax highlighting"
            );
        }
    }

    #[test]
    fn crlf_code_block_no_extra_blank_lines() {
        // pulldown-cmark can split CRLF code blocks into multiple Text events.
        // The buffer must concatenate them verbatim — no inserted separators.
        let markdown = "```rust\r\nfn main() {}\r\n    line2\r\n```\r\n";
        let rendered = render_markdown_text(markdown);
        let lines = lines_to_strings(&rendered);
        // Should be exactly two code lines; no spurious blank line between them.
        assert_eq!(
            lines,
            vec!["fn main() {}".to_string(), "    line2".to_string()],
            "CRLF code block should not produce extra blank lines: {lines:?}"
        );
    }
}
