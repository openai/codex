//! Semantic terminal hyperlinks carried separately from visible TUI text.
//!
//! Layout code measures and wraps ordinary ratatui lines. Hyperlink annotations are applied only
//! when text reaches a terminal buffer or scrollback writer so OSC 8 bytes never affect geometry.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;
use url::Url;

use crate::render::line_utils::line_to_borrowed;
use crate::render::line_utils::line_to_static;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalHyperlink {
    pub(crate) columns: Range<usize>,
    pub(crate) destination: String,
    destination_kind: DestinationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DestinationKind {
    Web,
    TrustedFile,
}

impl TerminalHyperlink {
    pub(crate) fn web(columns: Range<usize>, destination: String) -> Self {
        Self {
            columns,
            destination,
            destination_kind: DestinationKind::Web,
        }
    }

    pub(crate) fn retarget_to_trusted_file(&mut self, destination: &Url) {
        // Keep file URLs out of the general Markdown link path. Only generated visualization links
        // are promoted to this destination kind.
        debug_assert_eq!(destination.scheme(), "file");
        self.destination = destination.to_string();
        self.destination_kind = DestinationKind::TrustedFile;
    }

    fn with_columns(&self, columns: Range<usize>) -> Self {
        Self {
            columns,
            destination: self.destination.clone(),
            destination_kind: self.destination_kind,
        }
    }

    fn terminal_destination(&self) -> Option<String> {
        match self.destination_kind {
            DestinationKind::Web => web_destination(&self.destination),
            DestinationKind::TrustedFile => trusted_file_destination(&self.destination),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HyperlinkLine {
    pub(crate) line: Line<'static>,
    pub(crate) hyperlinks: Vec<TerminalHyperlink>,
}

impl HyperlinkLine {
    pub(crate) fn new(line: Line<'static>) -> Self {
        Self {
            line,
            hyperlinks: Vec::new(),
        }
    }

    pub(crate) fn width(&self) -> usize {
        self.line.width()
    }

    pub(crate) fn push_span(&mut self, span: Span<'static>, destination: Option<&str>) {
        let start = self.width();
        let end = start + span.content.width();
        self.line.push_span(span);
        if end > start
            && let Some(destination) = destination.and_then(web_destination)
        {
            self.hyperlinks
                .push(TerminalHyperlink::web(start..end, destination));
        }
    }

    pub(crate) fn style(mut self, style: ratatui::style::Style) -> Self {
        self.line = self.line.style(style);
        self
    }
}

impl From<Line<'static>> for HyperlinkLine {
    fn from(line: Line<'static>) -> Self {
        Self::new(line)
    }
}

impl From<&'static str> for HyperlinkLine {
    fn from(text: &'static str) -> Self {
        Self::new(Line::from(text))
    }
}

impl From<String> for HyperlinkLine {
    fn from(text: String) -> Self {
        Self::new(Line::from(text))
    }
}

pub(crate) fn visible_lines(lines: Vec<HyperlinkLine>) -> Vec<Line<'static>> {
    lines.into_iter().map(|line| line.line).collect()
}

pub(crate) fn visible_lines_ref(lines: &[HyperlinkLine]) -> Vec<Line<'_>> {
    lines
        .iter()
        .map(|line| line_to_borrowed(&line.line))
        .collect()
}

pub(crate) fn plain_hyperlink_lines(lines: Vec<Line<'static>>) -> Vec<HyperlinkLine> {
    lines.into_iter().map(HyperlinkLine::new).collect()
}

pub(crate) fn prefix_hyperlink_lines(
    lines: Vec<HyperlinkLine>,
    initial_prefix: Span<'static>,
    subsequent_prefix: Span<'static>,
) -> Vec<HyperlinkLine> {
    lines
        .into_iter()
        .enumerate()
        .map(|(index, mut line)| {
            let prefix = if index == 0 {
                initial_prefix.clone()
            } else {
                subsequent_prefix.clone()
            };
            let shift = prefix.content.width();
            let mut spans = Vec::with_capacity(line.line.spans.len() + 1);
            spans.push(prefix);
            spans.extend(line.line.spans);
            line.line = Line::from(spans).style(line.line.style);
            for hyperlink in &mut line.hyperlinks {
                hyperlink.columns = hyperlink.columns.start + shift..hyperlink.columns.end + shift;
            }
            line
        })
        .collect()
}

