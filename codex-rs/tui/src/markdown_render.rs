//! Low-level markdown event renderer for the TUI transcript.
//!
//! This module consumes `pulldown-cmark` events and emits styled `ratatui`
//! lines, including table layout, width-aware wrapping, and local file-link
//! display. It is the final rendering stage used by higher-level helpers in
//! `markdown.rs`.
//!
//! This renderer intentionally treats local file links differently from normal web links. For
//! local paths, the displayed text comes from the destination, not the markdown label, so
//! transcripts show the real file target (including normalized location suffixes) and can shorten
//! absolute paths relative to a known working directory.
//!
//! ## Table rendering pipeline
//!
//! When the parser emits `Tag::Table` .. `TagEnd::Table`, the writer
//! accumulates header and body rows into a `TableState`, then hands it to
//! `render_table_lines` which runs this pipeline:
//!
//! 1. **Filter spillover rows** -- heuristic extraction of rows that are
//!    artifacts of pulldown-cmark's lenient parsing.
//! 2. **Normalize column counts** -- pad or truncate so every row matches the
//!    alignment count.
//! 3. **Compute column widths** -- allocate widths with content-aware
//!    priority and iterative shrinking.
//! 4. **Choose presentation** -- render theme-accented row-separated columns
//!    while values remain scannable, otherwise transpose body rows
//!    into key/value records separated by muted rules.
//! 5. **Append spillover** -- extracted spillover rows rendered as plain text
//!    after the table.
//!
//! ## Width allocation
//!
//! Columns are classified as Narrative (long prose), TokenHeavy (paths, URLs,
//! or hashes), or Compact (short values such as counts and status labels).
//! Token-heavy columns give up excess width before narrative columns so an
//! oversized path does not collapse readable prose; compact values are
//! preserved last. When compact values split, token-heavy values collapse into
//! unusably short chunks, expansive cells form tall narrow strips across enough
//! body rows, or even 3-char-wide columns cannot fit, body rows render as
//! key/value records.

use crate::render::highlight::foreground_style_for_scopes;
use crate::render::highlight::highlight_code_to_lines;
use crate::render::line_utils::line_to_static;
use crate::style::table_separator_style;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::annotate_web_urls_in_line;
use crate::terminal_hyperlinks::remap_wrapped_line;
use crate::terminal_hyperlinks::visible_lines;
use crate::terminal_hyperlinks::web_destination;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use crate::wrapping::word_wrap_line;
use codex_utils_string::normalize_markdown_hash_location_suffix;
use dirs::home_dir;
use pulldown_cmark::Alignment;
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
use regex_lite::Regex;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;
use url::Url;

mod table_key_value;

const TABLE_COLUMN_GAP: usize = 2;
const TABLE_CELL_PADDING: usize = 1;
const TABLE_HEADER_SEPARATOR_CHAR: char = '━';
const TABLE_BODY_SEPARATOR_CHAR: char = '─';

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

/// Styled content of a single cell in the table being parsed.
///
/// A cell can contain multiple lines (hard breaks inside the cell) and rich inline spans (bold,
/// code, links).  The `plain_text()` projection is used for column-width measurement; the styled
/// `lines` are used for final rendering.
#[derive(Clone, Debug, Default)]
struct TableCell {
    lines: Vec<HyperlinkLine>,
}

// TableCell mutators inlined — called per-span during table event parsing.
impl TableCell {
    #[inline]
    fn ensure_line(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(HyperlinkLine::new(Line::default()));
        }
    }

    #[inline]
    fn push_span(&mut self, span: Span<'static>) {
        self.ensure_line();
        if let Some(line) = self.lines.last_mut() {
            line.line.push_span(span);
        }
    }

    fn push_annotated(&mut self, mut appended: HyperlinkLine) {
        self.ensure_line();
        if let Some(line) = self.lines.last_mut() {
            let shift = line.width();
            line.line.spans.append(&mut appended.line.spans);
            line.hyperlinks
                .extend(appended.hyperlinks.into_iter().map(|mut link| {
                    link.columns = link.columns.start + shift..link.columns.end + shift;
                    link
                }));
        }
    }

    #[inline]
    fn hard_break(&mut self) {
        self.lines.push(HyperlinkLine::new(Line::default()));
    }

    fn plain_text(&self) -> String {
        use std::fmt::Write;
        let mut buf = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            for span in &line.line.spans {
                let _ = write!(buf, "{}", span.content);
            }
        }
        buf
    }
}

/// Accumulates pulldown-cmark table events into a structured representation.
///
/// `TableState` is created on `Tag::Table` and consumed on `TagEnd::Table`. Between those events,
/// the Writer delegates cell content (text, code, html, breaks) into the `current_cell`, which is
/// flushed into `current_row` on `TagEnd::TableCell`, then into `header`/`rows` on row/head end
/// events.
#[derive(Debug)]
struct TableBodyRow {
    cells: Vec<TableCell>,
    has_table_pipe_syntax: bool,
}

#[derive(Debug)]
struct TableState {
    alignments: Vec<Alignment>,
    header: Option<Vec<TableCell>>,
    rows: Vec<TableBodyRow>,
    current_row: Option<Vec<TableCell>>,
    current_row_has_table_pipe_syntax: bool,
    current_cell: Option<TableCell>,
    in_header: bool,
}

impl TableState {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            header: None,
            rows: Vec::new(),
            current_row: None,
            current_row_has_table_pipe_syntax: false,
            current_cell: None,
            in_header: false,
        }
    }
}

/// Rendered table output split by wrapping behavior.
///
/// `table_lines` are prewrapped aligned rows or key/value records, except
/// header-only tables may retain pipe fallback rows for normal wrapping.
/// `spillover_lines` are prose rows extracted from parser artifacts and should
/// be routed through normal wrapping.
struct RenderedTableLines {
    table_lines: Vec<HyperlinkLine>,
    table_lines_prewrapped: bool,
    spillover_lines: Vec<HyperlinkLine>,
}

/// Classification of a table column for width-allocation priority.
///
/// Token-heavy columns such as paths and URLs are allowed to wrap before prose becomes unreadable.
/// Compact columns such as counts or status words resist wrapping so their values stay scannable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableColumnKind {
    /// Long-form prose content (>= 4 avg words/cell or >= 28 avg char width).
    Narrative,
    /// Content dominated by long tokens, such as paths, URLs, and hashes.
    TokenHeavy,
    /// Short values, such as counts and status labels, that should resist wrapping.
    Compact,
}

/// Per-column statistics used to drive the width-allocation algorithm.
///
/// Collected in a single pass over the header and body rows before any
/// shrinking decisions are made.
#[derive(Clone, Debug)]
struct TableColumnMetrics {
    /// Widest cell content (display width) across header and all body rows.
    max_width: usize,
    /// Display width of the longest whitespace-delimited token in the header.
    header_token_width: usize,
    /// Display width of the longest whitespace-delimited token across body rows.
    body_token_width: usize,
    /// Classification derived from body token density and average cell content.
    kind: TableColumnKind,
}

/// Render markdown with default wrapping behavior.
///
/// Use this when the caller does not have a concrete render width yet (for
/// example, snapshot tests or contexts that intentionally defer wrapping). If
/// a viewport width is known, prefer [`render_markdown_text_with_width`] so
/// table fallback and line wrapping decisions match the visible terminal.
pub fn render_markdown_text(input: &str) -> Text<'static> {
    render_markdown_text_with_width(input, /*width*/ None)
}

