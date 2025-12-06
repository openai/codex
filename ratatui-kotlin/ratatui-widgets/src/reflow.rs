//! Internal module for reflowing text to fit into a certain width.
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::mem;

use ratatui_core::layout::Alignment;
use ratatui_core::text::StyledGrapheme;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A state machine to pack styled symbols into lines.
/// Cannot implement it as Iterator since it yields slices of the internal buffer (need streaming
/// iterators for that).
pub trait LineComposer<'a> {
    fn next_line<'lend>(&'lend mut self) -> Option<WrappedLine<'lend, 'a>>;
}

/// A line that has been wrapped to a certain width.
pub struct WrappedLine<'lend, 'text> {
    /// One line reflowed to the correct width
    pub graphemes: &'lend [StyledGrapheme<'text>],
    /// The width of the line
    pub width: u16,
    /// Whether the line was aligned left or right
    pub alignment: Alignment,
}

/// A state machine that wraps lines on word boundaries.
#[derive(Debug, Default, Clone)]
pub struct WordWrapper<'a, O, I>
where
    // Outer iterator providing the individual lines
    O: Iterator<Item = (I, Alignment)>,
    // Inner iterator providing the styled symbols of a line Each line consists of an alignment and
    // a series of symbols
    I: Iterator<Item = StyledGrapheme<'a>>,
{
    /// The given, unprocessed lines
    input_lines: O,
    max_line_width: u16,
    wrapped_lines: VecDeque<Vec<StyledGrapheme<'a>>>,
    current_alignment: Alignment,
    current_line: Vec<StyledGrapheme<'a>>,
    /// Removes the leading whitespace from lines
    trim: bool,

    // These are cached allocations that hold no state across next_line invocations
    pending_word: Vec<StyledGrapheme<'a>>,
    pending_whitespace: VecDeque<StyledGrapheme<'a>>,
    pending_line_pool: Vec<Vec<StyledGrapheme<'a>>>,
}