pub(crate) fn adaptive_wrap_hyperlink_lines(
    lines: &[HyperlinkLine],
    options: RtOptions<'static>,
) -> Vec<HyperlinkLine> {
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let options = if index == 0 {
            options.clone()
        } else {
            options
                .clone()
                .initial_indent(options.subsequent_indent.clone())
        };
        out.extend(remap_wrapped_line(
            line,
            adaptive_wrap_line(&line.line, options)
                .into_iter()
                .map(|wrapped| line_to_static(&wrapped))
                .collect(),
        ));
    }
    out
}

pub(crate) fn annotate_web_urls(lines: Vec<Line<'static>>) -> Vec<HyperlinkLine> {
    lines.into_iter().map(annotate_web_urls_in_line).collect()
}

pub(crate) fn annotate_web_urls_in_line(line: Line<'static>) -> HyperlinkLine {
    let text = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let mut out = HyperlinkLine::new(line);
    out.hyperlinks = web_links_in_text(&text);
    out
}

/// Re-attach source hyperlink ranges after visible-text wrapping has split a line.
///
/// Link text is matched in display order so a URL split across table rows retains the complete
/// destination on every rendered fragment. Whitespace inserted or removed at line boundaries is
/// ignored while matching; hyperlink destinations themselves are never reconstructed from output.
pub(crate) fn remap_wrapped_line(
    source: &HyperlinkLine,
    wrapped: Vec<Line<'static>>,
) -> Vec<HyperlinkLine> {
    let mut out = plain_hyperlink_lines(wrapped);
    if source.hyperlinks.is_empty() {
        return out;
    }

    let source_text = line_text(&source.line);
    let mut source_byte = 0usize;
    let mut source_column = 0usize;
    let mut link_index = 0usize;
    for (index, line) in out.iter_mut().enumerate() {
        if index > 0 {
            let trimmed = source_text[source_byte..].trim_start_matches(char::is_whitespace);
            let skipped = source_text[source_byte..].len() - trimmed.len();
            source_column += source_text[source_byte..source_byte + skipped].width();
            source_byte += skipped;
        }

        let rendered = line_text(&line.line);
        let remaining = &source_text[source_byte..];
        let Some(rendered_start) = longest_suffix_matching_prefix(&rendered, remaining) else {
            continue;
        };
        let mapped = &rendered[rendered_start..];
        let mut output_column = rendered[..rendered_start].width();
        for ch in mapped.chars() {
            let width = ch.width().unwrap_or(/*default*/ 0);
            while source
                .hyperlinks
                .get(link_index)
                .is_some_and(|link| link.columns.end <= source_column)
            {
                link_index += 1;
            }
            if let Some(link) = source
                .hyperlinks
                .get(link_index)
                .filter(|link| link.columns.contains(&source_column))
            {
                push_link_range(line, output_column..output_column + width, link);
            }
            source_column += width;
            output_column += width;
        }
        source_byte += mapped.len();
    }
    out
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn longest_suffix_matching_prefix(rendered: &str, source: &str) -> Option<usize> {
    rendered
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(rendered.len()))
        .find(|index| source.starts_with(&rendered[*index..]) && *index < rendered.len())
}

fn push_link_range(line: &mut HyperlinkLine, range: Range<usize>, link: &TerminalHyperlink) {
    if range.is_empty() {
        return;
    }
    if let Some(previous) = line.hyperlinks.last_mut()
        && previous.destination == link.destination
        && previous.destination_kind == link.destination_kind
        && previous.columns.end == range.start
    {
        previous.columns.end = range.end;
        return;
    }
    line.hyperlinks.push(link.with_columns(range));
}

pub(crate) fn web_links_in_text(text: &str) -> Vec<TerminalHyperlink> {
    let mut links = Vec::new();
    let mut search_from = 0usize;
    for raw_token in text.split_ascii_whitespace() {
        let Some(relative_start) = text[search_from..].find(raw_token) else {
            continue;
        };
        let raw_start = search_from + relative_start;
        search_from = raw_start + raw_token.len();
        let trimmed_start = raw_token
            .find(|ch: char| !is_leading_punctuation(ch))
            .unwrap_or(raw_token.len());
        let trimmed_end = trailing_url_end(&raw_token[trimmed_start..]) + trimmed_start;
        if trimmed_start >= trimmed_end {
            continue;
        }
        let candidate = &raw_token[trimmed_start..trimmed_end];
        let Some(destination) = web_destination(candidate) else {
            continue;
        };
        let start = text[..raw_start + trimmed_start].width();
        let end = start + candidate.width();
        links.push(TerminalHyperlink::web(start..end, destination));
    }
    links
}

