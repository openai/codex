//! `textwrap` provides functions for word wrapping and filling text.
//!
//! Wrapping text can be very useful in commandline programs where you
//! want to format dynamic output nicely so it looks good in a
//! terminal. A quick example:
//!
//! ```no_run
//! extern crate textwrap;
//! use textwrap::fill;
//!
//! fn main() {
//!     let text = "textwrap: a small library for wrapping text.";
//!     println!("{}", fill(text, 18));
//! }
//! ```
//!
//! This will display the following output:
//!
//! ```text
//! textwrap: a small
//! library for
//! wrapping text.
//! ```
//!
//! # Displayed Width vs Byte Size
//!
//! To word wrap text, one must know the width of each word so one can
//! know when to break lines. This library measures the width of text
//! using the [displayed width][unicode-width], not the size in bytes.
//!
//! This is important for non-ASCII text. ASCII characters such as `a`
//! and `!` are simple and take up one column each. This means that
//! the displayed width is equal to the string length in bytes.
//! However, non-ASCII characters and symbols take up more than one
//! byte when UTF-8 encoded: `é` is `0xc3 0xa9` (two bytes) and `⚙` is
//! `0xe2 0x9a 0x99` (three bytes) in UTF-8, respectively.
//!
//! This is why we take care to use the displayed width instead of the
//! byte count when computing line lengths. All functions in this
//! library handle Unicode characters like this.
//!
//! [unicode-width]: https://docs.rs/unicode-width/

#![doc(html_root_url = "https://docs.rs/textwrap/0.11.0")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

#[cfg(feature = "hyphenation")]
extern crate hyphenation;
#[cfg(feature = "term_size")]
extern crate term_size;
extern crate unicode_width;

use std::borrow::Cow;
use std::str::CharIndices;

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// A non-breaking space.
const NBSP: char = '\u{a0}';

mod indentation;
pub use indentation::dedent;
pub use indentation::indent;

mod splitting;
pub use splitting::{HyphenSplitter, NoHyphenation, WordSplitter};

/// A Wrapper holds settings for wrapping and filling text. Use it
/// when the convenience [`wrap_iter`], [`wrap`] and [`fill`] functions
/// are not flexible enough.
///
/// [`wrap_iter`]: fn.wrap_iter.html
/// [`wrap`]: fn.wrap.html
/// [`fill`]: fn.fill.html
///
/// The algorithm used by the `WrapIter` iterator (returned from the
/// `wrap_iter` method)  works by doing successive partial scans over
/// words in the input string (where each single scan yields a single
/// line) so that the overall time and memory complexity is O(*n*) where
/// *n* is the length of the input string.
#[derive(Clone, Debug)]
pub struct Wrapper<'a, S: WordSplitter> {
    /// The width in columns at which the text will be wrapped.
    pub width: usize,
    /// Indentation used for the first line of output.
    pub initial_indent: &'a str,
    /// Indentation used for subsequent lines of output.
    pub subsequent_indent: &'a str,
    /// Allow long words to be broken if they cannot fit on a line.
    /// When set to `false`, some lines may be longer than
    /// `self.width`.
    pub break_words: bool,
    /// The method for splitting words. If the `hyphenation` feature
    /// is enabled, you can use a `hyphenation::Standard` dictionary
    /// here to get language-aware hyphenation.
    pub splitter: S,
}

impl<'a> Wrapper<'a, HyphenSplitter> {
    /// Create a new Wrapper for wrapping at the specified width. By
    /// default, we allow words longer than `width` to be broken. A
    /// [`HyphenSplitter`] will be used by default for splitting
    /// words. See the [`WordSplitter`] trait for other options.
    ///
    /// [`HyphenSplitter`]: struct.HyphenSplitter.html
    /// [`WordSplitter`]: trait.WordSplitter.html
    pub fn new(width: usize) -> Wrapper<'a, HyphenSplitter> {
        Wrapper::with_splitter(width, HyphenSplitter)
    }

    /// Create a new Wrapper for wrapping text at the current terminal
    /// width. If the terminal width cannot be determined (typically
    /// because the standard input and output is not connected to a
    /// terminal), a width of 80 characters will be used. Other
    /// settings use the same defaults as `Wrapper::new`.
    ///
    /// Equivalent to:
    ///
    /// ```no_run
    /// # #![allow(unused_variables)]
    /// use textwrap::{Wrapper, termwidth};
    ///
    /// let wrapper = Wrapper::new(termwidth());
    /// ```
    #[cfg(feature = "term_size")]
    pub fn with_termwidth() -> Wrapper<'a, HyphenSplitter> {
        Wrapper::new(termwidth())
    }
}

