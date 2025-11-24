//! Functionality for finding words.
//!
//! In order to wrap text, we need to know where the legal break
//! points are, i.e., where the words of the text are. This means that
//! we need to define what a "word" is.
//!
//! A simple approach is to simply split the text on whitespace, but
//! this does not work for East-Asian languages such as Chinese or
//! Japanese where there are no spaces between words. Breaking a long
//! sequence of emojis is another example where line breaks might be
//! wanted even if there are no whitespace to be found.
//!
//! The [`WordSeparator`] enum is responsible for determining where
//! there words are in a line of text. Please refer to the enum and
//! its variants for more information.

#[cfg(feature = "unicode-linebreak")]
use crate::core::skip_ansi_escape_sequence;
use crate::core::Word;

/// Describes where words occur in a line of text.
///
/// The simplest approach is say that words are separated by one or
/// more ASCII spaces (`' '`). This works for Western languages
/// without emojis. A more complex approach is to use the Unicode line
/// breaking algorithm, which finds break points in non-ASCII text.
///
/// The line breaks occur between words, please see
/// [`WordSplitter`](crate::WordSplitter) for options of how to handle
/// hyphenation of individual words.
///
/// # Examples
///
/// ```
/// use textwrap::core::Word;
/// use textwrap::WordSeparator::AsciiSpace;
///
/// let words = AsciiSpace.find_words("Hello World!").collect::<Vec<_>>();
/// assert_eq!(words, vec![Word::from("Hello "), Word::from("World!")]);
/// ```
#[derive(Debug, Clone, Copy)]
pub enum WordSeparator {
    /// Find words by splitting on runs of `' '` characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use textwrap::core::Word;
    /// use textwrap::WordSeparator::AsciiSpace;
    ///
    /// let words = AsciiSpace.find_words("Hello   World!").collect::<Vec<_>>();
    /// assert_eq!(words, vec![Word::from("Hello   "),
    ///                        Word::from("World!")]);
    /// ```
    AsciiSpace,

    /// Split `line` into words using Unicode break properties.
    ///
    /// This word separator uses the Unicode line breaking algorithm
    /// described in [Unicode Standard Annex
    /// #14](https://www.unicode.org/reports/tr14/) to find legal places
    /// to break lines. There is a small difference in that the U+002D
    /// (Hyphen-Minus) and U+00AD (Soft Hyphen) don’t create a line break:
    /// to allow a line break at a hyphen, use
    /// [`WordSplitter::HyphenSplitter`](crate::WordSplitter::HyphenSplitter).
    /// Soft hyphens are not currently supported.
    ///
    /// # Examples
    ///
    /// Unlike [`WordSeparator::AsciiSpace`], the Unicode line
    /// breaking algorithm will find line break opportunities between
    /// some characters with no intervening whitespace:
    ///
    /// ```
    /// #[cfg(feature = "unicode-linebreak")] {
    /// use textwrap::core::Word;
    /// use textwrap::WordSeparator::UnicodeBreakProperties;
    ///
    /// assert_eq!(UnicodeBreakProperties.find_words("Emojis: 😂😍").collect::<Vec<_>>(),
    ///            vec![Word::from("Emojis: "),
    ///                 Word::from("😂"),
    ///                 Word::from("😍")]);
    ///
    /// assert_eq!(UnicodeBreakProperties.find_words("CJK: 你好").collect::<Vec<_>>(),
    ///            vec![Word::from("CJK: "),
    ///                 Word::from("你"),
    ///                 Word::from("好")]);
    /// }
    /// ```
    ///
    /// A U+2060 (Word Joiner) character can be inserted if you want to
    /// manually override the defaults and keep the characters together:
    ///
    /// ```
    /// #[cfg(feature = "unicode-linebreak")] {
    /// use textwrap::core::Word;
    /// use textwrap::WordSeparator::UnicodeBreakProperties;
    ///
    /// assert_eq!(UnicodeBreakProperties.find_words("Emojis: 😂\u{2060}😍").collect::<Vec<_>>(),
    ///            vec![Word::from("Emojis: "),
    ///                 Word::from("😂\u{2060}😍")]);
    /// }
    /// ```
    ///
    /// The Unicode line breaking algorithm will also automatically
    /// suppress break breaks around certain punctuation characters::
    ///
    /// ```
    /// #[cfg(feature = "unicode-linebreak")] {
    /// use textwrap::core::Word;
    /// use textwrap::WordSeparator::UnicodeBreakProperties;
    ///
    /// assert_eq!(UnicodeBreakProperties.find_words("[ foo ] bar !").collect::<Vec<_>>(),
    ///            vec![Word::from("[ foo ] "),
    ///                 Word::from("bar !")]);
    /// }
    /// ```
    #[cfg(feature = "unicode-linebreak")]
    UnicodeBreakProperties,