/// Render markdown constrained to a known terminal width.
///
/// The renderer preserves columnar table structure while values remain
/// scannable and falls back to key/value records when body rows cannot fit
/// readably. Passing `None` keeps intrinsic line widths and disables
/// width-driven wrapping in the markdown writer. Local file links render
/// relative to the current process working directory.
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
    Text::from(visible_lines(render_markdown_lines_with_width_and_cwd(
        input, width, cwd,
    )))
}

pub(crate) fn render_markdown_lines_with_width_and_cwd(
    input: &str,
    width: Option<usize>,
    cwd: Option<&Path>,
) -> Vec<HyperlinkLine> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(input, options).into_offset_iter();
    let mut w = Writer::new(input, parser, width, cwd);
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

/// Stateful pulldown-cmark event consumer that builds styled `ratatui` output.
///
/// Tracks inline style nesting, indent/blockquote context, list numbering,
/// and an optional `TableState` for accumulating table events.  The
/// `wrap_width` field enables width-aware line wrapping and table column
/// allocation; when `None`, lines keep their intrinsic width.
struct Writer<'a, I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    input: &'a str,
    iter: I,
    text: Vec<HyperlinkLine>,
    styles: MarkdownStyles,
    inline_styles: Vec<Style>,
    indent_stack: Vec<IndentContext>,
    list_indices: Vec<Option<u64>>,
    list_needs_blank_before_next_item: Vec<bool>,
    list_item_start_line_counts: Vec<usize>,
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
    current_line_content: Option<HyperlinkLine>,
    current_initial_indent: Vec<Span<'static>>,
    current_subsequent_indent: Vec<Span<'static>>,
    current_line_style: Style,
    current_line_in_code_block: bool,
    table_state: Option<TableState>,
}

