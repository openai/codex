use crate::render::line_utils::line_to_static;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
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
use unicode_width::UnicodeWidthStr;

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

/// Styled content of a single cell in the table being parsed.
///
/// A cell can contain multiple lines (hard breaks inside the cell) and rich inline spans (bold,
/// code, links).  The `plain_text()` projection is used for column-width measurement; the styled
/// `lines` are used for final rendering.
#[derive(Clone, Debug, Default)]
struct TableCell {
    lines: Vec<Line<'static>>,
}

impl TableCell {
    fn ensure_line(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(Line::default());
        }
    }

    fn push_span(&mut self, span: Span<'static>) {
        self.ensure_line();
        if let Some(line) = self.lines.last_mut() {
            line.push_span(span);
        }
    }

    fn hard_break(&mut self) {
        self.lines.push(Line::default());
    }

    fn plain_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Accumulates pulldown-cmark table events into a structured representation.
///
/// `TableState` is created on `Tag::Table` and consumed on `TagEnd::Table`. Between those events,
/// the Writer delegates cell content (text, code, html, breaks) into the `current_cell`, which is
/// flushed into `current_row` on `TagEnd::TableCell`, then into `header`/`rows` on row/head end
/// events.
#[derive(Debug)]
struct TableState {
    alignments: Vec<Alignment>,
    header: Option<Vec<TableCell>>,
    rows: Vec<Vec<TableCell>>,
    current_row: Option<Vec<TableCell>>,
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
            current_cell: None,
            in_header: false,
        }
    }
}

/// Classification of a table column for width-allocation priority.
///
/// Narrative columns (long prose, many words per cell) are shrunk first when the table exceeds
/// available width.  Structured columns (short tokens like dates, status words, numbers) are
/// preserved as long as possible to keep their content on a single line.
///
/// The heuristic is simple: >= 4 average words per cell OR >= 28 average character width →
/// Narrative. Everything else → Structured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableColumnKind {
    /// Long-form prose content (>= 4 avg words/cell or >= 28 avg char width).
    Narrative,
    /// Short, token-like content that should resist wrapping.
    Structured,
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
    /// Average number of whitespace-delimited words per non-empty body cell.
    avg_words_per_cell: f64,
    /// Average display width of non-empty body cells.
    avg_cell_width: f64,
    /// Classification derived from `avg_words_per_cell` and `avg_cell_width`.
    kind: TableColumnKind,
}

pub fn render_markdown_text(input: &str) -> Text<'static> {
    render_markdown_text_with_width(input, None)
}