impl<'a, S: WordSplitter> Wrapper<'a, S> {
    /// Use the given [`WordSplitter`] to create a new Wrapper for
    /// wrapping at the specified width. By default, we allow words
    /// longer than `width` to be broken.
    ///
    /// [`WordSplitter`]: trait.WordSplitter.html
    pub fn with_splitter(width: usize, splitter: S) -> Wrapper<'a, S> {
        Wrapper {
            width: width,
            initial_indent: "",
            subsequent_indent: "",
            break_words: true,
            splitter: splitter,
        }
    }

    /// Change [`self.initial_indent`]. The initial indentation is
    /// used on the very first line of output.
    ///
    /// # Examples
    ///
    /// Classic paragraph indentation can be achieved by specifying an
    /// initial indentation and wrapping each paragraph by itself:
    ///
    /// ```no_run
    /// # #![allow(unused_variables)]
    /// use textwrap::Wrapper;
    ///
    /// let wrapper = Wrapper::new(15).initial_indent("    ");
    /// ```
    ///
    /// [`self.initial_indent`]: #structfield.initial_indent
    pub fn initial_indent(self, indent: &'a str) -> Wrapper<'a, S> {
        Wrapper {
            initial_indent: indent,
            ..self
        }
    }

    /// Change [`self.subsequent_indent`]. The subsequent indentation
    /// is used on lines following the first line of output.
    ///
    /// # Examples
    ///
    /// Combining initial and subsequent indentation lets you format a
    /// single paragraph as a bullet list:
    ///
    /// ```no_run
    /// # #![allow(unused_variables)]
    /// use textwrap::Wrapper;
    ///
    /// let wrapper = Wrapper::new(15)
    ///     .initial_indent("* ")
    ///     .subsequent_indent("  ");
    /// ```
    ///
    /// [`self.subsequent_indent`]: #structfield.subsequent_indent
    pub fn subsequent_indent(self, indent: &'a str) -> Wrapper<'a, S> {
        Wrapper {
            subsequent_indent: indent,
            ..self
        }
    }

    /// Change [`self.break_words`]. This controls if words longer
    /// than `self.width` can be broken, or if they will be left
    /// sticking out into the right margin.
    ///
    /// [`self.break_words`]: #structfield.break_words
    pub fn break_words(self, setting: bool) -> Wrapper<'a, S> {
        Wrapper {
            break_words: setting,
            ..self
        }
    }

    /// Fill a line of text at `self.width` characters. Strings are
    /// wrapped based on their displayed width, not their size in
    /// bytes.
    ///
    /// The result is a string with newlines between each line. Use
    /// the `wrap` method if you need access to the individual lines.
    ///
    /// # Complexities
    ///
    /// This method simply joins the lines produced by `wrap_iter`. As
    /// such, it inherits the O(*n*) time and memory complexity where
    /// *n* is the input string length.
    ///
    /// # Examples
    ///
    /// ```
    /// use textwrap::Wrapper;
    ///
    /// let wrapper = Wrapper::new(15);
    /// assert_eq!(wrapper.fill("Memory safety without garbage collection."),
    ///            "Memory safety\nwithout garbage\ncollection.");
    /// ```
    pub fn fill(&self, s: &str) -> String {
        // This will avoid reallocation in simple cases (no
        // indentation, no hyphenation).
        let mut result = String::with_capacity(s.len());

        for (i, line) in self.wrap_iter(s).enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(&line);
        }

        result
    }

    /// Wrap a line of text at `self.width` characters. Strings are
    /// wrapped based on their displayed width, not their size in
    /// bytes.
    ///
    /// # Complexities
    ///
    /// This method simply collects the lines produced by `wrap_iter`.
    /// As such, it inherits the O(*n*) overall time and memory
    /// complexity where *n* is the input string length.
    ///
    /// # Examples
    ///
    /// ```
    /// use textwrap::Wrapper;
    ///
    /// let wrap15 = Wrapper::new(15);
    /// assert_eq!(wrap15.wrap("Concurrency without data races."),
    ///            vec!["Concurrency",
    ///                 "without data",
    ///                 "races."]);
    ///
    /// let wrap20 = Wrapper::new(20);
    /// assert_eq!(wrap20.wrap("Concurrency without data races."),
    ///            vec!["Concurrency without",
    ///                 "data races."]);
    /// ```
    ///
    /// Notice that newlines in the input are preserved. This means
    /// that they force a line break, regardless of how long the
    /// current line is:
    ///
    /// ```
    /// use textwrap::Wrapper;
    ///
    /// let wrapper = Wrapper::new(40);
    /// assert_eq!(wrapper.wrap("First line.\nSecond line."),
    ///            vec!["First line.", "Second line."]);
    /// ```
    ///
    pub fn wrap(&self, s: &'a str) -> Vec<Cow<'a, str>> {
        self.wrap_iter(s).collect::<Vec<_>>()
    }

    /// Lazily wrap a line of text at `self.width` characters. Strings
    /// are wrapped based on their displayed width, not their size in
    /// bytes.
    ///
    /// The [`WordSplitter`] stored in [`self.splitter`] is used
    /// whenever when a word is too large to fit on the current line.
    /// By changing the field, different hyphenation strategies can be
    /// implemented.
    ///
    /// # Complexities
    ///
    /// This method returns a [`WrapIter`] iterator which borrows this
    /// `Wrapper`. The algorithm used has a linear complexity, so
    /// getting the next line from the iterator will take O(*w*) time,
    /// where *w* is the wrapping width. Fully processing the iterator
    /// will take O(*n*) time for an input string of length *n*.
    ///
    /// When no indentation is used, each line returned is a slice of
    /// the input string and the memory overhead is thus constant.
    /// Otherwise new memory is allocated for each line returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use textwrap::Wrapper;
    ///
    /// let wrap20 = Wrapper::new(20);
    /// let mut wrap20_iter = wrap20.wrap_iter("Zero-cost abstractions.");
    /// assert_eq!(wrap20_iter.next(), Some(Cow::from("Zero-cost")));
    /// assert_eq!(wrap20_iter.next(), Some(Cow::from("abstractions.")));
    /// assert_eq!(wrap20_iter.next(), None);
    ///
    /// let wrap25 = Wrapper::new(25);
    /// let mut wrap25_iter = wrap25.wrap_iter("Zero-cost abstractions.");
    /// assert_eq!(wrap25_iter.next(), Some(Cow::from("Zero-cost abstractions.")));
    /// assert_eq!(wrap25_iter.next(), None);
    /// ```
    ///
    /// [`self.splitter`]: #structfield.splitter
    /// [`WordSplitter`]: trait.WordSplitter.html
    /// [`WrapIter`]: struct.WrapIter.html
    pub fn wrap_iter<'w>(&'w self, s: &'a str) -> WrapIter<'w, 'a, S> {
        WrapIter {
            wrapper: self,
            inner: WrapIterImpl::new(self, s),
        }
    }

    /// Lazily wrap a line of text at `self.width` characters. Strings
    /// are wrapped based on their displayed width, not their size in
    /// bytes.
    ///
    /// The [`WordSplitter`] stored in [`self.splitter`] is used
    /// whenever when a word is too large to fit on the current line.
    /// By changing the field, different hyphenation strategies can be
    /// implemented.
    ///
    /// # Complexities
    ///
    /// This method consumes the `Wrapper` and returns a
    /// [`IntoWrapIter`] iterator. Fully processing the iterator has
    /// the same O(*n*) time complexity as [`wrap_iter`], where *n* is
    /// the length of the input string.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use textwrap::Wrapper;
    ///
    /// let wrap20 = Wrapper::new(20);
    /// let mut wrap20_iter = wrap20.into_wrap_iter("Zero-cost abstractions.");
    /// assert_eq!(wrap20_iter.next(), Some(Cow::from("Zero-cost")));
    /// assert_eq!(wrap20_iter.next(), Some(Cow::from("abstractions.")));
    /// assert_eq!(wrap20_iter.next(), None);
    /// ```
    ///
    /// [`self.splitter`]: #structfield.splitter
    /// [`WordSplitter`]: trait.WordSplitter.html
    /// [`IntoWrapIter`]: struct.IntoWrapIter.html
    /// [`wrap_iter`]: #method.wrap_iter
    pub fn into_wrap_iter(self, s: &'a str) -> IntoWrapIter<'a, S> {
        let inner = WrapIterImpl::new(&self, s);

        IntoWrapIter {
            wrapper: self,
            inner: inner,
        }
    }
}