fn is_leading_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | '.' | ';' | '!' | '\'' | '"'
    )
}

fn trailing_url_end(candidate: &str) -> usize {
    let mut end = candidate.len();
    while end > 0 {
        let remaining = &candidate[..end];
        let Some(ch) = remaining.chars().next_back() else {
            break;
        };
        let trim = matches!(ch, ',' | '.' | ';' | '!' | '\'' | '"')
            || matches!(ch, ')' | ']' | '}' | '>')
                && has_unmatched_closing_delimiter(remaining, ch);
        if !trim {
            break;
        }
        end -= ch.len_utf8();
    }
    end
}

fn has_unmatched_closing_delimiter(candidate: &str, closing: char) -> bool {
    let opening = match closing {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        '>' => '<',
        _ => return false,
    };
    candidate.chars().filter(|ch| *ch == closing).count()
        > candidate.chars().filter(|ch| *ch == opening).count()
}

pub(crate) fn web_destination(destination: &str) -> Option<String> {
    let safe_destination = sanitized_destination(destination);
    let parsed = Url::parse(&safe_destination).ok()?;
    matches!(parsed.scheme(), "http" | "https")
        .then(|| parsed.host_str())
        .flatten()?;
    Some(safe_destination)
}

fn trusted_file_destination(destination: &str) -> Option<String> {
    let safe_destination = sanitized_destination(destination);
    let parsed = Url::parse(&safe_destination).ok()?;
    (parsed.scheme() == "file" && parsed.to_file_path().is_ok()).then_some(safe_destination)
}

fn sanitized_destination(destination: &str) -> String {
    destination.chars().filter(|ch| !ch.is_control()).collect()
}

pub(crate) fn osc8_hyperlink(destination: &str, text: &str) -> String {
    let Some(safe_destination) = web_destination(destination) else {
        return text.to_string();
    };
    format!("\x1b]8;;{safe_destination}\x07{text}\x1b]8;;\x07")
}

#[cfg(test)]
pub(crate) fn strip_osc8(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut stripped = String::with_capacity(text.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index..].starts_with(b"\x1b]8;;") {
            index += 5;
            while index < bytes.len() {
                if bytes[index] == b'\x07' {
                    index += 1;
                    break;
                }
                if index + 1 < bytes.len() && bytes[index] == b'\x1b' && bytes[index + 1] == b'\\' {
                    index += 2;
                    break;
                }
                index += 1;
            }
            continue;
        }
        let ch = text[index..]
            .chars()
            .next()
            .expect("current byte index starts a character");
        stripped.push(ch);
        index += ch.len_utf8();
    }

    stripped
}

pub(crate) fn decorate_spans(line: &HyperlinkLine) -> Vec<Span<'static>> {
    if line.hyperlinks.is_empty() {
        return line.line.spans.clone();
    }

    let mut out = Vec::new();
    let mut column = 0usize;
    let mut link_index = 0usize;
    let mut active_link_index = None;
    let mut active_destination: Option<String> = None;
    for span in &line.line.spans {
        for ch in span.content.chars() {
            let width = ch.width().unwrap_or(/*default*/ 0);
            while line
                .hyperlinks
                .get(link_index)
                .is_some_and(|link| link.columns.end <= column)
            {
                link_index += 1;
            }
            let selected_link_index = line
                .hyperlinks
                .get(link_index)
                .and_then(|link| link.columns.contains(&column).then_some(link_index));
            if active_link_index != selected_link_index {
                if active_destination.is_some() {
                    append_to_last_span(&mut out, "\x1b]8;;\x07");
                }
                active_destination = selected_link_index
                    .and_then(|index| line.hyperlinks[index].terminal_destination());
                if let Some(destination) = active_destination.as_ref() {
                    push_styled_content(
                        &mut out,
                        &format!("\x1b]8;;{destination}\x07"),
                        span.style,
                    );
                }
                active_link_index = selected_link_index;
            }
            push_styled_content(&mut out, &ch.to_string(), span.style);
            column += width;
        }
    }
    if active_destination.is_some() {
        append_to_last_span(&mut out, "\x1b]8;;\x07");
    }
    out
}

fn push_styled_content(out: &mut Vec<Span<'static>>, content: &str, style: ratatui::style::Style) {
    if let Some(last) = out.last_mut()
        && last.style == style
    {
        last.content.to_mut().push_str(content);
        return;
    }
    out.push(Span::styled(content.to_string(), style));
}