pub(crate) fn render_markdown_text_with_width(input: &str, width: Option<usize>) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
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
    table_state: Option<TableState>,
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
            table_state: None,
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
            Tag::Table(alignments) => self.start_table(alignments),
            Tag::TableHead => self.start_table_head(),
            Tag::TableRow => self.start_table_row(),
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
        self.indent_stack
            .push(IndentContext::new(vec![Span::from("> ")], None, false));
    }

    fn end_blockquote(&mut self) {
        if self.in_table_cell() {
            return;
        }
        self.indent_stack.pop();
        self.needs_newline = true;
    }

    fn text(&mut self, text: CowStr<'a>) {
        if self.in_table_cell() {
            self.push_text_to_table_cell(&text);
            return;
        }

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
        if self.in_table_cell() {
            self.push_table_cell_hard_break();
            return;
        }
        self.push_line(Line::default());
    }

    fn soft_break(&mut self) {
        if self.in_table_cell() {
            let style = self.inline_styles.last().copied().unwrap_or_default();
            self.push_span_to_table_cell(Span::styled(" ".to_string(), style));
            return;
        }
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

        let table_lines = self.render_table_lines(table_state);
        let mut pending_marker_line = self.pending_marker_line;
        for line in table_lines {
            self.push_prewrapped_line(line, pending_marker_line);
            pending_marker_line = false;
        }
        self.pending_marker_line = false;
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

    fn start_table_row(&mut self) {
        if let Some(table_state) = self.table_state.as_mut() {
            table_state.current_row = Some(Vec::new());
        }
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
            table_state.rows.push(row);
        }
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
            self.push_span_to_table_cell(Span::styled(line.to_string(), style));
        }
    }

    /// Convert a completed `TableState` into styled `Line`s with Unicode
    /// box-drawing borders.
    ///
    /// Pipeline: filter spillover rows -> normalize column counts -> compute
    /// column widths -> render box grid (or fall back to pipe format if the
    /// minimum column widths exceed available terminal width). Spillover rows
    /// are appended as plain text after the table grid.
    ///
    /// Falls back to `render_table_pipe_fallback` (raw `| A | B |` format)
    /// when `compute_column_widths` returns `None` (terminal too narrow for
    /// even 3-char-wide columns).
    fn render_table_lines(&self, mut table_state: TableState) -> Vec<Line<'static>> {
        let column_count = table_state.alignments.len();
        if column_count == 0 {
            return Vec::new();
        }

        let mut spillover_rows: Vec<TableCell> = Vec::new();
        let mut rows: Vec<Vec<TableCell>> = Vec::new();
        for (row_idx, row) in table_state.rows.iter().enumerate() {
            let next_row = table_state.rows.get(row_idx + 1);
            // pulldown-cmark accepts body rows without pipes, which can turn a following paragraph
            // into a one-cell table row. For multi-column tables, treat those as spillover text
            // rendered after the table.
            if column_count > 1 && Self::is_spillover_row(row, next_row) {
                if let Some(cell) = row.first().cloned() {
                    spillover_rows.push(cell);
                }
            } else {
                rows.push(row.clone());
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

        let available_width = self.available_table_width(column_count);
        let widths =
            self.compute_column_widths(&header, &rows, &table_state.alignments, available_width);

        let Some(column_widths) = widths else {
            let mut fallback =
                self.render_table_pipe_fallback(&header, &rows, &table_state.alignments);
            for spillover in spillover_rows {
                fallback.extend(spillover.lines);
            }
            return fallback;
        };

        let border_style = Style::new().dim();
        let mut out = Vec::new();
        out.push(self.render_border_line('┌', '┬', '┐', &column_widths, border_style));
        out.extend(self.render_table_row(
            &header,
            &column_widths,
            &table_state.alignments,
            border_style,
        ));
        out.push(self.render_border_line('├', '┼', '┤', &column_widths, border_style));
        for row in &rows {
            out.extend(self.render_table_row(
                row,
                &column_widths,
                &table_state.alignments,
                border_style,
            ));
        }
        out.push(self.render_border_line('└', '┴', '┘', &column_widths, border_style));
        for spillover in spillover_rows {
            out.extend(spillover.lines);
        }
        out
    }

    fn normalize_row(row: &mut Vec<TableCell>, column_count: usize) {
        if row.len() > column_count {
            row.truncate(column_count);
        }
        if row.len() < column_count {
            row.resize(column_count, TableCell::default());
        }
    }

    /// subtracts the space eaten by border characters
    fn available_table_width(&self, column_count: usize) -> Option<usize> {
        self.wrap_width.map(|wrap_width| {
            let prefix_width =
                Self::spans_display_width(&self.prefix_spans(self.pending_marker_line));
            let reserved = prefix_width + 1 + (column_count * 3);
            wrap_width.saturating_sub(reserved)
        })
    }

    /// Allocate column widths for box-drawing table rendering.
    ///
    /// Each column starts at its natural (max cell content) width, then columns
    /// are iteratively shrunk one character at a time until the total fits within
    /// `available_width`. Narrative columns (long prose) are shrunk before
    /// Structured columns (short tokens). Returns `None` when even the minimum
    /// width (3 chars per column) cannot fit.
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
            // Relax preferred floors (starting with narrative columns) until we can satisfy the
            // width budget. We still keep hard minimums.
            while floor_total > max_width {
                let Some((idx, _)) = floors
                    .iter()
                    .enumerate()
                    .filter(|(_, floor)| **floor > min_column_width)
                    .min_by_key(|(idx, floor)| {
                        let kind_priority = match metrics[*idx].kind {
                            TableColumnKind::Narrative => 0,
                            TableColumnKind::Structured => 1,
                        };
                        (kind_priority, *floor)
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
            let kind = if avg_words_per_cell >= 4.0 || avg_cell_width >= 28.0 {
                TableColumnKind::Narrative
            } else {
                TableColumnKind::Structured
            };

            metrics.push(TableColumnMetrics {
                max_width,
                header_token_width,
                body_token_width,
                avg_words_per_cell,
                avg_cell_width,
                kind,
            });
        }

        metrics
    }

    /// Compute the preferred minimum width for a column before the shrink loop
    /// starts reducing it further.
    ///
    /// Narrative columns floor at the header's longest token (capped at 10).
    /// Structured columns floor at the larger of the header and body token widths
    /// (body capped at 16). The result is clamped to `[min_column_width, max_width]`.
    fn preferred_column_floor(metrics: &TableColumnMetrics, min_column_width: usize) -> usize {
        let token_target = match metrics.kind {
            TableColumnKind::Narrative => metrics.header_token_width.min(10),
            TableColumnKind::Structured => metrics
                .header_token_width
                .max(metrics.body_token_width.min(16)),
        };
        token_target.max(min_column_width).min(metrics.max_width)
    }

    /// Pick the next column to shrink by one character during width allocation.
    ///
    /// Priority: Narrative columns are shrunk before Structured. Within the same
    /// kind, the column with the most slack above its floor is chosen. A guard
    /// cost is added when the width would fall below the header's longest token
    /// (to avoid truncating column headers).
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
                let kind_cost = match metrics[*idx].kind {
                    TableColumnKind::Narrative => 0i32,
                    TableColumnKind::Structured => 2i32,
                };
                let header_guard = if **width <= metrics[*idx].header_token_width {
                    3i32
                } else {
                    0i32
                };
                let density_guard = if metrics[*idx].avg_words_per_cell >= 4.0
                    || metrics[*idx].avg_cell_width >= 24.0
                {
                    0i32
                } else {
                    1i32
                };
                (
                    kind_cost + header_guard + density_guard,
                    usize::MAX.saturating_sub(slack),
                )
            })
            .map(|(idx, _)| idx)
    }

    fn render_border_line(
        &self,
        left: char,
        sep: char,
        right: char,
        column_widths: &[usize],
        style: Style,
    ) -> Line<'static> {
        let mut spans = Vec::with_capacity(column_widths.len() * 2 + 1);
        spans.push(Span::styled(left.to_string(), style));
        for (idx, width) in column_widths.iter().enumerate() {
            spans.push(Span::styled("─".repeat(*width + 2), style));
            if idx + 1 == column_widths.len() {
                spans.push(Span::styled(right.to_string(), style));
            } else {
                spans.push(Span::styled(sep.to_string(), style));
            }
        }
        Line::from(spans)
    }

    fn render_table_row(
        &self,
        row: &[TableCell],
        column_widths: &[usize],
        alignments: &[Alignment],
        border_style: Style,
    ) -> Vec<Line<'static>> {
        let wrapped_cells: Vec<Vec<Line<'static>>> = row
            .iter()
            .zip(column_widths)
            .map(|(cell, width)| self.wrap_cell(cell, *width))
            .collect();
        let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1).max(1);

        let mut out = Vec::with_capacity(row_height);
        for row_line in 0..row_height {
            let mut spans = Vec::new();
            spans.push(Span::styled("│".to_string(), border_style));
            for (column, width) in column_widths.iter().enumerate() {
                spans.push(Span::raw(" "));
                let line = wrapped_cells[column]
                    .get(row_line)
                    .cloned()
                    .unwrap_or_default();
                let line_width = Self::line_display_width(&line);
                let remaining = width.saturating_sub(line_width);
                let (left_padding, right_padding) = match alignments[column] {
                    Alignment::Left | Alignment::None => (0, remaining),
                    Alignment::Center => (remaining / 2, remaining - (remaining / 2)),
                    Alignment::Right => (remaining, 0),
                };
                if left_padding > 0 {
                    spans.push(Span::raw(" ".repeat(left_padding)));
                }
                spans.extend(line.spans);
                if right_padding > 0 {
                    spans.push(Span::raw(" ".repeat(right_padding)));
                }
                spans.push(Span::raw(" "));
                spans.push(Span::styled("│".to_string(), border_style));
            }
            out.push(Line::from(spans));
        }
        out
    }

    fn render_table_pipe_fallback(
        &self,
        header: &[TableCell],
        rows: &[Vec<TableCell>],
        alignments: &[Alignment],
    ) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        out.push(Line::from(Self::row_to_pipe_string(header)));
        out.push(Line::from(Self::alignments_to_pipe_delimiter(alignments)));
        out.extend(
            rows.iter()
                .map(|row| Line::from(Self::row_to_pipe_string(row))),
        );
        out
    }

    fn row_to_pipe_string(row: &[TableCell]) -> String {
        let mut out = String::new();
        out.push('|');
        for cell in row {
            out.push(' ');
            out.push_str(&cell.plain_text());
            out.push(' ');
            out.push('|');
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

    fn wrap_cell(&self, cell: &TableCell, width: usize) -> Vec<Line<'static>> {
        if cell.lines.is_empty() {
            return vec![Line::default()];
        }
        let mut wrapped = Vec::new();
        for source_line in &cell.lines {
            let rendered = word_wrap_line(source_line, RtOptions::new(width.max(1)));
            if rendered.is_empty() {
                wrapped.push(Line::default());
            } else {
                push_owned_lines(&rendered, &mut wrapped);
            };
        }
        if wrapped.is_empty() {
            wrapped.push(Line::default());
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
    /// AND (the row has only one cell, or the content looks like HTML, or it's a
    /// label line followed by HTML content).
    fn is_spillover_row(row: &[TableCell], next_row: Option<&Vec<TableCell>>) -> bool {
        let Some(first_text) = Self::first_non_empty_only_text(row) else {
            return false;
        };

        if row.len() == 1 {
            return true;
        }

        if Self::looks_like_html_content(&first_text) {
            return true;
        }

        // Keep common intro + html-block spillover together:
        // "HTML block:" followed by "<div ...>".
        first_text.trim_end().ends_with(':')
            && next_row
                .and_then(|row| Self::first_non_empty_only_text(row))
                .is_some_and(|text| Self::looks_like_html_content(&text))
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
        text.contains('<') && text.contains('>')
    }

    fn spans_display_width(spans: &[Span<'_>]) -> usize {
        spans.iter().map(|span| span.content.width()).sum()
    }

    fn line_display_width(line: &Line<'_>) -> usize {
        line.spans.iter().map(|span| span.content.width()).sum()
    }

    fn cell_display_width(cell: &TableCell) -> usize {
        cell.lines
            .iter()
            .map(Self::line_display_width)
            .max()
            .unwrap_or(0)
    }

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

    /// Push a line that has already been laid out at the correct width, skipping
    /// word wrapping.
    ///
    /// Table lines are pre-formatted with exact column widths and box-drawing
    /// borders. Passing them through `word_wrap_line` would break the grid at
    /// arbitrary positions. This method prepends the indent/blockquote prefix
    /// and pushes directly to `self.text.lines`.
    fn push_prewrapped_line(&mut self, line: Line<'static>, pending_marker_line: bool) {
        self.flush_current_line();
        let blockquote_active = self
            .indent_stack
            .iter()
            .any(|ctx| ctx.prefix.iter().any(|p| p.content.contains('>')));
        let style = if blockquote_active {
            self.styles.blockquote.patch(line.style)
        } else {
            line.style
        };

        let mut spans = self.prefix_spans(pending_marker_line);
        spans.extend(line.spans);
        self.text.lines.push(Line::from(spans).style(style));
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
}
