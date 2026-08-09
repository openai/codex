//! Grapheme-safe composer wrapping that preserves textwrap's semantic breakpoints.
//!
//! Source ranges include a sentinel byte for cursor placement. Overflowing breakable whitespace
//! stays attached to the following word, while full logical lines reserve an insertion row.

use crate::width::display_width;
use std::ops::Range;
use textwrap::Options;
use unicode_segmentation::UnicodeSegmentation;

/// Cached source span and display width for a word identified by `textwrap`.
struct WrappedWord {
    range: Range<usize>,
    width: usize,
}

/// Returns grapheme-safe visual ranges, each including its cursor-position sentinel byte.
///
/// Breakable Unicode whitespace stays with the next word, nonbreaking whitespace remains part of
/// its word, and full logical lines receive an empty insertion row. Word boundaries and widths are
/// indexed once so maximum-size pasted lines remain linear to wrap.
pub(super) fn wrapped_lines(text: &str, width: u16) -> Vec<Range<usize>> {
    let width = usize::from(width);
    let options = Options::new(width).wrap_algorithm(textwrap::WrapAlgorithm::FirstFit);
    let wrapped_lines = crate::wrapping::wrap_ranges(text, &options);
    if width == 0 {
        return wrapped_lines;
    }

    let mut breakpoints = vec![false; text.len() + 1];
    let mut words = Vec::new();
    let mut logical_start = 0;
    for logical_line in text.split('\n') {
        let mut word_start = logical_start;
        for word in options.word_separator.find_words(logical_line) {
            let word_end = word_start + word.word.len();
            for breakpoint in options.word_splitter.split_points(word.word) {
                breakpoints[word_start + breakpoint] = true;
            }
            breakpoints[word_end] = true;
            words.push(WrappedWord {
                range: word_start..word_end,
                width: display_width(word.word),
            });
            word_start = word_end + word.whitespace.len();
        }
        logical_start += logical_line.len() + 1;
    }

    let mut lines = Vec::with_capacity(wrapped_lines.len());
    let mut line_start = 0;
    let mut line_width = 0;
    let mut line_has_text = false;
    let mut line_active = false;
    let mut processed_end = 0;
    let mut previous_was_whitespace = false;
    let mut word_index = 0;
    let mut consumed_word_width = 0;

    for wrapped_line in wrapped_lines {
        let line_end = wrapped_line.end.saturating_sub(1);
        // textwrap can repeat leading whitespace around punctuation boundaries.
        let fragment_start = wrapped_line.start.max(processed_end);
        if fragment_start > line_end {
            continue;
        }

        while word_index < words.len() && words[word_index].range.end <= fragment_start {
            word_index += 1;
            consumed_word_width = 0;
        }

        if !line_active {
            line_start = fragment_start;
            line_active = true;
        } else if line_has_text && breakpoints[fragment_start] {
            let remaining_word_width = words
                .get(word_index)
                .filter(|word| word.range.contains(&fragment_start))
                .map_or(0, |word| word.width.saturating_sub(consumed_word_width));
            // Keep textwrap's break if crossing it would split the next word.
            if line_width + remaining_word_width > width {
                lines.push(line_start..fragment_start + 1);
                line_start = fragment_start;
                line_width = 0;
                line_has_text = false;
            }
        }

        for (offset, grapheme) in
            text[fragment_start..line_end].grapheme_indices(/*is_extended*/ true)
        {
            let grapheme_start = fragment_start + offset;
            while word_index < words.len() && words[word_index].range.end <= grapheme_start {
                word_index += 1;
                consumed_word_width = 0;
            }

            let grapheme_end = grapheme_start + grapheme.len();
            let is_whitespace = grapheme.chars().all(char::is_whitespace)
                && (grapheme == " " || breakpoints[grapheme_end]);
            // Partial whitespace rows absorb the next word; text rows keep it intact.
            if !is_whitespace && line_has_text && previous_was_whitespace {
                let word_end = words
                    .get(word_index)
                    .filter(|word| word.range.contains(&grapheme_start))
                    .map_or(line_end, |word| word.range.end.min(line_end));
                if line_width + display_width(&text[grapheme_start..word_end]) > width {
                    lines.push(line_start..grapheme_start + 1);
                    line_start = grapheme_start;
                    line_width = 0;
                    line_has_text = false;
                }
            }

            let grapheme_width = display_width(grapheme);
            if line_width > 0 && line_width + grapheme_width > width {
                lines.push(line_start..grapheme_start + 1);
                line_start = grapheme_start;
                line_width = 0;
                line_has_text = false;
            }
            line_width += grapheme_width;
            line_has_text |= !is_whitespace;
            previous_was_whitespace = is_whitespace;

            if words
                .get(word_index)
                .is_some_and(|word| word.range.contains(&grapheme_start))
            {
                consumed_word_width += grapheme_width;
            }
        }

        if matches!(text.as_bytes().get(line_end), None | Some(b'\n')) {
            lines.push(line_start..line_end + 1);
            if line_width >= width {
                lines.push(line_end..line_end + 1);
            }
            line_width = 0;
            line_has_text = false;
            line_active = false;
            previous_was_whitespace = false;
        }
        processed_end = line_end;
    }

    lines
}

#[cfg(test)]
#[path = "wrapping_tests.rs"]
mod tests;
