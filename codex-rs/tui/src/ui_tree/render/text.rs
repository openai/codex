//! Prepared inline text retains styled chunks and their legal line breaks across leaves.

use super::AvailableWidth;
use super::Fragment;
use super::RenderTheme;
use super::Visit;
use super::sanitize_leaf;
use crate::line_truncation::span_width;
use crate::ui_tree::theme::Color;
use ratatui::buffer::CellWidth;
use ratatui::style::Style;
use ratatui::text::Span;
use std::ops::Deref;
use std::ops::Range;
use std::sync::Arc;
use unicode_linebreak::linebreaks;
use unicode_linebreak::split_at_safe;
use unicode_segmentation::GraphemeCursor;
use unicode_segmentation::GraphemeIncomplete;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Paint {
    pub(super) modifiers: Style,
    pub(super) foreground: Option<Color>,
    pub(super) background: Option<Color>,
    pub(super) href: Option<Arc<String>>,
    pub(super) inline_code: bool,
    pub(super) preserve_whitespace: bool,
}

impl Paint {
    pub(super) fn style(&self, theme: &mut RenderTheme<'_>) -> Style {
        let mut style = self.modifiers;
        style.fg = self.foreground.map(|c| theme.resolve(c));
        style.bg = self.background.map(|c| theme.resolve(c));
        if self.inline_code {
            theme.inline_code_style(style)
        } else {
            style
        }
    }
}

#[derive(Debug, PartialEq)]
struct Run {
    break_before: bool,
    text: RunText,
    paint: Paint,
}

// Splitting a leaf at style, whitespace or wrap boundaries shares its original buffer.
#[derive(Debug)]
enum RunText {
    Slice(Arc<String>, Range<usize>),
    Space,
}

impl RunText {
    fn new(text: String) -> Self {
        let len = text.len();
        Self::Slice(Arc::new(text), 0..len)
    }

    fn slice(&self, range: Range<usize>) -> Self {
        match self {
            Self::Slice(text, bytes) => Self::Slice(
                Arc::clone(text),
                bytes.start + range.start..bytes.start + range.end,
            ),
            Self::Space => Self::Space,
        }
    }
}

impl Deref for RunText {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Self::Slice(text, bytes) => &text[bytes.clone()],
            Self::Space => " ",
        }
    }
}

impl PartialEq for RunText {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct Chunk {
    runs: Vec<Run>,
    width_cells: usize,
    pub(super) trailing_space: Option<Paint>,
    break_before: BreakBefore,
}

#[derive(Debug, Default, PartialEq)]
enum BreakBefore {
    #[default]
    Preferred,
    AfterHyphenBeforeDigit,
    Emergency,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct PreparedText {
    pub(super) has_authored_break: bool,
    pub(super) lines: Vec<Vec<Chunk>>,
}

#[derive(Default)]
pub(super) struct InlineText {
    after_span: bool,
    text: PreparedText,
    line: Vec<Run>,
    space: Option<Paint>,
}

impl InlineText {
    /// Returning to normal prose allows a break before a sibling’s first literal space.
    pub(super) fn end_span(&mut self) {
        self.after_span = true;
    }

    /// Whitespace-only text nodes join an existing anonymous flex item but cannot start one.
    pub(super) fn push_flex_leaf(&mut self, raw: String, paint: Paint) {
        let raw = sanitize_leaf(raw);
        if self.line.is_empty() && self.text.lines.is_empty() && raw.trim_ascii().is_empty() {
            return;
        }
        self.push_sanitized(raw, paint);
    }

    pub(super) fn push_leaf(&mut self, raw: String, paint: Paint) {
        self.push_sanitized(sanitize_leaf(raw), paint);
    }