impl<'a, O, I> WordWrapper<'a, O, I>
where
    O: Iterator<Item = (I, Alignment)>,
    I: Iterator<Item = StyledGrapheme<'a>>,
{
    /// Create a new `WordWrapper` with the given lines and maximum line width.
    pub const fn new(lines: O, max_line_width: u16, trim: bool) -> Self {
        Self {
            input_lines: lines,
            max_line_width,
            wrapped_lines: VecDeque::new(),
            current_alignment: Alignment::Left,
            current_line: vec![],
            trim,

            pending_word: Vec::new(),
            pending_line_pool: Vec::new(),
            pending_whitespace: VecDeque::new(),
        }
    }

    /// Split an input line (`line_symbols`) into wrapped lines
    /// and cache them to be emitted later
    fn process_input(&mut self, line_symbols: impl IntoIterator<Item = StyledGrapheme<'a>>) {
        let mut pending_line = self.pending_line_pool.pop().unwrap_or_default();
        let mut line_width = 0;
        let mut word_width = 0;
        let mut whitespace_width = 0;
        let mut non_whitespace_previous = false;

        self.pending_word.clear();
        self.pending_whitespace.clear();
        pending_line.clear();

        for grapheme in line_symbols {
            let is_whitespace = grapheme.is_whitespace();
            let symbol_width = grapheme.symbol.width() as u16;

            // ignore symbols wider than line limit
            if symbol_width > self.max_line_width {
                continue;
            }

            let word_found = non_whitespace_previous && is_whitespace;
            // current word would overflow after removing whitespace
            let trimmed_overflow = pending_line.is_empty()
                && self.trim
                && word_width + symbol_width > self.max_line_width;
            // separated whitespace would overflow on its own
            let whitespace_overflow = pending_line.is_empty()
                && self.trim
                && whitespace_width + symbol_width > self.max_line_width;
            // current full word (including whitespace) would overflow
            let untrimmed_overflow = pending_line.is_empty()
                && !self.trim
                && word_width + whitespace_width + symbol_width > self.max_line_width;

            // append finished segment to current line
            if word_found || trimmed_overflow || whitespace_overflow || untrimmed_overflow {
                if !pending_line.is_empty() || !self.trim {
                    pending_line.extend(self.pending_whitespace.drain(..));
                    line_width += whitespace_width;
                }

                pending_line.append(&mut self.pending_word);
                line_width += word_width;

                self.pending_whitespace.clear();
                whitespace_width = 0;
                word_width = 0;
            }

            // pending line fills up limit
            let line_full = line_width >= self.max_line_width;
            // pending word would overflow line limit
            let pending_word_overflow = symbol_width > 0
                && line_width + whitespace_width + word_width >= self.max_line_width;

            // add finished wrapped line to remaining lines
            if line_full || pending_word_overflow {
                let mut remaining_width = u16::saturating_sub(self.max_line_width, line_width);

                self.wrapped_lines.push_back(mem::take(&mut pending_line));
                line_width = 0;

                // remove whitespace up to the end of line
                while let Some(grapheme) = self.pending_whitespace.front() {
                    let width = grapheme.symbol.width() as u16;

                    if width > remaining_width {
                        break;
                    }

                    whitespace_width -= width;
                    remaining_width -= width;
                    self.pending_whitespace.pop_front();
                }

                // don't count first whitespace toward next word
                if is_whitespace && self.pending_whitespace.is_empty() {
                    continue;
                }
            }

            // append symbol to a pending buffer
            if is_whitespace {
                whitespace_width += symbol_width;
                self.pending_whitespace.push_back(grapheme);
            } else {
                word_width += symbol_width;
                self.pending_word.push(grapheme);
            }

            non_whitespace_previous = !is_whitespace;
        }

        // append remaining text parts
        if pending_line.is_empty()
            && self.pending_word.is_empty()
            && !self.pending_whitespace.is_empty()
            && self.trim
        {
            self.wrapped_lines.push_back(vec![]);
        }
        if !pending_line.is_empty() || !self.trim {
            pending_line.extend(self.pending_whitespace.drain(..));
        }
        pending_line.append(&mut self.pending_word);

        #[expect(clippy::else_if_without_else)]
        if !pending_line.is_empty() {
            self.wrapped_lines.push_back(pending_line);
        } else if pending_line.capacity() > 0 {
            self.pending_line_pool.push(pending_line);
        }
        if self.wrapped_lines.is_empty() {
            self.wrapped_lines.push_back(vec![]);
        }
    }

    fn replace_current_line(&mut self, line: Vec<StyledGrapheme<'a>>) {
        let cache = mem::replace(&mut self.current_line, line);
        if cache.capacity() > 0 {
            self.pending_line_pool.push(cache);
        }
    }
}

impl<'a, O, I> LineComposer<'a> for WordWrapper<'a, O, I>
where
    O: Iterator<Item = (I, Alignment)>,
    I: Iterator<Item = StyledGrapheme<'a>>,
{
    fn next_line<'lend>(&'lend mut self) -> Option<WrappedLine<'lend, 'a>> {
        if self.max_line_width == 0 {
            return None;
        }

        loop {
            // emit next cached line if present
            if let Some(line) = self.wrapped_lines.pop_front() {
                let line_width = line
                    .iter()
                    .map(|grapheme| grapheme.symbol.width() as u16)
                    .sum();

                self.replace_current_line(line);
                return Some(WrappedLine {
                    graphemes: &self.current_line,
                    width: line_width,
                    alignment: self.current_alignment,
                });
            }

            // otherwise, process pending wrapped lines from input
            let (line_symbols, line_alignment) = self.input_lines.next()?;
            self.current_alignment = line_alignment;
            self.process_input(line_symbols);
        }
    }
}

