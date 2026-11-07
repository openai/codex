//! Prepared inline text retains styled chunks and their legal line breaks across leaves.

use super::push_link;
use super::sanitize_leaf;
use crate::line_truncation::span_width;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::style::Style;
use ratatui::text::Span;
use std::borrow::Cow;
use std::ops::Range;
use unicode_linebreak::linebreaks;
use unicode_linebreak::split_at_safe;
use unicode_segmentation::GraphemeCursor;
use unicode_segmentation::GraphemeIncomplete;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug)]
pub(super) struct Paint<'a> {
    pub(super) style: Style,
    pub(super) href: Option<&'a str>,
}

#[derive(Debug)]
struct Run<'a> {
    text: Cow<'a, str>,
    paint: Paint<'a>,
}

fn slice<'a>(text: &Cow<'a, str>, range: Range<usize>) -> Cow<'a, str> {
    match text {
        Cow::Borrowed(raw) => Cow::Borrowed(&raw[range]),
        Cow::Owned(raw) => Cow::Owned(raw[range].to_owned()),
    }
}

#[derive(Debug, Default)]
pub(super) struct Chunk<'a> {
    runs: Vec<Run<'a>>,
    pub(super) width_cells: usize,
    pub(super) trailing_space: Option<Paint<'a>>,
}

#[derive(Default)]
pub(super) struct PreparedText<'a> {
    pub(super) lines: Vec<Vec<Chunk<'a>>>,
}

#[derive(Default)]
pub(super) struct InlineText<'a> {
    text: PreparedText<'a>,
    line: Vec<Run<'a>>,
    space: Option<Paint<'a>>,
}

impl<'a> InlineText<'a> {
    pub(super) fn push_leaf(&mut self, raw: &'a str, paint: Paint<'a>) {
        let text = sanitize_leaf(raw);
        let mut start = 0;
        for (end, ch) in text
            .match_indices([' ', '\t', '\n'])
            .chain(std::iter::once((text.len(), "")))
        {
            if start < end {
                if let Some(paint) = self.space.take() {
                    self.line.push(Run {
                        text: " ".into(),
                        paint,
                    });
                }
                self.line.push(Run {
                    text: slice(&text, start..end),
                    paint,
                });
            }
            if ch == "\n" {
                self.text.lines.push(chunks(std::mem::take(&mut self.line)));
                self.space = None;
            } else if !ch.is_empty() && !self.line.is_empty() {
                self.space.get_or_insert(paint);
            }
            start = end + ch.len();
        }
    }

    pub(super) fn finish(mut self) -> PreparedText<'a> {
        if !self.line.is_empty() {
            self.text.lines.push(chunks(self.line));
        }
        self.text
    }
}

// Most seams already fall between graphemes. Only a cluster that actually crosses a
// seam needs contiguous storage, including when it spans more than two leaves.
fn join_graphemes(runs: Vec<Run<'_>>) -> Vec<Run<'_>> {
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
                text: slice(&run.text, 0..prefix),
                paint,
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
                    next.text = slice(&next.text, consumed..next.text.len());
                }
                break;
            }
            runs.next();
        }
        joined.push(Run {
            text: Cow::Owned(cluster),
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

fn chunks(runs: Vec<Run<'_>>) -> Vec<Chunk<'_>> {
    let runs = join_graphemes(runs);
    let mut breaks = break_offsets(runs.iter().map(|run| run.text.as_ref()))
        .into_iter()
        .peekable();
    let mut out = Vec::new();
    let mut chunk = Chunk::default();
    let mut offset = 0;
    for run in runs {
        let mut start = 0;
        for (i, _) in run.text.grapheme_indices(true) {
            while breaks.next_if(|b| *b < offset + i).is_some() {}
            if breaks.next_if_eq(&(offset + i)).is_none() {
                continue;
            }
            chunk.push(&run, start..i);
            if chunk.runs.last().is_some_and(|r| r.text == " ") {
                chunk.trailing_space = chunk.runs.pop().map(|r| r.paint);
                chunk.width_cells -= 1;
            }
            if !chunk.runs.is_empty() {
                out.push(std::mem::take(&mut chunk));
            }
            start = i;
        }
        chunk.push(&run, start..run.text.len());
        offset += run.text.len();
    }
    if !chunk.runs.is_empty() {
        out.push(chunk);
    }
    out
}

impl<'a> Chunk<'a> {
    fn push(&mut self, run: &Run<'a>, range: Range<usize>) {
        if !range.is_empty() {
            let text = slice(&run.text, range);
            self.width_cells += span_width(&Span::raw(text.as_ref()));
            self.runs.push(Run { text, ..*run });
        }
    }
}

pub(super) fn paint_chunks(chunks: &[Chunk<'_>], line: &mut HyperlinkLine, start: usize) -> usize {
    let mut col = start;
    for (index, chunk) in chunks.iter().enumerate() {
        for run in &chunk.runs {
            let span = Span::styled(run.text.to_string(), run.paint.style);
            let end = col + span_width(&span);
            if let Some(url) = run.paint.href
                && end > col
            {
                push_link(line, col..end, url);
            }
            line.line.push_span(span);
            col = end;
        }
        if index + 1 < chunks.len()
            && let Some(paint) = chunk.trailing_space
        {
            line.line.push_span(Span::styled(" ", paint.style));
            if let Some(url) = paint.href {
                push_link(line, col..col + 1, url);
            }
            col += 1;
        }
    }
    col - start
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