    fn push_sanitized(&mut self, raw: String, paint: Paint) {
        let after_span = !raw.is_empty() && std::mem::take(&mut self.after_span);
        let break_before = after_span && paint.preserve_whitespace && raw.starts_with([' ', '\t']);
        let separators: &[char] = if paint.preserve_whitespace {
            &['\n']
        } else {
            &[' ', '\t', '\n']
        };
        let text = RunText::new(raw);
        let mut start = 0;
        for (end, ch) in text
            .match_indices(separators)
            .chain(std::iter::once((text.len(), "")))
        {
            if start < end {
                if let Some(paint) = self.space.take() {
                    self.line.push(Run {
                        break_before: false,
                        text: RunText::Space,
                        paint,
                    });
                }
                self.line.push(Run {
                    break_before: break_before && start == 0,
                    text: text.slice(start..end),
                    paint: paint.clone(),
                });
            }
            if ch == "\n" {
                self.text.lines.push(chunks(std::mem::take(&mut self.line)));
                self.space = None;
            } else if !ch.is_empty() && !self.line.is_empty() {
                self.space.get_or_insert_with(|| paint.clone());
            }
            start = end + ch.len();
        }
    }

    pub(super) fn finish(mut self) -> PreparedText {
        self.text.has_authored_break = !self.text.lines.is_empty();
        if !self.line.is_empty() {
            self.text.lines.push(chunks(self.line));
        }
        self.text
    }
}

// Most seams already fall between graphemes. Only a cluster that actually crosses a
// seam needs contiguous storage, including when it spans more than two leaves.
fn join_graphemes(runs: Vec<Run>) -> Vec<Run> {
    let mut runs = runs.into_iter().peekable();
    let mut joined = Vec::new();
    while let Some(run) = runs.next() {
        let Some(next) = runs.peek() else {
            joined.push(run);
            break;
        };
        if run.text.as_bytes().last().is_some_and(u8::is_ascii)
            && next.text.as_bytes()[0].is_ascii()
        {
            joined.push(run);
            continue;
        }
        // Reverse segmentation in unicode-segmentation 1.12 disagrees on Indic conjuncts.
        #[expect(clippy::double_ended_iterator_last)]
        let Some(tail) = run.text.graphemes(true).last() else {
            continue;
        };
        let split = tail.len();
        let mut cursor = GraphemeCursor::new(split, usize::MAX, true);
        while let Err(GraphemeIncomplete::PreContext(_)) = cursor.is_boundary(&next.text, split) {
            cursor.provide_context(tail, 0);
        }
        if cursor.is_boundary(&next.text, split) == Ok(true) {
            joined.push(run);
            continue;
        }

        let mut cluster = tail.to_owned();
        let mut paint = run.paint;
        let prefix = run.text.len() - split;
        if prefix > 0 {
            joined.push(Run {
                break_before: run.break_before,
                text: run.text.slice(0..prefix),
                paint: paint.clone(),
            });
        }
        // Resume in the growing cluster so even a long chain of combining marks
        // advances once per character and Unicode lookbehind stays available.
        let mut cursor = GraphemeCursor::new(0, usize::MAX, true);
        let _ = cursor.next_boundary(&cluster, 0);
        while let Some(next) = runs.peek_mut() {
            let mut consumed = 0;
            for ch in next.text.chars() {
                cluster.push(ch);
                if cursor.next_boundary(&cluster, 0).is_ok() {
                    cluster.pop();
                    break;
                }
                consumed += ch.len_utf8();
            }
            if consumed > 0 && paint.href != next.paint.href {
                paint.href = None;
            }
            if consumed < next.text.len() {
                if consumed > 0 {
                    next.break_before = false;
                    next.text = next.text.slice(consumed..next.text.len());
                }
                break;
            }
            runs.next();
        }
        joined.push(Run {
            break_before: prefix == 0 && run.break_before,
            text: RunText::new(cluster),
            paint,
        });
    }
    joined
}

// Keep the context needed for breaks across style boundaries.
fn break_offsets<'a>(runs: impl Iterator<Item = &'a str>) -> Vec<usize> {
    let mut runs = runs.peekable();
    let mut breaks = Vec::new();
    let mut scratch = String::new();
    let mut base = 0;
    while let Some(&run) = runs.peek() {
        let scanned = scratch.len();
        let window = if scanned == 0 {
            runs.next();
            run
        } else {
            // Match the retained context with fresh bytes to keep rescanning linear.
            // Bound each read so a short prefix never pulls a large ASCII run into scratch.
            while scratch.len() - scanned < scanned
                && let Some(next) = runs.peek_mut()
            {
                let (part, rest) = next.split_at(next.floor_char_boundary(scanned.max(128)));
                scratch.push_str(part);
                *next = rest;
                runs.next_if(|s| s.is_empty());
            }
            scratch.as_str()
        };
        breaks.extend(
            linebreaks(window)
                .filter_map(|(i, _)| (i >= scanned && i < window.len()).then_some(base + i)),
        );
        if runs.peek().is_none() {
            break;
        }
        let discard = split_at_safe(window).0.len();
        base += discard;
        if scanned == 0 {
            scratch.push_str(&run[discard..]);
        } else {
            scratch.drain(..discard);
        }
    }
    breaks
}

/// Preferred breaks between ASCII graphics in Chromium's break-word layout.
fn ascii_word_break(previous: char, next: u8, word_hyphen: bool) -> bool {
    match previous {
        '-' if next.is_ascii_digit() => word_hyphen,
        '-' => !b"!$),./:;?]}".contains(&next),
        '?' => !b"!\"'),./:;?]}".contains(&next),
        _ if matches!(next, b'(' | b'<' | b'[' | b'{') => {
            "!\"#%&)*+,.:;=>\\]|}~".contains(previous)
        }
        _ => false,
    }
}

fn chunks(runs: Vec<Run>) -> Vec<Chunk> {
    let runs = join_graphemes(runs);
    let mut breaks = break_offsets(runs.iter().map(|run| run.text.as_ref()))
        .into_iter()
        .peekable();
    let mut out = Vec::new();
    let mut chunk = Chunk::default();
    let mut offset = 0;
    let mut previous: Option<char> = None;
    let mut word_hyphen = false;
    let mut previous_preserved = false;
    for run in runs {
        let mut start = 0;
        for (i, grapheme) in run.text.grapheme_indices(true) {
            let position = offset + i;
            while breaks.next_if(|b| *b < position).is_some() {}
            // Ordinary text uses textwrap’s word hyphens. In preserved text, Chromium also breaks
            // at flag hyphens and keeps paths and shell operators within a word.
            let first = grapheme.as_bytes()[0];
            let can_break = match previous {
                _ if i == 0 && run.break_before => true,
                Some(prev)
                    if (previous_preserved || run.paint.preserve_whitespace)
                        && prev.is_ascii_graphic()
                        && first.is_ascii_graphic() =>
                {
                    ascii_word_break(prev, first, word_hyphen)
                }
                Some('-') => word_hyphen && grapheme.starts_with(char::is_alphanumeric),
                // A collapsed separator can wrap before the first preserved space.
                Some(' ' | '\t')
                    if previous_preserved
                        || (run.paint.preserve_whitespace && matches!(first, b' ' | b'\t')) =>
                {
                    true
                }
                Some('\u{ad}') => false,
                _ => breaks.peek() == Some(&position),
            };
            // Preserve normal word breaks first. An overlong preserved word can also
            // wrap at a grapheme boundary, as it does under overflow-wrap: break-word.
            let preserved_boundary = position > 0
                && (run.paint.preserve_whitespace
                    || (previous_preserved && !matches!(run.text, RunText::Space)));
            if can_break || preserved_boundary {
                // A break after a literal space or tab can precede a collapsed
                // prose separator. Keep the separator on this chunk, so only
                // the prose space disappears when the line wraps.
                let end = if previous_preserved && matches!(run.text, RunText::Space) {
                    run.text.len()
                } else {
                    i
                };
                chunk.push(&run, start..end);
                start = end;
                if chunk
                    .runs
                    .last()
                    .is_some_and(|r| matches!(r.text, RunText::Space))
                {
                    chunk.trailing_space = chunk.runs.pop().map(|r| r.paint);
                    chunk.width_cells -= 1;
                }
                if !chunk.runs.is_empty() {
                    out.push(std::mem::take(&mut chunk));
                }
                chunk.break_before = if !can_break {
                    BreakBefore::Emergency
                } else if previous_preserved && word_hyphen && first.is_ascii_digit() {
                    BreakBefore::AfterHyphenBeforeDigit
                } else {
                    BreakBefore::Preferred
                };
            }
            previous_preserved = run.paint.preserve_whitespace;
            for ch in grapheme.chars() {
                word_hyphen = ch == '-' && previous.is_some_and(char::is_alphanumeric);
                previous = Some(ch);
            }
        }
        chunk.push(&run, start..run.text.len());
        offset += run.text.len();
    }
    if !chunk.runs.is_empty() {
        out.push(chunk);
    }
    out
}

const TAB_SPACES: &str = "        ";

impl Chunk {
    pub(super) fn prefers_break_before(&self, index_in_line: usize) -> bool {
        match self.break_before {
            BreakBefore::Preferred => true,
            // A preserved hyphen occupies its own chunk. When wrapping puts it at the
            // start of a visual line, keep it with the digits that follow.
            BreakBefore::AfterHyphenBeforeDigit => index_in_line > 1,
            BreakBefore::Emergency => false,
        }
    }