/// A state machine that truncates overhanging lines.
#[derive(Debug, Default, Clone)]
pub struct LineTruncator<'a, O, I>
where
    // Outer iterator providing the individual lines
    O: Iterator<Item = (I, Alignment)>,
    // Inner iterator providing the styled symbols of a line Each line consists of an alignment and
    // a series of symbols
    I: Iterator<Item = StyledGrapheme<'a>>,
{
    /// The given, unprocessed lines
    input_lines: O,
    max_line_width: u16,
    current_line: Vec<StyledGrapheme<'a>>,
    /// Record the offset to skip render
    horizontal_offset: u16,
}

impl<'a, O, I> LineTruncator<'a, O, I>
where
    O: Iterator<Item = (I, Alignment)>,
    I: Iterator<Item = StyledGrapheme<'a>>,
{
    /// Create a new `LineTruncator` with the given lines and maximum line width.
    pub const fn new(lines: O, max_line_width: u16) -> Self {
        Self {
            input_lines: lines,
            max_line_width,
            horizontal_offset: 0,
            current_line: vec![],
        }
    }

    /// Set the horizontal offset to skip render.
    pub const fn set_horizontal_offset(&mut self, horizontal_offset: u16) {
        self.horizontal_offset = horizontal_offset;
    }
}

impl<'a, O, I> LineComposer<'a> for LineTruncator<'a, O, I>
where
    O: Iterator<Item = (I, Alignment)>,
    I: Iterator<Item = StyledGrapheme<'a>>,
{
    fn next_line<'lend>(&'lend mut self) -> Option<WrappedLine<'lend, 'a>> {
        if self.max_line_width == 0 {
            return None;
        }

        self.current_line.truncate(0);
        let mut current_line_width = 0;

        let mut lines_exhausted = true;
        let mut horizontal_offset = self.horizontal_offset as usize;
        let mut current_alignment = Alignment::Left;
        if let Some((current_line, alignment)) = &mut self.input_lines.next() {
            lines_exhausted = false;
            current_alignment = *alignment;

            for StyledGrapheme { symbol, style } in current_line {
                // Ignore characters wider that the total max width.
                if symbol.width() as u16 > self.max_line_width {
                    continue;
                }

                if current_line_width + symbol.width() as u16 > self.max_line_width {
                    // Truncate line
                    break;
                }

                let symbol = if horizontal_offset == 0 || Alignment::Left != *alignment {
                    symbol
                } else {
                    let w = symbol.width();
                    if w > horizontal_offset {
                        let t = trim_offset(symbol, horizontal_offset);
                        horizontal_offset = 0;
                        t
                    } else {
                        horizontal_offset -= w;
                        ""
                    }
                };
                current_line_width += symbol.width() as u16;
                self.current_line.push(StyledGrapheme { symbol, style });
            }
        }

        if lines_exhausted {
            None
        } else {
            Some(WrappedLine {
                graphemes: &self.current_line,
                width: current_line_width,
                alignment: current_alignment,
            })
        }
    }
}

