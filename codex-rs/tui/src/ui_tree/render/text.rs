//! Prepared inline text retains styled chunks and their legal line breaks across leaves.

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
}

impl Paint {
    pub(super) fn style(&self, theme: &mut RenderTheme<'_>) -> Style {
        let mut style = self.modifiers;
        style.fg = self.foreground.map(|c| theme.resolve(c));
        style.bg = self.background.map(|c| theme.resolve(c));
        style
    }
}

#[derive(Debug, PartialEq)]
struct Run {
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
    pub(super) width_cells: usize,
    pub(super) trailing_space: Option<Paint>,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct PreparedText {
    pub(super) has_authored_break: bool,
    pub(super) lines: Vec<Vec<Chunk>>,
}

#[derive(Default)]
pub(super) struct InlineText {
    text: PreparedText,
    line: Vec<Run>,
    space: Option<Paint>,
}

impl InlineText {
    pub(super) fn push_leaf(&mut self, raw: String, paint: Paint) {
        let text = RunText::new(sanitize_leaf(raw));
        let mut start = 0;
        for (end, ch) in text
            .match_indices([' ', '\t', '\n'])
            .chain(std::iter::once((text.len(), "")))
        {
            if start < end {
                if let Some(paint) = self.space.take() {
                    self.line.push(Run {
                        text: RunText::Space,
                        paint,
                    });
                }
                self.line.push(Run {
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
                    next.text = next.text.slice(consumed..next.text.len());
                }
                break;
            }
            runs.next();
        }
        joined.push(Run {
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

fn chunks(runs: Vec<Run>) -> Vec<Chunk> {
    let runs = join_graphemes(runs);
    let mut breaks = break_offsets(runs.iter().map(|run| run.text.as_ref()))
        .into_iter()
        .peekable();
    let mut out = Vec::new();
    let mut chunk = Chunk::default();
    let mut offset = 0;
    let mut previous = None;
    let mut word_hyphen = false;
    for run in runs {
        let mut start = 0;
        for (i, grapheme) in run.text.grapheme_indices(true) {
            let position = offset + i;
            while breaks.next_if(|b| *b < position).is_some() {}
            // Match textwrap's default hyphen splitter, even across style boundaries.
            let can_break = match previous {
                Some('-') => word_hyphen && grapheme.starts_with(char::is_alphanumeric),
                Some('\u{ad}') => false,
                _ => breaks.peek() == Some(&position),
            };
            if can_break {
                chunk.push(&run, start..i);
                if chunk.runs.last().is_some_and(|r| &*r.text == " ") {
                    chunk.trailing_space = chunk.runs.pop().map(|r| r.paint);
                    chunk.width_cells -= 1;
                }
                if !chunk.runs.is_empty() {
                    out.push(std::mem::take(&mut chunk));
                }
                start = i;
            }
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

impl Chunk {
    fn push(&mut self, run: &Run, range: Range<usize>) {
        if !range.is_empty() {
            let text = run.text.slice(range);
            self.width_cells += span_width(&Span::raw(&*text));
            self.runs.push(Run {
                text,
                paint: run.paint.clone(),
            });
        }
    }
}

impl PreparedText {
    pub(super) fn layout<'a>(
        &'a self,
        width: usize,
        limit: usize,
        visit: &mut Visit<'a, '_>,
    ) -> usize {
        let mut y = 0;
        for line in &self.lines {
            let mut start = 0;
            let mut used = 0;
            let mut space = 0;
            for (i, chunk) in line.iter().enumerate() {
                if i > start && used + space + chunk.width_cells > width {
                    visit(y, Fragment::Text(&line[start..i], used));
                    y += 1;
                    if y == limit {
                        return y;
                    }
                    start = i;
                    used = 0;
                    space = 0;
                }
                used += space + chunk.width_cells;
                space = usize::from(chunk.trailing_space.is_some());
            }
            visit(y, Fragment::Text(&line[start..], used));
            y += 1;
            if y == limit {
                break;
            }
        }
        y
    }
}

pub(super) fn visit_chunks<'a>(
    chunks: &'a [Chunk],
    limit: usize,
    mut draw: impl FnMut(usize, &str, usize, &'a Paint),
) -> usize {
    let mut used = 0;

    for (index, chunk) in chunks.iter().enumerate() {
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