fn append_to_last_span(out: &mut [Span<'static>], content: &str) {
    if let Some(last) = out.last_mut() {
        last.content.to_mut().push_str(content);
    }
}

pub(crate) fn mark_buffer_hyperlinks(
    buf: &mut Buffer,
    area: Rect,
    lines: &[HyperlinkLine],
    scroll_rows: usize,
) {
    if area.width == 0 {
        return;
    }
    let mut logical_row = 0usize;
    for line in lines {
        let paragraph =
            Paragraph::new(Text::from(line_to_borrowed(&line.line))).wrap(Wrap { trim: false });
        let rendered_height = paragraph.line_count(area.width).max(/*other*/ 1);
        if line.hyperlinks.is_empty() {
            logical_row += rendered_height;
            continue;
        }

        let layout_area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            area.width,
            u16::try_from(rendered_height).unwrap_or(u16::MAX),
        );
        let mut layout = Buffer::empty(layout_area);
        paragraph.render(layout_area, &mut layout);
        let rendered_lines = (0..layout_area.height)
            .map(|row| {
                let text = (0..layout_area.width)
                    .filter_map(|column| {
                        let cell = &layout[(column, row)];
                        (!cell.skip).then(|| cell.symbol())
                    })
                    .collect::<String>();
                Line::from(text.trim_end().to_string())
            })
            .collect();
        for (row, rendered) in remap_wrapped_line(line, rendered_lines).iter().enumerate() {
            for link in &rendered.hyperlinks {
                for column in link.columns.clone() {
                    let row = logical_row + row;
                    if row < scroll_rows || row - scroll_rows >= usize::from(area.height) {
                        continue;
                    }
                    let x = area.x + column as u16;
                    let y = area.y + (row - scroll_rows) as u16;
                    let cell = &mut buf[(x, y)];
                    if cell.skip || cell.symbol().trim().is_empty() {
                        continue;
                    }
                    let symbol = link.terminal_destination().map_or_else(
                        || cell.symbol().to_string(),
                        |destination| {
                            format!("\x1b]8;;{destination}\x07{}\x1b]8;;\x07", cell.symbol())
                        },
                    );
                    cell.set_symbol(&symbol);
                }
            }
        }
        logical_row += rendered_height;
    }
}

pub(crate) fn mark_url_hyperlink(buf: &mut Buffer, area: Rect, destination: &str) {
    mark_matching_cells(buf, area, destination, |cell| {
        cell.fg == Color::Cyan && cell.modifier.contains(Modifier::UNDERLINED)
    });
}

pub(crate) fn mark_underlined_hyperlink(buf: &mut Buffer, area: Rect, destination: &str) {
    mark_matching_cells(buf, area, destination, |cell| {
        cell.modifier.contains(Modifier::UNDERLINED)
    });
}