    fn tab_paint(&self) -> Option<&Paint> {
        match self.runs.as_slice() {
            [run] if run.paint.preserve_whitespace && &*run.text == "\t" => Some(&run.paint),
            _ => None,
        }
    }

    pub(super) fn width_at(&self, column: usize) -> usize {
        if self.tab_paint().is_some() {
            TAB_SPACES.len() - column % TAB_SPACES.len()
        } else {
            self.width_cells
        }
    }

    fn push(&mut self, run: &Run, range: Range<usize>) {
        if !range.is_empty() {
            let text = run.text.slice(range);
            self.width_cells += span_width(&Span::raw(&*text));
            self.runs.push(Run {
                break_before: false,
                text,
                paint: run.paint.clone(),
            });
        }
    }
}

impl PreparedText {
    pub(super) fn layout<'a>(
        &'a self,
        width: AvailableWidth,
        limit: usize,
        visit: &mut Visit<'a, '_>,
    ) -> usize {
        let width = width.cells;
        let mut y = 0;
        for line in &self.lines {
            let mut remaining = line.as_slice();
            loop {
                let (end, cells) = fit_visual_line(remaining, width);
                let (fitted, rest) = remaining.split_at(end);
                visit(y, Fragment::Text(fitted, cells));
                y += 1;
                if y == limit {
                    return y;
                }
                if rest.is_empty() {
                    break;
                }
                remaining = rest;
            }
        }
        y
    }
}

