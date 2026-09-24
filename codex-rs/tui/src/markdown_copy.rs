//! Semantic copy annotations in rendered-byte coordinates, independent of terminal styling.
//!
//! Wrapping and selection snapshots share each logical line's annotations. Only copying
//! serializes Markdown; literal-only selections keep their original text and clipboard format.
//! Rich selections preserve selected words and semantic formatting with balanced markup,
//! rather than reproducing the complete response's source spelling (available through `/copy`).

use std::ops::Range;
use std::sync::Arc;

use pulldown_cmark::Event;
use pulldown_cmark::Tag;

use crate::clipboard_copy::CopyFormat;
use crate::terminal_hyperlinks::LogicalLineSource;

// Bound retained inline stacks and container prefixes independently of parser nesting.
pub(crate) const MAX_COPY_DEPTH: usize = 64;

// A fixed, deduplicated style order gives each nested wrapper a distinct delimiter family.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Inline {
    // Keep one link around its formatted label instead of repeating its destination per style.
    Link(Arc<str>),
    Delimiter(&'static str),
    Code,
    /// Balance parser events for links whose visible text has no copied wrapper.
    Ignored,
}

impl Inline {
    pub(crate) fn from_event(event: &Event<'_>) -> Option<Self> {
        match event {
            Event::Start(Tag::Emphasis) => Some(Self::Delimiter("_")),
            Event::Start(Tag::Strong) => Some(Self::Delimiter("**")),
            Event::Start(Tag::Strikethrough) => Some(Self::Delimiter("~~")),
            Event::Start(Tag::Link { dest_url, .. }) => Some(
                if crate::terminal_hyperlinks::web_destination(dest_url).is_some()
                    && !dest_url.starts_with(crate::inline_visualization::LINK_PLACEHOLDER_PREFIX)
                {
                    Self::Link(dest_url.as_ref().into())
                } else {
                    Self::Ignored
                },
            ),
            Event::Code(_) => Some(Self::Code),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CopyLine {
    pub(crate) prefix: String,
    pub(crate) continuation: String,
    /// Restore the containing item when a multiline selection starts in its later paragraph.
    pub(crate) item_prefix: String,
    pub(crate) code: bool,
    pub(crate) rule: bool,
    pub(crate) heading: usize,
    pub(crate) hard_break: bool,
    /// A visual separator inserted between sibling list items, never source content.
    pub(crate) omit: bool,
    runs: Vec<(Range<usize>, Vec<Inline>)>,
}

impl CopyLine {
    pub(crate) fn push(&mut self, length: usize, inline: &[Inline]) {
        if length == 0 {
            return;
        }
        let mut inline = inline[..inline.len().min(MAX_COPY_DEPTH)]
            .iter()
            .filter(|mark| **mark != Inline::Ignored)
            .cloned()
            .collect::<Vec<_>>();
        inline.sort();
        inline.dedup();
        let start = self
            .runs
            .last()
            .map_or(/*default*/ 0, |(range, _)| range.end);
        if let Some((range, previous)) = self.runs.last_mut()
            && previous == &inline
        {
            range.end += length;
        } else {
            self.runs.push((start..start + length, inline));
        }
    }

    fn render(&self, text: &str, range: Range<usize>, depth: usize) -> String {
        if self.rule && range == (0..text.len()) {
            return "***".to_owned();
        }
        let mut out = String::new();
        let first = self.runs.partition_point(|(run, _)| run.end <= range.start);
        let mut runs = self.runs[first..]
            .iter()
            .take_while(|(run, _)| run.start < range.end)
            .peekable();
        while let Some((run, inline)) = runs.next() {
            let start = run.start.max(range.start);
            let mut end = run.end.min(range.end);
            if start >= end {
                continue;
            }
            let mark = inline.get(depth);
            while let Some((next, inline)) = runs.peek()
                && next.start == end
                && inline.get(depth) == mark
                && end < range.end
            {
                end = next.end.min(range.end);
                runs.next();
            }
            match mark {
                Some(Inline::Code) => {
                    let content = &text[start..end];
                    let fence = fence(content, /*minimum*/ 1);
                    let padding =
                        if content.starts_with(['`', ' ']) || content.ends_with(['`', ' ']) {
                            if content.chars().all(|ch| ch == ' ') {
                                ""
                            } else {
                                " "
                            }
                        } else {
                            ""
                        };
                    append_inline(
                        &mut out,
                        &format!("{fence}{padding}{content}{padding}{fence}"),
                    );
                }
                Some(mark) => {
                    let body = self.render(text, start..end, depth + 1);
                    let trimmed = body.trim();
                    if trimmed.is_empty() {
                        out.push_str(&body);
                        continue;
                    }
                    out.push_str(&body[..body.len() - body.trim_start().len()]);
                    match mark {
                        Inline::Delimiter(delimiter) => {
                            append_inline(&mut out, &format!("{delimiter}{trimmed}{delimiter}"))
                        }
                        Inline::Link(destination) => {
                            let destination = destination
                                .replace('\\', "\\\\")
                                .replace('&', "&amp;")
                                .replace('<', "%3C")
                                .replace('>', "%3E")
                                .replace('\n', "%0A")
                                .replace('\r', "%0D");
                            append_inline(&mut out, &format!("[{trimmed}](<{destination}>)"));
                        }
                        Inline::Code | Inline::Ignored => unreachable!(),
                    }
                    out.push_str(&body[body.trim_end().len()..]);
                }
                None => {
                    let heading_end = self.heading.min(end).max(start);
                    out.push_str(&text[start..heading_end]);
                    append_inline(&mut out, &escape(&text[heading_end..end]));
                }
            }
        }
        out
    }
}

// Entity decoding and omitted link wrappers can turn a delimiter's punctuation neighbor into
// a word character. Protect that boundary without changing the selected text or adding spacing.
fn append_inline(out: &mut String, text: &str) {
    let word = |ch: char| !ch.is_whitespace() && !ch.is_ascii_punctuation();
    let left = text.starts_with('_')
        || text.starts_with(['*', '~'])
            && text
                .chars()
                .next()
                .and_then(|delimiter| text.trim_start_matches(delimiter).chars().next())
                .is_some_and(|ch| !ch.is_alphanumeric());
    if left && let Some(ch) = out.chars().last().filter(|ch| word(*ch)) {
        out.pop();
        out.push_str(&format!("&#{};", u32::from(ch)));
    }
    let right = out.ends_with('_')
        || out.ends_with(['*', '~'])
            && out
                .chars()
                .next_back()
                .and_then(|delimiter| out.trim_end_matches(delimiter).chars().next_back())
                .is_some_and(|ch| !ch.is_alphanumeric());
    if right && let Some(ch) = text.chars().next().filter(|ch| word(*ch)) {
        out.push_str(&format!("&#{};", u32::from(ch)));
        out.push_str(&text[ch.len_utf8()..]);
    } else {
        out.push_str(text);
    }
}

pub(crate) struct SelectedLine {
    source: LogicalLineSource,
    range: Range<usize>,
    separator: String,
}

impl SelectedLine {
    pub(crate) fn append(
        lines: &mut Vec<Self>,
        source: LogicalLineSource,
        range: Range<usize>,
        separator: &str,
    ) {
        // Streaming can split the same logical line across history cells.
        if let Some(previous) = lines.last_mut()
            && Arc::ptr_eq(&previous.source.text, &source.text)
            && previous.range.end <= range.start
        {
            previous.range.end = range.end;
        } else {
            let separator = if separator.starts_with('\n')
                && lines.last().is_some_and(|previous| {
                    previous.range.end == previous.source.text.len()
                        && (previous.source.copy_as_prose
                            || previous
                                .source
                                .copy
                                .as_ref()
                                .is_some_and(|copy| copy.hard_break))
                }) {
                format!("  {separator}")
            } else {
                separator.to_string()
            };
            lines.push(Self {
                source,
                range,
                separator,
            });
        }
    }
}

pub(crate) fn selection(lines: &[SelectedLine], plain: &str) -> (String, CopyFormat) {
    let rich = lines.iter().any(|line| {
        !line.range.is_empty()
            && line
                .source
                .copy
                .as_ref()
                .is_some_and(|copy| !copy.code && !copy.omit)
    });
    if !rich {
        return (plain.to_owned(), CopyFormat::PlainText);
    }
    let mut out = String::new();
    let mut lines = lines
        .iter()
        .filter(|line| !line.source.copy.as_ref().is_some_and(|copy| copy.omit))
        .peekable();
    let mut first = true;
    let mut indentation: Option<usize> = None;
    while let Some(line) = lines.next() {
        if !first {
            out.push_str(&line.separator);
        }
        let prefix = line.source.copy.as_ref().map_or("", |copy| {
            if indentation.is_none() && !line.range.is_empty() && lines.peek().is_some() {
                &copy.item_prefix
            } else {
                &copy.prefix
            }
        });
        if !line.range.is_empty() {
            let spaces = prefix
                .split("> ")
                .map(|part| part.len() - part.trim_start_matches(' ').len())
                .sum();
            indentation = Some(indentation.map_or(spaces, |previous| previous.min(spaces)));
        }
        let prefix = dedent(prefix, indentation.unwrap_or(/*default*/ 0));
        if line.range.is_empty() {
            if line.source.text.is_empty() {
                out.push_str(prefix.trim_end());
            }
            first = false;
            continue;
        }
        if line.source.copy_as_prose {
            let text = &line.source.text[line.range.clone()];
            push_prose_fragment(&mut out, &escape(text));
        } else if let Some(copy) = &line.source.copy
            && !copy.code
        {
            if line.range.start == 0 || first && lines.peek().is_some() {
                out.push_str(&prefix);
            }
            let body = copy.render(&line.source.text, line.range.clone(), /*depth*/ 0);
            push_prose_fragment(&mut out, &body);
        } else {
            // Preserve whitespace and Markdown-looking tool output within mixed selections.
            let mut code = line.source.text[line.range.clone()].to_owned();
            let continuation = line
                .source
                .copy
                .as_ref()
                .map_or("", |copy| copy.continuation.as_str());
            while let Some(next) = lines.peek()
                && !next.source.copy_as_prose
                && next.source.copy.is_some() == line.source.copy.is_some()
                && next.source.copy.as_ref().is_none_or(|copy| copy.code)
                && next
                    .source
                    .copy
                    .as_ref()
                    .map_or("", |copy| copy.prefix.as_str())
                    == continuation
                && next
                    .source
                    .copy
                    .as_ref()
                    .map_or("", |copy| copy.continuation.as_str())
                    == continuation
            {
                code.push_str(&next.separator);
                code.push_str(&next.source.text[next.range.clone()]);
                lines.next();
            }
            let fence = fence(&code, /*minimum*/ 3);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            let continuation = dedent(continuation, indentation.unwrap_or(/*default*/ 0));
            // A checkbox label must precede the fence on its own line.
            if prefix.ends_with("[ ] ") || prefix.ends_with("[x] ") {
                out.push_str(prefix.trim_end());
                out.push('\n');
                out.push_str(&continuation);
            } else {
                out.push_str(&prefix);
            }
            out.push_str(&format!("{fence}\n"));
            for row in code.split_inclusive('\n') {
                out.push_str(&continuation);
                out.push_str(row);
            }
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&continuation);
            out.push_str(&fence);
        }
        first = false;
    }
    out.retain(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'));
    (out, CopyFormat::Markdown)
}

// Remove unselected ancestor list padding without discarding blockquote markers.
fn dedent(prefix: &str, mut spaces: usize) -> String {
    prefix
        .split_inclusive("> ")
        .map(|part| {
            let remove = spaces.min(part.len() - part.trim_start_matches(' ').len());
            spaces -= remove;
            &part[remove..]
        })
        .collect()
}

fn fence(text: &str, minimum: usize) -> String {
    "`".repeat(
        text.split(|ch| ch != '`')
            .map(str::len)
            .max()
            .unwrap_or(/*default*/ 0)
            .saturating_add(/*rhs*/ 1)
            .max(minimum),
    )
}

// Append escaped prose without allowing its leading whitespace to become code indentation.
fn push_prose_fragment(out: &mut String, text: &str) {
    let body = text.trim_start_matches([' ', '\t']);
    for ch in text[..text.len() - body.len()].chars() {
        out.push_str(if ch == '\t' { "&#9;" } else { "&#32;" });
    }
    out.push_str(body);
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    let mut whitespace_prefix = true;
    let mut numeric_prefix = false;
    let mut number_has_trailing_space = false;
    for ch in text.chars() {
        let block_marker =
            "=-+".contains(ch) && whitespace_prefix || matches!(ch, '.' | ')') && numeric_prefix;
        if "\\`*_[]<>~#|&!".contains(ch) || block_marker {
            out.push('\\');
        }
        out.push(ch);
        if ch.is_whitespace() {
            number_has_trailing_space |= numeric_prefix;
        } else {
            numeric_prefix = ch.is_ascii_digit()
                && (whitespace_prefix || numeric_prefix && !number_has_trailing_space);
            whitespace_prefix = false;
        }
    }
    out
}

#[cfg(test)]
#[path = "markdown_copy_tests.rs"]
mod tests;