/// An iterator over the lines of the input string which owns a
/// `Wrapper`. An instance of `IntoWrapIter` is typically obtained
/// through either [`wrap_iter`] or [`Wrapper::into_wrap_iter`].
///
/// Each call of `.next()` method yields a line wrapped in `Some` if the
/// input hasn't been fully processed yet. Otherwise it returns `None`.
///
/// [`wrap_iter`]: fn.wrap_iter.html
/// [`Wrapper::into_wrap_iter`]: struct.Wrapper.html#method.into_wrap_iter
#[derive(Debug)]
pub struct IntoWrapIter<'a, S: WordSplitter> {
    wrapper: Wrapper<'a, S>,
    inner: WrapIterImpl<'a>,
}

impl<'a, S: WordSplitter> Iterator for IntoWrapIter<'a, S> {
    type Item = Cow<'a, str>;

    fn next(&mut self) -> Option<Cow<'a, str>> {
        self.inner.next(&self.wrapper)
    }
}

/// An iterator over the lines of the input string which borrows a
/// `Wrapper`. An instance of `WrapIter` is typically obtained
/// through the [`Wrapper::wrap_iter`] method.
///
/// Each call of `.next()` method yields a line wrapped in `Some` if the
/// input hasn't been fully processed yet. Otherwise it returns `None`.
///
/// [`Wrapper::wrap_iter`]: struct.Wrapper.html#method.wrap_iter
#[derive(Debug)]
pub struct WrapIter<'w, 'a: 'w, S: WordSplitter + 'w> {
    wrapper: &'w Wrapper<'a, S>,
    inner: WrapIterImpl<'a>,
}