/// This function will return a str slice which start at specified offset.
/// As src is a unicode str, start offset has to be calculated with each character.
fn trim_offset(src: &str, mut offset: usize) -> &str {
    let mut start = 0;
    for c in UnicodeSegmentation::graphemes(src, true) {
        let w = c.width();
        if w <= offset {
            offset -= w;
            start += c.len();
        } else {
            break;
        }
    }
    #[expect(clippy::string_slice)] // Is safe as it comes from UnicodeSegmentation
    &src[start..]
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::string::String;

    use ratatui_core::style::Style;
    use ratatui_core::text::{Line, Text};

    use super::*;

    #[derive(Clone, Copy)]
    enum Composer {
        WordWrapper { trim: bool },
        LineTruncator,
    }

    fn run_composer<'a>(
        which: Composer,
        text: impl Into<Text<'a>>,
        text_area_width: u16,
    ) -> (Vec<String>, Vec<u16>, Vec<Alignment>) {
        let text = text.into();
        let styled_lines = text.iter().map(|line| {
            (
                line.iter()
                    .flat_map(|span| span.styled_graphemes(Style::default())),
                line.alignment.unwrap_or(Alignment::Left),
            )
        });

        let mut composer: Box<dyn LineComposer> = match which {
            Composer::WordWrapper { trim } => {
                Box::new(WordWrapper::new(styled_lines, text_area_width, trim))
            }
            Composer::LineTruncator => Box::new(LineTruncator::new(styled_lines, text_area_width)),
        };
        let mut lines = vec![];
        let mut widths = vec![];
        let mut alignments = vec![];
        while let Some(WrappedLine {
            graphemes,
            width,
            alignment,
        }) = composer.next_line()
        {
            let line = graphemes
                .iter()
                .map(|StyledGrapheme { symbol, .. }| *symbol)
                .collect::<String>();
            assert!(width <= text_area_width);
            lines.push(line);
            widths.push(width);
            alignments.push(alignment);
        }
        (lines, widths, alignments)
    }

    #[test]
    fn line_composer_one_line() {
        let width = 40;
        for i in 1..width {
            let text = "a".repeat(i);
            let (word_wrapper, _, _) =
                run_composer(Composer::WordWrapper { trim: true }, &*text, width as u16);
            let (line_truncator, _, _) =
                run_composer(Composer::LineTruncator, &*text, width as u16);
            let expected = vec![text];
            assert_eq!(word_wrapper, expected);
            assert_eq!(line_truncator, expected);
        }
    }

    #[test]
    fn line_composer_short_lines() {
        let width = 20;
        let text =
            "abcdefg\nhijklmno\npabcdefg\nhijklmn\nopabcdefghijk\nlmnopabcd\n\n\nefghijklmno";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);

        let wrapped: Vec<&str> = text.split('\n').collect();
        assert_eq!(word_wrapper, wrapped);
        assert_eq!(line_truncator, wrapped);
    }

    #[test]
    fn line_composer_long_word() {
        let width = 20;
        let text = "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmno";
        let (word_wrapper, _, _) =
            run_composer(Composer::WordWrapper { trim: true }, text, width as u16);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width as u16);

        let wrapped = vec![
            text.get(..width).unwrap(),
            text.get(width..width * 2).unwrap(),
            text.get(width * 2..width * 3).unwrap(),
            text.get(width * 3..).unwrap(),
        ];
        assert_eq!(
            word_wrapper, wrapped,
            "WordWrapper should detect the line cannot be broken on word boundary and \
             break it at line width limit."
        );
        assert_eq!(line_truncator, [text.get(..width).unwrap()]);
    }

    #[test]
    fn line_composer_long_sentence() {
        let width = 20;
        let text =
            "abcd efghij klmnopabcd efgh ijklmnopabcdefg hijkl mnopab c d e f g h i j k l m n o";
        let text_multi_space = "abcd efghij    klmnopabcd efgh     ijklmnopabcdefg hijkl mnopab c d e f g h i j k l \
             m n o";
        let (word_wrapper_single_space, _, _) =
            run_composer(Composer::WordWrapper { trim: true }, text, width as u16);
        let (word_wrapper_multi_space, _, _) = run_composer(
            Composer::WordWrapper { trim: true },
            text_multi_space,
            width as u16,
        );
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width as u16);

        let word_wrapped = vec![
            "abcd efghij",
            "klmnopabcd efgh",
            "ijklmnopabcdefg",
            "hijkl mnopab c d e f",
            "g h i j k l m n o",
        ];
        assert_eq!(word_wrapper_single_space, word_wrapped);
        assert_eq!(word_wrapper_multi_space, word_wrapped);

        assert_eq!(line_truncator, [text.get(..width).unwrap()]);
    }

    #[test]
    fn line_composer_zero_width() {
        let width = 0;
        let text = "abcd efghij klmnopabcd efgh ijklmnopabcdefg hijkl mnopab ";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);

        let expected: Vec<&str> = Vec::new();
        assert_eq!(word_wrapper, expected);
        assert_eq!(line_truncator, expected);
    }

    #[test]
    fn line_composer_max_line_width_of_1() {
        let width = 1;
        let text = "abcd efghij klmnopabcd efgh ijklmnopabcdefg hijkl mnopab ";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);

        let expected: Vec<&str> = UnicodeSegmentation::graphemes(text, true)
            .filter(|g| g.chars().any(|c| !c.is_whitespace()))
            .collect();
        assert_eq!(word_wrapper, expected);
        assert_eq!(line_truncator, ["a"]);
    }

    #[test]
    fn line_composer_max_line_width_of_1_double_width_characters() {
        let width = 1;
        let text = "コンピュータ上で文字を扱う場合、典型的には文字\naaa\naによる通信を行う場合にその\
                    両端点では、";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
        assert_eq!(word_wrapper, ["", "a", "a", "a", "a"]);
        assert_eq!(line_truncator, ["", "a", "a"]);
    }

    /// Tests `WordWrapper` with words some of which exceed line length and some not.
    #[test]
    fn line_composer_word_wrapper_mixed_length() {
        let width = 20;
        let text = "abcd efghij klmnopabcdefghijklmnopabcdefghijkl mnopab cdefghi j klmno";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        assert_eq!(
            word_wrapper,
            vec![
                "abcd efghij",
                "klmnopabcdefghijklmn",
                "opabcdefghijkl",
                "mnopab cdefghi j",
                "klmno",
            ]
        );
    }

    #[test]
    fn line_composer_double_width_chars() {
        let width = 20;
        let text = "コンピュータ上で文字を扱う場合、典型的には文字による通信を行う場合にその両端点\
                    では、";
        let (word_wrapper, word_wrapper_width, _) =
            run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
        assert_eq!(line_truncator, ["コンピュータ上で文字"]);
        let wrapped = [
            "コンピュータ上で文字",
            "を扱う場合、典型的に",
            "は文字による通信を行",
            "う場合にその両端点で",
            "は、",
        ];
        assert_eq!(word_wrapper, wrapped);
        assert_eq!(word_wrapper_width, [width, width, width, width, 4]);
    }

    #[test]
    fn line_composer_leading_whitespace_removal() {
        let width = 20;
        let text = "AAAAAAAAAAAAAAAAAAAA    AAA";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
        assert_eq!(word_wrapper, ["AAAAAAAAAAAAAAAAAAAA", "AAA"]);
        assert_eq!(line_truncator, ["AAAAAAAAAAAAAAAAAAAA"]);
    }

    /// Tests truncation of leading whitespace.
    #[test]
    fn line_composer_lots_of_spaces() {
        let width = 20;
        let text = "                                                                     ";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
        assert_eq!(word_wrapper, [""]);
        assert_eq!(line_truncator, ["                    "]);
    }

    /// Tests an input starting with a letter, followed by spaces - some of the behaviour is
    /// incidental.
    #[test]
    fn line_composer_char_plus_lots_of_spaces() {
        let width = 20;
        let text = "a                                                                     ";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, text, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, text, width);
        // What's happening below is: the first line gets consumed, trailing spaces discarded,
        // after 20 of which a word break occurs (probably shouldn't). The second line break
        // discards all whitespace. The result should probably be vec!["a"] but it doesn't matter
        // that much.
        assert_eq!(word_wrapper, ["a", ""]);
        assert_eq!(line_truncator, ["a                   "]);
    }

    #[test]
    fn line_composer_word_wrapper_double_width_chars_mixed_with_spaces() {
        let width = 20;
        // Japanese seems not to use spaces but we should break on spaces anyway... We're using it
        // to test double-width chars.
        // You are more than welcome to add word boundary detection based of alterations of
        // hiragana and katakana...
        // This happens to also be a test case for mixed width because regular spaces are single
        // width.
        let text = "コンピュ ータ上で文字を扱う場合、 典型的には文 字による 通信を行 う場合にその両端点では、";
        let (word_wrapper, word_wrapper_width, _) =
            run_composer(Composer::WordWrapper { trim: true }, text, width);
        assert_eq!(
            word_wrapper,
            vec![
                "コンピュ",
                "ータ上で文字を扱う場",
                "合、 典型的には文",
                "字による 通信を行",
                "う場合にその両端点で",
                "は、",
            ]
        );
        // Odd-sized lines have a space in them.
        assert_eq!(word_wrapper_width, [8, 20, 17, 17, 20, 4]);
    }

    /// Ensure words separated by nbsp are wrapped as if they were a single one.
    #[test]
    fn line_composer_word_wrapper_nbsp() {
        let width = 20;
        let text = "AAAAAAAAAAAAAAA AAAA\u{00a0}AAA";
        let (word_wrapper, word_wrapper_widths, _) =
            run_composer(Composer::WordWrapper { trim: true }, text, width);
        assert_eq!(word_wrapper, ["AAAAAAAAAAAAAAA", "AAAA\u{00a0}AAA"]);
        assert_eq!(word_wrapper_widths, [15, 8]);

        // Ensure that if the character was a regular space, it would be wrapped differently.
        let text_space = text.replace('\u{00a0}', " ");
        let (word_wrapper_space, word_wrapper_widths, _) =
            run_composer(Composer::WordWrapper { trim: true }, text_space, width);
        assert_eq!(word_wrapper_space, ["AAAAAAAAAAAAAAA AAAA", "AAA"]);
        assert_eq!(word_wrapper_widths, [20, 3]);
    }

    #[test]
    fn line_composer_word_wrapper_preserve_indentation() {
        let width = 20;
        let text = "AAAAAAAAAAAAAAAAAAAA    AAA";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: false }, text, width);
        assert_eq!(word_wrapper, ["AAAAAAAAAAAAAAAAAAAA", "   AAA"]);
    }

    #[test]
    fn line_composer_word_wrapper_preserve_indentation_with_wrap() {
        let width = 10;
        let text = "AAA AAA AAAAA AA AAAAAA\n B\n  C\n   D";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: false }, text, width);
        assert_eq!(
            word_wrapper,
            vec!["AAA AAA", "AAAAA AA", "AAAAAA", " B", "  C", "   D"]
        );
    }

    #[test]
    fn line_composer_word_wrapper_preserve_indentation_lots_of_whitespace() {
        let width = 10;
        let text = "               4 Indent\n                 must wrap!";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: false }, text, width);
        assert_eq!(
            word_wrapper,
            vec![
                "          ",
                "    4",
                "Indent",
                "          ",
                "      must",
                "wrap!"
            ]
        );
    }

    #[test]
    fn line_composer_zero_width_at_end() {
        let width = 3;
        let line = "foo\u{200B}";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, line, width);
        let (line_truncator, _, _) = run_composer(Composer::LineTruncator, line, width);
        assert_eq!(word_wrapper, ["foo"]);
        assert_eq!(line_truncator, ["foo\u{200B}"]);
    }

    #[test]
    fn line_composer_preserves_line_alignment() {
        let width = 20;
        let lines = vec![
            Line::from("Something that is left aligned.").alignment(Alignment::Left),
            Line::from("This is right aligned and half short.").alignment(Alignment::Right),
            Line::from("This should sit in the center.").alignment(Alignment::Center),
        ];
        let (_, _, wrapped_alignments) =
            run_composer(Composer::WordWrapper { trim: true }, lines.clone(), width);
        let (_, _, truncated_alignments) = run_composer(Composer::LineTruncator, lines, width);
        assert_eq!(
            wrapped_alignments,
            vec![
                Alignment::Left,
                Alignment::Left,
                Alignment::Right,
                Alignment::Right,
                Alignment::Right,
                Alignment::Center,
                Alignment::Center
            ]
        );
        assert_eq!(
            truncated_alignments,
            vec![Alignment::Left, Alignment::Right, Alignment::Center]
        );
    }

    #[test]
    fn line_composer_zero_width_white_space() {
        let width = 3;
        let line = "foo\u{200b}bar";
        let (word_wrapper, _, _) = run_composer(Composer::WordWrapper { trim: true }, line, width);
        assert_eq!(word_wrapper, ["foo", "bar"]);
    }
}