    /// Find words using a custom word separator
    Custom(fn(line: &str) -> Box<dyn Iterator<Item = Word<'_>> + '_>),
}

impl PartialEq for WordSeparator {
    /// Compare two word separators.
    ///
    /// ```
    /// use textwrap::WordSeparator;
    ///
    /// assert_eq!(WordSeparator::AsciiSpace, WordSeparator::AsciiSpace);
    /// #[cfg(feature = "unicode-linebreak")] {
    ///     assert_eq!(WordSeparator::UnicodeBreakProperties,
    ///                WordSeparator::UnicodeBreakProperties);
    /// }
    /// ```
    ///
    /// Note that `WordSeparator::Custom` values never compare equal:
    ///
    /// ```
    /// use textwrap::WordSeparator;
    /// use textwrap::core::Word;
    /// fn word_separator(line: &str) -> Box<dyn Iterator<Item = Word<'_>> + '_> {
    ///     Box::new(line.split_inclusive(' ').map(Word::from))
    /// }
    /// assert_ne!(WordSeparator::Custom(word_separator),
    ///            WordSeparator::Custom(word_separator));
    /// ```
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (WordSeparator::AsciiSpace, WordSeparator::AsciiSpace) => true,
            #[cfg(feature = "unicode-linebreak")]
            (WordSeparator::UnicodeBreakProperties, WordSeparator::UnicodeBreakProperties) => true,
            (_, _) => false,
        }
    }
}

impl WordSeparator {
    /// Create a new word separator.
    ///
    /// The best available algorithm is used by default, i.e.,
    /// [`WordSeparator::UnicodeBreakProperties`] if available,
    /// otherwise [`WordSeparator::AsciiSpace`].
    pub const fn new() -> Self {
        #[cfg(feature = "unicode-linebreak")]
        {
            WordSeparator::UnicodeBreakProperties
        }

        #[cfg(not(feature = "unicode-linebreak"))]
        {
            WordSeparator::AsciiSpace
        }
    }

    // This function should really return impl Iterator<Item = Word>, but
    // this isn't possible until Rust supports higher-kinded types:
    // https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md
    /// Find all words in `line`.
    pub fn find_words<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = Word<'a>> + 'a> {
        match self {
            WordSeparator::AsciiSpace => find_words_ascii_space(line),
            #[cfg(feature = "unicode-linebreak")]
            WordSeparator::UnicodeBreakProperties => find_words_unicode_break_properties(line),
            WordSeparator::Custom(func) => func(line),
        }
    }
}

fn find_words_ascii_space<'a>(line: &'a str) -> Box<dyn Iterator<Item = Word<'a>> + 'a> {
    let mut start = 0;
    let mut in_whitespace = false;
    let mut char_indices = line.char_indices();

    Box::new(std::iter::from_fn(move || {
        for (idx, ch) in char_indices.by_ref() {
            if in_whitespace && ch != ' ' {
                let word = Word::from(&line[start..idx]);
                start = idx;
                in_whitespace = ch == ' ';
                return Some(word);
            }

            in_whitespace = ch == ' ';
        }

        if start < line.len() {
            let word = Word::from(&line[start..]);
            start = line.len();
            return Some(word);
        }

        None
    }))
}

// Strip all ANSI escape sequences from `text`.
#[cfg(feature = "unicode-linebreak")]
fn strip_ansi_escape_sequences(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if skip_ansi_escape_sequence(ch, &mut chars) {
            continue;
        }
        result.push(ch);
    }

    result
}