impl<'a, I> Writer<'a, I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    fn new(input: &'a str, iter: I, wrap_width: Option<usize>, cwd: Option<&Path>) -> Self {
        Self {
            input,
            iter,
            text: Vec::new(),
            styles: MarkdownStyles::default(),
            inline_styles: Vec::new(),
            indent_stack: Vec::new(),
            list_indices: Vec::new(),
            list_needs_blank_before_next_item: Vec::new(),
            list_item_start_line_counts: Vec::new(),
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
            table_state: None,
        }
    }

    fn run(&mut self) {
        while let Some((ev, range)) = self.iter.next() {
            self.handle_event(ev, range);
        }
        self.flush_current_line();
    }

    fn handle_event(&mut self, event: Event<'a>, range: Range<usize>) {
        self.prepare_for_event(&event);
        match event {
            Event::Start(tag) => self.start_tag(tag, range),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.code(code),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => {
                self.flush_current_line();
                if !self.text.is_empty() {
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

    fn start_tag(&mut self, tag: Tag<'a>, range: Range<usize>) {
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
            Tag::Table(alignments) => self.start_table(alignments),
            Tag::TableHead => self.start_table_head(),
            Tag::TableRow => self.start_table_row(range),
            Tag::TableCell => self.start_table_cell(),
            Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
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
                self.flush_current_line();
                let start_line_count = self.list_item_start_line_counts.pop().unwrap_or_default();
                if self.text.len().saturating_sub(start_line_count) > 1
                    && let Some(needs_blank) = self.list_needs_blank_before_next_item.last_mut()
                {
                    *needs_blank = true;
                }
                self.indent_stack.pop();
                self.pending_marker_line = false;
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.pop_inline_style(),
            TagEnd::Link => self.pop_link(),
            TagEnd::Table => self.end_table(),
            TagEnd::TableHead => self.end_table_head(),
            TagEnd::TableRow => self.end_table_row(),
            TagEnd::TableCell => self.end_table_cell(),
            TagEnd::HtmlBlock
            | TagEnd::FootnoteDefinition
            | TagEnd::Image
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn start_paragraph(&mut self) {
        if self.in_table_cell() {
            return;
        }
        if self.needs_newline {
            self.push_blank_line();
        }
        self.push_line(Line::default());
        self.needs_newline = false;
        self.in_paragraph = true;
    }

    fn end_paragraph(&mut self) {
        if self.in_table_cell() {
            return;
        }
        self.needs_newline = true;
        self.in_paragraph = false;
        self.pending_marker_line = false;
    }

    fn start_heading(&mut self, level: HeadingLevel) {
        if self.in_table_cell() {
            return;
        }
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
        if self.in_table_cell() {
            return;
        }
        self.needs_newline = true;
        self.pop_inline_style();
    }

    fn start_blockquote(&mut self) {
        if self.in_table_cell() {
            return;
        }
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
        if self.in_table_cell() {
            return;
        }
        self.indent_stack.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        if self.suppressing_local_link_label() {
            return;
        }
        self.line_ends_with_local_link_target = false;
        if self.in_table_cell() {
            self.push_text_to_table_cell(&text);
            return;
        }

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
                .map(|line| !line.line.spans.is_empty())
                .unwrap_or_else(|| {
                    self.text
                        .last()
                        .map(|line| !line.line.spans.is_empty())
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
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_text_spans(&content, style);
        }
        self.needs_newline = false;
    }

    fn code(&mut self, code: CowStr<'a>) {
        if self.suppressing_local_link_label() {
            return;
        }
        self.line_ends_with_local_link_target = false;
        if self.in_table_cell() {
            self.push_span_to_table_cell(Span::from(code.into_string()).style(self.styles.code));
            return;
        }

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
        if self.in_table_cell() {
            let style = self.inline_styles.last().copied().unwrap_or_default();
            for (i, line) in html.lines().enumerate() {
                if i > 0 {
                    self.push_table_cell_hard_break();
                }
                self.push_span_to_table_cell(Span::styled(line.to_string(), style));
            }
            if !inline {
                self.push_table_cell_hard_break();
            }
            return;
        }
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
        if self.in_table_cell() {
            self.push_table_cell_hard_break();
            return;
        }
        self.push_line(Line::default());
    }

    fn soft_break(&mut self) {
        if self.suppressing_local_link_label() {
            return;
        }
        if self.in_table_cell() {
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_span_to_table_cell(Span::styled(" ".to_string(), style));
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
        self.list_needs_blank_before_next_item.push(false);
    }

    fn end_list(&mut self) {
        self.list_indices.pop();
        self.list_needs_blank_before_next_item.pop();
        self.needs_newline = true;
    }

    fn start_item(&mut self) {
        if self
            .list_needs_blank_before_next_item
            .last_mut()
            .map(std::mem::take)
            .unwrap_or(false)
        {
            self.push_blank_line();
        }
        self.flush_current_line();
        self.list_item_start_line_counts.push(self.text.len());
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
        if !self.text.is_empty() {
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

    fn start_table(&mut self, alignments: Vec<Alignment>) {
        self.flush_current_line();
        if self.needs_newline {
            self.push_blank_line();
            self.needs_newline = false;
        }
        self.table_state = Some(TableState::new(alignments));
    }

    fn end_table(&mut self) {
        let Some(table_state) = self.table_state.take() else {
            return;
        };

        let RenderedTableLines {
            table_lines,
            table_lines_prewrapped,
            spillover_lines,
        } = self.render_table_lines(table_state);
        let mut pending_marker_line = self.pending_marker_line;
        for line in table_lines {
            if table_lines_prewrapped {
                self.push_prewrapped_line(line, pending_marker_line);
            } else {
                self.push_hyperlink_line(line);
                self.flush_current_line();
            }
            pending_marker_line = false;
        }
        self.pending_marker_line = false;
        for spillover_line in spillover_lines {
            self.push_hyperlink_line(spillover_line);
            self.flush_current_line();
        }
        self.needs_newline = true;
    }

    fn start_table_head(&mut self) {
        if let Some(table_state) = self.table_state.as_mut() {
            table_state.in_header = true;
            table_state.current_row = Some(Vec::new());
        }
    }

    fn end_table_head(&mut self) {
        let Some(table_state) = self.table_state.as_mut() else {
            return;
        };
        if let Some(current_cell) = table_state.current_cell.take() {
            table_state
                .current_row
                .get_or_insert_with(Vec::new)
                .push(current_cell);
        }
        if let Some(row) = table_state.current_row.take() {
            table_state.header = Some(row);
        }
        table_state.in_header = false;
    }

    fn start_table_row(&mut self, source_range: Range<usize>) {
        let has_table_pipe_syntax = self.has_table_row_boundary_pipe(source_range);
        if let Some(table_state) = self.table_state.as_mut() {
            table_state.current_row = Some(Vec::new());
            table_state.current_row_has_table_pipe_syntax = has_table_pipe_syntax;
        }
    }

    fn has_table_row_boundary_pipe(&self, source_range: Range<usize>) -> bool {
        let Some(source) = self.input.get(source_range) else {
            return false;
        };
        let source = source.trim();
        source.starts_with('|') || source.ends_with('|')
    }

    fn end_table_row(&mut self) {
        let Some(table_state) = self.table_state.as_mut() else {
            return;
        };

        if let Some(current_cell) = table_state.current_cell.take() {
            table_state
                .current_row
                .get_or_insert_with(Vec::new)
                .push(current_cell);
        }

        let Some(row) = table_state.current_row.take() else {
            return;
        };

        if table_state.in_header {
            table_state.header = Some(row);
        } else {
            table_state.rows.push(TableBodyRow {
                cells: row,
                has_table_pipe_syntax: table_state.current_row_has_table_pipe_syntax,
            });
        }
        table_state.current_row_has_table_pipe_syntax = false;
    }

    fn start_table_cell(&mut self) {
        if let Some(table_state) = self.table_state.as_mut() {
            table_state.current_cell = Some(TableCell::default());
        }
    }

    fn end_table_cell(&mut self) {
        let Some(table_state) = self.table_state.as_mut() else {
            return;
        };

        if let Some(cell) = table_state.current_cell.take() {
            table_state
                .current_row
                .get_or_insert_with(Vec::new)
                .push(cell);
        }
    }

    fn in_table_cell(&self) -> bool {
        self.table_state
            .as_ref()
            .and_then(|table_state| table_state.current_cell.as_ref())
            .is_some()
    }

    fn push_span_to_table_cell(&mut self, span: Span<'static>) {
        if let Some(table_state) = self.table_state.as_mut()
            && let Some(cell) = table_state.current_cell.as_mut()
        {
            cell.push_span(span);
        }
    }

    fn push_table_cell_hard_break(&mut self) {
        if let Some(table_state) = self.table_state.as_mut()
            && let Some(cell) = table_state.current_cell.as_mut()
        {
            cell.hard_break();
        }
    }

    fn push_text_to_table_cell(&mut self, text: &str) {
        let style = self.inline_styles.last().copied().unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if i > 0 {
                self.push_table_cell_hard_break();
            }
            self.push_text_spans_to_table_cell(line, style);
        }
    }

    fn push_text_spans_to_table_cell(&mut self, text: &str, style: Style) {
        let span = Span::styled(text.to_string(), style);
        let destination = self
            .link
            .as_ref()
            .and_then(|link| web_destination(&link.destination));
        let mut annotated = if let Some(destination) = destination {
            let mut annotated = HyperlinkLine::new(Line::default());
            annotated.push_span(span, Some(&destination));
            annotated
        } else if self.link.is_some() || self.in_code_block {
            HyperlinkLine::new(Line::from(span))
        } else {
            annotate_web_urls_in_line(Line::from(span))
        };
        if let Some(table_state) = self.table_state.as_mut()
            && let Some(cell) = table_state.current_cell.as_mut()
        {
            cell.push_annotated(std::mem::take(&mut annotated));
        }
    }

    /// Convert a completed `TableState` into styled table `Line`s.
    ///
    /// Pipeline: filter spillover rows -> normalize column counts -> compute
    /// column widths -> render aligned rows or key/value records when values
    /// systemically lose token readability or expansive cells become tall
    /// narrow strips. Spillover rows are appended as plain text after the
    /// table.
    ///
    /// Falls back to key/value records when body rows cannot fit in the aligned
    /// grid; header-only tables retain raw pipe output because they contain no
    /// records to transpose.
    fn render_table_lines(&self, mut table_state: TableState) -> RenderedTableLines {
        let column_count = table_state.alignments.len();
        if column_count == 0 {
            return RenderedTableLines {
                table_lines: Vec::new(),
                table_lines_prewrapped: true,
                spillover_lines: Vec::new(),
            };
        }

        let mut spillover_rows: Vec<TableCell> = Vec::with_capacity(4);
        let mut rows: Vec<Vec<TableCell>> = Vec::with_capacity(table_state.rows.len());
        for (row_idx, row) in table_state.rows.iter().enumerate() {
            let next_row = table_state.rows.get(row_idx + 1);
            // pulldown-cmark accepts body rows without pipes, which can turn a following paragraph
            // into a one-cell table row. For multi-column tables, treat those as spillover text
            // rendered after the table.
            if column_count > 1 && Self::is_spillover_row(row, next_row) {
                if let Some(cell) = row.cells.first().cloned() {
                    spillover_rows.push(cell);
                }
            } else {
                rows.push(row.cells.clone());
            }
        }

        let mut header = table_state
            .header
            .take()
            .unwrap_or_else(|| vec![TableCell::default(); column_count]);
        Self::normalize_row(&mut header, column_count);
        for row in &mut rows {
            Self::normalize_row(row, column_count);
        }

        let metrics = Self::collect_table_column_metrics(&header, &rows, column_count);
        let available_width = self.available_table_width(column_count);
        let widths =
            self.compute_column_widths(&header, &rows, &table_state.alignments, available_width);
        let spillover_lines: Vec<HyperlinkLine> = spillover_rows
            .into_iter()
            .flat_map(|spillover| spillover.lines)
            .collect();
        let header_style =
            foreground_style_for_scopes(&["entity.name.type", "support.type", "variable"])
                .unwrap_or(self.styles.strong)
                .bold();
        let separator_style = table_separator_style();

        let Some(column_widths) = widths else {
            if !rows.is_empty() {
                return RenderedTableLines {
                    table_lines: table_key_value::render_records(
                        &header,
                        &rows,
                        &metrics,
                        self.available_record_width(),
                        header_style,
                        separator_style,
                    ),
                    table_lines_prewrapped: true,
                    spillover_lines,
                };
            }
            return RenderedTableLines {
                table_lines: self.render_table_pipe_fallback(
                    &header,
                    &rows,
                    &table_state.alignments,
                ),
                table_lines_prewrapped: false,
                spillover_lines,
            };
        };

        if table_key_value::should_render_records(&rows, &column_widths, &metrics) {
            return RenderedTableLines {
                table_lines: table_key_value::render_records(
                    &header,
                    &rows,
                    &metrics,
                    self.available_record_width(),
                    header_style,
                    separator_style,
                ),
                table_lines_prewrapped: true,
                spillover_lines,
            };
        }

        let mut out = Vec::with_capacity(2 + rows.len() * 2);
        out.extend(self.render_table_row(
            &header,
            &column_widths,
            &table_state.alignments,
            header_style,
        ));
        out.push(Self::render_table_separator(
            &column_widths,
            TABLE_HEADER_SEPARATOR_CHAR,
            separator_style,
        ));
        for (row_idx, row) in rows.iter().enumerate() {
            out.extend(self.render_table_row(
                row,
                &column_widths,
                &table_state.alignments,
                Style::default(),
            ));
            if row_idx + 1 < rows.len() {
                out.push(Self::render_table_separator(
                    &column_widths,
                    TABLE_BODY_SEPARATOR_CHAR,
                    separator_style,
                ));
            }
        }
        RenderedTableLines {
            table_lines: out,
            table_lines_prewrapped: true,
            spillover_lines,
        }
    }

    fn normalize_row(row: &mut Vec<TableCell>, column_count: usize) {
        row.truncate(column_count);
        row.resize(column_count, TableCell::default());
    }

    /// Subtract horizontal gutters and per-cell padding from the content budget.
    fn available_table_width(&self, column_count: usize) -> Option<usize> {
        self.wrap_width.map(|wrap_width| {
            let prefix_width =
                Self::spans_display_width(&self.prefix_spans(self.pending_marker_line));
            let reserved = prefix_width
                + (column_count.saturating_sub(1) * TABLE_COLUMN_GAP)
                + (column_count * TABLE_CELL_PADDING * 2);
            wrap_width.saturating_sub(reserved)
        })
    }

    /// Return the full content budget for record fallback rendering.
    fn available_record_width(&self) -> Option<usize> {
        self.wrap_width.map(|wrap_width| {
            let prefix_width =
                Self::spans_display_width(&self.prefix_spans(self.pending_marker_line));
            wrap_width.saturating_sub(prefix_width)
        })
    }

    /// Allocate column widths for aligned, row-separated table rendering.
    ///
    /// Each column starts at its natural (max cell content) width, then columns
    /// are iteratively shrunk one character at a time until the total fits within
    /// `available_width`. Token-heavy columns surrender excess width before
    /// narrative prose; compact columns are preserved last. Returns `None` when
    /// even the minimum width (3 chars per column) cannot fit.
    fn compute_column_widths(
        &self,
        header: &[TableCell],
        rows: &[Vec<TableCell>],
        alignments: &[Alignment],
        available_width: Option<usize>,
    ) -> Option<Vec<usize>> {
        let min_column_width = 3usize;
        let metrics = Self::collect_table_column_metrics(header, rows, alignments.len());
        let mut widths: Vec<usize> = metrics
            .iter()
            .map(|col| col.max_width.max(min_column_width))
            .collect();

        let Some(max_width) = available_width else {
            return Some(widths);
        };
        let minimum_total = alignments.len() * min_column_width;
        if max_width < minimum_total {
            return None;
        }

        let mut floors: Vec<usize> = metrics
            .iter()
            .map(|col| Self::preferred_column_floor(col, min_column_width))
            .collect();
        let mut floor_total: usize = floors.iter().sum();
        if floor_total > max_width {
            // Relax preferred floors in wrapping priority order until the hard width budget fits.
            while floor_total > max_width {
                let Some((idx, _)) = floors
                    .iter()
                    .enumerate()
                    .filter(|(_, floor)| **floor > min_column_width)
                    .min_by_key(|(idx, floor)| {
                        (
                            Self::column_shrink_priority(metrics[*idx].kind),
                            usize::MAX.saturating_sub(**floor),
                        )
                    })
                else {
                    break;
                };

                floors[idx] -= 1;
                floor_total -= 1;
            }
        }

        let mut total_width: usize = widths.iter().sum();

        while total_width > max_width {
            let Some(idx) = Self::next_column_to_shrink(&widths, &floors, &metrics) else {
                break;
            };
            widths[idx] -= 1;
            total_width -= 1;
        }

        if total_width > max_width {
            return None;
        }

        Some(widths)
    }

    fn collect_table_column_metrics(
        header: &[TableCell],
        rows: &[Vec<TableCell>],
        column_count: usize,
    ) -> Vec<TableColumnMetrics> {
        let mut metrics = Vec::with_capacity(column_count);
        for column in 0..column_count {
            let header_cell = &header[column];
            let header_plain = header_cell.plain_text();
            let header_token_width = Self::longest_token_width(&header_plain);
            let mut max_width = Self::cell_display_width(header_cell);
            let mut body_token_width = 0usize;
            let mut body_token_count = 0usize;
            let mut long_body_token_count = 0usize;
            let mut total_words = 0usize;
            let mut total_cells = 0usize;
            let mut total_cell_width = 0usize;

            for row in rows {
                let cell = &row[column];
                max_width = max_width.max(Self::cell_display_width(cell));
                let plain = cell.plain_text();
                body_token_width = body_token_width.max(Self::longest_token_width(&plain));
                let word_count = plain.split_whitespace().count();
                if word_count > 0 {
                    body_token_count += word_count;
                    long_body_token_count += plain
                        .split_whitespace()
                        .filter(|token| token.width() >= 20)
                        .count();
                    total_words += word_count;
                    total_cells += 1;
                    total_cell_width += plain.width();
                }
            }

            let avg_words_per_cell = if total_cells == 0 {
                header_plain.split_whitespace().count() as f64
            } else {
                total_words as f64 / total_cells as f64
            };
            let avg_cell_width = if total_cells == 0 {
                header_plain.width() as f64
            } else {
                total_cell_width as f64 / total_cells as f64
            };
            let kind = if long_body_token_count > 0
                && long_body_token_count >= body_token_count.saturating_sub(long_body_token_count)
            {
                TableColumnKind::TokenHeavy
            } else if avg_words_per_cell >= 4.0 || avg_cell_width >= 28.0 {
                TableColumnKind::Narrative
            } else {
                TableColumnKind::Compact
            };

            metrics.push(TableColumnMetrics {
                max_width,
                header_token_width,
                body_token_width,
                kind,
            });
        }

        metrics
    }

    /// Compute the preferred minimum width for a column before the shrink loop
    /// starts reducing it further.
    ///
    /// Narrative and token-heavy columns retain a readable 16-cell soft floor.
    /// Compact columns floor at the larger of the header and body token widths
    /// (body capped at 16). The result is clamped to `[min_column_width, max_width]`.
    fn preferred_column_floor(metrics: &TableColumnMetrics, min_column_width: usize) -> usize {
        let token_target = match metrics.kind {
            TableColumnKind::Narrative | TableColumnKind::TokenHeavy => 16,
            TableColumnKind::Compact => metrics
                .header_token_width
                .max(metrics.body_token_width.min(16)),
        };
        token_target.max(min_column_width).min(metrics.max_width)
    }

    /// Pick the next column to shrink by one character during width allocation.
    ///
    /// Priority: TokenHeavy columns are shrunk before Narrative, then Compact.
    /// Within the same kind, the column with the most slack above its floor is
    /// chosen so similarly-shaped columns stay balanced.
    fn next_column_to_shrink(
        widths: &[usize],
        floors: &[usize],
        metrics: &[TableColumnMetrics],
    ) -> Option<usize> {
        widths
            .iter()
            .enumerate()
            .filter(|(idx, width)| **width > floors[*idx])
            .min_by_key(|(idx, width)| {
                let slack = width.saturating_sub(floors[*idx]);
                (
                    Self::column_shrink_priority(metrics[*idx].kind),
                    usize::MAX.saturating_sub(slack),
                )
            })
            .map(|(idx, _)| idx)
    }

    fn column_shrink_priority(kind: TableColumnKind) -> usize {
        match kind {
            TableColumnKind::TokenHeavy => 0,
            TableColumnKind::Narrative => 1,
            TableColumnKind::Compact => 2,
        }
    }

    fn render_table_separator(
        column_widths: &[usize],
        separator_char: char,
        style: Style,
    ) -> HyperlinkLine {
        let segment_char = separator_char.to_string();
        let gap = " ".repeat(TABLE_COLUMN_GAP);
        let text = column_widths
            .iter()
            .map(|width| segment_char.repeat(*width + (TABLE_CELL_PADDING * 2)))
            .collect::<Vec<_>>()
            .join(&gap);
        HyperlinkLine::new(Line::from(Span::styled(text, style)))
    }

    fn render_table_row(
        &self,
        row: &[TableCell],
        column_widths: &[usize],
        alignments: &[Alignment],
        row_style: Style,
    ) -> Vec<HyperlinkLine> {
        let wrapped_cells: Vec<Vec<HyperlinkLine>> = row
            .iter()
            .zip(column_widths)
            .map(|(cell, width)| self.wrap_cell(cell, *width))
            .collect();
        let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

        let mut out = Vec::with_capacity(row_height);
        for row_line in 0..row_height {
            let Some(last_visible_column) = wrapped_cells.iter().rposition(|lines| {
                lines
                    .get(row_line)
                    .is_some_and(|line| Self::line_display_width(&line.line) > 0)
            }) else {
                out.push(HyperlinkLine::new(Line::default().style(row_style)));
                continue;
            };
            let mut spans = Vec::new();
            for (column, width) in column_widths
                .iter()
                .enumerate()
                .take(last_visible_column + 1)
            {
                spans.push(Span::raw(" ".repeat(TABLE_CELL_PADDING)));
                let mut line = wrapped_cells[column]
                    .get(row_line)
                    .cloned()
                    .unwrap_or_default();
                let line_width = Self::line_display_width(&line.line);
                let remaining = width.saturating_sub(line_width);
                let (left_padding, right_padding) = match alignments[column] {
                    Alignment::Left | Alignment::None => (0, remaining),
                    Alignment::Center => (remaining / 2, remaining - (remaining / 2)),
                    Alignment::Right => (remaining, 0),
                };
                if left_padding > 0 {
                    spans.push(Span::raw(" ".repeat(left_padding)));
                }
                spans.append(&mut line.line.spans);
                let is_last_column = column == last_visible_column;
                if right_padding > 0 && !is_last_column {
                    spans.push(Span::raw(" ".repeat(right_padding)));
                }
                if !is_last_column {
                    spans.push(Span::raw(" ".repeat(TABLE_CELL_PADDING)));
                }
                if !is_last_column {
                    spans.push(Span::raw(" ".repeat(TABLE_COLUMN_GAP)));
                }
            }
            let mut out_line = HyperlinkLine::new(Line::from(spans).style(row_style));
            let mut column_start = 0usize;
            for (column, width) in column_widths
                .iter()
                .enumerate()
                .take(last_visible_column + 1)
            {
                column_start += TABLE_CELL_PADDING;
                if let Some(line) = wrapped_cells[column].get(row_line) {
                    let remaining = width.saturating_sub(Self::line_display_width(&line.line));
                    let left_padding = match alignments[column] {
                        Alignment::Left | Alignment::None => 0,
                        Alignment::Center => remaining / 2,
                        Alignment::Right => remaining,
                    };
                    out_line
                        .hyperlinks
                        .extend(line.hyperlinks.iter().cloned().map(|mut link| {
                            link.columns = link.columns.start + column_start + left_padding
                                ..link.columns.end + column_start + left_padding;
                            link
                        }));
                }
                column_start += *width + TABLE_CELL_PADDING + TABLE_COLUMN_GAP;
            }
            out.push(out_line);
        }
        out
    }

    /// Render a header-only table as raw pipe-delimited lines (`| A | B |`).
    ///
    /// Used when `compute_column_widths` returns `None` and there are no body
    /// records to transpose. Pipe characters inside cell content are escaped
    /// as `\|` so downstream parsers keep cell boundaries intact.
    fn render_table_pipe_fallback(
        &self,
        header: &[TableCell],
        rows: &[Vec<TableCell>],
        alignments: &[Alignment],
    ) -> Vec<HyperlinkLine> {
        let mut out = Vec::new();
        out.push(Self::row_to_pipe_line(header));
        out.push(HyperlinkLine::new(Line::from(
            Self::alignments_to_pipe_delimiter(alignments),
        )));
        out.extend(rows.iter().map(|row| Self::row_to_pipe_line(row)));
        out
    }

    fn row_to_pipe_line(row: &[TableCell]) -> HyperlinkLine {
        let mut out = HyperlinkLine::new(Line::default());
        out.push_span("|".into(), /*destination*/ None);
        for cell in row {
            out.push_span(" ".into(), /*destination*/ None);
            for (index, line) in cell.lines.iter().enumerate() {
                if index > 0 {
                    out.push_span(" ".into(), /*destination*/ None);
                }
                let text = line
                    .line
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>();
                let mut column = 0usize;
                let mut current_destination = None;
                let mut current_text = String::new();
                let flush = |out: &mut HyperlinkLine,
                             current_text: &mut String,
                             destination: Option<&str>| {
                    if !current_text.is_empty() {
                        out.push_span(Span::raw(std::mem::take(current_text)), destination);
                    }
                };
                for ch in text.chars() {
                    let destination = line
                        .hyperlinks
                        .iter()
                        .find(|link| link.columns.contains(&column))
                        .map(|link| link.destination.as_str());
                    if destination != current_destination {
                        flush(&mut out, &mut current_text, current_destination);
                        current_destination = destination;
                    }
                    if ch == '|' {
                        current_text.push_str("\\|");
                    } else {
                        current_text.push(ch);
                    }
                    column += UnicodeWidthChar::width(ch).unwrap_or(/*default*/ 0);
                }
                flush(&mut out, &mut current_text, current_destination);
            }
            out.push_span(" |".into(), /*destination*/ None);
        }
        out
    }

    fn alignments_to_pipe_delimiter(alignments: &[Alignment]) -> String {
        let mut out = String::new();
        out.push('|');
        for alignment in alignments {
            let segment = match alignment {
                Alignment::Left => ":---",
                Alignment::Center => ":---:",
                Alignment::Right => "---:",
                Alignment::None => "---",
            };
            out.push_str(segment);
            out.push('|');
        }
        out
    }

    /// Wrap a single table cell's content to `width`, preserving rich inline
    /// styling (bold, code, links) across wrapped lines.
    ///
    /// Each logical line within the cell (separated by hard breaks) is wrapped
    /// independently.  Empty cells produce a single blank line so the row grid
    /// stays aligned.
    fn wrap_cell(&self, cell: &TableCell, width: usize) -> Vec<HyperlinkLine> {
        if cell.lines.is_empty() {
            return vec![HyperlinkLine::new(Line::default())];
        }
        let mut wrapped = Vec::new();
        for source_line in &cell.lines {
            let rendered =
                word_wrap_line(&source_line.line, RtOptions::new(width.max(/*other*/ 1)))
                    .into_iter()
                    .map(|line| line_to_static(&line))
                    .collect::<Vec<_>>();
            if rendered.is_empty() {
                wrapped.push(HyperlinkLine::new(Line::default()));
            } else {
                wrapped.extend(remap_wrapped_line(source_line, rendered));
            };
        }
        if wrapped.is_empty() {
            wrapped.push(HyperlinkLine::new(Line::default()));
        }
        wrapped
    }

    /// Detect rows that are artifacts of pulldown-cmark's lenient table parsing.
    ///
    /// pulldown-cmark accepts body rows without leading pipes, which can absorb a
    /// trailing paragraph as a single-cell row in a multi-column table. These
    /// "spillover" rows are extracted and rendered as plain text after the table
    /// grid so they don't appear as malformed table content.
    ///
    /// Heuristic: a row is spillover if its only non-empty cell is the first one
    /// AND (a single-cell row lacked table pipe syntax, the content looks like
    /// HTML, it's a label line followed by HTML content, or a trailing
    /// HTML-intro label line).
    fn is_spillover_row(row: &TableBodyRow, next_row: Option<&TableBodyRow>) -> bool {
        let Some(first_text) = Self::first_non_empty_only_text(&row.cells) else {
            return false;
        };

        if row.cells.len() == 1 && !row.has_table_pipe_syntax {
            return true;
        }

        if Self::looks_like_html_content(&first_text) {
            return true;
        }

        // Keep common intro + html-block spillover together:
        // "HTML block:" followed by "<div ...>".
        if first_text.trim_end().ends_with(':') {
            if next_row
                .and_then(|row| Self::first_non_empty_only_text(&row.cells))
                .is_some_and(|text| Self::looks_like_html_content(&text))
            {
                return true;
            }

            // pulldown can end the table before the corresponding HTML block line.
            // In that case, treat trailing HTML-intro labels (e.g., "HTML block:")
            // as spillover while keeping explicit sparse labels in real tables.
            if next_row.is_none() && Self::looks_like_html_label_line(&first_text) {
                return true;
            }
        }

        false
    }

    fn first_non_empty_only_text(row: &[TableCell]) -> Option<String> {
        let first = row.first()?.plain_text();
        if first.trim().is_empty() {
            return None;
        }
        let rest_empty = row[1..]
            .iter()
            .all(|cell| cell.plain_text().trim().is_empty());
        rest_empty.then_some(first)
    }

    fn looks_like_html_content(text: &str) -> bool {
        let bytes = text.as_bytes();
        for (idx, &byte) in bytes.iter().enumerate() {
            if byte != b'<' {
                continue;
            }

            let mut tag_start = idx + 1;
            if tag_start < bytes.len() && (bytes[tag_start] == b'/' || bytes[tag_start] == b'!') {
                tag_start += 1;
            }

            if bytes.get(tag_start).is_some_and(u8::is_ascii_alphabetic)
                && bytes
                    .get(tag_start + 1..)
                    .is_some_and(|suffix| suffix.contains(&b'>'))
            {
                return true;
            }
        }
        false
    }

    fn looks_like_html_label_line(text: &str) -> bool {
        let trimmed = text.trim();
        if !trimmed.ends_with(':') {
            return false;
        }
        let prefix = trimmed.trim_end_matches(':').trim();
        prefix
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case("html"))
    }

    // Width-measurement helpers inlined — called per-cell during table column
    // width computation, which runs on every re-render.

    #[inline]
    fn spans_display_width(spans: &[Span<'_>]) -> usize {
        spans.iter().map(|span| span.content.width()).sum()
    }

    #[inline]
    fn line_display_width(line: &Line<'_>) -> usize {
        Self::spans_display_width(&line.spans)
    }

    #[inline]
    fn cell_display_width(cell: &TableCell) -> usize {
        cell.lines
            .iter()
            .map(|line| Self::line_display_width(&line.line))
            .max()
            .unwrap_or(0)
    }

    #[inline]
    fn longest_token_width(text: &str) -> usize {
        text.split_whitespace().map(str::width).max().unwrap_or(0)
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
                // Link destinations are rendered as " (url)" suffixes. When parsing table cells,
                // append the suffix into the active cell buffer rather than the outer paragraph
                // line to avoid detached url lines.
                if self.in_table_cell() {
                    self.push_span_to_table_cell(" (".into());
                    let mut destination = HyperlinkLine::new(Line::default());
                    destination.push_span(
                        Span::styled(link.destination.clone(), self.styles.link),
                        web_destination(&link.destination).as_deref(),
                    );
                    if let Some(table_state) = self.table_state.as_mut()
                        && let Some(cell) = table_state.current_cell.as_mut()
                    {
                        cell.push_annotated(destination);
                    }
                    self.push_span_to_table_cell(")".into());
                } else {
                    self.push_span(" (".into());
                    let mut destination = HyperlinkLine::new(Line::default());
                    destination.push_span(
                        Span::styled(link.destination.clone(), self.styles.link),
                        web_destination(&link.destination).as_deref(),
                    );
                    self.push_annotated(destination);
                    self.push_span(")".into());
                }
            } else if let Some(local_target_display) = link.local_target_display {
                // Local file links are rendered as code-like path text so the transcript shows the
                // resolved target instead of arbitrary caller-provided label text.
                let style = self
                    .inline_styles
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .patch(self.styles.code);
                let span = Span::styled(local_target_display, style);
                if self.in_table_cell() {
                    self.push_span_to_table_cell(span);
                } else {
                    if self.pending_marker_line {
                        self.push_line(Line::default());
                    }
                    self.push_span(span);
                    self.line_ends_with_local_link_target = true;
                }
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
        if let Some(mut line) = self.current_line_content.take() {
            let style = self.current_line_style;
            // NB we don't wrap code in code blocks, in order to preserve whitespace for copy/paste.
            if !self.current_line_in_code_block
                && let Some(width) = self.wrap_width
            {
                let opts = RtOptions::new(width)
                    .initial_indent(self.current_initial_indent.clone().into())
                    .subsequent_indent(self.current_subsequent_indent.clone().into());
                let wrapped = adaptive_wrap_line(&line.line, opts)
                    .into_iter()
                    .map(|wrapped| line_to_static(&wrapped))
                    .collect();
                for wrapped in remap_wrapped_line(&line, wrapped) {
                    self.push_output_line(wrapped.style(style));
                }
            } else {
                let mut spans = self.current_initial_indent.clone();
                let shift = spans.iter().map(|span| span.content.width()).sum::<usize>();
                spans.append(&mut line.line.spans);
                for hyperlink in &mut line.hyperlinks {
                    hyperlink.columns =
                        hyperlink.columns.start + shift..hyperlink.columns.end + shift;
                }
                line.line = Line::from_iter(spans);
                self.push_output_line(line.style(style));
            }
            self.current_initial_indent.clear();
            self.current_subsequent_indent.clear();
            self.current_line_in_code_block = false;
            self.line_ends_with_local_link_target = false;
        }
    }

    /// Push a line that has already been laid out at the correct width, skipping
    /// word wrapping.
    ///
    /// Table lines are pre-formatted with exact column widths and separators.
    /// Passing them through `word_wrap_line` would break the layout at
    /// arbitrary positions. This method prepends the indent/blockquote prefix
    /// and pushes directly to `self.text`.
    fn is_blockquote_active(&self) -> bool {
        self.indent_stack
            .iter()
            .any(|ctx| ctx.prefix.iter().any(|p| p.content.contains('>')))
    }

    fn push_prewrapped_line(&mut self, mut line: HyperlinkLine, pending_marker_line: bool) {
        self.flush_current_line();
        let blockquote_active = self.is_blockquote_active();
        let style = if blockquote_active {
            self.styles.blockquote.patch(line.line.style)
        } else {
            line.line.style
        };

        let mut spans = self.prefix_spans(pending_marker_line);
        let shift = spans.iter().map(|span| span.content.width()).sum::<usize>();
        spans.append(&mut line.line.spans);
        for hyperlink in &mut line.hyperlinks {
            hyperlink.columns = hyperlink.columns.start + shift..hyperlink.columns.end + shift;
        }
        line.line = Line::from(spans);
        self.push_output_line(line.style(style));
    }

    fn push_line(&mut self, line: Line<'static>) {
        self.flush_current_line();
        let blockquote_active = self.is_blockquote_active();
        let style = if blockquote_active {
            self.styles.blockquote
        } else {
            line.style
        };
        let was_pending = self.pending_marker_line;

        self.current_initial_indent = self.prefix_spans(was_pending);
        self.current_subsequent_indent = self.prefix_spans(/*pending_marker_line*/ false);
        self.current_line_style = style;
        self.current_line_content = Some(HyperlinkLine::new(line));
        self.current_line_in_code_block = self.in_code_block;
        self.line_ends_with_local_link_target = false;

        self.pending_marker_line = false;
    }

    fn push_hyperlink_line(&mut self, line: HyperlinkLine) {
        let hyperlinks = line.hyperlinks;
        self.push_line(line.line);
        if let Some(current) = self.current_line_content.as_mut() {
            current.hyperlinks = hyperlinks;
        }
    }

    fn push_span(&mut self, span: Span<'static>) {
        if let Some(line) = self.current_line_content.as_mut() {
            line.line.push_span(span);
        } else {
            self.push_line(Line::from(vec![span]));
        }
    }

    fn push_annotated(&mut self, mut appended: HyperlinkLine) {
        if self.current_line_content.is_none() {
            self.push_line(Line::default());
        }
        if let Some(line) = self.current_line_content.as_mut() {
            let shift = line.width();
            line.line.spans.append(&mut appended.line.spans);
            line.hyperlinks
                .extend(appended.hyperlinks.into_iter().map(|mut link| {
                    link.columns = link.columns.start + shift..link.columns.end + shift;
                    link
                }));
        }
    }

    fn push_text_spans(&mut self, text: &str, style: Style) {
        let span = Span::styled(text.to_string(), style);
        let destination = self
            .link
            .as_ref()
            .and_then(|link| web_destination(&link.destination));
        let annotated = if let Some(destination) = destination {
            let mut annotated = HyperlinkLine::new(Line::default());
            annotated.push_span(span, Some(&destination));
            annotated
        } else if self.link.is_some() || self.in_code_block {
            HyperlinkLine::new(Line::from(span))
        } else {
            annotate_web_urls_in_line(Line::from(span))
        };
        self.push_annotated(annotated);
    }

    fn push_blank_line(&mut self) {
        self.flush_current_line();
        if self.indent_stack.iter().all(|ctx| ctx.is_list) {
            self.push_output_line(HyperlinkLine::new(Line::default()));
        } else {
            self.push_line(Line::default());
            self.flush_current_line();
        }
    }

    fn push_output_line(&mut self, line: HyperlinkLine) {
        self.text.push(line);
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

    let decoded_path_text =
        urlencoding::decode(path_text).unwrap_or(std::borrow::Cow::Borrowed(path_text));
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

    #[test]
    fn wrap_cell_preserves_hard_break_lines() {
        let mut cell = TableCell::default();
        cell.push_span("first line".into());
        cell.hard_break();
        cell.push_span("second line".into());

        let writer = W::new("", std::iter::empty(), Some(80), /*cwd*/ None);
        let wrapped = writer.wrap_cell(&cell, /*width*/ 40);
        let rendered = wrapped
            .iter()
            .map(|line| {
                line.line
                    .spans
                    .iter()
                    .map(|span| span.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered,
            vec!["first line".to_string(), "second line".to_string()]
        );
    }

    // ---------------------------------------------------------------
    // Type alias for calling private associated functions on Writer.
    // ---------------------------------------------------------------
    type W<'a> = Writer<'a, std::iter::Empty<(Event<'a>, Range<usize>)>>;

    /// Build a single-line `TableCell` from plain text.
    fn make_cell(text: &str) -> TableCell {
        let mut cell = TableCell::default();
        cell.push_span(Span::raw(text.to_string()));
        cell
    }

    fn make_body_row(cells: Vec<TableCell>, has_table_pipe_syntax: bool) -> TableBodyRow {
        TableBodyRow {
            cells,
            has_table_pipe_syntax,
        }
    }

    // ===== Column-metrics unit tests =====

    #[test]
    fn column_classification_narrative_by_word_count() {
        // Col 0: short tokens (1-2 words each) -> Compact
        // Col 1: prose (≥4 words per cell) → Narrative
        let header = vec![make_cell("ID"), make_cell("Description")];
        let rows = vec![
            vec![make_cell("1"), make_cell("a long description of the item")],
            vec![make_cell("2"), make_cell("another verbose body cell here")],
        ];
        let metrics = W::collect_table_column_metrics(&header, &rows, /*column_count*/ 2);
        assert_eq!(metrics[0].kind, TableColumnKind::Compact);
        assert_eq!(metrics[1].kind, TableColumnKind::Narrative);
    }

    #[test]
    fn column_classification_token_heavy_by_url_like_tokens() {
        let header = vec![make_cell("URL")];
        let rows = vec![
            vec![make_cell("https://example.com/very/long/path")],
            vec![make_cell("https://another.example.org/deep")],
        ];
        let metrics = W::collect_table_column_metrics(&header, &rows, /*column_count*/ 1);
        assert_eq!(metrics[0].kind, TableColumnKind::TokenHeavy);
    }

    #[test]
    fn column_classification_token_heavy_for_local_path_lists() {
        let header = vec![make_cell("Files")];
        let rows = vec![
            vec![make_cell(
                "codex-rs/core/src/next_prompt_suggestion.rs:1, codex-rs/core/src/next_prompt_suggestion_tests.rs:1",
            )],
            vec![make_cell(
                "codex-rs/core/src/context/next_prompt_suggestion.rs:1, codex-rs/core/src/context/contextual_user_message_tests.rs:1",
            )],
        ];
        let metrics = W::collect_table_column_metrics(&header, &rows, /*column_count*/ 1);
        assert_eq!(metrics[0].kind, TableColumnKind::TokenHeavy);
    }

    #[test]
    fn column_classification_compact_all_short() {
        // Both columns short tokens -> both Compact
        let header = vec![make_cell("Status"), make_cell("Count")];
        let rows = vec![
            vec![make_cell("ok"), make_cell("42")],
            vec![make_cell("err"), make_cell("7")],
        ];
        let metrics = W::collect_table_column_metrics(&header, &rows, /*column_count*/ 2);
        assert_eq!(metrics[0].kind, TableColumnKind::Compact);
        assert_eq!(metrics[1].kind, TableColumnKind::Compact);
    }

    #[test]
    fn preferred_floor_narrative_retains_readable_width() {
        let m = TableColumnMetrics {
            max_width: 40,
            header_token_width: 15,
            body_token_width: 8,
            kind: TableColumnKind::Narrative,
        };
        assert_eq!(W::preferred_column_floor(&m, /*min_column_width*/ 3), 16);

        let m2 = TableColumnMetrics {
            max_width: 12,
            header_token_width: 6,
            body_token_width: 8,
            kind: TableColumnKind::Narrative,
        };
        assert_eq!(W::preferred_column_floor(&m2, /*min_column_width*/ 3), 12);
    }

    #[test]
    fn preferred_floor_token_heavy_retains_readable_width() {
        let m = TableColumnMetrics {
            max_width: 80,
            header_token_width: 5,
            body_token_width: 60,
            kind: TableColumnKind::TokenHeavy,
        };
        assert_eq!(W::preferred_column_floor(&m, /*min_column_width*/ 3), 16);
    }

    #[test]
    fn preferred_floor_compact_uses_body_token() {
        // Compact: max(header_token_width, body_token_width.min(16))
        let m = TableColumnMetrics {
            max_width: 30,
            header_token_width: 5,
            body_token_width: 12,
            kind: TableColumnKind::Compact,
        };
        // max(5, min(12, 16)) = max(5, 12) = 12
        assert_eq!(W::preferred_column_floor(&m, /*min_column_width*/ 3), 12);

        // Body token exceeds 16 cap → capped at 16, then max with header
        let m2 = TableColumnMetrics {
            max_width: 30,
            header_token_width: 5,
            body_token_width: 20,
            kind: TableColumnKind::Compact,
        };
        // max(5, min(20, 16)) = max(5, 16) = 16
        assert_eq!(W::preferred_column_floor(&m2, /*min_column_width*/ 3), 16);
    }

    #[test]
    fn next_column_to_shrink_prefers_token_heavy_then_narrative() {
        let widths = [20usize, 20, 20];
        let floors = [8usize, 8, 8];
        let metrics = [
            TableColumnMetrics {
                max_width: 30,
                header_token_width: 8,
                body_token_width: 6,
                kind: TableColumnKind::Narrative,
            },
            TableColumnMetrics {
                max_width: 30,
                header_token_width: 8,
                body_token_width: 28,
                kind: TableColumnKind::TokenHeavy,
            },
            TableColumnMetrics {
                max_width: 30,
                header_token_width: 8,
                body_token_width: 6,
                kind: TableColumnKind::Compact,
            },
        ];
        let idx = W::next_column_to_shrink(&widths, &floors, &metrics);
        assert_eq!(idx, Some(1), "token-heavy column should shrink first");

        let widths = [20usize, 8, 20];
        let idx = W::next_column_to_shrink(&widths, &floors, &metrics);
        assert_eq!(
            idx,
            Some(0),
            "narrative column should shrink before compact"
        );
    }

    // ===== Spillover-detection unit tests =====

    #[test]
    fn spillover_detects_single_cell_row() {
        let row = make_body_row(
            vec![make_cell("some trailing text")],
            /*has_table_pipe_syntax*/ false,
        );
        assert!(W::is_spillover_row(&row, /*next_row*/ None));
    }

    #[test]
    fn spillover_keeps_single_cell_row_with_table_pipe_syntax() {
        let row = make_body_row(
            vec![make_cell("some sparse value")],
            /*has_table_pipe_syntax*/ true,
        );
        assert!(!W::is_spillover_row(&row, /*next_row*/ None));
    }

    #[test]
    fn spillover_detects_html_content() {
        // 3-cell row where only cell 0 has HTML content
        let row = make_body_row(
            vec![
                make_cell("<div>content</div>"),
                make_cell(""),
                make_cell(""),
            ],
            /*has_table_pipe_syntax*/ false,
        );
        assert!(W::is_spillover_row(&row, /*next_row*/ None));
    }

    #[test]
    fn spillover_detects_label_followed_by_html() {
        // cell 0 = "HTML block:" and next_row cell 0 = "<div>x</div>"
        let row = make_body_row(
            vec![make_cell("HTML block:"), make_cell(""), make_cell("")],
            /*has_table_pipe_syntax*/ false,
        );
        let next = make_body_row(
            vec![make_cell("<div>x</div>"), make_cell(""), make_cell("")],
            /*has_table_pipe_syntax*/ false,
        );
        assert!(W::is_spillover_row(&row, Some(&next)));
    }

    #[test]
    fn spillover_detects_trailing_html_label() {
        // "HTML block:" with no next_row → trailing HTML label spillover
        let row = make_body_row(
            vec![make_cell("HTML block:"), make_cell(""), make_cell("")],
            /*has_table_pipe_syntax*/ false,
        );
        assert!(W::is_spillover_row(&row, /*next_row*/ None));
    }

    #[test]
    fn spillover_keeps_normal_multi_cell_row() {
        // 3 cells all non-empty → not spillover
        let row = make_body_row(
            vec![make_cell("one"), make_cell("two"), make_cell("three")],
            /*has_table_pipe_syntax*/ true,
        );
        assert!(!W::is_spillover_row(&row, /*next_row*/ None));
    }

    #[test]
    fn spillover_keeps_label_when_next_is_not_html() {
        // cell 0 = "Status:" and next_row cell 0 = "ok" → not spillover (not HTML)
        let row = make_body_row(
            vec![make_cell("Status:"), make_cell(""), make_cell("")],
            /*has_table_pipe_syntax*/ true,
        );
        let next = make_body_row(
            vec![make_cell("ok"), make_cell(""), make_cell("")],
            /*has_table_pipe_syntax*/ true,
        );
        assert!(!W::is_spillover_row(&row, Some(&next)));
    }

    #[test]
    fn annotates_explicit_web_link_label_and_visible_destination() {
        let lines = render_markdown_lines_with_width_and_cwd(
            "See [docs](https://example.com/reference).",
            /*width*/ Some(80),
            /*cwd*/ None,
        );
        let links = lines
            .iter()
            .flat_map(|line| line.hyperlinks.iter())
            .collect::<Vec<_>>();

        assert_eq!(links.len(), 2);
        assert!(
            links
                .iter()
                .all(|link| link.destination == "https://example.com/reference")
        );
    }

    #[test]
    fn wrapped_table_url_fragments_keep_complete_web_destination() {
        let destination = "https://example.com/a/very/long/path/to/a/table/artifact";
        let markdown = format!("| Item | URL |\n| --- | --- |\n| report | {destination} |\n");
        let lines = render_markdown_lines_with_width_and_cwd(
            &markdown,
            /*width*/ Some(32),
            /*cwd*/ None,
        );
        let linked_rows = lines
            .iter()
            .filter(|line| !line.hyperlinks.is_empty())
            .collect::<Vec<_>>();

        assert!(
            linked_rows.len() > 1,
            "expected a URL wrapped across table rows"
        );
        assert!(linked_rows.iter().all(|line| {
            line.hyperlinks
                .iter()
                .all(|link| link.destination == destination)
        }));
    }

    #[test]
    fn key_value_table_keeps_web_annotations() {
        let destination = "https://example.com/a/very/long/path";
        let markdown = format!(
            "| c1 | c2 | c3 | c4 | c5 | c6 |\n| --- | --- | --- | --- | --- | --- |\n| {destination} | 2 | 3 | 4 | 5 | 6 |\n"
        );
        let lines = render_markdown_lines_with_width_and_cwd(
            &markdown,
            /*width*/ Some(20),
            /*cwd*/ None,
        );
        let destinations = lines
            .iter()
            .flat_map(|line| line.hyperlinks.iter().map(|link| link.destination.as_str()))
            .collect::<Vec<_>>();

        assert!(!destinations.is_empty());
        assert!(destinations.iter().all(|link| *link == destination));
    }

    #[test]
    fn does_not_annotate_code_or_non_web_markdown_links() {
        let markdown = "`https://example.com/inline`\n\n```text\nhttps://example.com/block\n```\n\n[mail](mailto:test@example.com)\n\n[https://example.com/label](mailto:test@example.com)\n\n| Target |\n| --- |\n| [https://example.com/table-label](mailto:test@example.com) |";
        let lines = render_markdown_lines_with_width_and_cwd(
            markdown,
            /*width*/ Some(80),
            /*cwd*/ None,
        );

        assert!(lines.iter().all(|line| line.hyperlinks.is_empty()));
    }

    #[test]
    fn pipe_table_fallback_keeps_web_annotations() {
        let destination = "https://example.com/a/long/path";
        let target = "https://target.example/path";
        let code_url = "https://code.example/not-a-link";
        let markdown = format!(
            "| URL | Code | Label |\n| --- | --- | --- |\n| {destination} | `{code_url}` | [https://shown.example]({target}) |\n"
        );
        let lines = render_markdown_lines_with_width_and_cwd(
            &markdown,
            /*width*/ Some(5),
            /*cwd*/ None,
        );
        let destinations = lines
            .iter()
            .flat_map(|line| line.hyperlinks.iter().map(|link| link.destination.as_str()))
            .collect::<Vec<_>>();

        assert!(destinations.contains(&destination));
        assert!(destinations.contains(&target));
        assert!(!destinations.contains(&code_url));
        assert!(!destinations.contains(&"https://shown.example"));
    }
}