impl<'w, 'a: 'w, S: WordSplitter> Iterator for WrapIter<'w, 'a, S> {
    type Item = Cow<'a, str>;

    fn next(&mut self) -> Option<Cow<'a, str>> {
        self.inner.next(self.wrapper)
    }
}

/// Like `char::is_whitespace`, but non-breaking spaces don't count.
#[inline]
fn is_whitespace(ch: char) -> bool {
    ch.is_whitespace() && ch != NBSP
}

/// Common implementation details for `WrapIter` and `IntoWrapIter`.
#[derive(Debug)]
struct WrapIterImpl<'a> {
    // String to wrap.
    source: &'a str,
    // CharIndices iterator over self.source.
    char_indices: CharIndices<'a>,
    // Byte index where the current line starts.
    start: usize,
    // Byte index of the last place where the string can be split.
    split: usize,
    // Size in bytes of the character at self.source[self.split].
    split_len: usize,
    // Width of self.source[self.start..idx].
    line_width: usize,
    // Width of self.source[self.start..self.split].
    line_width_at_split: usize,
    // Tracking runs of whitespace characters.
    in_whitespace: bool,
    // Has iterator finished producing elements?
    finished: bool,
}

impl<'a> WrapIterImpl<'a> {
    fn new<S: WordSplitter>(wrapper: &Wrapper<'a, S>, s: &'a str) -> WrapIterImpl<'a> {
        WrapIterImpl {
            source: s,
            char_indices: s.char_indices(),
            start: 0,
            split: 0,
            split_len: 0,
            line_width: wrapper.initial_indent.width(),
            line_width_at_split: wrapper.initial_indent.width(),
            in_whitespace: false,
            finished: false,
        }
    }

    fn create_result_line<S: WordSplitter>(&self, wrapper: &Wrapper<'a, S>) -> Cow<'a, str> {
        if self.start == 0 {
            Cow::from(wrapper.initial_indent)
        } else {
            Cow::from(wrapper.subsequent_indent)
        }
    }

    fn next<S: WordSplitter>(&mut self, wrapper: &Wrapper<'a, S>) -> Option<Cow<'a, str>> {
        if self.finished {
            return None;
        }

        while let Some((idx, ch)) = self.char_indices.next() {
            let char_width = ch.width().unwrap_or(0);
            let char_len = ch.len_utf8();

            if ch == '\n' {
                self.split = idx;
                self.split_len = char_len;
                self.line_width_at_split = self.line_width;
                self.in_whitespace = false;

                // If this is not the final line, return the current line. Otherwise,
                // we will return the line with its line break after exiting the loop
                if self.split + self.split_len < self.source.len() {
                    let mut line = self.create_result_line(wrapper);
                    line += &self.source[self.start..self.split];

                    self.start = self.split + self.split_len;
                    self.line_width = wrapper.subsequent_indent.width();

                    return Some(line);
                }
            } else if is_whitespace(ch) {
                // Extend the previous split or create a new one.
                if self.in_whitespace {
                    self.split_len += char_len;
                } else {
                    self.split = idx;
                    self.split_len = char_len;
                }
                self.line_width_at_split = self.line_width + char_width;
                self.in_whitespace = true;
            } else if self.line_width + char_width > wrapper.width {
                // There is no room for this character on the current
                // line. Try to split the final word.
                self.in_whitespace = false;
                let remaining_text = &self.source[self.split + self.split_len..];
                let final_word = match remaining_text.find(is_whitespace) {
                    Some(i) => &remaining_text[..i],
                    None => remaining_text,
                };

                let mut hyphen = "";
                let splits = wrapper.splitter.split(final_word);
                for &(head, hyp, _) in splits.iter().rev() {
                    if self.line_width_at_split + head.width() + hyp.width() <= wrapper.width {
                        // We can fit head into the current line.
                        // Advance the split point by the width of the
                        // whitespace and the head length.
                        self.split += self.split_len + head.len();
                        self.split_len = 0;
                        hyphen = hyp;
                        break;
                    }
                }

                if self.start >= self.split {
                    // The word is too big to fit on a single line, so we
                    // need to split it at the current index.
                    if wrapper.break_words {
                        // Break work at current index.
                        self.split = idx;
                        self.split_len = 0;
                        self.line_width_at_split = self.line_width;
                    } else {
                        // Add smallest split.
                        self.split = self.start + splits[0].0.len();
                        self.split_len = 0;
                        self.line_width_at_split = self.line_width;
                    }
                }

                if self.start < self.split {
                    let mut line = self.create_result_line(wrapper);
                    line += &self.source[self.start..self.split];
                    line += hyphen;

                    self.start = self.split + self.split_len;
                    self.line_width += wrapper.subsequent_indent.width();
                    self.line_width -= self.line_width_at_split;
                    self.line_width += char_width;

                    return Some(line);
                }
            } else {
                self.in_whitespace = false;
            }
            self.line_width += char_width;
        }

        self.finished = true;

        // Add final line.
        if self.start < self.source.len() {
            let mut line = self.create_result_line(wrapper);
            line += &self.source[self.start..];
            return Some(line);
        }

        None
    }
}

/// Return the current terminal width. If the terminal width cannot be
/// determined (typically because the standard output is not connected
/// to a terminal), a default width of 80 characters will be used.
///
/// # Examples
///
/// Create a `Wrapper` for the current terminal with a two column
/// margin:
///
/// ```no_run
/// # #![allow(unused_variables)]
/// use textwrap::{Wrapper, NoHyphenation, termwidth};
///
/// let width = termwidth() - 4; // Two columns on each side.
/// let wrapper = Wrapper::with_splitter(width, NoHyphenation)
///     .initial_indent("  ")
///     .subsequent_indent("  ");
/// ```
#[cfg(feature = "term_size")]
pub fn termwidth() -> usize {
    term_size::dimensions_stdout().map_or(80, |(w, _)| w)
}

/// Fill a line of text at `width` characters. Strings are wrapped
/// based on their displayed width, not their size in bytes.
///
/// The result is a string with newlines between each line. Use
/// [`wrap`] if you need access to the individual lines or
/// [`wrap_iter`] for its iterator counterpart.
///
/// ```
/// use textwrap::fill;
///
/// assert_eq!(fill("Memory safety without garbage collection.", 15),
///            "Memory safety\nwithout garbage\ncollection.");
/// ```
///
/// This function creates a Wrapper on the fly with default settings.
/// If you need to set a language corpus for automatic hyphenation, or
/// need to fill many strings, then it is suggested to create a Wrapper
/// and call its [`fill` method].
///
/// [`wrap`]: fn.wrap.html
/// [`wrap_iter`]: fn.wrap_iter.html
/// [`fill` method]: struct.Wrapper.html#method.fill
pub fn fill(s: &str, width: usize) -> String {
    Wrapper::new(width).fill(s)
}

/// Wrap a line of text at `width` characters. Strings are wrapped
/// based on their displayed width, not their size in bytes.
///
/// This function creates a Wrapper on the fly with default settings.
/// If you need to set a language corpus for automatic hyphenation, or
/// need to wrap many strings, then it is suggested to create a Wrapper
/// and call its [`wrap` method].
///
/// The result is a vector of strings. Use [`wrap_iter`] if you need an
/// iterator version.
///
/// # Examples
///
/// ```
/// use textwrap::wrap;
///
/// assert_eq!(wrap("Concurrency without data races.", 15),
///            vec!["Concurrency",
///                 "without data",
///                 "races."]);
///
/// assert_eq!(wrap("Concurrency without data races.", 20),
///            vec!["Concurrency without",
///                 "data races."]);
/// ```
///
/// [`wrap_iter`]: fn.wrap_iter.html
/// [`wrap` method]: struct.Wrapper.html#method.wrap
pub fn wrap(s: &str, width: usize) -> Vec<Cow<str>> {
    Wrapper::new(width).wrap(s)
}

/// Lazily wrap a line of text at `width` characters. Strings are
/// wrapped based on their displayed width, not their size in bytes.
///
/// This function creates a Wrapper on the fly with default settings.
/// It then calls the [`into_wrap_iter`] method. Hence, the return
/// value is an [`IntoWrapIter`], not a [`WrapIter`] as the function
/// name would otherwise suggest.
///
/// If you need to set a language corpus for automatic hyphenation, or
/// need to wrap many strings, then it is suggested to create a Wrapper
/// and call its [`wrap_iter`] or [`into_wrap_iter`] methods.
///
/// # Examples
///
/// ```
/// use std::borrow::Cow;
/// use textwrap::wrap_iter;
///
/// let mut wrap20_iter = wrap_iter("Zero-cost abstractions.", 20);
/// assert_eq!(wrap20_iter.next(), Some(Cow::from("Zero-cost")));
/// assert_eq!(wrap20_iter.next(), Some(Cow::from("abstractions.")));
/// assert_eq!(wrap20_iter.next(), None);
///
/// let mut wrap25_iter = wrap_iter("Zero-cost abstractions.", 25);
/// assert_eq!(wrap25_iter.next(), Some(Cow::from("Zero-cost abstractions.")));
/// assert_eq!(wrap25_iter.next(), None);
/// ```
///
/// [`wrap_iter`]: struct.Wrapper.html#method.wrap_iter
/// [`into_wrap_iter`]: struct.Wrapper.html#method.into_wrap_iter
/// [`IntoWrapIter`]: struct.IntoWrapIter.html
/// [`WrapIter`]: struct.WrapIter.html
pub fn wrap_iter(s: &str, width: usize) -> IntoWrapIter<HyphenSplitter> {
    Wrapper::new(width).into_wrap_iter(s)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "hyphenation")]
    extern crate hyphenation;

    use super::*;
    #[cfg(feature = "hyphenation")]
    use hyphenation::{Language, Load, Standard};

    #[test]
    fn no_wrap() {
        assert_eq!(wrap("foo", 10), vec!["foo"]);
    }

    #[test]
    fn simple() {
        assert_eq!(wrap("foo bar baz", 5), vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn multi_word_on_line() {
        assert_eq!(wrap("foo bar baz", 10), vec!["foo bar", "baz"]);
    }

    #[test]
    fn long_word() {
        assert_eq!(wrap("foo", 0), vec!["f", "o", "o"]);
    }

    #[test]
    fn long_words() {
        assert_eq!(wrap("foo bar", 0), vec!["f", "o", "o", "b", "a", "r"]);
    }

    #[test]
    fn max_width() {
        assert_eq!(wrap("foo bar", usize::max_value()), vec!["foo bar"]);
    }

    #[test]
    fn leading_whitespace() {
        assert_eq!(wrap("  foo bar", 6), vec!["  foo", "bar"]);
    }

    #[test]
    fn trailing_whitespace() {
        assert_eq!(wrap("foo bar  ", 6), vec!["foo", "bar  "]);
    }

    #[test]
    fn interior_whitespace() {
        assert_eq!(wrap("foo:   bar baz", 10), vec!["foo:   bar", "baz"]);
    }

    #[test]
    fn extra_whitespace_start_of_line() {
        // Whitespace is only significant inside a line. After a line
        // gets too long and is broken, the first word starts in
        // column zero and is not indented. The line before might end
        // up with trailing whitespace.
        assert_eq!(wrap("foo               bar", 5), vec!["foo", "bar"]);
    }

    #[test]
    fn issue_99() {
        // We did not reset the in_whitespace flag correctly and did
        // not handle single-character words after a line break.
        assert_eq!(
            wrap("aaabbbccc x yyyzzzwww", 9),
            vec!["aaabbbccc", "x", "yyyzzzwww"]
        );
    }

    #[test]
    fn issue_129() {
        // The dash is an em-dash which takes up four bytes. We used
        // to panic since we tried to index into the character.
        assert_eq!(wrap("x – x", 1), vec!["x", "–", "x"]);
    }

    #[test]
    fn wide_character_handling() {
        assert_eq!(wrap("Hello, World!", 15), vec!["Hello, World!"]);
        assert_eq!(
            wrap("Ｈｅｌｌｏ, Ｗｏｒｌｄ!", 15),
            vec!["Ｈｅｌｌｏ,", "Ｗｏｒｌｄ!"]
        );
    }

    #[test]
    fn empty_input_not_indented() {
        let wrapper = Wrapper::new(10).initial_indent("!!!");
        assert_eq!(wrapper.fill(""), "");
    }

    #[test]
    fn indent_single_line() {
        let wrapper = Wrapper::new(10).initial_indent(">>>"); // No trailing space
        assert_eq!(wrapper.fill("foo"), ">>>foo");
    }

    #[test]
    fn indent_multiple_lines() {
        let wrapper = Wrapper::new(6).initial_indent("* ").subsequent_indent("  ");
        assert_eq!(wrapper.wrap("foo bar baz"), vec!["* foo", "  bar", "  baz"]);
    }

    #[test]
    fn indent_break_words() {
        let wrapper = Wrapper::new(5).initial_indent("* ").subsequent_indent("  ");
        assert_eq!(wrapper.wrap("foobarbaz"), vec!["* foo", "  bar", "  baz"]);
    }

    #[test]
    fn hyphens() {
        assert_eq!(wrap("foo-bar", 5), vec!["foo-", "bar"]);
    }

    #[test]
    fn trailing_hyphen() {
        let wrapper = Wrapper::new(5).break_words(false);
        assert_eq!(wrapper.wrap("foobar-"), vec!["foobar-"]);
    }

    #[test]
    fn multiple_hyphens() {
        assert_eq!(wrap("foo-bar-baz", 5), vec!["foo-", "bar-", "baz"]);
    }

    #[test]
    fn hyphens_flag() {
        let wrapper = Wrapper::new(5).break_words(false);
        assert_eq!(
            wrapper.wrap("The --foo-bar flag."),
            vec!["The", "--foo-", "bar", "flag."]
        );
    }

    #[test]
    fn repeated_hyphens() {
        let wrapper = Wrapper::new(4).break_words(false);
        assert_eq!(wrapper.wrap("foo--bar"), vec!["foo--bar"]);
    }

    #[test]
    fn hyphens_alphanumeric() {
        assert_eq!(wrap("Na2-CH4", 5), vec!["Na2-", "CH4"]);
    }

    #[test]
    fn hyphens_non_alphanumeric() {
        let wrapper = Wrapper::new(5).break_words(false);
        assert_eq!(wrapper.wrap("foo(-)bar"), vec!["foo(-)bar"]);
    }

    #[test]
    fn multiple_splits() {
        assert_eq!(wrap("foo-bar-baz", 9), vec!["foo-bar-", "baz"]);
    }

    #[test]
    fn forced_split() {
        let wrapper = Wrapper::new(5).break_words(false);
        assert_eq!(wrapper.wrap("foobar-baz"), vec!["foobar-", "baz"]);
    }

    #[test]
    fn no_hyphenation() {
        let wrapper = Wrapper::with_splitter(8, NoHyphenation);
        assert_eq!(wrapper.wrap("foo bar-baz"), vec!["foo", "bar-baz"]);
    }

    #[test]
    #[cfg(feature = "hyphenation")]
    fn auto_hyphenation() {
        let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
        let wrapper = Wrapper::new(10);
        assert_eq!(
            wrapper.wrap("Internationalization"),
            vec!["Internatio", "nalization"]
        );

        let wrapper = Wrapper::with_splitter(10, dictionary);
        assert_eq!(
            wrapper.wrap("Internationalization"),
            vec!["Interna-", "tionaliza-", "tion"]
        );
    }

    #[test]
    #[cfg(feature = "hyphenation")]
    fn split_len_hyphenation() {
        // Test that hyphenation takes the width of the wihtespace
        // into account.
        let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
        let wrapper = Wrapper::with_splitter(15, dictionary);
        assert_eq!(
            wrapper.wrap("garbage   collection"),
            vec!["garbage   col-", "lection"]
        );
    }

    #[test]
    #[cfg(feature = "hyphenation")]
    fn borrowed_lines() {
        // Lines that end with an extra hyphen are owned, the final
        // line is borrowed.
        use std::borrow::Cow::{Borrowed, Owned};
        let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
        let wrapper = Wrapper::with_splitter(10, dictionary);
        let lines = wrapper.wrap("Internationalization");
        if let Borrowed(s) = lines[0] {
            assert!(false, "should not have been borrowed: {:?}", s);
        }
        if let Borrowed(s) = lines[1] {
            assert!(false, "should not have been borrowed: {:?}", s);
        }
        if let Owned(ref s) = lines[2] {
            assert!(false, "should not have been owned: {:?}", s);
        }
    }

    #[test]
    #[cfg(feature = "hyphenation")]
    fn auto_hyphenation_with_hyphen() {
        let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
        let wrapper = Wrapper::new(8).break_words(false);
        assert_eq!(wrapper.wrap("over-caffinated"), vec!["over-", "caffinated"]);

        let wrapper = Wrapper::with_splitter(8, dictionary).break_words(false);
        assert_eq!(
            wrapper.wrap("over-caffinated"),
            vec!["over-", "caffi-", "nated"]
        );
    }

    #[test]
    fn break_words() {
        assert_eq!(wrap("foobarbaz", 3), vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn break_words_wide_characters() {
        assert_eq!(wrap("Ｈｅｌｌｏ", 5), vec!["Ｈｅ", "ｌｌ", "ｏ"]);
    }

    #[test]
    fn break_words_zero_width() {
        assert_eq!(wrap("foobar", 0), vec!["f", "o", "o", "b", "a", "r"]);
    }

    #[test]
    fn break_words_line_breaks() {
        assert_eq!(fill("ab\ncdefghijkl", 5), "ab\ncdefg\nhijkl");
        assert_eq!(fill("abcdefgh\nijkl", 5), "abcde\nfgh\nijkl");
    }

    #[test]
    fn preserve_line_breaks() {
        assert_eq!(fill("test\n", 11), "test\n");
        assert_eq!(fill("test\n\na\n\n", 11), "test\n\na\n\n");
        assert_eq!(fill("1 3 5 7\n1 3 5 7", 7), "1 3 5 7\n1 3 5 7");
    }

    #[test]
    fn wrap_preserve_line_breaks() {
        assert_eq!(fill("1 3 5 7\n1 3 5 7", 5), "1 3 5\n7\n1 3 5\n7");
    }

    #[test]
    fn non_breaking_space() {
        let wrapper = Wrapper::new(5).break_words(false);
        assert_eq!(wrapper.fill("foo bar baz"), "foo bar baz");
    }

    #[test]
    fn non_breaking_hyphen() {
        let wrapper = Wrapper::new(5).break_words(false);
        assert_eq!(wrapper.fill("foo‑bar‑baz"), "foo‑bar‑baz");
    }

    #[test]
    fn fill_simple() {
        assert_eq!(fill("foo bar baz", 10), "foo bar\nbaz");
    }
}