/// Soft hyphen, also knows as a “shy hyphen”. Should show up as ‘-’
/// if a line is broken at this point, and otherwise be invisible.
/// Textwrap does not currently support breaking words at soft
/// hyphens.
#[cfg(feature = "unicode-linebreak")]
const SHY: char = '\u{00ad}';

/// Find words in line. ANSI escape sequences are ignored in `line`.
#[cfg(feature = "unicode-linebreak")]
fn find_words_unicode_break_properties<'a>(
    line: &'a str,
) -> Box<dyn Iterator<Item = Word<'a>> + 'a> {
    // Construct an iterator over (original index, stripped index)
    // tuples. We find the Unicode linebreaks on a stripped string,
    // but we need the original indices so we can form words based on
    // the original string.
    let mut last_stripped_idx = 0;
    let mut char_indices = line.char_indices();
    let mut idx_map = std::iter::from_fn(move || match char_indices.next() {
        Some((orig_idx, ch)) => {
            let stripped_idx = last_stripped_idx;
            if !skip_ansi_escape_sequence(ch, &mut char_indices.by_ref().map(|(_, ch)| ch)) {
                last_stripped_idx += ch.len_utf8();
            }
            Some((orig_idx, stripped_idx))
        }
        None => None,
    });

    let stripped = strip_ansi_escape_sequences(line);
    let mut opportunities = unicode_linebreak::linebreaks(&stripped)
        .filter(|(idx, _)| {
            #[allow(clippy::match_like_matches_macro)]
            match &stripped[..*idx].chars().next_back() {
                // We suppress breaks at ‘-’ since we want to control
                // this via the WordSplitter.
                Some('-') => false,
                // Soft hyphens are currently not supported since we
                // require all `Word` fragments to be continuous in
                // the input string.
                Some(SHY) => false,
                // Other breaks should be fine!
                _ => true,
            }
        })
        .collect::<Vec<_>>()
        .into_iter();

    // Remove final break opportunity, we will add it below using
    // &line[start..]; This ensures that we correctly include a
    // trailing ANSI escape sequence.
    opportunities.next_back();

    let mut start = 0;
    Box::new(std::iter::from_fn(move || {
        for (idx, _) in opportunities.by_ref() {
            if let Some((orig_idx, _)) = idx_map.find(|&(_, stripped_idx)| stripped_idx == idx) {
                let word = Word::from(&line[start..orig_idx]);
                start = orig_idx;
                return Some(word);
            }
        }

        if start < line.len() {
            let word = Word::from(&line[start..]);
            start = line.len();
            return Some(word);
        }

        None
    }))
}

#[cfg(test)]
mod tests {
    use super::WordSeparator::*;
    use super::*;

    // Like assert_eq!, but the left expression is an iterator.
    macro_rules! assert_iter_eq {
        ($left:expr, $right:expr) => {
            assert_eq!($left.collect::<Vec<_>>(), $right);
        };
    }