fn mark_matching_cells(
    buf: &mut Buffer,
    area: Rect,
    destination: &str,
    matches: impl Fn(&ratatui::buffer::Cell) -> bool,
) {
    if web_destination(destination).is_none() {
        return;
    }
    for position in area.positions() {
        let cell = &mut buf[position];
        if !cell.skip && !cell.symbol().trim().is_empty() && matches(cell) {
            let symbol = osc8_hyperlink(destination, cell.symbol());
            cell.set_symbol(&symbol);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn only_web_destinations_receive_osc8() {
        assert!(osc8_hyperlink("https://example.com/a", "a").contains("\x1b]8;;"));
        assert_eq!(osc8_hyperlink("mailto:a@example.com", "a"), "a");
        assert_eq!(
            osc8_hyperlink("https://example.com/\u{7}safe", "a"),
            "\x1b]8;;https://example.com/safe\x07a\x1b]8;;\x07"
        );
        assert_eq!(
            strip_osc8(&osc8_hyperlink("https://example.com/a", "visible")),
            "visible"
        );
    }

    #[test]
    fn discovers_punctuated_web_url_columns() {
        assert_eq!(
            web_links_in_text("See (https://example.com/a)."),
            vec![TerminalHyperlink::web(
                /*columns*/ 5..26,
                "https://example.com/a".to_string(),
            )]
        );
    }

    #[test]
    fn preserves_balanced_parentheses_in_bare_web_urls() {
        let destination = "https://en.wikipedia.org/wiki/Function_(mathematics)";
        assert_eq!(
            web_links_in_text(&format!("See ({destination}).")),
            vec![TerminalHyperlink::web(
                /*columns*/ 5..5 + destination.width(),
                destination.to_string(),
            )]
        );
    }

    #[test]
    fn decorates_a_contiguous_web_link_with_one_osc8_pair() {
        let destination = "https://example.com/a/very/long/path";
        let line = HyperlinkLine {
            line: Line::from(destination),
            hyperlinks: vec![TerminalHyperlink::web(
                /*columns*/ 0..destination.width(),
                destination.to_string(),
            )],
        };

        assert_eq!(
            decorate_spans(&line),
            vec![Span::from(osc8_hyperlink(destination, destination))]
        );
        assert_eq!(
            decorate_spans(&HyperlinkLine::new(Line::from("not linked"))),
            vec![Span::from("not linked")]
        );
    }

    #[test]
    fn wrapping_maps_repeated_link_labels_by_source_position() {
        let mut source = HyperlinkLine::new(Line::from("here here"));
        source.hyperlinks.push(TerminalHyperlink::web(
            /*columns*/ 5..9,
            "https://example.com".to_string(),
        ));

        let wrapped = remap_wrapped_line(&source, vec![Line::from("here here")]);

        assert_eq!(
            wrapped[0].hyperlinks,
            vec![TerminalHyperlink::web(
                /*columns*/ 5..9,
                "https://example.com".to_string(),
            )]
        );
    }

    #[test]
    fn wrapping_maps_multiple_links_across_indented_unicode_lines() {
        let text = "alpha 😀here middle there end";
        let first_start = text.find("here").expect("first link");
        let second_start = text.find("there").expect("second link");
        let mut source = HyperlinkLine::new(Line::from(text));
        source.hyperlinks.push(TerminalHyperlink::web(
            text[..first_start].width()..text[..first_start].width() + "here".width(),
            "https://example.com/first".to_string(),
        ));
        source.hyperlinks.push(TerminalHyperlink::web(
            text[..second_start].width()..text[..second_start].width() + "there".width(),
            "https://example.com/second".to_string(),
        ));

        let wrapped = remap_wrapped_line(
            &source,
            vec![
                Line::from("  alpha 😀here"),
                Line::from("    middle there end"),
            ],
        );

        assert_eq!(
            wrapped,
            vec![
                HyperlinkLine {
                    line: Line::from("  alpha 😀here"),
                    hyperlinks: vec![TerminalHyperlink::web(
                        /*columns*/ 10..14,
                        "https://example.com/first".to_string(),
                    )],
                },
                HyperlinkLine {
                    line: Line::from("    middle there end"),
                    hyperlinks: vec![TerminalHyperlink::web(
                        /*columns*/ 11..16,
                        "https://example.com/second".to_string(),
                    )],
                },
            ]
        );
    }

    #[test]
    fn buffer_hyperlinks_follow_word_wrapping() {
        let destination = "https://example.com/path";
        let mut line = HyperlinkLine::new(Line::from(format!("See {destination} now")));
        line.hyperlinks.push(TerminalHyperlink::web(
            /*columns*/ 4..4 + destination.width(),
            destination.to_string(),
        ));
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 18, /*height*/ 4,
        );
        let mut buf = Buffer::empty(area);

        Paragraph::new(Text::from(line.line.clone()))
            .wrap(Wrap { trim: false })
            .render(area, &mut buf);
        mark_buffer_hyperlinks(&mut buf, area, &[line], /*scroll_rows*/ 0);

        let linked_text = area
            .positions()
            .filter_map(|position| {
                let symbol = buf[position].symbol();
                symbol
                    .contains(&format!("\x1b]8;;{destination}\x07"))
                    .then(|| strip_osc8(symbol))
            })
            .collect::<String>();
        assert_eq!(linked_text, destination);
    }

    #[test]
    fn trusted_file_destination_receives_osc8_without_enabling_plain_file_links() {
        let temp_dir = tempfile::tempdir().expect("temp directory");
        let file_url = Url::from_file_path(temp_dir.path().join("viewer.html"))
            .expect("test path should convert to file URL");
        let mut link = TerminalHyperlink::web(
            /*columns*/ 0..4,
            "https://codex.invalid/viewer".to_string(),
        );
        link.retarget_to_trusted_file(&file_url);
        let line = HyperlinkLine {
            line: Line::from("view"),
            hyperlinks: vec![link],
        };

        assert_eq!(
            decorate_spans(&line),
            vec![Span::from(format!(
                "\x1b]8;;{file_url}\x07view\x1b]8;;\x07"
            ))]
        );
        assert_eq!(osc8_hyperlink(file_url.as_str(), "view"), "view");
    }
}