/// Fit one screen row, preferring the last word boundary before splitting a preserved word.
fn fit_visual_line(line: &[Chunk], width: usize) -> (usize, usize) {
    let mut used = 0;
    let mut space = 0;
    let mut last_break = None;
    for (i, chunk) in line.iter().enumerate() {
        if i > 0 && chunk.prefers_break_before(i) {
            last_break = Some((i, used));
        }
        let next = used + space + chunk.width_at(used + space);
        if i > 0 && next > width {
            return last_break.unwrap_or((i, used));
        }
        // A whole tab wider than its line paints only the visible spaces.
        used = if i == 0 && chunk.tab_paint().is_some() {
            next.min(width)
        } else {
            next
        };
        space = usize::from(chunk.trailing_space.is_some());
    }
    (line.len(), used)
}

pub(super) fn visit_chunks<'a>(
    chunks: &'a [Chunk],
    limit: usize,
    mut draw: impl FnMut(usize, &str, usize, &'a Paint),
) -> usize {
    let mut used = 0;

    for (index, chunk) in chunks.iter().enumerate() {
        if let Some(paint) = chunk.tab_paint() {
            let cells = chunk.width_at(used).min(limit.saturating_sub(used));
            if cells == 0 {
                return used;
            }
            draw(used, &TAB_SPACES[..cells], cells, paint);
            used += cells;
        } else {
            for run in &chunk.runs {
                let text = &*run.text;
                let mut bytes = 0;
                let mut cells = 0;
                for g in text.graphemes(true) {
                    let w = usize::from(g.cell_width());
                    if used + cells + w > limit {
                        break;
                    }
                    cells += w;
                    bytes += g.len();
                }
                if bytes > 0 && (used < limit || bytes == text.len()) {
                    draw(used, &text[..bytes], cells, &run.paint);
                }
                used += cells;
                if bytes < text.len() {
                    return used;
                }
            }
        }
        if index + 1 < chunks.len()
            && let Some(paint) = &chunk.trailing_space
        {
            if used == limit {
                return used;
            }
            draw(used, " ", 1, paint);
            used += 1;
        }
    }
    used
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