    fn to_words(words: Vec<&str>) -> Vec<Word<'_>> {
        words.into_iter().map(Word::from).collect()
    }

    macro_rules! test_find_words {
        ($ascii_name:ident,
         $unicode_name:ident,
         $([ $line:expr, $ascii_words:expr, $unicode_words:expr ]),+) => {
            #[test]
            fn $ascii_name() {
                $(
                    let expected_words = to_words($ascii_words.to_vec());
                    let actual_words = WordSeparator::AsciiSpace
                        .find_words($line)
                        .collect::<Vec<_>>();
                    assert_eq!(actual_words, expected_words, "Line: {:?}", $line);
                )+
            }

            #[test]
            #[cfg(feature = "unicode-linebreak")]
            fn $unicode_name() {
                $(
                    let expected_words = to_words($unicode_words.to_vec());
                    let actual_words = WordSeparator::UnicodeBreakProperties
                        .find_words($line)
                        .collect::<Vec<_>>();
                    assert_eq!(actual_words, expected_words, "Line: {:?}", $line);
                )+
            }
        };
    }

    test_find_words!(ascii_space_empty, unicode_empty, ["", [], []]);

    test_find_words!(
        ascii_single_word,
        unicode_single_word,
        ["foo", ["foo"], ["foo"]]
    );

    test_find_words!(
        ascii_two_words,
        unicode_two_words,
        ["foo bar", ["foo ", "bar"], ["foo ", "bar"]]
    );

    test_find_words!(
        ascii_multiple_words,
        unicode_multiple_words,
        ["foo bar", ["foo ", "bar"], ["foo ", "bar"]],
        ["x y z", ["x ", "y ", "z"], ["x ", "y ", "z"]]
    );

    test_find_words!(
        ascii_only_whitespace,
        unicode_only_whitespace,
        [" ", [" "], [" "]],
        ["    ", ["    "], ["    "]]
    );

    test_find_words!(
        ascii_inter_word_whitespace,
        unicode_inter_word_whitespace,
        ["foo   bar", ["foo   ", "bar"], ["foo   ", "bar"]]
    );

    test_find_words!(
        ascii_trailing_whitespace,
        unicode_trailing_whitespace,
        ["foo   ", ["foo   "], ["foo   "]]
    );

    test_find_words!(
        ascii_leading_whitespace,
        unicode_leading_whitespace,
        ["   foo", ["   ", "foo"], ["   ", "foo"]]
    );

    test_find_words!(
        ascii_multi_column_char,
        unicode_multi_column_char,
        ["\u{1f920}", ["\u{1f920}"], ["\u{1f920}"]] // cowboy emoji 🤠
    );

    test_find_words!(
        ascii_hyphens,
        unicode_hyphens,
        ["foo-bar", ["foo-bar"], ["foo-bar"]],
        ["foo- bar", ["foo- ", "bar"], ["foo- ", "bar"]],
        ["foo - bar", ["foo ", "- ", "bar"], ["foo ", "- ", "bar"]],
        ["foo -bar", ["foo ", "-bar"], ["foo ", "-bar"]]
    );

    test_find_words!(
        ascii_newline,
        unicode_newline,
        ["foo\nbar", ["foo\nbar"], ["foo\n", "bar"]]
    );

    test_find_words!(
        ascii_tab,
        unicode_tab,
        ["foo\tbar", ["foo\tbar"], ["foo\t", "bar"]]
    );

    test_find_words!(
        ascii_non_breaking_space,
        unicode_non_breaking_space,
        ["foo\u{00A0}bar", ["foo\u{00A0}bar"], ["foo\u{00A0}bar"]]
    );

    #[test]
    #[cfg(unix)]
    fn find_words_colored_text() {
        use termion::color::{Blue, Fg, Green, Reset};

        let green_hello = format!("{}Hello{} ", Fg(Green), Fg(Reset));
        let blue_world = format!("{}World!{}", Fg(Blue), Fg(Reset));
        assert_iter_eq!(
            AsciiSpace.find_words(&format!("{}{}", green_hello, blue_world)),
            vec![Word::from(&green_hello), Word::from(&blue_world)]
        );

        #[cfg(feature = "unicode-linebreak")]
        assert_iter_eq!(
            UnicodeBreakProperties.find_words(&format!("{}{}", green_hello, blue_world)),
            vec![Word::from(&green_hello), Word::from(&blue_world)]
        );
    }

    #[test]
    fn find_words_color_inside_word() {
        let text = "foo\u{1b}[0m\u{1b}[32mbar\u{1b}[0mbaz";
        assert_iter_eq!(AsciiSpace.find_words(text), vec![Word::from(text)]);

        #[cfg(feature = "unicode-linebreak")]
        assert_iter_eq!(
            UnicodeBreakProperties.find_words(text),
            vec![Word::from(text)]
        );
    }

    #[test]
    fn word_separator_new() {
        #[cfg(feature = "unicode-linebreak")]
        assert!(matches!(WordSeparator::new(), UnicodeBreakProperties));

        #[cfg(not(feature = "unicode-linebreak"))]
        assert!(matches!(WordSeparator::new(), AsciiSpace));
    }
}
